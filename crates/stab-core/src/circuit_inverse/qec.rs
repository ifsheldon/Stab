use std::collections::HashSet;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, PauliBasis, Target,
};

use super::{is_plain_qubit_target, reset_inverse_gate_and_basis};

pub(super) fn selected_qec_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    if let Some(inverse) = selected_reset_cx_measure_two_to_one_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_measure_reset_pass_through_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_reset_measure_detector_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    Ok(None)
}

fn selected_reset_cx_measure_two_to_one_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(cx),
        CircuitItem::Instruction(measurement),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if reset.gate().canonical_name() != "R"
        || cx.gate().canonical_name() != "CX"
        || measurement.gate().canonical_name() != "M"
        || detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_reset_cx_measure_two_to_one_inverse(
        reset,
        cx,
        measurement,
        detector,
    )?))
}

fn build_selected_reset_cx_measure_two_to_one_inverse(
    reset: &CircuitInstruction,
    cx: &CircuitInstruction,
    measurement: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !reset.args().is_empty() || !cx.args().is_empty() || !measurement.args().is_empty() {
        return Err(inverse_qec_two_to_one_error(
            "reset, CX, and measurement instructions must be noiseless",
        ));
    }

    let reset_targets = plain_unique_single_qubit_targets(reset)
        .ok_or_else(|| inverse_qec_two_to_one_error("reset targets must be plain unique qubits"))?;
    let measurement_targets = plain_unique_single_qubit_targets(measurement).ok_or_else(|| {
        inverse_qec_two_to_one_error("measurement targets must be plain unique qubits")
    })?;
    if reset_targets.len() != 2 {
        return Err(inverse_qec_two_to_one_error(
            "reset and measurement must each have exactly two targets",
        ));
    }
    if reset_targets != measurement_targets {
        return Err(inverse_qec_two_to_one_error(
            "reset and measurement targets must match exactly",
        ));
    }

    let cx_target_groups = cx.target_groups();
    let [cx_targets] = cx_target_groups.as_slice() else {
        return Err(inverse_qec_two_to_one_error(
            "CX must have exactly one target pair",
        ));
    };
    if *cx_targets != reset_targets.as_slice() || !cx_targets.iter().all(is_plain_qubit_target) {
        return Err(inverse_qec_two_to_one_error(
            "CX target pair must match the reset and measurement target order exactly",
        ));
    }

    let detector_offsets = detector
        .targets()
        .iter()
        .map(|target| {
            target
                .measurement_record_offset()
                .map(MeasureRecordOffset::get)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            inverse_qec_two_to_one_error("detector targets must be measurement records")
        })?;
    if detector_offsets.as_slice() != [-1, -2] {
        return Err(inverse_qec_two_to_one_error(
            "detector must reference exactly rec[-1] rec[-2]",
        ));
    }

    let reversed_reset_targets = reset_targets.iter().rev().cloned().collect::<Vec<_>>();
    let mut result = Circuit::new();
    append_target_instruction(
        &mut result,
        reset.gate(),
        measurement.args(),
        reversed_reset_targets.clone(),
        measurement.tag(),
    )?;
    append_target_instruction(
        &mut result,
        cx.gate(),
        cx.args(),
        cx.targets().to_vec(),
        cx.tag(),
    )?;
    append_target_instruction(
        &mut result,
        measurement.gate(),
        reset.args(),
        reversed_reset_targets,
        reset.tag(),
    )?;
    append_target_instruction(
        &mut result,
        detector.gate(),
        detector.args(),
        vec![Target::measurement_record(MeasureRecordOffset::try_new(
            -2,
        )?)],
        detector.tag(),
    )?;

    Ok(result)
}

fn selected_measure_reset_pass_through_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(measurement),
        CircuitItem::Instruction(measure_reset),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    let Some((measurement_gate, basis)) =
        reset_inverse_gate_and_basis(reset.gate().canonical_name())
    else {
        return Ok(None);
    };
    if measurement.gate().canonical_name() != measurement_gate
        || measure_reset.gate().canonical_name() != measure_reset_gate_for_basis(basis)
        || detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_measure_reset_pass_through_inverse(
        reset,
        measurement,
        measure_reset,
        detector,
    )?))
}

fn build_selected_measure_reset_pass_through_inverse(
    reset: &CircuitInstruction,
    measurement: &CircuitInstruction,
    measure_reset: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !reset.args().is_empty()
        || !measurement.args().is_empty()
        || !measure_reset.args().is_empty()
    {
        return Err(inverse_qec_measure_reset_pass_through_error(
            "reset, measurement, and measure-reset instructions must be noiseless",
        ));
    }

    let reset_targets = plain_unique_single_qubit_targets(reset).ok_or_else(|| {
        inverse_qec_measure_reset_pass_through_error("reset targets must be plain unique qubits")
    })?;
    let measurement_targets = plain_unique_single_qubit_targets(measurement).ok_or_else(|| {
        inverse_qec_measure_reset_pass_through_error(
            "measurement targets must be plain unique qubits",
        )
    })?;
    let measure_reset_targets =
        plain_unique_single_qubit_targets(measure_reset).ok_or_else(|| {
            inverse_qec_measure_reset_pass_through_error(
                "measure-reset targets must be plain unique qubits",
            )
        })?;
    if reset_targets.is_empty() {
        return Err(inverse_qec_measure_reset_pass_through_error(
            "target lists must be non-empty",
        ));
    }
    if reset_targets != measurement_targets || reset_targets != measure_reset_targets {
        return Err(inverse_qec_measure_reset_pass_through_error(
            "reset, measurement, and measure-reset targets must match exactly",
        ));
    }

    let measure_reset_count = i64::try_from(measure_reset_targets.len()).map_err(|_| {
        inverse_qec_measure_reset_pass_through_error(
            "measure-reset target count exceeds supported range",
        )
    })?;
    let mut detector_record_deps = vec![false; measure_reset_targets.len()];
    for target in detector.targets() {
        let Some(offset) = target.measurement_record_offset() else {
            return Err(inverse_qec_measure_reset_pass_through_error(
                "detector targets must be measurement records",
            ));
        };
        let index = measure_reset_count + i64::from(offset.get());
        if !(0..measure_reset_count).contains(&index) {
            return Err(inverse_qec_measure_reset_pass_through_error(
                "detector record target is outside the selected measure-reset group",
            ));
        }
        let detector_record_index = usize::try_from(index).map_err(|_| {
            inverse_qec_measure_reset_pass_through_error(
                "detector record target index exceeds supported range",
            )
        })?;
        let Some(record_dep) = detector_record_deps.get_mut(detector_record_index) else {
            return Err(inverse_qec_measure_reset_pass_through_error(
                "detector record target index is outside the selected measure-reset group",
            ));
        };
        *record_dep = !*record_dep;
    }

    let mut result = Circuit::new();
    append_target_instruction(
        &mut result,
        measure_reset.gate(),
        measure_reset.args(),
        measure_reset_targets.iter().rev().cloned().collect(),
        measure_reset.tag(),
    )?;
    append_target_instruction(
        &mut result,
        measurement.gate(),
        measurement.args(),
        measurement_targets.iter().rev().cloned().collect(),
        measurement.tag(),
    )?;
    append_target_instruction(
        &mut result,
        measurement.gate(),
        reset.args(),
        reset_targets.iter().rev().cloned().collect(),
        reset.tag(),
    )?;

    let total_measurements = measure_reset_targets.len().checked_mul(3).ok_or_else(|| {
        inverse_qec_measure_reset_pass_through_error(
            "new measurement count exceeds supported range",
        )
    })?;
    let mut detector_measurements = Vec::new();
    for (original_index, &record_dep) in detector_record_deps.iter().enumerate() {
        if record_dep {
            let measurement_index = total_measurements
                .checked_sub(original_index + 1)
                .ok_or_else(|| {
                    inverse_qec_measure_reset_pass_through_error(
                        "new detector measurement index exceeds supported range",
                    )
                })?;
            detector_measurements.push(measurement_index);
        }
    }
    detector_measurements.sort_unstable();
    if !detector_measurements.is_empty() {
        let total_measurements = i32::try_from(total_measurements).map_err(|_| {
            inverse_qec_measure_reset_pass_through_error(
                "new measurement count exceeds supported range",
            )
        })?;
        let mut detector_targets = Vec::with_capacity(detector_measurements.len());
        for measurement_index in detector_measurements {
            let measurement_index = i32::try_from(measurement_index).map_err(|_| {
                inverse_qec_measure_reset_pass_through_error(
                    "new detector measurement index exceeds supported range",
                )
            })?;
            detector_targets.push(Target::measurement_record(MeasureRecordOffset::try_new(
                measurement_index - total_measurements,
            )?));
        }
        append_target_instruction(
            &mut result,
            detector.gate(),
            detector.args(),
            detector_targets,
            detector.tag(),
        )?;
    }

    Ok(result)
}

fn selected_reset_measure_detector_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(measurement),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    let Some((measurement_gate, _basis)) =
        reset_inverse_gate_and_basis(reset.gate().canonical_name())
    else {
        return Ok(None);
    };
    if measurement.gate().canonical_name() != measurement_gate
        || detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_reset_measure_detector_inverse(
        reset,
        measurement,
        detector,
    )?))
}

fn build_selected_reset_measure_detector_inverse(
    reset: &CircuitInstruction,
    measurement: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !reset.args().is_empty() || !measurement.args().is_empty() {
        return Err(inverse_qec_reset_measure_detector_error(
            "reset and measurement instructions must be noiseless",
        ));
    }

    let reset_targets = plain_unique_single_qubit_targets(reset).ok_or_else(|| {
        inverse_qec_reset_measure_detector_error("reset targets must be plain unique qubits")
    })?;
    let measurement_targets = plain_unique_single_qubit_targets(measurement).ok_or_else(|| {
        inverse_qec_reset_measure_detector_error("measurement targets must be plain unique qubits")
    })?;
    if reset_targets != measurement_targets {
        return Err(inverse_qec_reset_measure_detector_error(
            "reset and measurement targets must match exactly",
        ));
    }

    let mut detector_record_touched = vec![false; measurement_targets.len()];
    let mut detector_record_deps = vec![false; measurement_targets.len()];
    let measurement_count = i64::try_from(measurement_targets.len()).map_err(|_| {
        inverse_qec_reset_measure_detector_error("measurement target count exceeds supported range")
    })?;
    for target in detector.targets() {
        let Some(offset) = target.measurement_record_offset() else {
            return Err(inverse_qec_reset_measure_detector_error(
                "detector targets must be measurement records",
            ));
        };
        let index = measurement_count + i64::from(offset.get());
        if !(0..measurement_count).contains(&index) {
            return Err(inverse_qec_reset_measure_detector_error(
                "detector record target is outside the selected measurement group",
            ));
        }
        let detector_record_index = usize::try_from(index).map_err(|_| {
            inverse_qec_reset_measure_detector_error(
                "detector record target index exceeds supported range",
            )
        })?;
        let Some(record_touched) = detector_record_touched.get_mut(detector_record_index) else {
            return Err(inverse_qec_reset_measure_detector_error(
                "detector record target index is outside the selected measurement group",
            ));
        };
        *record_touched = true;
        let Some(record_dep) = detector_record_deps.get_mut(detector_record_index) else {
            return Err(inverse_qec_reset_measure_detector_error(
                "detector record target index is outside the selected measurement group",
            ));
        };
        *record_dep = !*record_dep;
    }

    let mut result = Circuit::new();
    let mut qubit_active = vec![false; measurement_targets.len()];
    let mut detector_measurements = Vec::new();
    let mut new_measurements = 0usize;

    for (((target, &record_touched), &record_dep), active) in measurement_targets
        .iter()
        .zip(detector_record_touched.iter())
        .zip(detector_record_deps.iter())
        .zip(qubit_active.iter_mut())
        .rev()
    {
        if record_touched && !*active {
            append_one_target_instruction(
                &mut result,
                reset.gate(),
                measurement.args(),
                target.clone(),
                measurement.tag(),
            )?;
        } else {
            if record_dep {
                detector_measurements.push(new_measurements);
            }
            append_one_target_instruction(
                &mut result,
                measurement.gate(),
                measurement.args(),
                target.clone(),
                measurement.tag(),
            )?;
            new_measurements += 1;
        }
        if record_dep {
            *active = !*active;
        }
    }

    for active in qubit_active.iter_mut().rev() {
        if *active {
            detector_measurements.push(new_measurements);
        }
        *active = false;
        new_measurements += 1;
    }
    append_target_instruction(
        &mut result,
        measurement.gate(),
        reset.args(),
        reset_targets.iter().rev().cloned().collect(),
        reset.tag(),
    )?;

    detector_measurements.sort_unstable();
    detector_measurements.dedup();
    if !detector_measurements.is_empty() {
        let total_measurements = i32::try_from(new_measurements).map_err(|_| {
            inverse_qec_reset_measure_detector_error(
                "new measurement count exceeds supported range",
            )
        })?;
        let mut detector_targets = Vec::with_capacity(detector_measurements.len());
        for measurement_index in detector_measurements {
            let measurement_index = i32::try_from(measurement_index).map_err(|_| {
                inverse_qec_reset_measure_detector_error(
                    "new detector measurement index exceeds supported range",
                )
            })?;
            detector_targets.push(Target::measurement_record(MeasureRecordOffset::try_new(
                measurement_index - total_measurements,
            )?));
        }
        append_target_instruction(
            &mut result,
            detector.gate(),
            detector.args(),
            detector_targets,
            detector.tag(),
        )?;
    }

    Ok(result)
}

fn append_one_target_instruction(
    circuit: &mut Circuit,
    gate: Gate,
    args: &[f64],
    target: Target,
    tag: Option<&str>,
) -> CircuitResult<()> {
    append_target_instruction(circuit, gate, args, vec![target], tag)
}

fn append_target_instruction(
    circuit: &mut Circuit,
    gate: Gate,
    args: &[f64],
    targets: Vec<Target>,
    tag: Option<&str>,
) -> CircuitResult<()> {
    if targets.is_empty() {
        return Ok(());
    }
    circuit.append_instruction(CircuitInstruction::new(
        gate,
        args.to_vec(),
        targets,
        tag.map(str::to_owned),
    )?);
    Ok(())
}

fn plain_unique_single_qubit_targets(instruction: &CircuitInstruction) -> Option<Vec<Target>> {
    let groups = instruction.target_groups();
    if groups.is_empty() && instruction.targets().is_empty() {
        return Some(Vec::new());
    }
    let mut seen = HashSet::with_capacity(groups.len());
    let mut targets = Vec::with_capacity(groups.len());
    for group in groups {
        let [target] = group else {
            return None;
        };
        if !is_plain_qubit_target(target) {
            return None;
        }
        let qubit = target.qubit_id()?.get();
        if !seen.insert(qubit) {
            return None;
        }
        targets.push(target.clone());
    }
    Some(targets)
}

fn measure_reset_gate_for_basis(basis: PauliBasis) -> &'static str {
    match basis {
        PauliBasis::X => "MRX",
        PauliBasis::Y => "MRY",
        PauliBasis::Z => "MR",
        PauliBasis::I => unreachable!("reset_inverse_gate_and_basis never returns identity basis"),
    }
}

fn inverse_qec_reset_measure_detector_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected reset-measure-detector subset requires one noiseless plain reset instruction, one matching noiseless plain measurement instruction, and one detector referencing only those measurement records; {reason}"
    ))
}

fn inverse_qec_measure_reset_pass_through_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected measure-reset pass-through subset requires one noiseless plain reset instruction, one matching noiseless plain measurement instruction, one matching noiseless plain measure-reset instruction, and one detector referencing only those measure-reset records; {reason}"
    ))
}

fn inverse_qec_two_to_one_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected two_to_one subset requires one noiseless plain two-target R instruction, one matching CX pair, one matching noiseless plain two-target M instruction, and one detector containing exactly rec[-1] rec[-2]; {reason}"
    ))
}
