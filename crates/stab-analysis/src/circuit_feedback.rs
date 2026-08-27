use std::collections::{BTreeMap, BTreeSet};

use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, DemTarget, Gate, GateCategory, MeasureRecordOffset,
    Pauli, QubitId, RepeatBlock, RepeatCount, Target,
    advanced::{
        CircuitBuilder, ClassicalControl, ControlledPauliTargetPair,
        classify_controlled_pauli_target_pair,
    },
};

use crate::{
    AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};
const MAX_FEEDBACK_REPEAT_WORK_UNITS: u64 = 1_000_000;
const MAX_FEEDBACK_REPEAT_NESTING: usize = 256;

pub fn circuit_with_inlined_feedback(circuit: &Circuit) -> AnalysisResult<Circuit> {
    validate_feedback_repeat_budget(circuit)?;
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit usize while inlining feedback",
        )
    })?;
    let detector_count = detector_count(circuit)?;
    let mut helper = WithoutFeedbackHelper {
        reversed_output: Vec::new(),
        tracker: SparseReverseFrameTracker::new(
            stab_model::advanced::circuit_simulated_qubit_count(circuit),
            measurement_count,
            detector_count,
            false,
        ),
        observable_changes: BTreeMap::new(),
        detector_changes: BTreeMap::new(),
    };
    helper.undo_circuit(circuit)?;
    helper
        .build_output()
        .and_then(fuse_identical_adjacent_loops)
}

struct WithoutFeedbackHelper {
    reversed_output: Vec<CircuitItem>,
    tracker: SparseReverseFrameTracker,
    observable_changes: BTreeMap<u64, BTreeSet<MeasureRecordOffset>>,
    detector_changes: BTreeMap<u64, BTreeSet<usize>>,
}

impl WithoutFeedbackHelper {
    fn undo_circuit(&mut self, circuit: &Circuit) -> AnalysisResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => self.undo_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => self.undo_repeat_block(repeat)?,
            }
        }
        Ok(())
    }

    fn undo_repeat_block(&mut self, repeat: &RepeatBlock) -> AnalysisResult<()> {
        let repeat_count = repeat.repeat_count().get();
        let mut outer_output = std::mem::take(&mut self.reversed_output);
        for _ in 0..repeat_count {
            self.reversed_output.clear();
            self.undo_circuit(repeat.body())?;
            let body =
                CircuitBuilder::from_unfused_items(std::mem::take(&mut self.reversed_output))
                    .finish();
            outer_output.push(CircuitItem::RepeatBlock(
                stab_model::advanced::repeat_block_with_tag_bytes(
                    RepeatCount::try_new(1)?,
                    body,
                    repeat.tag_bytes(),
                ),
            ));
        }
        self.reversed_output = outer_output;
        Ok(())
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        if matches!(
            instruction.gate().canonical_name(),
            "CX" | "CY" | "CZ" | "XCZ" | "YCZ"
        ) {
            return self.undo_feedback_capable_controlled_pauli(instruction);
        }
        if instruction.gate().category() == GateCategory::Controlled
            && instruction
                .targets()
                .iter()
                .any(Target::is_classical_bit_target)
        {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "feedback inlining does not support {} with classical controls",
                instruction.gate().canonical_name()
            )));
        }
        self.reversed_output
            .push(CircuitItem::Instruction(instruction.clone()));
        self.tracker.undo_instruction(instruction)?;
        Ok(())
    }

    fn undo_feedback_capable_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let piece = instruction_with_targets(instruction, group.to_vec())?;
            match classify_controlled_pauli_target_pair(instruction.gate(), group) {
                ControlledPauliTargetPair::Quantum { .. } => {
                    self.reversed_output
                        .push(CircuitItem::Instruction(piece.clone()));
                    self.tracker.undo_instruction(&piece)?;
                }
                ControlledPauliTargetPair::Classical { control, target } => {
                    match control {
                        ClassicalControl::Record(record) => {
                            self.inline_feedback(instruction, record, target)?;
                        }
                        ClassicalControl::Sweep(_) => self
                            .reversed_output
                            .push(CircuitItem::Instruction(piece.clone())),
                    }
                    self.tracker.undo_instruction(&piece)?;
                }
                ControlledPauliTargetPair::ClassicalNoop { first, second } => {
                    if !first.is_record() && !second.is_record() {
                        self.reversed_output
                            .push(CircuitItem::Instruction(piece.clone()));
                        self.tracker.undo_instruction(&piece)?;
                    }
                }
                ControlledPauliTargetPair::Unsupported => {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "{} has an unsupported controlled-Pauli target orientation during feedback inlining",
                        instruction.gate().canonical_name()
                    )));
                }
            }
        }
        self.flush_observable_changes(instruction)?;
        Ok(())
    }

    fn inline_feedback(
        &mut self,
        instruction: &CircuitInstruction,
        record: MeasureRecordOffset,
        qubit: QubitId,
    ) -> AnalysisResult<()> {
        let feedback = feedback_pauli(instruction.gate())?;
        let sensitivity = self.tracker.feedback_sensitivity(qubit, feedback)?;
        let absolute_record = self
            .tracker
            .absolute_record_index_from_offset(record.get())?;
        for target in sensitivity {
            match target {
                DemTarget::RelativeDetector(detector) => {
                    toggle_value(
                        self.detector_changes.entry(detector.get()).or_default(),
                        absolute_record,
                    );
                }
                DemTarget::LogicalObservable(observable) => {
                    toggle_value(
                        self.observable_changes.entry(observable.get()).or_default(),
                        record,
                    );
                }
                DemTarget::Separator | DemTarget::Numeric(_) => {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "feedback sensitivity unexpectedly contained DEM target {target}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn flush_observable_changes(&mut self, source: &CircuitInstruction) -> AnalysisResult<()> {
        let changes = std::mem::take(&mut self.observable_changes);
        for (observable, records) in changes {
            if records.is_empty() {
                continue;
            }
            let instruction = stab_model::advanced::circuit_instruction_with_tag_bytes(
                Gate::from_name("OBSERVABLE_INCLUDE")?,
                vec![observable as f64],
                records
                    .into_iter()
                    .map(Target::measurement_record)
                    .collect(),
                source.tag_bytes(),
            )?;
            self.reversed_output
                .push(CircuitItem::Instruction(instruction));
        }
        Ok(())
    }

    fn build_output(&self) -> AnalysisResult<Circuit> {
        let mut measurements_in_past = 0usize;
        let mut detectors_in_past = 0u64;
        self.build_output_from_items(
            &self.reversed_output,
            &mut measurements_in_past,
            &mut detectors_in_past,
        )
    }

    fn build_output_from_items(
        &self,
        items: &[CircuitItem],
        measurements_in_past: &mut usize,
        detectors_in_past: &mut u64,
    ) -> AnalysisResult<Circuit> {
        let mut result = Circuit::new();
        for item in items.iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    *measurements_in_past = measurements_in_past
                        .checked_add(
                            stab_model::advanced::circuit_instruction_measurement_result_count(
                                instruction,
                            ),
                        )
                        .ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "measurement count overflowed while building feedback-free circuit",
                            )
                        })?;
                    if instruction.gate().canonical_name() == "DETECTOR" {
                        let detector_id = *detectors_in_past;
                        *detectors_in_past = detectors_in_past.checked_add(1).ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "detector count overflowed while building feedback-free circuit",
                            )
                        })?;
                        if let Some(changes) = self.detector_changes.get(&detector_id) {
                            result.append_instruction(rewritten_detector(
                                instruction,
                                changes,
                                *measurements_in_past,
                            )?);
                            continue;
                        }
                    }
                    result.append_instruction(instruction.clone());
                }
                CircuitItem::RepeatBlock(repeat) => {
                    for _ in 0..repeat.repeat_count().get() {
                        let body = self.build_output_from_items(
                            repeat.body().items(),
                            measurements_in_past,
                            detectors_in_past,
                        )?;
                        result.append_repeat_block(
                            stab_model::advanced::repeat_block_with_tag_bytes(
                                RepeatCount::try_new(1)?,
                                body,
                                repeat.tag_bytes(),
                            ),
                        );
                    }
                }
            }
        }
        Ok(result)
    }
}

fn append_items(circuit: &mut Circuit, items: Vec<CircuitItem>) {
    for item in items {
        match item {
            CircuitItem::Instruction(instruction) => circuit.append_instruction(instruction),
            CircuitItem::RepeatBlock(repeat) => circuit.append_repeat_block(repeat),
        }
    }
}

fn fuse_identical_adjacent_loops(circuit: Circuit) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    let mut growing_loop: Option<GrowingLoop> = None;

    for item in circuit.items() {
        match item {
            CircuitItem::RepeatBlock(repeat) => {
                if let Some(growing) = growing_loop.as_mut()
                    && growing.body == *repeat.body()
                    && growing.tag.as_deref() == repeat.tag_bytes()
                {
                    growing.repetitions = growing
                        .repetitions
                        .checked_add(repeat.repeat_count().get())
                        .ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "feedback inlining fused repeat count overflowed",
                            )
                        })?;
                    continue;
                }
                flush_growing_loop(&mut result, &mut growing_loop)?;
                growing_loop = Some(GrowingLoop {
                    body: repeat.body().clone(),
                    repetitions: repeat.repeat_count().get(),
                    tag: repeat.tag_bytes().map(Box::<[u8]>::from),
                });
            }
            CircuitItem::Instruction(instruction) => {
                flush_growing_loop(&mut result, &mut growing_loop)?;
                result.append_instruction(instruction.clone());
            }
        }
    }
    flush_growing_loop(&mut result, &mut growing_loop)?;
    Ok(result)
}

struct GrowingLoop {
    body: Circuit,
    repetitions: u64,
    tag: Option<Box<[u8]>>,
}

fn flush_growing_loop(
    result: &mut Circuit,
    growing_loop: &mut Option<GrowingLoop>,
) -> AnalysisResult<()> {
    let Some(GrowingLoop {
        body,
        repetitions,
        tag,
    }) = growing_loop.take()
    else {
        return Ok(());
    };
    let fused_body = fuse_identical_adjacent_loops(body)?;
    if repetitions == 1 {
        append_items(result, fused_body.items().to_vec());
    } else {
        result.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
            RepeatCount::try_new(repetitions)?,
            fused_body,
            tag.as_deref(),
        ));
    }
    Ok(())
}

#[derive(Default)]
struct FeedbackRepeatBudget {
    expanded_work_units: u64,
    repeat_iterations: u64,
}

impl FeedbackRepeatBudget {
    fn add_expanded_work_units(&mut self, count: u64) -> AnalysisResult<()> {
        let actual = self.expanded_work_units.saturating_add(count);
        if actual > MAX_FEEDBACK_REPEAT_WORK_UNITS {
            return Err(feedback_resource_error(
                ResourceKind::ExpandedOperations,
                actual,
                MAX_FEEDBACK_REPEAT_WORK_UNITS,
            ));
        }
        self.expanded_work_units = actual;
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> AnalysisResult<()> {
        let actual = self.repeat_iterations.saturating_add(count);
        if actual > MAX_FEEDBACK_REPEAT_WORK_UNITS {
            return Err(feedback_resource_error(
                ResourceKind::RepeatIterations,
                actual,
                MAX_FEEDBACK_REPEAT_WORK_UNITS,
            ));
        }
        self.repeat_iterations = actual;
        Ok(())
    }
}

fn feedback_resource_error(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisError {
    ResourceLimitError::fixed_operation(
        ResourceOperation::FeedbackInlining,
        resource,
        actual,
        limit,
    )
    .into()
}

fn validate_feedback_repeat_budget(circuit: &Circuit) -> AnalysisResult<()> {
    let mut budget = FeedbackRepeatBudget::default();
    validate_feedback_repeat_budget_inner(circuit, 1, 0, &mut budget)
}

fn validate_feedback_repeat_budget_inner(
    circuit: &Circuit,
    multiplier: u64,
    depth: usize,
    budget: &mut FeedbackRepeatBudget,
) -> AnalysisResult<()> {
    if depth > MAX_FEEDBACK_REPEAT_NESTING {
        return Err(feedback_resource_error(
            ResourceKind::RepeatNesting,
            u64::try_from(depth).unwrap_or(u64::MAX),
            MAX_FEEDBACK_REPEAT_NESTING as u64,
        ));
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                let work_units = instruction_work_units(instruction)?.saturating_mul(multiplier);
                budget.add_expanded_work_units(work_units)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                let repeated_multiplier = multiplier.saturating_mul(repeat_count);
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_feedback_repeat_budget_inner(
                    repeat.body(),
                    repeated_multiplier,
                    depth.saturating_add(1),
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn instruction_work_units(instruction: &CircuitInstruction) -> AnalysisResult<u64> {
    let target_count = u64::try_from(instruction.targets().len()).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "feedback inlining instruction target count does not fit u64",
        )
    })?;
    Ok(target_count.max(1))
}

fn rewritten_detector(
    instruction: &CircuitInstruction,
    changes: &BTreeSet<usize>,
    measurements_in_past: usize,
) -> AnalysisResult<CircuitInstruction> {
    let mut targets = changes.clone();
    for target in instruction.targets() {
        let offset = target.measurement_record_offset().ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(format!(
                "DETECTOR target {target} is not a measurement record"
            ))
        })?;
        let index = absolute_record_index(measurements_in_past, offset)?;
        toggle_value(&mut targets, index);
    }
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        instruction.args().to_vec(),
        targets
            .into_iter()
            .map(|index| relative_record_target(index, measurements_in_past))
            .collect::<AnalysisResult<Vec<_>>>()?,
        instruction.tag_bytes(),
    )?)
}

fn detector_count(circuit: &Circuit) -> AnalysisResult<u64> {
    let mut count = 0u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "DETECTOR" {
                    count = count.checked_add(1).ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model("detector count overflowed")
                    })?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = detector_count(repeat.body())?;
                let repeated = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(
                            "repeat detector count overflowed",
                        )
                    })?;
                count = count.checked_add(repeated).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model("detector count overflowed")
                })?;
            }
        }
    }
    Ok(count)
}

fn instruction_with_targets(
    instruction: &CircuitInstruction,
    targets: Vec<Target>,
) -> AnalysisResult<CircuitInstruction> {
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        instruction.args().to_vec(),
        targets,
        instruction.tag_bytes(),
    )?)
}

fn feedback_pauli(gate: Gate) -> AnalysisResult<Pauli> {
    match gate.canonical_name() {
        "CX" | "XCZ" => Ok(Pauli::X),
        "CY" | "YCZ" => Ok(Pauli::Y),
        "CZ" => Ok(Pauli::Z),
        name => Err(AnalysisError::invalid_detector_error_model(format!(
            "{name} is not a supported feedback gate"
        ))),
    }
}

fn absolute_record_index(
    measurements_in_past: usize,
    offset: MeasureRecordOffset,
) -> AnalysisResult<usize> {
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let index = current
        .checked_add(i64::from(offset.get()))
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "measurement record offset overflowed while rewriting detector",
            )
        })?;
    if index < 0 || index >= current {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "measurement record target rec[{}] is outside feedback rewrite history",
            offset.stim_text()
        )));
    }
    usize::try_from(index).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement record index does not fit usize while rewriting detector",
        )
    })
}

fn relative_record_target(
    absolute_index: usize,
    measurements_in_past: usize,
) -> AnalysisResult<Target> {
    let absolute = i64::try_from(absolute_index).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "absolute measurement index does not fit i64 while rewriting detector",
        )
    })?;
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let offset = absolute.checked_sub(current).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(
            "relative measurement offset overflowed while rewriting detector",
        )
    })?;
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        i32::try_from(offset).map_err(|_| {
            AnalysisError::invalid_detector_error_model(format!(
                "relative measurement offset {offset} does not fit i32"
            ))
        })?,
    )?))
}

fn toggle_value<T: Copy + Ord>(values: &mut BTreeSet<T>, value: T) {
    if !values.insert(value) {
        values.remove(&value);
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::panic,
        clippy::unwrap_used,
        reason = "transform unit tests use exact circuit text for compact parity diagnostics"
    )]

    use super::*;

    fn transform(text: &str) -> String {
        let circuit = Circuit::from_stim_str(text).unwrap();
        circuit_with_inlined_feedback(&circuit)
            .unwrap()
            .to_stim_string()
    }

    #[test]
    fn feedback_inlining_common_semantic_matrix_matches_stim() {
        let controlled_pauli_expected = "R 0 1 2\n\
             X_ERROR(0.125) 0\n\
             M 0 1 2\n\
             DETECTOR rec[-3] rec[-2]\n\
             DETECTOR rec[-3] rec[-1]\n";
        let cases = [
            (
                "mixed controls and observable propagation",
                "MR 0\n\
                 H 0\n\
                 CX sweep[5] 0\n\
                 CY rec[-1] 0 rec[-1] 0 2 3 rec[-1] 0\n\
                 H 0\n\
                 M 0\n\
                 DETECTOR rec[-1]\n\
                 OBSERVABLE_INCLUDE(2) rec[-1]\n",
                "MR 0\n\
                 H 0\n\
                 CX sweep[5] 0\n\
                 OBSERVABLE_INCLUDE(2) rec[-1]\n\
                 CY 2 3\n\
                 H 0\n\
                 M 0\n\
                 DETECTOR rec[-2] rec[-1]\n\
                 OBSERVABLE_INCLUDE(2) rec[-1]\n",
            ),
            (
                "demolition feedback",
                "CX 0 1\n\
                 M 1\n\
                 CX rec[-1] 1\n\
                 CX 0 1\n\
                 M 1\n\
                 DETECTOR rec[-1] rec[-2]\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n",
                "CX 0 1\n\
                 M 1\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n\
                 CX 0 1\n\
                 M 1\n\
                 DETECTOR rec[-1]\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n",
            ),
            (
                "interleaved unitary operations",
                "H 0\nCZ\nH 1\n",
                "H 0 1\n",
            ),
            (
                "interleaved measurements",
                "M 0 1\n\
                 CX\n\
                 M 2\n\
                 CX rec[-1] 3\n\
                 M 3\n\
                 DETECTOR rec[-1]\n",
                "M 0 1 2 3\nDETECTOR rec[-2] rec[-1]\n",
            ),
            (
                "MPP feedback",
                "RX 0\n\
                 RY 1\n\
                 RZ 2\n\
                 MPP X0*Y1*Z2 Z5\n\
                 CX rec[-2] 3\n\
                 M 3\n\
                 DETECTOR rec[-1]\n",
                "RX 0\n\
                 RY 1\n\
                 R 2\n\
                 MPP X0*Y1*Z2 Z5\n\
                 M 3\n\
                 DETECTOR rec[-3] rec[-1]\n",
            ),
            (
                "refolded repeat",
                "R 0 1\n\
                 X_ERROR(0.125) 0 1\n\
                 CX 0 1\n\
                 M 1\n\
                 CX rec[-1] 1\n\
                 DETECTOR rec[-1]\n\
                 REPEAT 30 {\n\
                     X_ERROR(0.125) 0 1\n\
                     CX 0 1\n\
                     M 1\n\
                     CX rec[-1] 1\n\
                     DETECTOR rec[-1] rec[-2]\n\
                 }\n\
                 M 0\n\
                 DETECTOR rec[-1] rec[-2]\n",
                concat!(
                    "R 0 1\n",
                    "X_ERROR(0.125) 0 1\n",
                    "CX 0 1\n",
                    "M 1\n",
                    "DETECTOR rec[-1]\n",
                    "X_ERROR(0.125) 0 1\n",
                    "CX 0 1\n",
                    "M 1\n",
                    "DETECTOR rec[-1]\n",
                    "REPEAT 29 {\n",
                    "    X_ERROR(0.125) 0 1\n",
                    "    CX 0 1\n",
                    "    M 1\n",
                    "    DETECTOR rec[-3] rec[-1]\n",
                    "}\n",
                    "M 0\n",
                    "DETECTOR rec[-3] rec[-2] rec[-1]\n",
                ),
            ),
            (
                "nested bounded repeats",
                "R 0 1 2\n\
                 REPEAT 2 {\n\
                     X_ERROR(0.125) 0\n\
                     M 0\n\
                     CY rec[-1] 1\n\
                     REPEAT 2 {\n\
                         X_ERROR(0.25) 1\n\
                         M 1\n\
                         DETECTOR rec[-1] rec[-2]\n\
                         CZ rec[-1] 2\n\
                         M 2\n\
                         DETECTOR rec[-1] rec[-2]\n\
                         R 1 2\n\
                     }\n\
                     R 0\n\
                 }\n",
                concat!(
                    "R 0 1 2\n",
                    "REPEAT 2 {\n",
                    "    X_ERROR(0.125) 0\n",
                    "    M 0\n",
                    "    X_ERROR(0.25) 1\n",
                    "    M 1\n",
                    "    DETECTOR rec[-1]\n",
                    "    M 2\n",
                    "    DETECTOR rec[-3] rec[-2] rec[-1]\n",
                    "    R 1 2\n",
                    "    X_ERROR(0.25) 1\n",
                    "    M 1\n",
                    "    DETECTOR rec[-1] rec[-2]\n",
                    "    M 2\n",
                    "    DETECTOR rec[-1] rec[-2]\n",
                    "    R 1 2 0\n",
                    "}\n",
                ),
            ),
            (
                "CX and CY record controls",
                "R 0 1 2\n\
                 X_ERROR(0.125) 0\n\
                 M 0\n\
                 CX rec[-1] 1\n\
                 CY rec[-1] 2\n\
                 M 1 2\n\
                 DETECTOR rec[-2]\n\
                DETECTOR rec[-1]\n",
                controlled_pauli_expected,
            ),
            (
                "both CZ record-control orientations",
                "R 0\n\
                 RX 1 2\n\
                 X_ERROR(0.125) 0\n\
                 M 0\n\
                 CZ rec[-1] 1\n\
                 CZ 2 rec[-1]\n\
                 MX 1 2\n\
                 DETECTOR rec[-3] rec[-2]\n\
                 DETECTOR rec[-3] rec[-1]\n",
                "R 0\n\
                 RX 1 2\n\
                 X_ERROR(0.125) 0\n\
                 M 0\n\
                 MX 1 2\n\
                 DETECTOR rec[-2]\n\
                 DETECTOR rec[-1]\n",
            ),
        ];

        for (name, input, expected) in cases {
            assert_eq!(transform(input), expected, "{name}");
        }
    }

    #[test]
    fn feedback_inlining_xcz_ycz_extension_matches_equivalent_controls() {
        let input = "R 0 1 2\n\
             X_ERROR(0.125) 0\n\
             M 0\n\
             XCZ 1 rec[-1]\n\
             YCZ 2 rec[-1]\n\
             M 1 2\n\
             DETECTOR rec[-2]\n\
             DETECTOR rec[-1]\n";
        let expected = "R 0 1 2\n\
             X_ERROR(0.125) 0\n\
             M 0 1 2\n\
             DETECTOR rec[-3] rec[-2]\n\
             DETECTOR rec[-3] rec[-1]\n";

        // Pinned Stim rejects these direct spellings in this transform. Stab's
        // extension must stay equivalent to accepted CX/CY record controls.
        assert_eq!(transform(input), expected);
    }

    #[test]
    fn circuit_with_inlined_feedback_rejects_anti_hermitian_mpp() {
        let circuit = Circuit::from_stim_str(
            "MPP X0*Z0\n\
             CX rec[-1] 1\n\
             M 1\n\
             DETECTOR rec[-1]\n",
        )
        .unwrap();
        let error = circuit_with_inlined_feedback(&circuit).unwrap_err();

        assert!(error.to_string().contains("anti-Hermitian"));
    }

    #[test]
    fn circuit_with_inlined_feedback_rejects_unsupported_feedback_gate() {
        let circuit = Circuit::from_stim_str(
            "M 0\n\
             XCZ rec[-1] 1\n\
             M 1\n\
             DETECTOR rec[-1]\n",
        )
        .unwrap();
        let error = circuit_with_inlined_feedback(&circuit).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported controlled-Pauli target orientation")
        );
    }

    #[test]
    fn circuit_with_inlined_feedback_drops_classical_cz_noops_with_record_targets() {
        for (text, expected) in [
            ("M 0\nCZ rec[-1] sweep[0]\n", "M 0\n"),
            ("M 0\nCZ sweep[0] rec[-1]\n", "M 0\n"),
            ("M 0 1\nCZ rec[-1] rec[-2]\n", "M 0 1\n"),
        ] {
            let circuit = Circuit::from_stim_str(text).unwrap();
            assert_eq!(
                circuit_with_inlined_feedback(&circuit).unwrap(),
                Circuit::from_stim_str(expected).unwrap()
            );
        }
    }

    #[test]
    fn circuit_with_inlined_feedback_preserves_cz_sweep_only_groups() {
        let circuit = Circuit::from_stim_str("CZ sweep[0] sweep[1]\n").unwrap();
        assert_eq!(circuit_with_inlined_feedback(&circuit).unwrap(), circuit);
    }

    #[test]
    fn circuit_with_inlined_feedback_rejects_non_cz_sweep_only_groups() {
        for gate in ["CX", "CY", "XCZ", "YCZ"] {
            let circuit = Circuit::from_stim_str(&format!("{gate} sweep[0] sweep[1]\n")).unwrap();
            let error = circuit_with_inlined_feedback(&circuit).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("unsupported controlled-Pauli target orientation"),
                "{gate}: {error}"
            );
        }
    }

    #[test]
    fn feedback_repeat_resource_divergence_has_one_aggregate_policy() {
        let above_old_per_block_cap = Circuit::from_stim_str("REPEAT 100001 {\nTICK\n}\n").unwrap();
        let accepted = validate_feedback_repeat_budget(&above_old_per_block_cap);
        assert!(
            accepted.is_ok(),
            "aggregate budget should admit work above the obsolete per-block cap: {accepted:?}"
        );

        let excessive_work = Circuit::from_stim_str("REPEAT 1000001 {\nTICK\n}\n").unwrap();
        let error = validate_feedback_repeat_budget(&excessive_work).unwrap_err();
        let resource = error.resource_limit_error().unwrap();
        assert_eq!(resource.operation(), ResourceOperation::FeedbackInlining);
        assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
        assert_eq!(resource.actual(), 1_000_001);
        assert_eq!(resource.limit(), MAX_FEEDBACK_REPEAT_WORK_UNITS);

        let mut expanded = FeedbackRepeatBudget::default();
        expanded
            .add_expanded_work_units(MAX_FEEDBACK_REPEAT_WORK_UNITS)
            .unwrap();
        let error = expanded.add_expanded_work_units(1).unwrap_err();
        let resource = error.resource_limit_error().unwrap();
        assert_eq!(resource.operation(), ResourceOperation::FeedbackInlining);
        assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
        assert_eq!(resource.actual(), 1_000_001);
        assert_eq!(resource.limit(), MAX_FEEDBACK_REPEAT_WORK_UNITS);

        let mut nested = Circuit::from_stim_str("TICK\n").unwrap();
        for _ in 0..=MAX_FEEDBACK_REPEAT_NESTING {
            let mut outer = Circuit::new();
            outer.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                RepeatCount::try_new(1).unwrap(),
                nested,
                None,
            ));
            nested = outer;
        }
        let error = validate_feedback_repeat_budget(&nested).unwrap_err();
        let resource = error.resource_limit_error().unwrap();
        assert_eq!(resource.operation(), ResourceOperation::FeedbackInlining);
        assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), 257);
        assert_eq!(resource.limit(), MAX_FEEDBACK_REPEAT_NESTING as u64);
    }
}
