use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget, Gate,
    GateCategory, MeasureRecordOffset, Pauli, Target, detection::instruction_measurement_count,
    measurement_record_count, sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

pub fn circuit_with_inlined_feedback(circuit: &Circuit) -> CircuitResult<Circuit> {
    let measurement_count = measurement_record_count(circuit)?;
    let detector_count = detector_count(circuit)?;
    let mut helper = WithoutFeedbackHelper {
        reversed_output: Vec::new(),
        tracker: SparseReverseFrameTracker::new(
            circuit.count_qubits(),
            measurement_count,
            detector_count,
            false,
        ),
        observable_changes: BTreeMap::new(),
        detector_changes: BTreeMap::new(),
    };
    helper.undo_circuit(circuit)?;
    helper.build_output()
}

struct WithoutFeedbackHelper {
    reversed_output: Vec<CircuitInstruction>,
    tracker: SparseReverseFrameTracker,
    observable_changes: BTreeMap<u64, BTreeSet<MeasureRecordOffset>>,
    detector_changes: BTreeMap<u64, BTreeSet<usize>>,
}

impl WithoutFeedbackHelper {
    fn undo_circuit(&mut self, circuit: &Circuit) -> CircuitResult<()> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => self.undo_instruction(instruction)?,
                CircuitItem::RepeatBlock(repeat) => {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "feedback inlining does not support repeat blocks with count {}",
                        repeat.repeat_count().get()
                    )));
                }
            }
        }
        Ok(())
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        if matches!(instruction.gate().canonical_name(), "CX" | "CY" | "CZ") {
            return self.undo_feedback_capable_controlled_pauli(instruction);
        }
        if instruction.gate().category() == GateCategory::Controlled
            && instruction
                .targets()
                .iter()
                .any(Target::is_classical_bit_target)
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "feedback inlining does not support {} with classical controls",
                instruction.gate().canonical_name()
            )));
        }
        self.reversed_output.push(instruction.clone());
        self.tracker.undo_instruction(instruction)
    }

    fn undo_feedback_capable_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups().into_iter().rev() {
            let [first, second] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} expected paired targets during feedback inlining",
                    instruction.gate().canonical_name()
                )));
            };
            let piece = instruction_with_targets(instruction, group.to_vec())?;
            match (
                first.measurement_record_offset(),
                second.measurement_record_offset(),
            ) {
                (Some(record), None) => self.inline_feedback(instruction, record, second)?,
                (None, Some(record)) => self.inline_feedback(instruction, record, first)?,
                (Some(_), Some(_)) => {}
                (None, None) => self.reversed_output.push(piece.clone()),
            }
            self.tracker.undo_instruction(&piece)?;
        }
        self.flush_observable_changes(instruction)?;
        Ok(())
    }

    fn inline_feedback(
        &mut self,
        instruction: &CircuitInstruction,
        record: MeasureRecordOffset,
        target: &Target,
    ) -> CircuitResult<()> {
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{} feedback target {target} is not a qubit",
                instruction.gate().canonical_name()
            ))
        })?;
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
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "feedback sensitivity unexpectedly contained DEM target {target}"
                    )));
                }
            }
        }
        Ok(())
    }

    fn flush_observable_changes(&mut self, source: &CircuitInstruction) -> CircuitResult<()> {
        let changes = std::mem::take(&mut self.observable_changes);
        for (observable, records) in changes {
            if records.is_empty() {
                continue;
            }
            let instruction = CircuitInstruction::new(
                Gate::from_name("OBSERVABLE_INCLUDE")?,
                vec![observable as f64],
                records
                    .into_iter()
                    .map(Target::measurement_record)
                    .collect(),
                source.tag().map(str::to_owned),
            )?;
            self.reversed_output.push(instruction);
        }
        Ok(())
    }

    fn build_output(&self) -> CircuitResult<Circuit> {
        let mut result = Circuit::new();
        let mut measurements_in_past = 0usize;
        let mut detectors_in_past = 0u64;
        for instruction in self.reversed_output.iter().rev() {
            measurements_in_past = measurements_in_past
                .checked_add(instruction_measurement_count(instruction))
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "measurement count overflowed while building feedback-free circuit",
                    )
                })?;
            if instruction.gate().canonical_name() == "DETECTOR" {
                let detector_id = detectors_in_past;
                detectors_in_past = detectors_in_past.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "detector count overflowed while building feedback-free circuit",
                    )
                })?;
                if let Some(changes) = self.detector_changes.get(&detector_id) {
                    result.append_instruction(rewritten_detector(
                        instruction,
                        changes,
                        measurements_in_past,
                    )?);
                    continue;
                }
            }
            result.append_instruction(instruction.clone());
        }
        Ok(result)
    }
}

fn rewritten_detector(
    instruction: &CircuitInstruction,
    changes: &BTreeSet<usize>,
    measurements_in_past: usize,
) -> CircuitResult<CircuitInstruction> {
    let mut targets = changes.clone();
    for target in instruction.targets() {
        let offset = target.measurement_record_offset().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "DETECTOR target {target} is not a measurement record"
            ))
        })?;
        let index = absolute_record_index(measurements_in_past, offset.get())?;
        toggle_value(&mut targets, index);
    }
    CircuitInstruction::new(
        instruction.gate(),
        instruction.args().to_vec(),
        targets
            .into_iter()
            .map(|index| relative_record_target(index, measurements_in_past))
            .collect::<CircuitResult<Vec<_>>>()?,
        instruction.tag().map(str::to_owned),
    )
}

fn detector_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut count = 0u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "DETECTOR" {
                    count = count.checked_add(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("detector count overflowed")
                    })?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = detector_count(repeat.body())?;
                let repeated = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "repeat detector count overflowed",
                        )
                    })?;
                count = count.checked_add(repeated).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("detector count overflowed")
                })?;
            }
        }
    }
    Ok(count)
}

fn instruction_with_targets(
    instruction: &CircuitInstruction,
    targets: Vec<Target>,
) -> CircuitResult<CircuitInstruction> {
    CircuitInstruction::new(
        instruction.gate(),
        instruction.args().to_vec(),
        targets,
        instruction.tag().map(str::to_owned),
    )
}

fn feedback_pauli(gate: Gate) -> CircuitResult<Pauli> {
    match gate.canonical_name() {
        "CX" => Ok(Pauli::X),
        "CY" => Ok(Pauli::Y),
        "CZ" => Ok(Pauli::Z),
        name => Err(CircuitError::invalid_detector_error_model(format!(
            "{name} is not a supported feedback gate"
        ))),
    }
}

fn absolute_record_index(measurements_in_past: usize, offset: i32) -> CircuitResult<usize> {
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let index = current.checked_add(i64::from(offset)).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "measurement record offset overflowed while rewriting detector",
        )
    })?;
    if index < 0 || index >= current {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "measurement record target rec[{offset}] is outside feedback rewrite history"
        )));
    }
    usize::try_from(index).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement record index does not fit usize while rewriting detector",
        )
    })
}

fn relative_record_target(
    absolute_index: usize,
    measurements_in_past: usize,
) -> CircuitResult<Target> {
    let absolute = i64::try_from(absolute_index).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "absolute measurement index does not fit i64 while rewriting detector",
        )
    })?;
    let current = i64::try_from(measurements_in_past).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "measurement count does not fit i64 while rewriting detector",
        )
    })?;
    let offset = absolute.checked_sub(current).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "relative measurement offset overflowed while rewriting detector",
        )
    })?;
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        i32::try_from(offset).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
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

    use crate::{ErrorAnalyzerOptions, circuit_to_detector_error_model};

    use super::*;

    fn transform(text: &str) -> String {
        let circuit = Circuit::from_stim_str(text).unwrap();
        circuit_with_inlined_feedback(&circuit)
            .unwrap()
            .to_stim_string()
    }

    #[test]
    fn circuit_with_inlined_feedback_basic() {
        assert_eq!(
            transform(
                "MR 0\n\
                 H 0\n\
                 CX sweep[5] 0\n\
                 CY rec[-1] 0 rec[-1] 0 2 3 rec[-1] 0\n\
                 H 0\n\
                 M 0\n\
                 DETECTOR rec[-1]\n\
                 OBSERVABLE_INCLUDE(2) rec[-1]\n"
            ),
            "MR 0\n\
             H 0\n\
             CX sweep[5] 0\n\
             OBSERVABLE_INCLUDE(2) rec[-1]\n\
             CY 2 3\n\
             H 0\n\
             M 0\n\
             DETECTOR rec[-2] rec[-1]\n\
             OBSERVABLE_INCLUDE(2) rec[-1]\n"
        );
    }

    #[test]
    fn circuit_with_inlined_feedback_demolition_feedback() {
        assert_eq!(
            transform(
                "CX 0 1\n\
                 M 1\n\
                 CX rec[-1] 1\n\
                 CX 0 1\n\
                 M 1\n\
                 DETECTOR rec[-1] rec[-2]\n\
                 OBSERVABLE_INCLUDE(0) rec[-1]\n"
            ),
            "CX 0 1\n\
             M 1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             CX 0 1\n\
             M 1\n\
             DETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n"
        );
    }

    #[test]
    fn circuit_with_inlined_feedback_interleaved_order() {
        assert_eq!(transform("H 0\nCZ\nH 1\n"), "H 0 1\n");
        assert_eq!(transform("M 0\nCX\nM 1\n"), "M 0 1\n");
        assert_eq!(
            transform(
                "M 0 1\n\
                 CX\n\
                 M 2\n\
                 CX rec[-1] 3\n\
                 M 3\n\
                 DETECTOR rec[-1]\n"
            ),
            "M 0 1 2 3\n\
             DETECTOR rec[-2] rec[-1]\n"
        );
    }

    #[test]
    fn circuit_with_inlined_feedback_mpp() {
        let input = Circuit::from_stim_str(
            "RX 0\n\
             RY 1\n\
             RZ 2\n\
             MPP X0*Y1*Z2 Z5\n\
             CX rec[-2] 3\n\
             M 3\n\
             DETECTOR rec[-1]\n",
        )
        .unwrap();
        let actual = circuit_with_inlined_feedback(&input).unwrap();

        assert_eq!(
            actual.to_stim_string(),
            "RX 0\n\
             RY 1\n\
             R 2\n\
             MPP X0*Y1*Z2 Z5\n\
             M 3\n\
             DETECTOR rec[-3] rec[-1]\n"
        );

        let expected_dem = circuit_to_detector_error_model(&input, ErrorAnalyzerOptions::default())
            .unwrap()
            .to_dem_string();
        let actual_dem = circuit_to_detector_error_model(&actual, ErrorAnalyzerOptions::default())
            .unwrap()
            .to_dem_string();
        assert_eq!(actual_dem, expected_dem);
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

        assert!(error.to_string().contains("does not support XCZ"));
    }

    #[test]
    fn circuit_with_inlined_feedback_rejects_repeat_blocks() {
        let circuit = Circuit::from_stim_str(
            "REPEAT 2 {\n\
                 M 0\n\
                 CX rec[-1] 0\n\
             }\n",
        )
        .unwrap();
        let error = circuit_with_inlined_feedback(&circuit).unwrap_err();

        assert!(error.to_string().contains("does not support repeat blocks"));
    }
}
