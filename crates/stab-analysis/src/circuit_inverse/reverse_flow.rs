use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::{Flow, PauliSign, PauliString};
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, DemTarget, FoldedCount, Gate, GateKind,
    MeasureRecordOffset, MemoryBudget, ParseLimits, QubitId, Target, advanced::FoldedCircuitEvent,
};

use crate::{
    AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation,
    circuit_flow::{
        UnsignedFlowCheckOptions, check_unsigned_flows_with_sparse_tracker, stops_flow_check,
        transitions::ReverseFlowTransition,
    },
    sparse_rev_frame_tracker::{AnticommutationPolicy, SparseReverseFrameTracker},
};

use super::{TimeReversedForFlowsOptions, reversed_target_groups};
use crate::circuit_flow::transitions::reverse_flow_transition;
use crate::controlled_pauli::{ControlSlot, controlled_pauli_form};

pub(super) fn requires_general_reversal(circuit: &Circuit, flows: &[Flow]) -> bool {
    flows
        .iter()
        .any(|flow| flow.measurements().next().is_some() || flow.observables().next().is_some())
        || any_nested_instruction(circuit, instruction_requires_general_reversal)
}

/// Reports whether any instruction at any nesting depth satisfies `predicate`, walking repeat
/// bodies with an explicit stack.
fn any_nested_instruction(
    circuit: &Circuit,
    mut predicate: impl FnMut(&CircuitInstruction) -> bool,
) -> bool {
    let mut stack = vec![circuit.items().iter()];
    while let Some(items) = stack.last_mut() {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => {
                if predicate(instruction) {
                    return true;
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => stack.push(repeat.body().items().iter()),
            None => {
                stack.pop();
            }
        }
    }
    false
}

pub(super) fn reverse_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
) -> AnalysisResult<(Circuit, Vec<Flow>)> {
    reject_flow_observable_terms(flows)?;
    validate_general_reversal(circuit, options.memory_budget)?;
    let measurement_count = usize::try_from(circuit.count_measurements()?)
        .map_err(|_| reverse_error("measurement count does not fit the platform index width"))?;
    let detector_count = circuit.count_detectors()?;
    let observable_count = circuit.count_observables()?;
    let qubit_count = flow_aware_qubit_count(circuit, flows)?;
    let flow_states = reverse_flow_states(observable_count, measurement_count, flows)?;
    let mut tracker = SparseReverseFrameTracker::new(
        qubit_count,
        measurement_count,
        detector_count,
        AnticommutationPolicy::Fail,
    );
    tracker.set_cancellation(options.cancellation.clone());
    tracker.set_memory_budget(
        ResourceOperation::TimeReversal,
        ResourceKind::MaterializedBytes,
        options.memory_budget,
    );
    let mut engine = ReverseFlowEngine {
        tracker,
        inverted: Circuit::new(),
        qubit_coordinates: Vec::new(),
        coordinate_shift: Vec::new(),
        detector_measurements: BTreeMap::new(),
        target_coordinates: BTreeMap::new(),
        target_tags: BTreeMap::new(),
        new_measurement_count: 0,
        observable_count,
        options,
    };

    engine.seed_flow_outputs(&flow_states)?;
    engine.tracker.check_live_budget()?;
    for instruction in circuit.iter_flattened_instructions_reverse() {
        engine.tracker.poll_cancellation()?;
        engine.reverse_instruction(instruction)?;
    }
    engine.seed_flow_inputs(&flow_states)?;
    engine.verify_closed_targets(&flow_states)?;
    let output_flows = engine.build_output_flows(&flow_states)?;
    let flow_check_options = engine.options.flow_check_options();
    let output_circuit = engine.finish();
    validate_output_flows(&output_circuit, &output_flows, &flow_check_options)?;
    Ok((output_circuit, output_flows))
}

struct ReverseFlowEngine {
    tracker: SparseReverseFrameTracker,
    inverted: Circuit,
    qubit_coordinates: Vec<CircuitInstruction>,
    coordinate_shift: Vec<f64>,
    detector_measurements: BTreeMap<DemTarget, BTreeSet<usize>>,
    target_coordinates: BTreeMap<DemTarget, Vec<f64>>,
    target_tags: BTreeMap<DemTarget, Option<Box<[u8]>>>,
    new_measurement_count: usize,
    observable_count: u64,
    options: TimeReversedForFlowsOptions,
}

struct ReverseFlowState<'a> {
    target: DemTarget,
    input: &'a PauliString,
    output: &'a PauliString,
    measurements: Vec<i32>,
}

impl ReverseFlowState<'_> {
    fn original_flow(&self) -> AnalysisResult<Flow> {
        Flow::new(
            self.input.clone(),
            self.output.clone(),
            self.measurements.iter().copied(),
            [],
        )
        .map_err(|error| reverse_error(error.to_string()))
    }
}

impl ReverseFlowEngine {
    fn seed_flow_outputs(&mut self, states: &[ReverseFlowState<'_>]) -> AnalysisResult<()> {
        for state in states {
            self.toggle_pauli(state.output, state.target)?;
            for measurement in state.measurements.iter().copied() {
                let index =
                    absolute_measurement_index(measurement, self.tracker.measurement_count())?;
                self.tracker
                    .toggle_record_target_absolute(index, state.target)?;
            }
        }
        Ok(())
    }

    fn seed_flow_inputs(&mut self, states: &[ReverseFlowState<'_>]) -> AnalysisResult<()> {
        for state in states {
            self.toggle_pauli(state.input, state.target)?;
        }
        Ok(())
    }

    fn toggle_pauli(&mut self, pauli: &PauliString, target: DemTarget) -> AnalysisResult<()> {
        for (index, basis) in pauli.active_terms() {
            let qubit = QubitId::new(u32::try_from(index).map_err(|_| {
                reverse_error(format!("flow qubit index {index} exceeds {}", u32::MAX))
            })?)?;
            self.tracker.toggle_pauli_target(qubit, basis, target)?;
        }
        Ok(())
    }

    fn reverse_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        match reverse_flow_transition(instruction) {
            ReverseFlowTransition::Measurement(_) => self.reverse_measurement(instruction),
            ReverseFlowTransition::Reset(_) | ReverseFlowTransition::MeasureReset(_) => {
                self.reverse_reset_or_measure_reset(instruction)
            }
            ReverseFlowTransition::PairMeasurement(_)
            | ReverseFlowTransition::PauliProductMeasurement
            | ReverseFlowTransition::MeasurementPad => {
                self.reverse_measuring_instruction(instruction)
            }
            ReverseFlowTransition::Detector => self.reverse_detector(instruction),
            ReverseFlowTransition::Observable => self.reverse_observable(instruction),
            ReverseFlowTransition::ControlledPauli(_)
            | ReverseFlowTransition::SweepControlledPauliNoop
            | ReverseFlowTransition::PauliProductUnitary
            | ReverseFlowTransition::Tableau => self.reverse_simple(instruction),
            ReverseFlowTransition::HeraldedMeasurement => self.reverse_simple(instruction),
            ReverseFlowTransition::Ignored => self.reverse_ignored(instruction),
            ReverseFlowTransition::Unsupported => Err(reverse_error(format!(
                "don't know how to time-reverse {}",
                instruction_text(instruction)
            ))),
        }
    }

    fn reverse_measurement(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let reset_gate = measurement_reset_gate(instruction.gate())?;
        for group in instruction.target_groups().into_iter().rev() {
            let [target] = group else {
                return Err(reverse_error(format!(
                    "measurement target group is not a single qubit in {}",
                    instruction_text(instruction)
                )));
            };
            let qubit = target.qubit_id().ok_or_else(|| {
                reverse_error(format!("measurement target {target} is not a qubit"))
            })?;
            let record_index = self
                .tracker
                .measurement_count()
                .checked_sub(1)
                .ok_or_else(|| reverse_error("measurement count underflowed during reversal"))?;
            let record_targets = self.tracker.record_targets_at(record_index)?;
            let pauli_targets = self.tracker.pauli_targets_at(qubit)?;
            let turn_into_reset = !self.options.dont_turn_measurements_into_resets
                && instruction.args().is_empty()
                && pauli_targets.is_empty()
                && !record_targets.is_empty();

            if turn_into_reset {
                self.append_instruction(
                    reset_gate,
                    Vec::new(),
                    vec![*target],
                    instruction.tag_bytes(),
                )?;
            } else {
                self.record_new_measurement(&record_targets);
                self.append_instruction(
                    instruction.gate().generalized_inverse(),
                    instruction.args().to_vec(),
                    vec![*target],
                    instruction.tag_bytes(),
                )?;
            }

            let tracker_instruction = stab_model::advanced::circuit_instruction_with_tag_bytes(
                instruction.gate(),
                Vec::new(),
                vec![*target],
                instruction.tag_bytes(),
            )?;
            self.tracker.undo_instruction(&tracker_instruction)?;
        }
        self.flush_detectors_and_observables()
    }

    fn reverse_reset_or_measure_reset(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let transition = reverse_flow_transition(instruction);
        let is_measure_reset = matches!(transition, ReverseFlowTransition::MeasureReset(_));
        let mut segment_end = instruction.targets().len();
        while segment_end > 0 {
            let mut segment_qubits = BTreeSet::new();
            let mut segment_start = segment_end;
            while segment_start > 0 {
                let target = instruction
                    .targets()
                    .get(segment_start - 1)
                    .ok_or_else(|| {
                        reverse_error("reset segment index escaped the instruction target list")
                    })?;
                let qubit = target.qubit_id().ok_or_else(|| {
                    reverse_error(format!("reset target {target} is not a qubit"))
                })?;
                if !segment_qubits.insert(qubit) {
                    break;
                }
                segment_start -= 1;
            }
            self.reverse_reset_segment(instruction, segment_start, segment_end, is_measure_reset)?;
            segment_end = segment_start;
        }
        self.flush_detectors_and_observables()
    }

    fn reverse_reset_segment(
        &mut self,
        instruction: &CircuitInstruction,
        start: usize,
        end: usize,
        is_measure_reset: bool,
    ) -> AnalysisResult<()> {
        let segment_targets = instruction
            .targets()
            .get(start..end)
            .ok_or_else(|| reverse_error("reset segment range escaped the instruction targets"))?
            .to_vec();
        for target in segment_targets.iter().rev() {
            let qubit = target
                .qubit_id()
                .ok_or_else(|| reverse_error(format!("reset target {target} is not a qubit")))?;
            let pauli_targets = self.tracker.pauli_targets_at(qubit)?;
            self.record_new_measurement(&pauli_targets);
        }

        let tracker_instruction = stab_model::advanced::circuit_instruction_with_tag_bytes(
            instruction.gate(),
            Vec::new(),
            segment_targets.clone(),
            instruction.tag_bytes(),
        )?;
        self.tracker.undo_instruction(&tracker_instruction)?;

        let reversed_targets = segment_targets.into_iter().rev().collect::<Vec<_>>();
        self.append_instruction(
            instruction.gate().generalized_inverse(),
            Vec::new(),
            reversed_targets.clone(),
            instruction.tag_bytes(),
        )?;
        if is_measure_reset && !instruction.args().is_empty() {
            let error_gate = match instruction.gate().kind() {
                GateKind::MR => GateKind::X_ERROR,
                GateKind::MRX | GateKind::MRY => GateKind::Z_ERROR,
                GateKind::DETECTOR
                | GateKind::OBSERVABLE_INCLUDE
                | GateKind::TICK
                | GateKind::QUBIT_COORDS
                | GateKind::SHIFT_COORDS
                | GateKind::REPEAT
                | GateKind::MPAD
                | GateKind::MX
                | GateKind::MY
                | GateKind::M
                | GateKind::RX
                | GateKind::RY
                | GateKind::R
                | GateKind::XCX
                | GateKind::XCY
                | GateKind::XCZ
                | GateKind::YCX
                | GateKind::YCY
                | GateKind::YCZ
                | GateKind::CX
                | GateKind::CY
                | GateKind::CZ
                | GateKind::H
                | GateKind::H_XY
                | GateKind::H_YZ
                | GateKind::H_NXY
                | GateKind::H_NXZ
                | GateKind::H_NYZ
                | GateKind::DEPOLARIZE1
                | GateKind::DEPOLARIZE2
                | GateKind::X_ERROR
                | GateKind::Y_ERROR
                | GateKind::Z_ERROR
                | GateKind::I_ERROR
                | GateKind::II_ERROR
                | GateKind::PAULI_CHANNEL_1
                | GateKind::PAULI_CHANNEL_2
                | GateKind::E
                | GateKind::ELSE_CORRELATED_ERROR
                | GateKind::HERALDED_ERASE
                | GateKind::HERALDED_PAULI_CHANNEL_1
                | GateKind::I
                | GateKind::X
                | GateKind::Y
                | GateKind::Z
                | GateKind::C_XYZ
                | GateKind::C_ZYX
                | GateKind::C_NXYZ
                | GateKind::C_XNYZ
                | GateKind::C_XYNZ
                | GateKind::C_NZYX
                | GateKind::C_ZNYX
                | GateKind::C_ZYNX
                | GateKind::SQRT_X
                | GateKind::SQRT_X_DAG
                | GateKind::SQRT_Y
                | GateKind::SQRT_Y_DAG
                | GateKind::S
                | GateKind::S_DAG
                | GateKind::II
                | GateKind::SQRT_XX
                | GateKind::SQRT_XX_DAG
                | GateKind::SQRT_YY
                | GateKind::SQRT_YY_DAG
                | GateKind::SQRT_ZZ
                | GateKind::SQRT_ZZ_DAG
                | GateKind::MPP
                | GateKind::SPP
                | GateKind::SPP_DAG
                | GateKind::SWAP
                | GateKind::ISWAP
                | GateKind::CXSWAP
                | GateKind::SWAPCX
                | GateKind::CZSWAP
                | GateKind::ISWAP_DAG
                | GateKind::MXX
                | GateKind::MYY
                | GateKind::MZZ => {
                    return Err(reverse_error(format!(
                        "don't know how to eject measurement noise from {}",
                        instruction.gate().canonical_name()
                    )));
                }
            };
            self.append_instruction(
                error_gate.gate(),
                instruction.args().to_vec(),
                reversed_targets,
                instruction.tag_bytes(),
            )?;
        }
        Ok(())
    }

    fn reverse_measuring_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        let measurement_count = instruction.target_groups().len();
        let remaining_measurements = self.tracker.measurement_count();
        if remaining_measurements < measurement_count {
            return Err(reverse_error(format!(
                "measurement count underflowed while reversing {}",
                instruction_text(instruction)
            )));
        }
        for offset in 0..measurement_count {
            let record_index = remaining_measurements - offset - 1;
            let record_targets = self.tracker.record_targets_at(record_index)?;
            self.record_new_measurement(&record_targets);
        }
        self.tracker.undo_instruction(instruction)?;
        self.append_instruction(
            instruction.gate().generalized_inverse(),
            instruction.args().to_vec(),
            reversed_target_groups(instruction),
            instruction.tag_bytes(),
        )?;
        self.flush_detectors_and_observables()
    }

    fn reverse_detector(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.tracker.undo_instruction(instruction)?;
        let target = DemTarget::relative_detector(self.tracker.detector_count())?;
        self.target_coordinates.insert(
            target,
            shifted_coordinates(instruction.args(), &self.coordinate_shift),
        );
        self.target_tags
            .insert(target, instruction.tag_bytes().map(Box::<[u8]>::from));
        Ok(())
    }

    fn reverse_observable(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        let observable = instruction
            .observable_id_argument()?
            .ok_or_else(|| reverse_error("OBSERVABLE_INCLUDE is missing an observable id"))?;
        let target = DemTarget::logical_observable(observable.get())?;
        let pauli_targets = instruction
            .targets()
            .iter()
            .filter(|target| target.is_pauli_target())
            .copied()
            .collect::<Vec<_>>();
        if pauli_targets.is_empty() {
            self.target_tags
                .insert(target, instruction.tag_bytes().map(Box::<[u8]>::from));
        } else {
            self.append_instruction(
                instruction.gate(),
                instruction.args().to_vec(),
                pauli_targets,
                instruction.tag_bytes(),
            )?;
        }
        self.tracker.undo_instruction(instruction)?;
        Ok(())
    }

    fn reverse_simple(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.tracker.undo_instruction(instruction)?;
        self.append_instruction(
            instruction.gate().generalized_inverse(),
            instruction.args().to_vec(),
            reversed_target_groups(instruction),
            instruction.tag_bytes(),
        )
    }

    fn reverse_ignored(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        match instruction.gate().kind() {
            GateKind::QUBIT_COORDS => {
                let shifted = shifted_coordinates(instruction.args(), &self.coordinate_shift);
                self.qubit_coordinates.push(
                    stab_model::advanced::circuit_instruction_with_tag_bytes(
                        instruction.gate(),
                        shifted,
                        instruction.targets().to_vec(),
                        instruction.tag_bytes(),
                    )?,
                );
                Ok(())
            }
            GateKind::SHIFT_COORDS => {
                add_coordinate_shift(&mut self.coordinate_shift, instruction.args());
                Ok(())
            }
            GateKind::DETECTOR
            | GateKind::OBSERVABLE_INCLUDE
            | GateKind::TICK
            | GateKind::REPEAT
            | GateKind::MPAD
            | GateKind::MX
            | GateKind::MY
            | GateKind::M
            | GateKind::MRX
            | GateKind::MRY
            | GateKind::MR
            | GateKind::RX
            | GateKind::RY
            | GateKind::R
            | GateKind::XCX
            | GateKind::XCY
            | GateKind::XCZ
            | GateKind::YCX
            | GateKind::YCY
            | GateKind::YCZ
            | GateKind::CX
            | GateKind::CY
            | GateKind::CZ
            | GateKind::H
            | GateKind::H_XY
            | GateKind::H_YZ
            | GateKind::H_NXY
            | GateKind::H_NXZ
            | GateKind::H_NYZ
            | GateKind::DEPOLARIZE1
            | GateKind::DEPOLARIZE2
            | GateKind::X_ERROR
            | GateKind::Y_ERROR
            | GateKind::Z_ERROR
            | GateKind::I_ERROR
            | GateKind::II_ERROR
            | GateKind::PAULI_CHANNEL_1
            | GateKind::PAULI_CHANNEL_2
            | GateKind::E
            | GateKind::ELSE_CORRELATED_ERROR
            | GateKind::HERALDED_ERASE
            | GateKind::HERALDED_PAULI_CHANNEL_1
            | GateKind::I
            | GateKind::X
            | GateKind::Y
            | GateKind::Z
            | GateKind::C_XYZ
            | GateKind::C_ZYX
            | GateKind::C_NXYZ
            | GateKind::C_XNYZ
            | GateKind::C_XYNZ
            | GateKind::C_NZYX
            | GateKind::C_ZNYX
            | GateKind::C_ZYNX
            | GateKind::SQRT_X
            | GateKind::SQRT_X_DAG
            | GateKind::SQRT_Y
            | GateKind::SQRT_Y_DAG
            | GateKind::S
            | GateKind::S_DAG
            | GateKind::II
            | GateKind::SQRT_XX
            | GateKind::SQRT_XX_DAG
            | GateKind::SQRT_YY
            | GateKind::SQRT_YY_DAG
            | GateKind::SQRT_ZZ
            | GateKind::SQRT_ZZ_DAG
            | GateKind::MPP
            | GateKind::SPP
            | GateKind::SPP_DAG
            | GateKind::SWAP
            | GateKind::ISWAP
            | GateKind::CXSWAP
            | GateKind::SWAPCX
            | GateKind::CZSWAP
            | GateKind::ISWAP_DAG
            | GateKind::MXX
            | GateKind::MYY
            | GateKind::MZZ => self.reverse_simple(instruction),
        }
    }

    fn record_new_measurement(&mut self, targets: &BTreeSet<DemTarget>) {
        for target in targets {
            self.detector_measurements
                .entry(*target)
                .or_default()
                .insert(self.new_measurement_count);
        }
        self.new_measurement_count += 1;
    }

    fn flush_detectors_and_observables(&mut self) -> AnalysisResult<()> {
        let active_targets = self.tracker.active_targets();
        let ready = self
            .detector_measurements
            .keys()
            .copied()
            .filter(|target| match target {
                DemTarget::LogicalObservable(observable) => {
                    observable.get() < self.observable_count
                }
                DemTarget::RelativeDetector(_) => !active_targets.contains(target),
                DemTarget::Separator | DemTarget::Numeric(_) => false,
            })
            .collect::<Vec<_>>();
        for target in ready {
            let measurements = self.detector_measurements.remove(&target).ok_or_else(|| {
                reverse_error(format!(
                    "ready reverse-flow target {target} lost its measurement mapping"
                ))
            })?;
            let record_targets = measurements
                .into_iter()
                .map(|measurement| {
                    output_measurement_target(measurement, self.new_measurement_count)
                })
                .collect::<AnalysisResult<Vec<_>>>()?;
            let tag = self.target_tags.remove(&target).flatten();
            match target {
                DemTarget::RelativeDetector(_) => {
                    let coordinates = self.target_coordinates.remove(&target).ok_or_else(|| {
                        reverse_error(format!(
                            "ready detector target {target} lost its coordinate mapping"
                        ))
                    })?;
                    self.append_instruction(
                        GateKind::DETECTOR.gate(),
                        coordinates,
                        record_targets,
                        tag.as_deref(),
                    )?;
                }
                DemTarget::LogicalObservable(observable) => {
                    self.append_instruction(
                        GateKind::OBSERVABLE_INCLUDE.gate(),
                        vec![observable.get() as f64],
                        record_targets,
                        tag.as_deref(),
                    )?;
                }
                DemTarget::Separator | DemTarget::Numeric(_) => {
                    return Err(reverse_error(format!(
                        "unexpected reverse-flow target {target}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn verify_closed_targets(&self, states: &[ReverseFlowState<'_>]) -> AnalysisResult<()> {
        let active = self.tracker.active_targets();
        if let Some(target) = active.first() {
            if let Some(state) = states.iter().find(|state| state.target == *target) {
                let original_flow = state.original_flow()?;
                return Err(reverse_error(format!(
                    "the circuit didn't satisfy one of the given flows (ignoring sign): {}",
                    original_flow
                )));
            }
            return Err(reverse_error(format!(
                "the detecting region of {target} reached the start of the circuit; only flows given as arguments may touch the circuit boundary"
            )));
        }
        Ok(())
    }

    fn build_output_flows(&self, states: &[ReverseFlowState<'_>]) -> AnalysisResult<Vec<Flow>> {
        states
            .iter()
            .map(|state| {
                let measurements = self
                    .detector_measurements
                    .get(&state.target)
                    .into_iter()
                    .flatten()
                    .copied()
                    .map(|measurement| {
                        output_measurement_index(measurement, self.new_measurement_count)
                    })
                    .collect::<AnalysisResult<Vec<_>>>()?;
                Flow::new(
                    state.output.with_sign(PauliSign::Plus),
                    state.input.with_sign(PauliSign::Plus),
                    measurements,
                    [],
                )
                .map_err(|error| reverse_error(error.to_string()))
            })
            .collect()
    }

    fn append_instruction(
        &mut self,
        gate: Gate,
        args: Vec<f64>,
        targets: Vec<Target>,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        self.inverted
            .append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
                gate, args, targets, tag,
            )?);
        Ok(())
    }

    fn finish(self) -> Circuit {
        if self.qubit_coordinates.is_empty() {
            return self.inverted;
        }
        let mut result = Circuit::new();
        for instruction in self.qubit_coordinates.into_iter().rev() {
            result.append_instruction(instruction);
        }
        for item in self.inverted.items() {
            if let CircuitItem::Instruction(instruction) = item {
                result.append_instruction(instruction.clone());
            }
        }
        result
    }
}

fn instruction_requires_general_reversal(instruction: &CircuitInstruction) -> bool {
    !instruction.gate().is_unitary()
        || instruction
            .targets()
            .iter()
            .any(Target::is_classical_bit_target)
}

/// Measurement-rich reversal expands every repeat into the inverted circuit, its detector and
/// coordinate maps, and the finished copy.
const MEASUREMENT_RICH_RETAINED_COPIES: u64 = 3;

fn validate_general_reversal(circuit: &Circuit, budget: MemoryBudget) -> AnalysisResult<()> {
    let projected = projected_reversal_bytes(circuit)?
        .checked_mul(MEASUREMENT_RICH_RETAINED_COPIES)
        .ok_or_else(|| reverse_error("measurement-rich repeat expansion size overflowed"))?;
    if !budget.admits(u128::from(projected)) {
        return Err(ResourceLimitError::memory_budget(
            ResourceOperation::TimeReversal,
            ResourceKind::MaterializedBytes,
            projected,
            budget.bytes(),
        )
        .into());
    }
    validate_instructions(circuit)
}

/// Conservative retained bytes of one expanded copy of `circuit`, with checked arithmetic.
fn projected_reversal_bytes(circuit: &Circuit) -> AnalysisResult<u64> {
    let overflow = || reverse_error("measurement-rich repeat expansion size overflowed");
    let mut total = 0_u64;
    crate::folded_walk::walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |depth, limit| {
            reverse_error(format!(
                "measurement-rich repeat expansion nests repeat blocks {depth} deep, beyond the {limit}-level envelope"
            ))
        },
        |event| {
            let FoldedCircuitEvent::Instruction {
                instruction,
                executions,
            } = event
            else {
                return Ok(());
            };
            let bytes = (size_of::<CircuitItem>() as u64)
                .checked_add(
                    (instruction.targets().len() as u64)
                        .checked_mul(size_of::<Target>() as u64)
                        .ok_or_else(overflow)?,
                )
                .and_then(|bytes| bytes.checked_add(instruction.args().len() as u64 * 8))
                .ok_or_else(overflow)?;
            let expanded = FoldedCount::new(bytes)
                .times(executions)
                .exact()
                .ok_or_else(overflow)?;
            total = total.checked_add(expanded).ok_or_else(overflow)?;
            Ok(())
        },
    )?;
    Ok(total)
}

fn validate_instructions(circuit: &Circuit) -> AnalysisResult<()> {
    let mut failure = None;
    any_nested_instruction(circuit, |instruction| {
        failure = validate_instruction(instruction).err();
        failure.is_some()
    });
    failure.map_or(Ok(()), Err)
}

fn validate_instruction(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    if instruction.args().iter().any(|arg| !arg.is_finite()) {
        return Err(reverse_error(format!(
            "time reversal requires finite instruction arguments: {}",
            instruction_text(instruction)
        )));
    }
    if matches!(
        reverse_flow_transition(instruction),
        ReverseFlowTransition::ControlledPauli(_)
    ) && instruction
        .targets()
        .iter()
        .any(Target::is_measurement_record_target)
    {
        return Err(reverse_error(format!(
            "time-reversing feedback isn't supported yet; found feedback in: {}",
            instruction_text(instruction)
        )));
    }
    validate_sweep_target_order(instruction)?;
    if instruction.gate().kind() == GateKind::ELSE_CORRELATED_ERROR
        || matches!(
            reverse_flow_transition(instruction),
            ReverseFlowTransition::Unsupported
        )
    {
        return Err(reverse_error(format!(
            "don't know how to time-reverse {}",
            instruction_text(instruction)
        )));
    }
    Ok(())
}

fn validate_sweep_target_order(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    // A sweep bit is valid only in the gate's classical-control slot.
    let invalid_side = match controlled_pauli_form(instruction.gate()).map(|form| form.slot) {
        Some(ControlSlot::First) => SweepInvalidSide::Right,
        Some(ControlSlot::Second) => SweepInvalidSide::Left,
        Some(ControlSlot::Either) | None => return Ok(()),
    };
    for group in instruction.target_groups() {
        let [left, right] = group else {
            continue;
        };
        let target = match invalid_side {
            SweepInvalidSide::Left => left,
            SweepInvalidSide::Right => right,
        };
        if target.is_sweep_bit_target() {
            return Err(reverse_error(format!(
                "time reversal requires gate-valid sweep target ordering; {} has sweep target {target} on its qubit-only side",
                instruction_text(instruction)
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SweepInvalidSide {
    Left,
    Right,
}

fn flow_aware_qubit_count(circuit: &Circuit, flows: &[Flow]) -> AnalysisResult<usize> {
    let flow_qubits = flows
        .iter()
        .flat_map(|flow| [flow.input().len(), flow.output().len()])
        .max()
        .unwrap_or(0);
    let count = stab_model::advanced::circuit_simulated_qubit_count(circuit).max(flow_qubits);
    if count > u32::MAX as usize {
        return Err(reverse_error(format!(
            "flow qubit count {count} exceeds {}",
            u32::MAX
        )));
    }
    Ok(count)
}

fn reverse_flow_states<'a>(
    observable_count: u64,
    measurement_count: usize,
    flows: &'a [Flow],
) -> AnalysisResult<Vec<ReverseFlowState<'a>>> {
    flows
        .iter()
        .enumerate()
        .map(|(index, flow)| {
            reject_measurement_record_aliases(index, flow, measurement_count)?;
            let index =
                u64::try_from(index).map_err(|_| reverse_error("flow count does not fit u64"))?;
            let observable = observable_count
                .checked_add(index)
                .ok_or_else(|| reverse_error("flow target observable id overflowed"))?;
            Ok(ReverseFlowState {
                target: DemTarget::logical_observable(observable)?,
                input: flow.input(),
                output: flow.output(),
                measurements: flow.measurements().collect(),
            })
        })
        .collect()
}

fn reject_flow_observable_terms(flows: &[Flow]) -> AnalysisResult<()> {
    for (flow_index, flow) in flows.iter().enumerate() {
        if let Some(observable) = flow.observables().next() {
            return Err(reverse_error(format!(
                "flow {flow_index} contains obs[{observable}], but time reversal does not define how observable dependencies map into returned flows"
            )));
        }
    }
    Ok(())
}

fn reject_measurement_record_aliases(
    flow_index: usize,
    flow: &Flow,
    measurement_count: usize,
) -> AnalysisResult<()> {
    let mut resolved = BTreeMap::new();
    for measurement in flow.measurements() {
        let absolute = absolute_measurement_index(measurement, measurement_count)?;
        if let Some(previous) = resolved.insert(absolute, measurement)
            && previous != measurement
        {
            return Err(reverse_error(format!(
                "flow {flow_index} contains distinct measurement terms rec[{previous}] and rec[{measurement}] that alias absolute record {absolute}; pinned Stim rejects this flow instead of XOR-cancelling the aliases"
            )));
        }
    }
    Ok(())
}

fn absolute_measurement_index(index: i32, measurement_count: usize) -> AnalysisResult<usize> {
    let count = i64::try_from(measurement_count)
        .map_err(|_| reverse_error("measurement count does not fit i64"))?;
    let index = i64::from(index);
    let absolute = if index < 0 {
        count
            .checked_add(index)
            .ok_or_else(|| reverse_error("flow measurement index underflowed"))?
    } else {
        index
    };
    if !(0..count).contains(&absolute) {
        return Err(reverse_error(format!(
            "out of range measurement rec[{index}] in a flow with {measurement_count} circuit measurements"
        )));
    }
    usize::try_from(absolute)
        .map_err(|_| reverse_error("absolute flow measurement index does not fit usize"))
}

fn output_measurement_target(index: usize, count: usize) -> AnalysisResult<Target> {
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        output_measurement_index(index, count)?,
    )?))
}

fn output_measurement_index(index: usize, count: usize) -> AnalysisResult<i32> {
    let index = i64::try_from(index)
        .map_err(|_| reverse_error("output measurement index does not fit i64"))?;
    let count = i64::try_from(count)
        .map_err(|_| reverse_error("output measurement count does not fit i64"))?;
    i32::try_from(index - count)
        .map_err(|_| reverse_error("output measurement record offset does not fit i32"))
}

fn measurement_reset_gate(measurement: Gate) -> AnalysisResult<Gate> {
    match measurement.kind() {
        GateKind::M => Ok(GateKind::R.gate()),
        GateKind::MX => Ok(GateKind::RX.gate()),
        GateKind::MY => Ok(GateKind::RY.gate()),
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => Err(reverse_error(format!(
            "don't know how to turn measurement {} into a reset",
            measurement.canonical_name()
        ))),
    }
}

fn shifted_coordinates(args: &[f64], shift: &[f64]) -> Vec<f64> {
    args.iter()
        .enumerate()
        .map(|(index, value)| value + shift.get(index).copied().unwrap_or(0.0))
        .collect()
}

fn add_coordinate_shift(shift: &mut Vec<f64>, delta: &[f64]) {
    if shift.len() < delta.len() {
        shift.resize(delta.len(), 0.0);
    }
    for (value, delta) in shift.iter_mut().zip(delta) {
        *value += delta;
    }
}

fn validate_output_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: &UnsignedFlowCheckOptions,
) -> AnalysisResult<()> {
    let checks =
        check_unsigned_flows_with_sparse_tracker(circuit, flows, options).map_err(|error| {
            if stops_flow_check(&error) {
                error
            } else {
                reverse_error(format!("failed to validate reversed flows: {error}"))
            }
        })?;
    for (index, (flow, satisfied)) in flows.iter().zip(checks).enumerate() {
        if !satisfied {
            return Err(reverse_error(format!(
                "reversed flow {index} is not satisfied by the reversed circuit: {flow}"
            )));
        }
    }
    Ok(())
}

fn instruction_text(instruction: &CircuitInstruction) -> String {
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction.clone());
    circuit.to_stim_string().trim().to_owned()
}

fn reverse_error(message: impl Into<String>) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(message)
}
