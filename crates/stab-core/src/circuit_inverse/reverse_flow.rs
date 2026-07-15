use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget, Flow, Gate,
    MeasureRecordOffset, PauliSign, QubitId, Target,
    circuit_flow::{check_unsigned_flows_with_sparse_tracker, transitions::ReverseFlowTransition},
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

use super::{TimeReversedForFlowsOptions, is_unitary_category, reversed_target_groups};
use crate::circuit_flow::transitions::reverse_flow_transition;

const MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;

pub(super) fn requires_general_reversal(circuit: &Circuit, flows: &[Flow]) -> bool {
    flows
        .iter()
        .any(|flow| flow.measurements().next().is_some() || flow.observables().next().is_some())
        || circuit.items().iter().any(item_requires_general_reversal)
}

pub(super) fn reverse_flows(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    validate_general_reversal(circuit)?;
    let measurement_count = usize::try_from(circuit.count_measurements()?)
        .map_err(|_| reverse_error("measurement count does not fit the platform index width"))?;
    let detector_count = circuit.count_detectors()?;
    let observable_count = circuit.count_observables()?;
    let qubit_count = flow_aware_qubit_count(circuit, flows)?;
    let flow_states = reverse_flow_states(observable_count, measurement_count, flows)?;
    let mut engine = ReverseFlowEngine {
        tracker: SparseReverseFrameTracker::new(
            qubit_count,
            measurement_count,
            detector_count,
            true,
        ),
        inverted: Circuit::new(),
        qubit_coordinates: Vec::new(),
        coordinate_shift: Vec::new(),
        detector_measurements: BTreeMap::new(),
        target_coordinates: BTreeMap::new(),
        target_tags: BTreeMap::new(),
        remaining_measurements: measurement_count,
        remaining_detectors: detector_count,
        new_measurement_count: 0,
        observable_count,
        options,
    };

    engine.seed_flow_outputs(&flow_states)?;
    for instruction in circuit.iter_flattened_instructions_reverse() {
        engine.reverse_instruction(instruction)?;
    }
    engine.seed_flow_inputs(&flow_states)?;
    engine.verify_closed_targets(&flow_states)?;
    let output_flows = engine.build_output_flows(&flow_states)?;
    let output_circuit = engine.finish();
    validate_output_flows(&output_circuit, &output_flows)?;
    Ok((output_circuit, output_flows))
}

struct ReverseFlowEngine {
    tracker: SparseReverseFrameTracker,
    inverted: Circuit,
    qubit_coordinates: Vec<CircuitInstruction>,
    coordinate_shift: Vec<f64>,
    detector_measurements: BTreeMap<DemTarget, BTreeSet<usize>>,
    target_coordinates: BTreeMap<DemTarget, Vec<f64>>,
    target_tags: BTreeMap<DemTarget, Option<String>>,
    remaining_measurements: usize,
    remaining_detectors: u64,
    new_measurement_count: usize,
    observable_count: u64,
    options: TimeReversedForFlowsOptions,
}

struct ReverseFlowState<'a> {
    target: DemTarget,
    input: &'a crate::PauliString,
    output: &'a crate::PauliString,
    measurements: Vec<i32>,
    observables: Vec<u32>,
}

impl ReverseFlowState<'_> {
    fn original_flow(&self) -> CircuitResult<Flow> {
        Flow::new(
            self.input.clone(),
            self.output.clone(),
            self.measurements.iter().copied(),
            self.observables.iter().copied(),
        )
        .map_err(|error| reverse_error(error.to_string()))
    }
}

impl ReverseFlowEngine {
    fn seed_flow_outputs(&mut self, states: &[ReverseFlowState<'_>]) -> CircuitResult<()> {
        for state in states {
            self.toggle_pauli(state.output, state.target)?;
            for measurement in state.measurements.iter().copied() {
                let index = absolute_measurement_index(measurement, self.remaining_measurements)?;
                self.tracker
                    .toggle_record_target_absolute(index, state.target)?;
            }
        }
        Ok(())
    }

    fn seed_flow_inputs(&mut self, states: &[ReverseFlowState<'_>]) -> CircuitResult<()> {
        for state in states {
            self.toggle_pauli(state.input, state.target)?;
        }
        Ok(())
    }

    fn toggle_pauli(&mut self, pauli: &crate::PauliString, target: DemTarget) -> CircuitResult<()> {
        for (index, basis) in pauli.active_terms() {
            let qubit = QubitId::new(u32::try_from(index).map_err(|_| {
                reverse_error(format!("flow qubit index {index} exceeds {}", u32::MAX))
            })?)?;
            self.tracker.toggle_pauli_target(qubit, basis, target)?;
        }
        Ok(())
    }

    fn reverse_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
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
            ReverseFlowTransition::HeraldedMeasurement => Err(reverse_error(format!(
                "time-reversing heralded measurement records is outside the selected Rust transform scope: {}",
                instruction_text(instruction)
            ))),
            ReverseFlowTransition::Ignored => self.reverse_ignored(instruction),
            ReverseFlowTransition::Unsupported => Err(reverse_error(format!(
                "don't know how to time-reverse {}",
                instruction_text(instruction)
            ))),
        }
    }

    fn reverse_measurement(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let reset_gate = measurement_reset_gate(instruction.gate().canonical_name())?;
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
                .remaining_measurements
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
                    Gate::from_name(reset_gate)?,
                    Vec::new(),
                    vec![target.clone()],
                    instruction.tag(),
                )?;
            } else {
                self.record_new_measurement(&record_targets);
                self.append_instruction(
                    instruction.gate().best_candidate_inverse()?,
                    instruction.args().to_vec(),
                    vec![target.clone()],
                    instruction.tag(),
                )?;
            }

            let tracker_instruction = CircuitInstruction::new(
                instruction.gate(),
                Vec::new(),
                vec![target.clone()],
                instruction.tag().map(str::to_owned),
            )?;
            self.tracker.undo_instruction(&tracker_instruction)?;
            self.remaining_measurements = record_index;
        }
        self.flush_detectors_and_observables()
    }

    fn reverse_reset_or_measure_reset(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let transition = reverse_flow_transition(instruction);
        let is_measure_reset = matches!(transition, ReverseFlowTransition::MeasureReset(_));
        let targets = reversed_target_groups(instruction);
        for target in &targets {
            let qubit = target
                .qubit_id()
                .ok_or_else(|| reverse_error(format!("reset target {target} is not a qubit")))?;
            let pauli_targets = self.tracker.pauli_targets_at(qubit)?;
            self.record_new_measurement(&pauli_targets);
        }

        self.tracker.undo_instruction(instruction)?;
        if is_measure_reset {
            self.remaining_measurements = self
                .remaining_measurements
                .checked_sub(targets.len())
                .ok_or_else(|| reverse_error("measure-reset count underflowed during reversal"))?;
        }
        self.append_instruction(
            instruction.gate().best_candidate_inverse()?,
            Vec::new(),
            targets.clone(),
            instruction.tag(),
        )?;
        if is_measure_reset && !instruction.args().is_empty() {
            let error_gate = match instruction.gate().canonical_name() {
                "MR" => "X_ERROR",
                "MRX" | "MRY" => "Z_ERROR",
                name => {
                    return Err(reverse_error(format!(
                        "don't know how to eject measurement noise from {name}"
                    )));
                }
            };
            self.append_instruction(
                Gate::from_name(error_gate)?,
                instruction.args().to_vec(),
                targets.clone(),
                instruction.tag(),
            )?;
        }
        self.flush_detectors_and_observables()
    }

    fn reverse_measuring_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let measurement_count = instruction.target_groups().len();
        if self.remaining_measurements < measurement_count {
            return Err(reverse_error(format!(
                "measurement count underflowed while reversing {}",
                instruction_text(instruction)
            )));
        }
        for offset in 0..measurement_count {
            let record_index = self.remaining_measurements - offset - 1;
            let record_targets = self.tracker.record_targets_at(record_index)?;
            self.record_new_measurement(&record_targets);
        }
        self.tracker.undo_instruction(instruction)?;
        self.remaining_measurements -= measurement_count;
        self.append_instruction(
            instruction.gate().best_candidate_inverse()?,
            instruction.args().to_vec(),
            reversed_target_groups(instruction),
            instruction.tag(),
        )?;
        self.flush_detectors_and_observables()
    }

    fn reverse_detector(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.tracker.undo_instruction(instruction)?;
        self.remaining_detectors = self
            .remaining_detectors
            .checked_sub(1)
            .ok_or_else(|| reverse_error("detector count underflowed during reversal"))?;
        let target = DemTarget::relative_detector(self.remaining_detectors)?;
        self.target_coordinates.insert(
            target,
            shifted_coordinates(instruction.args(), &self.coordinate_shift),
        );
        self.target_tags
            .insert(target, instruction.tag().map(str::to_owned));
        Ok(())
    }

    fn reverse_observable(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        let observable = instruction
            .observable_id_argument()?
            .ok_or_else(|| reverse_error("OBSERVABLE_INCLUDE is missing an observable id"))?;
        let target = DemTarget::logical_observable(observable.get())?;
        let pauli_targets = instruction
            .targets()
            .iter()
            .filter(|target| matches!(target, Target::Pauli { .. }))
            .cloned()
            .collect::<Vec<_>>();
        if pauli_targets.is_empty() {
            self.target_tags
                .insert(target, instruction.tag().map(str::to_owned));
        } else {
            self.append_instruction(
                instruction.gate(),
                instruction.args().to_vec(),
                pauli_targets,
                instruction.tag(),
            )?;
        }
        self.tracker.undo_instruction(instruction)
    }

    fn reverse_simple(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        self.tracker.undo_instruction(instruction)?;
        self.append_instruction(
            instruction.gate().best_candidate_inverse()?,
            instruction.args().to_vec(),
            reversed_target_groups(instruction),
            instruction.tag(),
        )
    }

    fn reverse_ignored(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "QUBIT_COORDS" => {
                let shifted = shifted_coordinates(instruction.args(), &self.coordinate_shift);
                self.qubit_coordinates.push(CircuitInstruction::new(
                    instruction.gate(),
                    shifted,
                    instruction.targets().to_vec(),
                    instruction.tag().map(str::to_owned),
                )?);
                Ok(())
            }
            "SHIFT_COORDS" => {
                add_coordinate_shift(&mut self.coordinate_shift, instruction.args());
                Ok(())
            }
            _ => self.reverse_simple(instruction),
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

    fn flush_detectors_and_observables(&mut self) -> CircuitResult<()> {
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
                .collect::<CircuitResult<Vec<_>>>()?;
            let tag = self.target_tags.remove(&target).flatten();
            match target {
                DemTarget::RelativeDetector(_) => {
                    let coordinates = self.target_coordinates.remove(&target).ok_or_else(|| {
                        reverse_error(format!(
                            "ready detector target {target} lost its coordinate mapping"
                        ))
                    })?;
                    self.append_instruction(
                        Gate::from_name("DETECTOR")?,
                        coordinates,
                        record_targets,
                        tag.as_deref(),
                    )?;
                }
                DemTarget::LogicalObservable(observable) => {
                    self.append_instruction(
                        Gate::from_name("OBSERVABLE_INCLUDE")?,
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

    fn verify_closed_targets(&self, states: &[ReverseFlowState<'_>]) -> CircuitResult<()> {
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

    fn build_output_flows(&self, states: &[ReverseFlowState<'_>]) -> CircuitResult<Vec<Flow>> {
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
                    .collect::<CircuitResult<Vec<_>>>()?;
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
        tag: Option<&str>,
    ) -> CircuitResult<()> {
        if targets.is_empty() && gate.canonical_name() != "TICK" {
            return Ok(());
        }
        self.inverted.append_instruction(CircuitInstruction::new(
            gate,
            args,
            targets,
            tag.map(str::to_owned),
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

fn item_requires_general_reversal(item: &CircuitItem) -> bool {
    match item {
        CircuitItem::Instruction(instruction) => {
            !is_unitary_category(instruction.gate().category())
                || instruction
                    .targets()
                    .iter()
                    .any(Target::is_classical_bit_target)
        }
        CircuitItem::RepeatBlock(repeat) => repeat
            .body()
            .items()
            .iter()
            .any(item_requires_general_reversal),
    }
}

fn validate_general_reversal(circuit: &Circuit) -> CircuitResult<()> {
    let expanded = expanded_instruction_count(circuit)?;
    if expanded > MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS {
        return Err(reverse_error(format!(
            "measurement-rich repeat expansion requires {expanded} instructions, exceeding the {MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS} instruction limit"
        )));
    }
    validate_items(circuit.items())
}

fn expanded_instruction_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut total = 0_u64;
    for item in circuit.items() {
        let count = match item {
            CircuitItem::Instruction(_) => 1,
            CircuitItem::RepeatBlock(repeat) => expanded_instruction_count(repeat.body())?
                .checked_mul(repeat.repeat_count().get())
                .ok_or_else(|| reverse_error("measurement-rich repeat work overflowed"))?,
        };
        total = total
            .checked_add(count)
            .ok_or_else(|| reverse_error("measurement-rich repeat work overflowed"))?;
        if total > MAX_MEASUREMENT_RICH_EXPANDED_INSTRUCTIONS {
            return Ok(total);
        }
    }
    Ok(total)
}

fn validate_items(items: &[CircuitItem]) -> CircuitResult<()> {
    for item in items {
        match item {
            CircuitItem::Instruction(instruction) => validate_instruction(instruction)?,
            CircuitItem::RepeatBlock(repeat) => validate_items(repeat.body().items())?,
        }
    }
    Ok(())
}

fn validate_instruction(instruction: &CircuitInstruction) -> CircuitResult<()> {
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
    if matches!(
        reverse_flow_transition(instruction),
        ReverseFlowTransition::Measurement(_)
            | ReverseFlowTransition::Reset(_)
            | ReverseFlowTransition::MeasureReset(_)
            | ReverseFlowTransition::PairMeasurement(_)
    ) {
        reject_duplicate_qubits(instruction)?;
    }
    if instruction.gate().canonical_name() == "ELSE_CORRELATED_ERROR"
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

fn validate_sweep_target_order(instruction: &CircuitInstruction) -> CircuitResult<()> {
    let invalid_side = match instruction.gate().canonical_name() {
        "CX" | "CY" => SweepInvalidSide::Right,
        "XCZ" | "YCZ" => SweepInvalidSide::Left,
        _ => return Ok(()),
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

fn reject_duplicate_qubits(instruction: &CircuitInstruction) -> CircuitResult<()> {
    let mut seen = BTreeSet::new();
    for target in instruction.targets() {
        if let Some(qubit) = target.qubit_id()
            && !seen.insert(qubit)
        {
            return Err(reverse_error(format!(
                "time reversal rejects duplicate target qubit {} in {} under the locked duplicate-target hardening policy",
                qubit.get(),
                instruction_text(instruction)
            )));
        }
    }
    Ok(())
}

fn flow_aware_qubit_count(circuit: &Circuit, flows: &[Flow]) -> CircuitResult<usize> {
    let flow_qubits = flows
        .iter()
        .flat_map(|flow| [flow.input().len(), flow.output().len()])
        .max()
        .unwrap_or(0);
    let count = circuit.count_qubits().max(flow_qubits);
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
) -> CircuitResult<Vec<ReverseFlowState<'a>>> {
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
                observables: flow.observables().collect(),
            })
        })
        .collect()
}

fn reject_measurement_record_aliases(
    flow_index: usize,
    flow: &Flow,
    measurement_count: usize,
) -> CircuitResult<()> {
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

fn absolute_measurement_index(index: i32, measurement_count: usize) -> CircuitResult<usize> {
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

fn output_measurement_target(index: usize, count: usize) -> CircuitResult<Target> {
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        output_measurement_index(index, count)?,
    )?))
}

fn output_measurement_index(index: usize, count: usize) -> CircuitResult<i32> {
    let index = i64::try_from(index)
        .map_err(|_| reverse_error("output measurement index does not fit i64"))?;
    let count = i64::try_from(count)
        .map_err(|_| reverse_error("output measurement count does not fit i64"))?;
    i32::try_from(index - count)
        .map_err(|_| reverse_error("output measurement record offset does not fit i32"))
}

fn measurement_reset_gate(measurement: &str) -> CircuitResult<&'static str> {
    match measurement {
        "M" => Ok("R"),
        "MX" => Ok("RX"),
        "MY" => Ok("RY"),
        name => Err(reverse_error(format!(
            "don't know how to turn measurement {name} into a reset"
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

fn validate_output_flows(circuit: &Circuit, flows: &[Flow]) -> CircuitResult<()> {
    let checks = check_unsigned_flows_with_sparse_tracker(circuit, flows)
        .map_err(|error| reverse_error(format!("failed to validate reversed flows: {error}")))?;
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

fn reverse_error(message: impl Into<String>) -> CircuitError {
    CircuitError::invalid_tableau_conversion(message)
}
