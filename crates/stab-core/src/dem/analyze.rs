use std::collections::{BTreeMap, BTreeSet};

mod error_decomp;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Pauli, Probability,
    QubitId, RepeatBlock,
};

use super::{DemInstruction, DemRepeatBlock, DemTarget, DetectorErrorModel};
use error_decomp::{
    depolarize2_independent_channel_probability, pauli_channel2_components,
    try_disjoint_to_independent_xyz_errors,
};

const MAX_ANALYZER_REPEAT_UNROLL: u64 = 100_000;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ErrorAnalyzerOptions {
    pub fold_loops: bool,
    pub decompose_errors: bool,
    pub allow_gauge_detectors: bool,
    pub approximate_disjoint_errors_threshold: Option<Probability>,
}

pub fn circuit_to_detector_error_model(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<DetectorErrorModel> {
    if options.fold_loops
        && circuit
            .items()
            .iter()
            .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return FoldedAnalyzer::new(options).analyze(circuit);
    }
    Analyzer::new(options).analyze(circuit)
}

struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    fn analyze(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        let mut dem = DetectorErrorModel::new();
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(_) => {
                    return Err(CircuitError::invalid_detector_error_model(
                        "analyze_errors --fold_loops currently supports top-level repeat blocks only",
                    ));
                }
                CircuitItem::RepeatBlock(repeat) => {
                    dem.push_repeat_block(self.analyze_repeat(repeat)?);
                }
            }
        }
        Ok(dem)
    }

    fn analyze_repeat(&self, repeat: &RepeatBlock) -> CircuitResult<DemRepeatBlock> {
        let mut body_options = self.options;
        body_options.fold_loops = false;
        let mut result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        if result.detector_count > 0 {
            result.dem.push_instruction(DemInstruction::shift_detectors(
                Vec::new(),
                result.detector_count,
                None,
            )?);
        }
        Ok(DemRepeatBlock::new(
            repeat.repeat_count(),
            result.dem,
            repeat.tag().map(ToOwned::to_owned),
        ))
    }
}

struct AnalyzerResult {
    dem: DetectorErrorModel,
    detector_count: u64,
}

struct Analyzer {
    options: ErrorAnalyzerOptions,
    measurement_count: usize,
    detector_count: u64,
    coord_offset: Vec<f64>,
    pending_errors: Vec<PendingError>,
    pending_pauli_channels: Vec<PendingSingleQubitPauliChannel>,
    else_correlated_error_remainder: Option<Probability>,
    next_disjoint_group_id: u64,
    completed_errors: Vec<PendingError>,
    detector_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    observable_terms_by_measurement: BTreeMap<usize, Vec<u64>>,
    detector_declarations: Vec<DetectorDeclaration>,
}

impl Analyzer {
    fn new(options: ErrorAnalyzerOptions) -> Self {
        Self {
            options,
            measurement_count: 0,
            detector_count: 0,
            coord_offset: Vec::new(),
            pending_errors: Vec::new(),
            pending_pauli_channels: Vec::new(),
            else_correlated_error_remainder: None,
            next_disjoint_group_id: 0,
            completed_errors: Vec::new(),
            detector_terms_by_measurement: BTreeMap::new(),
            observable_terms_by_measurement: BTreeMap::new(),
            detector_declarations: Vec::new(),
        }
    }

    fn analyze(self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        self.analyze_with_stats(circuit).map(|result| result.dem)
    }

    fn analyze_with_stats(mut self, circuit: &Circuit) -> CircuitResult<AnalyzerResult> {
        self.visit_circuit(circuit)?;
        let detector_count = self.detector_count;
        let dem = self.into_dem()?;
        Ok(AnalyzerResult {
            dem,
            detector_count,
        })
    }

    fn visit_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => self.visit_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    self.end_else_correlated_error_block();
                    self.visit_repeat(repeat)?;
                    self.end_else_correlated_error_block();
                }
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock) -> CircuitResult<()> {
        let repeat_count = repeat.repeat_count().get();
        if self.options.fold_loops {
            return Err(CircuitError::invalid_detector_error_model(
                "analyze_errors --fold_loops is not implemented for repeated circuits yet",
            ));
        }
        if repeat_count > MAX_ANALYZER_REPEAT_UNROLL {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "analyze_errors currently supports repeat counts up to {MAX_ANALYZER_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        for _ in 0..repeat_count {
            self.end_else_correlated_error_block();
            self.visit_circuit(repeat.body())?;
            self.end_else_correlated_error_block();
        }
        Ok(())
    }

    fn visit_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        if !matches!(
            gate_name,
            "E" | "CORRELATED_ERROR" | "ELSE_CORRELATED_ERROR"
        ) {
            self.end_else_correlated_error_block();
        }
        match gate_name {
            "X_ERROR" | "Y_ERROR" | "Z_ERROR" => self.record_single_pauli_error(instruction),
            "E" | "CORRELATED_ERROR" => self.record_correlated_error(instruction),
            "ELSE_CORRELATED_ERROR" => self.record_else_correlated_error(instruction),
            "I_ERROR" | "II_ERROR" => Ok(()),
            "PAULI_CHANNEL_1" => self.record_pauli_channel1(instruction),
            "PAULI_CHANNEL_2" => self.record_pauli_channel2(instruction),
            "DEPOLARIZE1" => self.record_depolarize1(instruction),
            "DEPOLARIZE2" => self.record_depolarize2(instruction),
            "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => self.record_measurements(instruction),
            "R" | "RX" | "RY" => self.record_resets(instruction),
            "MPAD" => self.record_measurement_pads(instruction),
            "DETECTOR" => self.record_detector(instruction),
            "OBSERVABLE_INCLUDE" => self.record_observable(instruction),
            "SHIFT_COORDS" => self.shift_coordinates(instruction),
            "TICK" | "QUBIT_COORDS" => Ok(()),
            name if is_noise_instruction(name) => Err(CircuitError::invalid_detector_error_model(
                format!("analyze_errors does not yet support {name}"),
            )),
            name if is_measurement_instruction(name) => {
                Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors does not yet support measurement instruction {name}"
                )))
            }
            _ => Ok(()),
        }
    }

    fn record_single_pauli_error(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let pauli = match instruction.gate().canonical_name() {
            "X_ERROR" => AnalyzerPauli::X,
            "Y_ERROR" => AnalyzerPauli::Y,
            "Z_ERROR" => AnalyzerPauli::Z,
            _ => unreachable!("caller restricts gate names"),
        };
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            self.push_single_qubit_pauli_error(probability, qubit, pauli);
        }
        Ok(())
    }

    fn end_else_correlated_error_block(&mut self) {
        self.else_correlated_error_remainder = None;
    }

    fn record_correlated_error(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        self.record_correlated_error_with_probability(instruction, probability)?;
        self.else_correlated_error_remainder = Some(Probability::try_new(1.0 - probability.get())?);
        Ok(())
    }

    fn record_else_correlated_error(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR requires approximate_disjoint_errors during error analysis",
            ));
        };
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        let Some(remainder) = self.else_correlated_error_remainder else {
            return Err(CircuitError::invalid_detector_error_model(
                "ELSE_CORRELATED_ERROR must immediately follow CORRELATED_ERROR or ELSE_CORRELATED_ERROR",
            ));
        };
        let actual_probability = Probability::try_new(remainder.get() * probability.get())?;
        if actual_probability.get() > threshold.get() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "CORRELATED_ERROR/ELSE_CORRELATED_ERROR block has a component probability {} larger than the approximate_disjoint_errors threshold {}",
                actual_probability.get(),
                threshold.get()
            )));
        }
        self.record_correlated_error_with_probability(instruction, actual_probability)?;
        self.else_correlated_error_remainder = Some(Probability::try_new(
            remainder.get() * (1.0 - probability.get()),
        )?);
        Ok(())
    }

    fn record_correlated_error_with_probability(
        &mut self,
        instruction: &CircuitInstruction,
        probability: Probability,
    ) -> CircuitResult<()> {
        if probability.get() == 0.0 {
            return Ok(());
        }
        let mut effects_by_qubit = BTreeMap::new();
        for target in instruction.targets() {
            let Some(pauli) = target.pauli_type() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "E target {target} is not a Pauli target"
                )));
            };
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "E target {target} does not identify a qubit"
                )));
            };
            let entry = effects_by_qubit.entry(qubit).or_insert(0);
            *entry ^= pauli_mask(pauli);
            if *entry == 0 {
                effects_by_qubit.remove(&qubit);
            }
        }
        if effects_by_qubit.is_empty() {
            return Ok(());
        }
        self.pending_errors.push(PendingError {
            probability,
            effects: effects_by_qubit
                .into_iter()
                .map(|(qubit, mask)| NoiseEffect {
                    qubit,
                    pauli: analyzer_pauli_from_mask(mask),
                })
                .collect(),
            measurements: Vec::new(),
            disjoint_group: None,
        });
        Ok(())
    }

    fn record_pauli_channel1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let [x_probability, y_probability, z_probability] = probabilities.as_slice() else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 expected three probabilities",
            ));
        };
        if let Some(independent) =
            try_disjoint_to_independent_xyz_errors(*x_probability, *y_probability, *z_probability)?
        {
            for target in instruction.targets() {
                let Some(qubit) = target.qubit_id() else {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "PAULI_CHANNEL_1 target {target} is not a qubit"
                    )));
                };
                self.push_single_qubit_pauli_error(independent.x, qubit, AnalyzerPauli::X);
                self.push_single_qubit_pauli_error(independent.y, qubit, AnalyzerPauli::Y);
                self.push_single_qubit_pauli_error(independent.z, qubit, AnalyzerPauli::Z);
            }
            return Ok(());
        }
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_1 requires approximate_disjoint_errors during error analysis",
            ));
        };
        for probability in &probabilities {
            if probability.get() > threshold.get() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_1 has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                    probability.get(),
                    threshold.get()
                )));
            }
        }
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_1 target {target} is not a qubit"
                )));
            };
            self.pending_pauli_channels
                .push(PendingSingleQubitPauliChannel {
                    qubit,
                    x_probability: *x_probability,
                    y_probability: *y_probability,
                    z_probability: *z_probability,
                });
        }
        Ok(())
    }

    fn push_single_qubit_pauli_error(
        &mut self,
        probability: Probability,
        qubit: QubitId,
        pauli: AnalyzerPauli,
    ) {
        if probability.get() == 0.0 {
            return;
        }
        self.pending_errors.push(PendingError {
            probability,
            effects: vec![NoiseEffect { qubit, pauli }],
            measurements: Vec::new(),
            disjoint_group: None,
        });
    }

    fn record_pauli_channel2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(threshold) = self.options.approximate_disjoint_errors_threshold else {
            return Err(CircuitError::invalid_detector_error_model(
                "PAULI_CHANNEL_2 requires approximate_disjoint_errors during error analysis",
            ));
        };
        let Some(probabilities) = instruction.probability_arguments()? else {
            return Ok(());
        };
        let probabilities: [Probability; 15] =
            probabilities.try_into().map_err(|probabilities: Vec<_>| {
                CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 expected 15 probabilities, got {}",
                    probabilities.len()
                ))
            })?;
        for probability in &probabilities {
            if probability.get() > threshold.get() {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                    probability.get(),
                    threshold.get()
                )));
            }
        }
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "PAULI_CHANNEL_2 expected paired qubit targets",
                ));
            };
            let Some(left_qubit) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {left} is not a qubit"
                )));
            };
            let Some(right_qubit) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "PAULI_CHANNEL_2 target {right} is not a qubit"
                )));
            };
            let group_id = self.allocate_disjoint_group_id()?;
            for (probability, left_pauli, right_pauli) in pauli_channel2_components(probabilities) {
                if probability.get() == 0.0 {
                    continue;
                }
                self.push_two_qubit_pauli_error(
                    probability,
                    left_qubit,
                    left_pauli,
                    right_qubit,
                    right_pauli,
                    Some(group_id),
                );
            }
        }
        Ok(())
    }

    fn allocate_disjoint_group_id(&mut self) -> CircuitResult<u64> {
        let group_id = self.next_disjoint_group_id;
        self.next_disjoint_group_id =
            self.next_disjoint_group_id.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("disjoint error group count overflowed")
            })?;
        Ok(group_id)
    }

    fn record_depolarize2(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        let channel_probability = depolarize2_independent_channel_probability(probability)?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(
                    "DEPOLARIZE2 expected paired qubit targets",
                ));
            };
            let Some(left_qubit) = left.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {left} is not a qubit"
                )));
            };
            let Some(right_qubit) = right.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE2 target {right} is not a qubit"
                )));
            };
            let paulis = [AnalyzerPauli::X, AnalyzerPauli::Y, AnalyzerPauli::Z];
            for left_pauli in [None, Some(paulis[0]), Some(paulis[1]), Some(paulis[2])] {
                for right_pauli in [None, Some(paulis[0]), Some(paulis[1]), Some(paulis[2])] {
                    if left_pauli.is_none() && right_pauli.is_none() {
                        continue;
                    }
                    self.push_two_qubit_pauli_error(
                        channel_probability,
                        left_qubit,
                        left_pauli,
                        right_qubit,
                        right_pauli,
                        None,
                    );
                }
            }
        }
        Ok(())
    }

    fn push_two_qubit_pauli_error(
        &mut self,
        probability: Probability,
        left_qubit: QubitId,
        left_pauli: Option<AnalyzerPauli>,
        right_qubit: QubitId,
        right_pauli: Option<AnalyzerPauli>,
        disjoint_group: Option<u64>,
    ) {
        let mut effects = Vec::new();
        if let Some(pauli) = left_pauli {
            effects.push(NoiseEffect {
                qubit: left_qubit,
                pauli,
            });
        }
        if let Some(pauli) = right_pauli {
            effects.push(NoiseEffect {
                qubit: right_qubit,
                pauli,
            });
        }
        self.pending_errors.push(PendingError {
            probability,
            effects,
            measurements: Vec::new(),
            disjoint_group,
        });
    }

    fn record_depolarize1(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let Some(probability) = instruction.probability_argument()? else {
            return Ok(());
        };
        if probability.get() == 0.0 {
            return Ok(());
        }
        if probability.get() > 0.75 {
            return Err(CircuitError::invalid_detector_error_model(
                "cannot analyze over-mixing DEPOLARIZE1 probability above 3/4",
            ));
        }
        let axis_probability = Probability::try_new(probability.get() / 3.0)?;
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEPOLARIZE1 target {target} is not a qubit"
                )));
            };
            self.pending_pauli_channels
                .push(PendingSingleQubitPauliChannel {
                    qubit,
                    x_probability: axis_probability,
                    y_probability: axis_probability,
                    z_probability: axis_probability,
                });
        }
        Ok(())
    }

    fn record_measurements(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let basis = measurement_basis(instruction.gate().canonical_name()).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("unknown measurement basis")
        })?;
        for group in instruction.target_groups() {
            let Some(target) = group.first() else {
                continue;
            };
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let measurement_index = self.measurement_count;
            self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
            for pending in &mut self.pending_errors {
                if pending.flips_measurement(qubit, basis) {
                    pending.measurements.push(measurement_index);
                }
            }
            if let Some(probability) = instruction.probability_argument()?
                && probability.get() > 0.0
            {
                self.completed_errors.push(PendingError {
                    probability,
                    effects: Vec::new(),
                    measurements: vec![measurement_index],
                    disjoint_group: None,
                });
            }
            let mut still_pending_channels = Vec::new();
            for pending in self.pending_pauli_channels.drain(..) {
                if pending.qubit == qubit {
                    let probability = pending.flip_probability(basis)?;
                    if probability.get() > 0.0 {
                        self.completed_errors.push(PendingError {
                            probability,
                            effects: Vec::new(),
                            measurements: vec![measurement_index],
                            disjoint_group: None,
                        });
                    }
                } else {
                    still_pending_channels.push(pending);
                }
            }
            self.pending_pauli_channels = still_pending_channels;
            self.cut_pending_errors_at_qubit(qubit);
        }
        Ok(())
    }

    fn record_measurement_pads(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.measurement_count = self
            .measurement_count
            .checked_add(instruction.target_groups().len())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
        Ok(())
    }

    fn record_resets(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for target in instruction.targets() {
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            self.cut_pending_errors_at_qubit(qubit);
            self.pending_pauli_channels
                .retain(|pending| pending.qubit != qubit);
        }
        Ok(())
    }

    fn cut_pending_errors_at_qubit(&mut self, qubit: QubitId) {
        let mut still_pending = Vec::new();
        for mut pending in self.pending_errors.drain(..) {
            if pending.touches_qubit(qubit) {
                pending.remove_effects_touching(qubit);
                if pending.effects.is_empty() {
                    self.completed_errors.push(pending);
                } else {
                    still_pending.push(pending);
                }
            } else {
                still_pending.push(pending);
            }
        }
        self.pending_errors = still_pending;
    }

    fn record_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let detector_id = self.detector_count;
        self.detector_count = self.detector_count.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("detector count overflowed")
        })?;
        let coordinates = shifted_coordinates(&self.coord_offset, instruction.args());
        for target in instruction.targets() {
            let Some(offset) = target.measurement_record_offset() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DETECTOR target {target} is not a measurement record"
                )));
            };
            let measurement = self.measurement_index_from_offset(offset.get())?;
            self.detector_terms_by_measurement
                .entry(measurement)
                .or_default()
                .push(detector_id);
        }
        self.detector_declarations.push(DetectorDeclaration {
            detector_id,
            coordinates,
        });
        Ok(())
    }

    fn record_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction.observable_id_argument()?.ok_or_else(|| {
            CircuitError::invalid_detector_error_model("OBSERVABLE_INCLUDE missing observable id")
        })?;
        for target in instruction.targets() {
            if let Some(offset) = target.measurement_record_offset() {
                let measurement = self.measurement_index_from_offset(offset.get())?;
                self.observable_terms_by_measurement
                    .entry(measurement)
                    .or_default()
                    .push(observable.get());
            } else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "analyze_errors does not yet support OBSERVABLE_INCLUDE target {target}"
                )));
            }
        }
        Ok(())
    }

    fn shift_coordinates(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for (index, value) in instruction.args().iter().copied().enumerate() {
            if index == self.coord_offset.len() {
                self.coord_offset.push(value);
            } else if let Some(offset) = self.coord_offset.get_mut(index) {
                *offset += value;
            }
        }
        Ok(())
    }

    fn measurement_index_from_offset(&self, offset: i32) -> CircuitResult<usize> {
        let measurement_count = i64::try_from(self.measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model("measurement count does not fit i64")
        })?;
        let index = measurement_count
            .checked_add(i64::from(offset))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement offset overflowed")
            })?;
        if index < 0 || index >= measurement_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "measurement record offset rec[{offset}] is out of range"
            )));
        }
        usize::try_from(index).map_err(|_| {
            CircuitError::invalid_detector_error_model("measurement index does not fit usize")
        })
    }

    fn into_dem(self) -> CircuitResult<DetectorErrorModel> {
        let mut dem = DetectorErrorModel::new();
        let mut merged_error_probabilities = BTreeMap::new();
        let mut disjoint_error_probabilities = BTreeMap::new();
        let mut touched_detectors = BTreeSet::new();
        for pending in self
            .completed_errors
            .into_iter()
            .chain(self.pending_errors)
            .filter(|pending| !pending.measurements.is_empty())
        {
            let mut detectors = BTreeSet::new();
            let mut observables = BTreeSet::new();
            for measurement in pending.measurements {
                toggle_all(
                    &mut detectors,
                    self.detector_terms_by_measurement
                        .get(&measurement)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
                toggle_all(
                    &mut observables,
                    self.observable_terms_by_measurement
                        .get(&measurement)
                        .into_iter()
                        .flatten()
                        .copied(),
                );
            }
            if detectors.is_empty() && observables.is_empty() {
                continue;
            }
            let mut targets = Vec::with_capacity(detectors.len() + observables.len());
            for detector in detectors {
                targets.push(DemTarget::relative_detector(detector)?);
            }
            for observable in observables {
                targets.push(DemTarget::logical_observable(observable)?);
            }
            if let Some(group_id) = pending.disjoint_group {
                merge_disjoint_probability(
                    &mut disjoint_error_probabilities,
                    (group_id, targets),
                    pending.probability,
                )?;
            } else {
                merge_independent_probability(
                    &mut merged_error_probabilities,
                    targets,
                    pending.probability,
                )?;
            }
        }

        for ((_group_id, targets), probability) in disjoint_error_probabilities {
            merge_independent_probability(&mut merged_error_probabilities, targets, probability)?;
        }

        for (targets, probability) in merged_error_probabilities {
            if probability.get() == 0.0 {
                continue;
            }
            touched_detectors.extend(targets.iter().filter_map(|target| match target {
                DemTarget::RelativeDetector(id) => Some(id.get()),
                _ => None,
            }));
            dem.push_instruction(DemInstruction::error(probability, targets, None)?);
        }

        for declaration in self.detector_declarations {
            if declaration.coordinates.is_empty()
                && touched_detectors.contains(&declaration.detector_id)
            {
                continue;
            }
            dem.push_instruction(DemInstruction::detector(
                declaration.coordinates,
                DemTarget::relative_detector(declaration.detector_id)?,
                None,
            )?);
        }
        Ok(dem)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnalyzerPauli {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AnalyzerBasis {
    X,
    Y,
    Z,
}

#[derive(Clone, Debug)]
struct NoiseEffect {
    qubit: QubitId,
    pauli: AnalyzerPauli,
}

#[derive(Clone, Debug)]
struct PendingError {
    probability: Probability,
    effects: Vec<NoiseEffect>,
    measurements: Vec<usize>,
    disjoint_group: Option<u64>,
}

#[derive(Clone, Debug)]
struct PendingSingleQubitPauliChannel {
    qubit: QubitId,
    x_probability: Probability,
    y_probability: Probability,
    z_probability: Probability,
}

impl PendingSingleQubitPauliChannel {
    fn flip_probability(&self, basis: AnalyzerBasis) -> CircuitResult<Probability> {
        let probability = match basis {
            AnalyzerBasis::X => self.y_probability.get() + self.z_probability.get(),
            AnalyzerBasis::Y => self.x_probability.get() + self.z_probability.get(),
            AnalyzerBasis::Z => self.x_probability.get() + self.y_probability.get(),
        };
        Probability::try_new(probability)
    }
}

impl PendingError {
    fn touches_qubit(&self, qubit: QubitId) -> bool {
        self.effects.iter().any(|effect| effect.qubit == qubit)
    }

    fn remove_effects_touching(&mut self, qubit: QubitId) {
        self.effects.retain(|effect| effect.qubit != qubit);
    }

    fn flips_measurement(&self, qubit: QubitId, basis: AnalyzerBasis) -> bool {
        self.effects.iter().any(|effect| {
            effect.qubit == qubit
                && matches!(
                    (effect.pauli, basis),
                    (AnalyzerPauli::X, AnalyzerBasis::Y | AnalyzerBasis::Z)
                        | (AnalyzerPauli::Y, AnalyzerBasis::X | AnalyzerBasis::Z)
                        | (AnalyzerPauli::Z, AnalyzerBasis::X | AnalyzerBasis::Y)
                )
        })
    }
}

#[derive(Clone, Debug)]
struct DetectorDeclaration {
    detector_id: u64,
    coordinates: Vec<f64>,
}

fn shifted_coordinates(offset: &[f64], local: &[f64]) -> Vec<f64> {
    local
        .iter()
        .copied()
        .enumerate()
        .map(|(index, value)| offset.get(index).copied().unwrap_or(0.0) + value)
        .collect()
}

fn measurement_basis(name: &str) -> Option<AnalyzerBasis> {
    match name {
        "M" | "MR" => Some(AnalyzerBasis::Z),
        "MX" | "MRX" => Some(AnalyzerBasis::X),
        "MY" | "MRY" => Some(AnalyzerBasis::Y),
        _ => None,
    }
}

fn is_measurement_instruction(name: &str) -> bool {
    matches!(
        name,
        "MXX" | "MYY" | "MZZ" | "MPP" | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn is_noise_instruction(name: &str) -> bool {
    matches!(
        name,
        "DEPOLARIZE1"
            | "DEPOLARIZE2"
            | "I_ERROR"
            | "II_ERROR"
            | "PAULI_CHANNEL_1"
            | "PAULI_CHANNEL_2"
            | "ELSE_CORRELATED_ERROR"
            | "E"
    )
}

fn toggle_all(target: &mut BTreeSet<u64>, values: impl Iterator<Item = u64>) {
    for value in values {
        if !target.insert(value) {
            target.remove(&value);
        }
    }
}

fn merge_independent_probability(
    probabilities: &mut BTreeMap<Vec<DemTarget>, Probability>,
    targets: Vec<DemTarget>,
    probability: Probability,
) -> CircuitResult<()> {
    if let Some(existing) = probabilities.get_mut(&targets) {
        *existing = xor_probability(*existing, probability)?;
    } else {
        probabilities.insert(targets, probability);
    }
    Ok(())
}

fn merge_disjoint_probability(
    probabilities: &mut BTreeMap<(u64, Vec<DemTarget>), Probability>,
    key: (u64, Vec<DemTarget>),
    probability: Probability,
) -> CircuitResult<()> {
    if let Some(existing) = probabilities.get_mut(&key) {
        *existing = Probability::try_new(existing.get() + probability.get())?;
    } else {
        probabilities.insert(key, probability);
    }
    Ok(())
}

fn xor_probability(left: Probability, right: Probability) -> CircuitResult<Probability> {
    Probability::try_new(left.get() + right.get() - 2.0 * left.get() * right.get())
}

fn pauli_mask(pauli: Pauli) -> u8 {
    match pauli {
        Pauli::X => 0b01,
        Pauli::Y => 0b11,
        Pauli::Z => 0b10,
    }
}

fn analyzer_pauli_from_mask(mask: u8) -> AnalyzerPauli {
    match mask {
        0b01 => AnalyzerPauli::X,
        0b10 => AnalyzerPauli::Z,
        0b11 => AnalyzerPauli::Y,
        _ => unreachable!("pauli masks are maintained by xor of X/Z bits"),
    }
}
