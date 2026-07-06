use std::collections::BTreeMap;

mod budget;
mod filter;

use crate::{
    Circuit, CircuitError, CircuitErrorLocation, CircuitErrorLocationStackFrame,
    CircuitInstruction, CircuitItem, CircuitResult, CircuitTargetsInsideInstruction,
    DetectorErrorModel, ErrorAnalyzerOptions, ExplainedError, FlippedMeasurement, Gate,
    GateTargetWithCoords, Pauli, Probability, QubitId, RepeatBlock, Target,
    circuit_to_detector_error_model,
};
use budget::validate_error_matcher_circuit;
use filter::error_keys_from_dem;

pub fn explain_errors_from_circuit(
    circuit: &Circuit,
    filter: Option<&DetectorErrorModel>,
    reduce_to_one_representative_error: bool,
) -> CircuitResult<Vec<ExplainedError>> {
    validate_error_matcher_circuit(circuit)?;
    let detector_coords = detector_coordinates(circuit)?;
    let filter_keys = filter
        .map(error_keys_from_dem)
        .transpose()?
        .unwrap_or_default();
    let allow_new_entries = filter.is_none();
    let mut output = BTreeMap::new();
    for key in filter_keys {
        output.entry(key).or_insert_with(|| ExplainedError {
            dem_error_terms: Vec::new(),
            circuit_error_locations: Vec::new(),
        });
    }

    for candidate in CandidateCollector::new(circuit)?.collect()? {
        let candidate_circuit = isolate_candidate(circuit, &candidate)?;
        let candidate_dem = circuit_to_detector_error_model(
            &candidate_circuit,
            ErrorAnalyzerOptions {
                approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0)?),
                ..ErrorAnalyzerOptions::default()
            },
        )?;
        for key in error_keys_from_dem(&candidate_dem)? {
            if key.is_empty() {
                continue;
            }
            let entry_exists = output.contains_key(&key);
            if !allow_new_entries && !entry_exists {
                continue;
            }
            let entry = output.entry(key).or_insert_with(|| ExplainedError {
                dem_error_terms: Vec::new(),
                circuit_error_locations: Vec::new(),
            });
            add_location(
                entry,
                candidate.location.clone(),
                reduce_to_one_representative_error,
            );
        }
    }

    let mut result = Vec::new();
    for (key, mut explained) in output {
        explained.fill_in_dem_targets(&key, &detector_coords);
        result.push(explained);
    }
    Ok(result)
}

fn add_location(
    explained: &mut ExplainedError,
    location: CircuitErrorLocation,
    reduce_to_one_representative_error: bool,
) {
    if explained.circuit_error_locations.is_empty() || !reduce_to_one_representative_error {
        explained.circuit_error_locations.push(location);
    } else if let Some(existing) = explained.circuit_error_locations.first_mut()
        && location.is_simpler_than(existing)
    {
        *existing = location;
    }
}

#[derive(Clone, Debug)]
struct ErrorCandidate {
    instruction_offset: usize,
    target_range_start: usize,
    target_range_end: usize,
    replacement: CandidateReplacement,
    location: CircuitErrorLocation,
}

#[derive(Clone, Debug, PartialEq)]
enum CandidateReplacement {
    Noise {
        gate_name: Option<&'static str>,
        args: Option<Vec<f64>>,
        targets: Option<Vec<Target>>,
    },
    Measurement,
}

struct CandidateCollector<'a> {
    circuit: &'a Circuit,
    state: ScanState,
}

impl<'a> CandidateCollector<'a> {
    fn new(circuit: &'a Circuit) -> CircuitResult<Self> {
        Ok(Self {
            circuit,
            state: ScanState::default(),
        })
    }

    fn collect(mut self) -> CircuitResult<Vec<ErrorCandidate>> {
        let mut candidates = Vec::new();
        for (instruction_offset, item) in self.circuit.items().iter().enumerate() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.collect_instruction_candidates(
                        instruction_offset,
                        instruction,
                        &mut candidates,
                    )?;
                    self.state.apply_instruction(instruction)?;
                }
                CircuitItem::RepeatBlock(repeat) => self.state.apply_repeat(repeat)?,
            }
        }
        Ok(candidates)
    }

    fn collect_instruction_candidates(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        match gate_name {
            "X_ERROR" => self.collect_single_qubit_pauli_errors(
                instruction_offset,
                instruction,
                Pauli::X,
                candidates,
            ),
            "Y_ERROR" => self.collect_single_qubit_pauli_errors(
                instruction_offset,
                instruction,
                Pauli::Y,
                candidates,
            ),
            "Z_ERROR" => self.collect_single_qubit_pauli_errors(
                instruction_offset,
                instruction,
                Pauli::Z,
                candidates,
            ),
            "E" | "CORRELATED_ERROR" | "ELSE_CORRELATED_ERROR" => {
                self.collect_correlated_error(instruction_offset, instruction, candidates)
            }
            "DEPOLARIZE1" => {
                self.collect_depolarize1_errors(instruction_offset, instruction, candidates)
            }
            "DEPOLARIZE2" => {
                self.collect_depolarize2_errors(instruction_offset, instruction, candidates)
            }
            "PAULI_CHANNEL_2" => {
                self.collect_pauli_channel2_errors(instruction_offset, instruction, candidates)
            }
            "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => {
                self.collect_single_measurement_errors(instruction_offset, instruction, candidates)
            }
            "MXX" | "MYY" | "MZZ" => {
                self.collect_pair_measurement_errors(instruction_offset, instruction, candidates)
            }
            _ => Ok(()),
        }
    }

    fn collect_single_qubit_pauli_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        pauli: Pauli,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        for (target_index, target) in instruction.targets().iter().enumerate() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let flipped_pauli_product = vec![self.pauli_with_coords(pauli, qubit)?];
            candidates.push(ErrorCandidate {
                instruction_offset,
                target_range_start: target_index,
                target_range_end: target_index + 1,
                replacement: CandidateReplacement::Noise {
                    gate_name: None,
                    args: None,
                    targets: None,
                },
                location: self.location(
                    instruction,
                    instruction_offset,
                    target_index,
                    target_index + 1,
                    flipped_pauli_product,
                    FlippedMeasurement::none(),
                )?,
            });
        }
        Ok(())
    }

    fn collect_correlated_error(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        let flipped_pauli_product = self.resolve_targets_with_coords(instruction.targets())?;
        let gate_name =
            (instruction.gate().canonical_name() == "ELSE_CORRELATED_ERROR").then_some("E");
        candidates.push(ErrorCandidate {
            instruction_offset,
            target_range_start: 0,
            target_range_end: instruction.targets().len(),
            replacement: CandidateReplacement::Noise {
                gate_name,
                args: None,
                targets: None,
            },
            location: self.location(
                instruction,
                instruction_offset,
                0,
                instruction.targets().len(),
                flipped_pauli_product,
                FlippedMeasurement::none(),
            )?,
        });
        Ok(())
    }

    fn collect_depolarize1_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        for (target_index, target) in instruction.targets().iter().enumerate() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE1 target {target} is not a qubit"
                )));
            };
            for pauli in [Pauli::X, Pauli::Y, Pauli::Z] {
                candidates.push(ErrorCandidate {
                    instruction_offset,
                    target_range_start: target_index,
                    target_range_end: target_index + 1,
                    replacement: CandidateReplacement::Noise {
                        gate_name: Some(pauli_error_gate(pauli)),
                        args: None,
                        targets: None,
                    },
                    location: self.location(
                        instruction,
                        instruction_offset,
                        target_index,
                        target_index + 1,
                        vec![self.pauli_with_coords(pauli, qubit)?],
                        FlippedMeasurement::none(),
                    )?,
                });
            }
        }
        Ok(())
    }

    fn collect_depolarize2_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        let mut groups = instruction.targets().chunks_exact(2);
        for (group_index, group) in groups.by_ref().enumerate() {
            let [left_target, right_target] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 expected paired targets during error matching",
                ));
            };
            let Some(left) = left_target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {left_target} is not a qubit"
                )));
            };
            let Some(right) = right_target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {right_target} is not a qubit"
                )));
            };
            let target_range_start = group_index.checked_mul(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            let target_range_end = target_range_start.checked_add(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            for (left_pauli, right_pauli) in depolarize2_component_order() {
                let flipped_pauli_product =
                    self.pauli_product_with_coords(left, left_pauli, right, right_pauli)?;
                let replacement_targets =
                    pauli_product_targets(left, left_pauli, right, right_pauli);
                candidates.push(ErrorCandidate {
                    instruction_offset,
                    target_range_start,
                    target_range_end,
                    replacement: CandidateReplacement::Noise {
                        gate_name: Some("E"),
                        args: None,
                        targets: Some(replacement_targets),
                    },
                    location: self.location(
                        instruction,
                        instruction_offset,
                        target_range_start,
                        target_range_end,
                        flipped_pauli_product,
                        FlippedMeasurement::none(),
                    )?,
                });
            }
        }
        if !groups.remainder().is_empty() {
            return Err(CircuitError::invalid_detector_error_model(
                "DEPOLARIZE2 expected an even number of targets during error matching",
            ));
        }
        Ok(())
    }

    fn collect_pauli_channel2_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction.args().len() != 15 {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_2 expected 15 probability arguments during error matching",
            ));
        }
        let mut groups = instruction.targets().chunks_exact(2);
        for (group_index, group) in groups.by_ref().enumerate() {
            let [left_target, right_target] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 expected paired targets during error matching",
                ));
            };
            let Some(left) = left_target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {left_target} is not a qubit"
                )));
            };
            let Some(right) = right_target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {right_target} is not a qubit"
                )));
            };
            let target_range_start = group_index.checked_mul(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            let target_range_end = target_range_start.checked_add(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            for (component_index, (left_pauli, right_pauli)) in
                depolarize2_component_order().into_iter().enumerate()
            {
                let probability = instruction
                    .args()
                    .get(component_index)
                    .copied()
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "PAULI_CHANNEL_2 component index is outside probability arguments",
                        )
                    })?;
                if probability == 0.0 {
                    continue;
                }
                let flipped_pauli_product =
                    self.pauli_product_with_coords(left, left_pauli, right, right_pauli)?;
                let replacement_targets =
                    pauli_product_targets(left, left_pauli, right, right_pauli);
                candidates.push(ErrorCandidate {
                    instruction_offset,
                    target_range_start,
                    target_range_end,
                    replacement: CandidateReplacement::Noise {
                        gate_name: Some("E"),
                        args: Some(vec![probability]),
                        targets: Some(replacement_targets),
                    },
                    location: self.location(
                        instruction,
                        instruction_offset,
                        target_range_start,
                        target_range_end,
                        flipped_pauli_product,
                        FlippedMeasurement::none(),
                    )?,
                });
            }
        }
        if !groups.remainder().is_empty() {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_2 expected an even number of targets during error matching",
            ));
        }
        Ok(())
    }

    fn collect_single_measurement_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        let basis = measurement_basis(instruction.gate().canonical_name()).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("unknown measurement basis")
        })?;
        for (target_index, target) in instruction.targets().iter().enumerate() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let measurement_record_index = self
                .state
                .measurement_count
                .checked_add(target_index)
                .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement index overflowed")
            })?;
            candidates.push(ErrorCandidate {
                instruction_offset,
                target_range_start: target_index,
                target_range_end: target_index + 1,
                replacement: CandidateReplacement::Measurement,
                location: self.location(
                    instruction,
                    instruction_offset,
                    target_index,
                    target_index + 1,
                    Vec::new(),
                    FlippedMeasurement {
                        measurement_record_index: Some(
                            u64::try_from(measurement_record_index).map_err(|_| {
                                CircuitError::invalid_detector_error_model(
                                    "measurement index does not fit u64",
                                )
                            })?,
                        ),
                        measured_observable: vec![self.pauli_with_coords(basis, qubit)?],
                    },
                )?,
            });
        }
        Ok(())
    }

    fn collect_pair_measurement_errors(
        &self,
        instruction_offset: usize,
        instruction: &CircuitInstruction,
        candidates: &mut Vec<ErrorCandidate>,
    ) -> CircuitResult<()> {
        if instruction
            .probability_argument()?
            .is_none_or(|probability| probability.get() == 0.0)
        {
            return Ok(());
        }
        let basis =
            pair_measurement_basis(instruction.gate().canonical_name()).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("unknown pair measurement basis")
            })?;
        for (group_index, group) in instruction.target_groups().iter().enumerate() {
            let [left, right] = *group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired targets during error matching",
                    instruction.gate().canonical_name()
                )));
            };
            let Some(left) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {left} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let Some(right) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {right} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let target_range_start = group_index.checked_mul(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            let target_range_end = target_range_start.checked_add(2).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("target range overflowed")
            })?;
            let measurement_record_index = self
                .state
                .measurement_count
                .checked_add(group_index)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("measurement index overflowed")
                })?;
            candidates.push(ErrorCandidate {
                instruction_offset,
                target_range_start,
                target_range_end,
                replacement: CandidateReplacement::Measurement,
                location: self.location(
                    instruction,
                    instruction_offset,
                    target_range_start,
                    target_range_end,
                    Vec::new(),
                    FlippedMeasurement {
                        measurement_record_index: Some(
                            u64::try_from(measurement_record_index).map_err(|_| {
                                CircuitError::invalid_detector_error_model(
                                    "measurement index does not fit u64",
                                )
                            })?,
                        ),
                        measured_observable: vec![
                            self.pauli_with_coords(basis, left)?,
                            self.pauli_with_coords(basis, right)?,
                        ],
                    },
                )?,
            });
        }
        Ok(())
    }

    fn location(
        &self,
        instruction: &CircuitInstruction,
        instruction_offset: usize,
        target_range_start: usize,
        target_range_end: usize,
        flipped_pauli_product: Vec<GateTargetWithCoords>,
        flipped_measurement: FlippedMeasurement,
    ) -> CircuitResult<CircuitErrorLocation> {
        let mut instruction_targets = CircuitTargetsInsideInstruction {
            gate: None,
            gate_tag: None,
            args: Vec::new(),
            target_range_start,
            target_range_end,
            targets_in_range: Vec::new(),
        };
        instruction_targets
            .fill_args_and_targets_in_range(instruction, &self.state.qubit_coords)?;
        Ok(CircuitErrorLocation {
            noise_tag: instruction.tag().map(ToOwned::to_owned),
            tick_offset: self.state.tick_count,
            flipped_pauli_product,
            flipped_measurement,
            instruction_targets,
            stack_frames: vec![CircuitErrorLocationStackFrame {
                instruction_offset: u64::try_from(instruction_offset).map_err(|_| {
                    CircuitError::invalid_detector_error_model(
                        "instruction offset does not fit u64",
                    )
                })?,
                iteration_index: 0,
                instruction_repetitions_arg: 0,
            }],
        })
    }

    fn pauli_with_coords(
        &self,
        pauli: Pauli,
        qubit: QubitId,
    ) -> CircuitResult<GateTargetWithCoords> {
        Ok(GateTargetWithCoords {
            gate_target: Target::pauli(pauli, qubit, false),
            coords: self
                .state
                .qubit_coords
                .get(&u64::from(qubit.get()))
                .cloned()
                .unwrap_or_default(),
        })
    }

    fn pauli_product_with_coords(
        &self,
        left: QubitId,
        left_pauli: Option<Pauli>,
        right: QubitId,
        right_pauli: Option<Pauli>,
    ) -> CircuitResult<Vec<GateTargetWithCoords>> {
        let mut result = Vec::new();
        if let Some(pauli) = left_pauli {
            result.push(self.pauli_with_coords(pauli, left)?);
        }
        if let Some(pauli) = right_pauli {
            result.push(self.pauli_with_coords(pauli, right)?);
        }
        Ok(result)
    }

    fn resolve_targets_with_coords(
        &self,
        targets: &[Target],
    ) -> CircuitResult<Vec<GateTargetWithCoords>> {
        let mut result = Vec::new();
        for target in targets {
            if target.is_combiner() {
                continue;
            }
            let coords = target
                .qubit_id()
                .and_then(|qubit| self.state.qubit_coords.get(&u64::from(qubit.get())))
                .cloned()
                .unwrap_or_default();
            result.push(GateTargetWithCoords {
                gate_target: target.clone(),
                coords,
            });
        }
        Ok(result)
    }
}

#[derive(Clone, Debug, Default)]
struct ScanState {
    tick_count: u64,
    measurement_count: usize,
    detector_count: u64,
    coord_offset: Vec<f64>,
    qubit_coords: BTreeMap<u64, Vec<f64>>,
    detector_coords: BTreeMap<u64, Vec<f64>>,
}

impl ScanState {
    fn apply_repeat(&mut self, repeat: &RepeatBlock) -> CircuitResult<()> {
        for _ in 0..repeat.repeat_count().get() {
            for item in repeat.body().items() {
                match item {
                    CircuitItem::Instruction(instruction) => self.apply_instruction(instruction)?,
                    CircuitItem::RepeatBlock(nested) => self.apply_repeat(nested)?,
                }
            }
        }
        Ok(())
    }

    fn apply_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "TICK" => {
                self.tick_count = self.tick_count.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("tick count overflowed")
                })?;
            }
            "QUBIT_COORDS" => self.apply_qubit_coords(instruction)?,
            "SHIFT_COORDS" => self.apply_shift_coords(instruction),
            "DETECTOR" => self.apply_detector(instruction)?,
            name if produces_single_measurements(name) => {
                self.measurement_count = self
                    .measurement_count
                    .checked_add(instruction.targets().len())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("measurement count overflowed")
                    })?;
            }
            "MXX" | "MYY" | "MZZ" | "MPP" => {
                self.measurement_count = self
                    .measurement_count
                    .checked_add(instruction.target_groups().len())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("measurement count overflowed")
                    })?;
            }
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" | "MPAD" => {
                self.measurement_count = self
                    .measurement_count
                    .checked_add(instruction.targets().len())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("measurement count overflowed")
                    })?;
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_qubit_coords(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let coords = shifted_coordinates(instruction.args(), &self.coord_offset);
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "QUBIT_COORDS target {target} is not a qubit"
                )));
            };
            self.qubit_coords
                .insert(u64::from(qubit.get()), coords.clone());
        }
        Ok(())
    }

    fn apply_shift_coords(&mut self, instruction: &CircuitInstruction) {
        if self.coord_offset.len() < instruction.args().len() {
            self.coord_offset.resize(instruction.args().len(), 0.0);
        }
        for (index, value) in instruction.args().iter().copied().enumerate() {
            if let Some(offset) = self.coord_offset.get_mut(index) {
                *offset += value;
            }
        }
    }

    fn apply_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        if !instruction.args().is_empty() {
            self.detector_coords.insert(
                self.detector_count,
                shifted_coordinates(instruction.args(), &self.coord_offset),
            );
        }
        self.detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("detector count overflowed")
        })?;
        Ok(())
    }
}

fn detector_coordinates(circuit: &Circuit) -> CircuitResult<BTreeMap<u64, Vec<f64>>> {
    let mut state = ScanState::default();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => state.apply_instruction(instruction)?,
            CircuitItem::RepeatBlock(repeat) => state.apply_repeat(repeat)?,
        }
    }
    Ok(state.detector_coords)
}

fn isolate_candidate(circuit: &Circuit, candidate: &ErrorCandidate) -> CircuitResult<Circuit> {
    let mut result = Circuit::new();
    for (instruction_offset, item) in circuit.items().iter().enumerate() {
        match item {
            CircuitItem::Instruction(instruction)
                if instruction_offset == candidate.instruction_offset =>
            {
                append_candidate_instruction(&mut result, instruction, candidate)?;
            }
            CircuitItem::Instruction(instruction) => {
                append_sanitized_instruction(&mut result, instruction)?;
            }
            CircuitItem::RepeatBlock(repeat) => result.append_repeat_block(repeat.clone()),
        }
    }
    Ok(result)
}

fn append_candidate_instruction(
    circuit: &mut Circuit,
    instruction: &CircuitInstruction,
    candidate: &ErrorCandidate,
) -> CircuitResult<()> {
    match &candidate.replacement {
        CandidateReplacement::Noise {
            gate_name,
            args,
            targets,
        } => {
            let gate = Gate::from_name(gate_name.unwrap_or(instruction.gate().canonical_name()))?;
            let args = args
                .as_ref()
                .cloned()
                .unwrap_or_else(|| instruction.args().to_vec());
            let targets = match targets {
                Some(targets) => targets.clone(),
                None => instruction
                    .targets()
                    .get(candidate.target_range_start..candidate.target_range_end)
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "candidate target range is outside instruction targets",
                        )
                    })?
                    .to_vec(),
            };
            circuit.append_instruction(CircuitInstruction::new(
                gate,
                args,
                targets,
                instruction.tag().map(ToOwned::to_owned),
            )?);
        }
        CandidateReplacement::Measurement => {
            append_measurement_slice(
                circuit,
                instruction,
                0,
                candidate.target_range_start,
                Vec::new(),
            )?;
            append_measurement_slice(
                circuit,
                instruction,
                candidate.target_range_start,
                candidate.target_range_end,
                instruction.args().to_vec(),
            )?;
            append_measurement_slice(
                circuit,
                instruction,
                candidate.target_range_end,
                instruction.targets().len(),
                Vec::new(),
            )?;
        }
    }
    Ok(())
}

fn append_measurement_slice(
    circuit: &mut Circuit,
    instruction: &CircuitInstruction,
    start: usize,
    end: usize,
    args: Vec<f64>,
) -> CircuitResult<()> {
    if start == end {
        return Ok(());
    }
    let targets = instruction
        .targets()
        .get(start..end)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "candidate target range is outside instruction targets",
            )
        })?
        .to_vec();
    circuit.append_instruction(CircuitInstruction::new(
        instruction.gate(),
        args,
        targets,
        instruction.tag().map(ToOwned::to_owned),
    )?);
    Ok(())
}

fn append_sanitized_instruction(
    circuit: &mut Circuit,
    instruction: &CircuitInstruction,
) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        name if is_pure_noise(name) => {}
        name if measurement_basis(name).is_some() || pair_measurement_basis(name).is_some() => {
            circuit.append_instruction(CircuitInstruction::new(
                instruction.gate(),
                Vec::new(),
                instruction.targets().to_vec(),
                instruction.tag().map(ToOwned::to_owned),
            )?);
        }
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
            circuit.append_instruction(CircuitInstruction::new(
                instruction.gate(),
                vec![0.0; instruction.args().len()],
                instruction.targets().to_vec(),
                instruction.tag().map(ToOwned::to_owned),
            )?);
        }
        _ => circuit.append_instruction(instruction.clone()),
    }
    Ok(())
}

fn shifted_coordinates(args: &[f64], coord_offset: &[f64]) -> Vec<f64> {
    args.iter()
        .copied()
        .enumerate()
        .map(|(index, value)| value + coord_offset.get(index).copied().unwrap_or(0.0))
        .collect()
}

fn is_pure_noise(name: &str) -> bool {
    matches!(
        name,
        "X_ERROR"
            | "Y_ERROR"
            | "Z_ERROR"
            | "I_ERROR"
            | "II_ERROR"
            | "E"
            | "CORRELATED_ERROR"
            | "ELSE_CORRELATED_ERROR"
            | "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
    )
}

fn produces_single_measurements(name: &str) -> bool {
    matches!(
        name,
        "M" | "MX"
            | "MY"
            | "MR"
            | "MRX"
            | "MRY"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1"
            | "MPAD"
    )
}

fn measurement_basis(name: &str) -> Option<Pauli> {
    match name {
        "MX" | "MRX" => Some(Pauli::X),
        "MY" | "MRY" => Some(Pauli::Y),
        "M" | "MR" => Some(Pauli::Z),
        _ => None,
    }
}

fn pair_measurement_basis(name: &str) -> Option<Pauli> {
    match name {
        "MXX" => Some(Pauli::X),
        "MYY" => Some(Pauli::Y),
        "MZZ" => Some(Pauli::Z),
        _ => None,
    }
}

fn pauli_error_gate(pauli: Pauli) -> &'static str {
    match pauli {
        Pauli::X => "X_ERROR",
        Pauli::Y => "Y_ERROR",
        Pauli::Z => "Z_ERROR",
    }
}

fn depolarize2_component_order() -> [(Option<Pauli>, Option<Pauli>); 15] {
    [
        (None, Some(Pauli::X)),
        (None, Some(Pauli::Y)),
        (None, Some(Pauli::Z)),
        (Some(Pauli::X), None),
        (Some(Pauli::X), Some(Pauli::X)),
        (Some(Pauli::X), Some(Pauli::Y)),
        (Some(Pauli::X), Some(Pauli::Z)),
        (Some(Pauli::Y), None),
        (Some(Pauli::Y), Some(Pauli::X)),
        (Some(Pauli::Y), Some(Pauli::Y)),
        (Some(Pauli::Y), Some(Pauli::Z)),
        (Some(Pauli::Z), None),
        (Some(Pauli::Z), Some(Pauli::X)),
        (Some(Pauli::Z), Some(Pauli::Y)),
        (Some(Pauli::Z), Some(Pauli::Z)),
    ]
}

fn pauli_product_targets(
    left: QubitId,
    left_pauli: Option<Pauli>,
    right: QubitId,
    right_pauli: Option<Pauli>,
) -> Vec<Target> {
    let mut result = Vec::new();
    if let Some(pauli) = left_pauli {
        result.push(Target::pauli(pauli, left, false));
    }
    if let Some(pauli) = right_pauli {
        result.push(Target::pauli(pauli, right, false));
    }
    result
}
