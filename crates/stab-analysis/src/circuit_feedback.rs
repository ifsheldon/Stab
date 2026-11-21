use std::collections::{BTreeMap, BTreeSet};

use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, ClassicalControl, ControlledPairs,
    ControlledPauliTargetPair, DemTargetKind, FoldedCircuitSummary, FoldedWalkError, Gate,
    GateCategory, GateKind, GateTargets, MeasureRecordOffset, MemoryBudget, ObservableId,
    ParseLimits, Pauli, QubitId, RepeatBlock, RepeatCount, Target,
    advanced::{CircuitBuilder, FoldedCircuitEvent},
};

use crate::folded_walk::walk_folded;
use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, ResourceKind, ResourceLimitError,
    sparse_rev_frame_tracker::{AnticommutationPolicy, SparseReverseFrameTracker},
};
/// The unrolled reversed buffer and the rebuilt output circuit each retain the expansion.
const FEEDBACK_RETAINED_COPIES: u64 = 2;

/// Options of [`circuit_with_inlined_feedback`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FeedbackInliningOptions {
    /// Budget for the conservative size of the unrolled intermediate and output circuits,
    /// projected before unrolling; expanded work itself is not capped.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per undone instruction and unrolled repeat iteration.
    pub cancellation: Cancellation,
}

/// Inlines feedback after admitting the conservative size of its unrolled intermediate and
/// output circuits against the options' memory budget, polling the options' cancellation signal.
pub fn circuit_with_inlined_feedback(
    circuit: &Circuit,
    options: &FeedbackInliningOptions,
) -> AnalysisResult<Circuit> {
    validate_feedback_repeat_budget(circuit, options.memory_budget)?;
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "measurement count does not fit usize while inlining feedback",
        )
    })?;
    let detector_count = detector_count(circuit)?;
    let mut tracker = SparseReverseFrameTracker::new(
        circuit.simulated_qubit_count(),
        measurement_count,
        detector_count,
        AnticommutationPolicy::Fail,
    );
    tracker.set_cancellation(options.cancellation.clone());
    tracker.set_memory_budget(
        AnalysisOperation::FeedbackInlining,
        ResourceKind::MaterializedBytes,
        options.memory_budget,
    );
    let mut helper = WithoutFeedbackHelper {
        reversed_output: Vec::new(),
        tracker,
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
    observable_changes: BTreeMap<ObservableId, BTreeSet<MeasureRecordOffset>>,
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
            self.tracker.poll_cancellation()?;
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
        if let GateTargets::ControlledPairs(pairs) = instruction.gate_targets() {
            return self.undo_feedback_capable_controlled_pauli(instruction, pairs);
        }
        if instruction.gate().category() == GateCategory::Controlled
            && instruction
                .targets()
                .iter()
                .any(Target::is_classical_bit_target)
        {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::FeedbackInlining,
                instruction.gate().kind(),
                format!(
                    "feedback inlining does not support {} with classical controls",
                    instruction.gate().canonical_name()
                ),
            ));
        }
        self.reversed_output
            .push(CircuitItem::Instruction(instruction.clone()));
        self.tracker.undo_instruction(instruction)?;
        Ok(())
    }

    fn undo_feedback_capable_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
        pairs: ControlledPairs<'_>,
    ) -> AnalysisResult<()> {
        let raw_pairs = pairs.as_target_pairs().iter().rev();
        for (raw_pair, pair) in raw_pairs.zip(pairs.rev()) {
            let piece = instruction_with_targets(instruction, raw_pair.to_vec())?;
            match pair {
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
                    return Err(AnalysisError::unsupported(
                        AnalysisOperation::FeedbackInlining,
                        instruction.gate().kind(),
                        format!(
                            "{} has an unsupported controlled-Pauli target orientation during feedback inlining",
                            instruction.gate().canonical_name()
                        ),
                    ));
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
            match target.kind() {
                DemTargetKind::RelativeDetector(detector) => {
                    toggle_value(
                        self.detector_changes.entry(detector.get()).or_default(),
                        absolute_record,
                    );
                }
                DemTargetKind::LogicalObservable(observable) => {
                    toggle_value(
                        self.observable_changes.entry(observable).or_default(),
                        record,
                    );
                }
                DemTargetKind::Separator => {
                    return Err(AnalysisError::internal(
                        AnalysisOperation::FeedbackInlining,
                        format!("feedback sensitivity unexpectedly contained DEM target {target}"),
                    ));
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
                GateKind::OBSERVABLE_INCLUDE.gate(),
                vec![f64::from(observable.get())],
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
                        .checked_add(instruction.measurement_result_count())
                        .ok_or_else(|| {
                            AnalysisError::count_overflow(
                                AnalysisOperation::FeedbackInlining,
                                "measurement count overflowed while building feedback-free circuit",
                            )
                        })?;
                    if instruction.gate().kind() == GateKind::DETECTOR {
                        let detector_id = *detectors_in_past;
                        *detectors_in_past = detectors_in_past.checked_add(1).ok_or_else(|| {
                            AnalysisError::count_overflow(
                                AnalysisOperation::FeedbackInlining,
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
                        self.tracker.check_cancelled()?;
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
                            AnalysisError::count_overflow(
                                AnalysisOperation::FeedbackInlining,
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

struct FeedbackRepeatBudget {
    budget: MemoryBudget,
    projected_bytes: u64,
}

impl FeedbackRepeatBudget {
    fn add_projected_bytes(&mut self, count: u64) -> AnalysisResult<()> {
        let actual = self
            .projected_bytes
            .saturating_add(count.saturating_mul(FEEDBACK_RETAINED_COPIES));
        if !self.budget.admits(u128::from(actual)) {
            return Err(ResourceLimitError::memory_budget(
                AnalysisOperation::FeedbackInlining,
                ResourceKind::MaterializedBytes,
                actual,
                self.budget,
            )
            .into());
        }
        self.projected_bytes = actual;
        Ok(())
    }
}

fn feedback_resource_error(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisError {
    ResourceLimitError::fixed_operation(
        AnalysisOperation::FeedbackInlining,
        resource,
        actual,
        limit,
    )
    .into()
}

/// Admits the projected unrolled bytes of `circuit` and returns them.
fn validate_feedback_repeat_budget(circuit: &Circuit, budget: MemoryBudget) -> AnalysisResult<u64> {
    let mut budget = FeedbackRepeatBudget {
        budget,
        projected_bytes: 0,
    };
    walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |depth, limit| feedback_resource_error(ResourceKind::RepeatNesting, depth, limit),
        |event| match event {
            FoldedCircuitEvent::Instruction {
                instruction,
                executions,
            } => budget.add_projected_bytes(
                instruction_retained_bytes(instruction).saturating_mul(executions.saturating()),
            ),
            FoldedCircuitEvent::Repeat { iterations, .. } => {
                // Each unrolled iteration retains one single-iteration repeat block.
                let block_bytes = (size_of::<CircuitItem>() + size_of::<Circuit>()) as u64;
                budget.add_projected_bytes(block_bytes.saturating_mul(iterations.saturating()))
            }
        },
    )?;
    Ok(budget.projected_bytes)
}

/// Conservative retained bytes of one copy of `instruction` in an owned circuit.
fn instruction_retained_bytes(instruction: &CircuitInstruction) -> u64 {
    (size_of::<CircuitItem>() as u64)
        .saturating_add(
            (instruction.targets().len() as u64).saturating_mul(size_of::<Target>() as u64),
        )
        .saturating_add((instruction.args().len() as u64).saturating_mul(8))
        .saturating_add(instruction.tag_bytes().map_or(0, <[u8]>::len) as u64)
}

fn rewritten_detector(
    instruction: &CircuitInstruction,
    changes: &BTreeSet<usize>,
    measurements_in_past: usize,
) -> AnalysisResult<CircuitInstruction> {
    let mut targets = changes.clone();
    for offset in
        crate::instruction_shape::records(instruction, AnalysisOperation::FeedbackInlining)?
    {
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
    FoldedCircuitSummary::of(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
    )
    .map_err(|error| match error {
        FoldedWalkError::RepeatNesting { depth, limit } => feedback_resource_error(
            ResourceKind::RepeatNesting,
            u64::try_from(depth).unwrap_or(u64::MAX),
            limit as u64,
        ),
        FoldedWalkError::Cancelled(cancelled) => AnalysisError::from(cancelled),
    })?
    .detectors()
    .exact()
    .ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "detector count overflowed",
        )
    })
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
    gate.controlled_pauli().ok_or_else(|| {
        AnalysisError::unsupported(
            AnalysisOperation::FeedbackInlining,
            gate.kind(),
            format!("{} is not a supported feedback gate", gate.canonical_name()),
        )
    })
}

fn absolute_record_index(
    measurements_in_past: usize,
    offset: MeasureRecordOffset,
) -> AnalysisResult<usize> {
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let index = current
        .checked_add(i64::from(offset.get()))
        .ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::FeedbackInlining,
                "measurement record offset overflowed while rewriting detector",
            )
        })?;
    if index < 0 || index >= current {
        return Err(AnalysisError::record_out_of_range(
            AnalysisOperation::FeedbackInlining,
            format!(
                "measurement record target rec[{}] is outside feedback rewrite history",
                offset.stim_text()
            ),
        ));
    }
    usize::try_from(index).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "measurement record index does not fit usize while rewriting detector",
        )
    })
}

fn relative_record_target(
    absolute_index: usize,
    measurements_in_past: usize,
) -> AnalysisResult<Target> {
    let absolute = i64::try_from(absolute_index).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "absolute measurement index does not fit i64 while rewriting detector",
        )
    })?;
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let offset = absolute.checked_sub(current).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::FeedbackInlining,
            "relative measurement offset overflowed while rewriting detector",
        )
    })?;
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        i32::try_from(offset).map_err(|_| {
            AnalysisError::count_overflow(
                AnalysisOperation::FeedbackInlining,
                format!("relative measurement offset {offset} does not fit i32"),
            )
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
        circuit_with_inlined_feedback(&circuit, &Default::default())
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
        let error = circuit_with_inlined_feedback(&circuit, &Default::default()).unwrap_err();

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
        let error = circuit_with_inlined_feedback(&circuit, &Default::default()).unwrap_err();

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
                circuit_with_inlined_feedback(&circuit, &Default::default()).unwrap(),
                Circuit::from_stim_str(expected).unwrap()
            );
        }
    }

    #[test]
    fn circuit_with_inlined_feedback_preserves_cz_sweep_only_groups() {
        let circuit = Circuit::from_stim_str("CZ sweep[0] sweep[1]\n").unwrap();
        assert_eq!(
            circuit_with_inlined_feedback(&circuit, &Default::default()).unwrap(),
            circuit
        );
    }

    #[test]
    fn circuit_with_inlined_feedback_rejects_non_cz_sweep_only_groups() {
        for gate in ["CX", "CY", "XCZ", "YCZ"] {
            let circuit = Circuit::from_stim_str(&format!("{gate} sweep[0] sweep[1]\n")).unwrap();
            let error = circuit_with_inlined_feedback(&circuit, &Default::default()).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains("unsupported controlled-Pauli target orientation"),
                "{gate}: {error}"
            );
        }
    }

    #[test]
    fn feedback_unrolling_is_admitted_against_the_memory_budget_and_nesting_stays_fixed() {
        let above_old_work_cap = Circuit::from_stim_str("REPEAT 1000001 {\nTICK\n}\n").unwrap();
        validate_feedback_repeat_budget(&above_old_work_cap, MemoryBudget::DEFAULT).unwrap();

        let one_iteration = Circuit::from_stim_str("REPEAT 1 {\nTICK\n}\n").unwrap();
        let exact =
            validate_feedback_repeat_budget(&one_iteration, MemoryBudget::UNLIMITED).unwrap();
        assert!(exact > 0);
        validate_feedback_repeat_budget(&one_iteration, MemoryBudget::from_bytes(exact)).unwrap();
        let error =
            validate_feedback_repeat_budget(&one_iteration, MemoryBudget::from_bytes(exact - 1))
                .unwrap_err();
        let resource = error.resource_limit_error().unwrap();
        assert_eq!(resource.operation(), AnalysisOperation::FeedbackInlining);
        assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
        assert_eq!((resource.actual(), resource.limit()), (exact, exact - 1));
        assert!(error.to_string().contains("memory budget"), "{error}");

        // A compact repeat whose unrolling would retain far more than the budget is refused
        // before any unrolled output is built.
        let hostile = Circuit::from_stim_str("REPEAT 1000000000000 {\nTICK\n}\n").unwrap();
        let error = circuit_with_inlined_feedback(&hostile, &Default::default()).unwrap_err();
        assert_eq!(
            error.resource_limit_error().unwrap().resource(),
            ResourceKind::MaterializedBytes
        );

        let mut nested = Circuit::from_stim_str("TICK\n").unwrap();
        for _ in 0..=stab_model::RepeatNestingLimit::HARD_MAX {
            let mut outer = Circuit::new();
            outer.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                RepeatCount::try_new(1).unwrap(),
                nested,
                None,
            ));
            nested = outer;
        }
        let error = validate_feedback_repeat_budget(&nested, MemoryBudget::DEFAULT).unwrap_err();
        let resource = error.resource_limit_error().unwrap();
        assert_eq!(resource.operation(), AnalysisOperation::FeedbackInlining);
        assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), 257);
        assert_eq!(
            resource.limit(),
            stab_model::RepeatNestingLimit::HARD_MAX as u64
        );
    }
}
