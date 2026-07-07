use std::collections::HashSet;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, Pauli, PauliBasis, Target,
};

use super::{InverseQecOptions, is_plain_qubit_target, reset_inverse_gate_and_basis};

mod m_det;
mod mpad;
mod mzz;
mod obs_include;

pub(super) fn selected_qec_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    if let Some(inverse) = selected_reset_cx_measure_two_to_one_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = mpad::selected_mpad_record_tail_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_mpp_detector_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_noisy_measurement_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_noisy_measure_reset_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_noisy_measure_reset_detector_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = m_det::selected_m_det_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = mzz::selected_mzz_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = obs_include::selected_obs_include_pauli_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) = selected_measure_reset_pass_through_inverse(circuit)? {
        return Ok(Some(inverse));
    }
    if let Some(inverse) =
        selected_reset_measure_detector_inverse(circuit, InverseQecOptions::default())?
    {
        return Ok(Some(inverse));
    }
    Ok(None)
}

pub(super) fn selected_keep_measurements_qec_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(measurement),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if !is_exact_keep_measurements_reset_measure_detector_packet(reset, measurement, detector)? {
        return Ok(None);
    }

    Ok(Some(build_selected_reset_measure_detector_inverse(
        reset,
        measurement,
        detector,
        InverseQecOptions {
            keep_measurements: true,
        },
    )?))
}

fn is_exact_keep_measurements_reset_measure_detector_packet(
    reset: &CircuitInstruction,
    measurement: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<bool> {
    if reset.gate().canonical_name() != "R"
        || measurement.gate().canonical_name() != "M"
        || detector.gate().canonical_name() != "DETECTOR"
        || !reset.args().is_empty()
        || !measurement.args().is_empty()
        || !detector.args().is_empty()
        || reset.tag().is_some()
        || measurement.tag().is_some()
        || detector.tag().is_some()
    {
        return Ok(false);
    }

    let Some(reset_targets) = plain_unique_single_qubit_targets(reset) else {
        return Ok(false);
    };
    let Some(measurement_targets) = plain_unique_single_qubit_targets(measurement) else {
        return Ok(false);
    };
    let [detector_target] = detector.targets() else {
        return Ok(false);
    };
    if reset_targets.len() != 1 || reset_targets != measurement_targets {
        return Ok(false);
    }

    let Some(offset) = detector_target.measurement_record_offset() else {
        return Ok(false);
    };
    Ok(offset == MeasureRecordOffset::try_new(-1)?)
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

fn selected_mpp_detector_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(mpp),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if mpp.gate().canonical_name() != "MPP" || detector.gate().canonical_name() != "DETECTOR" {
        return Ok(None);
    }

    Ok(Some(build_selected_mpp_detector_inverse(mpp, detector)?))
}

fn build_selected_mpp_detector_inverse(
    mpp: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !mpp.args().is_empty() {
        return Err(inverse_qec_mpp_detector_error(
            "MPP instruction must be noiseless",
        ));
    }

    let product_groups = mpp.target_groups();
    if product_groups.is_empty() {
        return Err(inverse_qec_mpp_detector_error(
            "MPP must contain at least one Pauli product",
        ));
    }
    for group in &product_groups {
        validate_hermitian_mpp_product(group)?;
    }
    validate_selected_mpp_detector_parity_determined(&product_groups)?;

    let detector_offsets = detector_offsets(detector, inverse_qec_mpp_detector_error)?;
    let expected_detector_offsets =
        consecutive_negative_offsets(product_groups.len(), inverse_qec_mpp_detector_error)?;
    if detector_offsets != expected_detector_offsets {
        return Err(inverse_qec_mpp_detector_error(
            "detector must reference exactly every selected MPP record in order",
        ));
    }

    let mut result = Circuit::new();
    append_target_instruction(
        &mut result,
        mpp.gate(),
        mpp.args(),
        reversed_pauli_product_targets(&product_groups)?,
        mpp.tag(),
    )?;
    append_target_instruction(
        &mut result,
        detector.gate(),
        detector.args(),
        expected_detector_offsets
            .iter()
            .rev()
            .map(|offset| MeasureRecordOffset::try_new(*offset).map(Target::measurement_record))
            .collect::<CircuitResult<Vec<_>>>()?,
        detector.tag(),
    )?;
    Ok(result)
}

fn selected_noisy_measurement_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    if circuit.items().is_empty() {
        return Ok(None);
    }
    let mut instructions = Vec::with_capacity(circuit.items().len());
    for item in circuit.items() {
        let CircuitItem::Instruction(instruction) = item else {
            return Ok(None);
        };
        match instruction.gate().canonical_name() {
            "M" | "MX" | "MY" => instructions.push(instruction),
            _ => return Ok(None),
        }
    }

    let mut result = Circuit::new();
    for instruction in instructions.into_iter().rev() {
        append_target_instruction(
            &mut result,
            instruction.gate(),
            instruction.args(),
            reversed_measurement_targets(instruction)?,
            instruction.tag(),
        )?;
    }
    Ok(Some(result))
}

fn reversed_measurement_targets(instruction: &CircuitInstruction) -> CircuitResult<Vec<Target>> {
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for group in instruction.target_groups().into_iter().rev() {
        let [target] = group else {
            return Err(inverse_qec_noisy_measurement_error(
                "measurement target groups must contain one qubit target",
            ));
        };
        if !target.is_qubit_target() {
            return Err(inverse_qec_noisy_measurement_error(
                "measurement targets must be qubit targets",
            ));
        }
        targets.push(target.clone());
    }
    Ok(targets)
}

fn selected_noisy_measure_reset_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    if circuit.items().is_empty() {
        return Ok(None);
    }
    let mut instructions = Vec::with_capacity(circuit.items().len());
    for item in circuit.items() {
        let CircuitItem::Instruction(instruction) = item else {
            return Ok(None);
        };
        match instruction.gate().canonical_name() {
            "MR" | "MRX" | "MRY" => instructions.push(instruction),
            _ => return Ok(None),
        }
    }

    let mut result = Circuit::new();
    for instruction in instructions.into_iter().rev() {
        append_measure_reset_inverse(&mut result, instruction)?;
    }
    Ok(Some(result))
}

fn append_measure_reset_inverse(
    result: &mut Circuit,
    instruction: &CircuitInstruction,
) -> CircuitResult<()> {
    let noisy = !instruction.args().is_empty();
    if instruction.args().len() > 1 {
        return Err(inverse_qec_noisy_measure_reset_error(
            "measure-reset instructions must have at most one probability argument",
        ));
    }
    let targets = reversed_measure_reset_targets(instruction, !noisy)?;
    if !noisy {
        return append_target_instruction(
            result,
            instruction.gate(),
            instruction.args(),
            targets,
            instruction.tag(),
        );
    }

    let error_gate = Gate::from_name(noisy_measure_reset_error_gate(
        instruction.gate().canonical_name(),
    )?)?;
    for chunk in split_noisy_measure_reset_targets(targets)? {
        append_target_instruction(
            result,
            instruction.gate(),
            &[],
            chunk.clone(),
            instruction.tag(),
        )?;
        append_target_instruction(
            result,
            error_gate,
            instruction.args(),
            chunk,
            instruction.tag(),
        )?;
    }
    Ok(())
}

fn reversed_measure_reset_targets(
    instruction: &CircuitInstruction,
    allow_inverted: bool,
) -> CircuitResult<Vec<Target>> {
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for group in instruction.target_groups().into_iter().rev() {
        let [target] = group else {
            return Err(inverse_qec_noisy_measure_reset_error(
                "measure-reset target groups must contain one qubit target",
            ));
        };
        if !target.is_qubit_target() {
            return Err(inverse_qec_noisy_measure_reset_error(
                "measure-reset targets must be qubit targets",
            ));
        }
        if !allow_inverted && target.is_inverted_result_target() {
            return Err(inverse_qec_noisy_measure_reset_error(
                "noisy measure-reset targets must not be inverted",
            ));
        }
        targets.push(target.clone());
    }
    Ok(targets)
}

fn split_noisy_measure_reset_targets(targets: Vec<Target>) -> CircuitResult<Vec<Vec<Target>>> {
    let mut chunks = Vec::new();
    let mut chunk = Vec::new();
    let mut seen = HashSet::new();
    for target in targets {
        let qubit = target
            .qubit_id()
            .ok_or_else(|| inverse_qec_noisy_measure_reset_error("target must have a qubit"))?
            .get();
        if !seen.insert(qubit) {
            chunks.push(chunk);
            chunk = Vec::new();
            seen.clear();
            seen.insert(qubit);
        }
        chunk.push(target);
    }
    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    Ok(chunks)
}

fn noisy_measure_reset_error_gate(gate_name: &str) -> CircuitResult<&'static str> {
    match gate_name {
        "MR" => Ok("X_ERROR"),
        "MRX" | "MRY" => Ok("Z_ERROR"),
        _ => Err(inverse_qec_noisy_measure_reset_error(
            "unsupported measure-reset gate",
        )),
    }
}

fn selected_noisy_measure_reset_detector_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(pre_tick),
        CircuitItem::Instruction(tick),
        CircuitItem::Instruction(middle),
        CircuitItem::Instruction(last),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if !is_measure_reset_gate(pre_tick.gate().canonical_name())
        || tick.gate().canonical_name() != "TICK"
        || !is_measure_reset_gate(middle.gate().canonical_name())
        || !is_measure_reset_gate(last.gate().canonical_name())
        || detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_noisy_measure_reset_detector_inverse(
        pre_tick, tick, middle, last, detector,
    )?))
}

fn build_selected_noisy_measure_reset_detector_inverse(
    pre_tick: &CircuitInstruction,
    tick: &CircuitInstruction,
    middle: &CircuitInstruction,
    last: &CircuitInstruction,
    detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !tick.args().is_empty() || !tick.targets().is_empty() {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "TICK must not have arguments or targets",
        ));
    }
    let basis = pre_tick.gate().canonical_name();
    if middle.gate().canonical_name() != basis || last.gate().canonical_name() != basis {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "measure-reset instructions must use the same basis",
        ));
    }

    let pre_tick_target = single_noisy_measure_reset_target(pre_tick)?;
    let middle_target = single_noisy_measure_reset_target(middle)?;
    let last_target = single_noisy_measure_reset_target(last)?;
    if pre_tick_target != middle_target || pre_tick_target != last_target {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "measure-reset instructions must target the same single plain qubit",
        ));
    }

    let detector_record_offsets =
        detector_offsets(detector, inverse_qec_noisy_measure_reset_detector_error)?;
    if detector_record_offsets.as_slice() != [-1] {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "detector must reference exactly rec[-1]",
        ));
    }

    let mut result = Circuit::new();
    append_measure_reset_inverse(&mut result, last)?;
    append_measure_reset_inverse(&mut result, middle)?;
    append_target_instruction(
        &mut result,
        detector.gate(),
        detector.args(),
        vec![Target::measurement_record(MeasureRecordOffset::try_new(
            -1,
        )?)],
        detector.tag(),
    )?;
    result.append_instruction(tick.clone());
    append_measure_reset_inverse(&mut result, pre_tick)?;
    Ok(result)
}

fn is_measure_reset_gate(gate_name: &str) -> bool {
    matches!(gate_name, "MR" | "MRX" | "MRY")
}

fn single_noisy_measure_reset_target(instruction: &CircuitInstruction) -> CircuitResult<Target> {
    if instruction.args().len() != 1 {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "measure-reset instructions must have exactly one probability argument",
        ));
    }
    let targets = reversed_measure_reset_targets(instruction, false)?;
    let [target] = targets.as_slice() else {
        return Err(inverse_qec_noisy_measure_reset_detector_error(
            "measure-reset instructions must each have exactly one target",
        ));
    };
    Ok(target.clone())
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

fn selected_reset_measure_detector_inverse(
    circuit: &Circuit,
    options: InverseQecOptions,
) -> CircuitResult<Option<Circuit>> {
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
        options,
    )?))
}

fn build_selected_reset_measure_detector_inverse(
    reset: &CircuitInstruction,
    measurement: &CircuitInstruction,
    detector: &CircuitInstruction,
    options: InverseQecOptions,
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
            if options.keep_measurements {
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
            } else {
                append_one_target_instruction(
                    &mut result,
                    reset.gate(),
                    measurement.args(),
                    target.clone(),
                    measurement.tag(),
                )?;
            }
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

fn detector_offsets(
    detector: &CircuitInstruction,
    error: fn(&str) -> CircuitError,
) -> CircuitResult<Vec<i32>> {
    detector
        .targets()
        .iter()
        .map(|target| {
            target
                .measurement_record_offset()
                .map(MeasureRecordOffset::get)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| error("detector targets must be measurement records"))
}

fn consecutive_negative_offsets(
    count: usize,
    error: fn(&str) -> CircuitError,
) -> CircuitResult<Vec<i32>> {
    (1..=count)
        .map(|index| {
            i32::try_from(index)
                .map(|index| -index)
                .map_err(|_| error("selected measurement count exceeds supported range"))
        })
        .collect()
}

fn reversed_pauli_product_targets(groups: &[&[Target]]) -> CircuitResult<Vec<Target>> {
    let mut targets = Vec::new();
    for group in groups.iter().rev() {
        let factors = group
            .iter()
            .filter(|target| !target.is_combiner())
            .collect::<Vec<_>>();
        if factors.is_empty() {
            return Err(inverse_qec_mpp_detector_error(
                "MPP products must be non-empty",
            ));
        }
        for (index, target) in factors.iter().rev().enumerate() {
            if index > 0 {
                targets.push(Target::combiner());
            }
            targets.push((*target).clone());
        }
    }
    Ok(targets)
}

fn validate_hermitian_mpp_product(group: &[Target]) -> CircuitResult<()> {
    let mut terms = Vec::new();
    let mut phase = 0u8;
    for target in group {
        if target.is_combiner() {
            continue;
        }
        let pauli = target.pauli_type().ok_or_else(|| {
            inverse_qec_mpp_detector_error("MPP product targets must be Pauli targets")
        })?;
        let qubit = target
            .qubit_id()
            .ok_or_else(|| inverse_qec_mpp_detector_error("MPP product targets must have qubits"))?
            .get();
        multiply_mpp_term(&mut terms, qubit, pauli, &mut phase);
    }
    match phase {
        0 | 2 => Ok(()),
        _ => Err(inverse_qec_mpp_detector_error(
            "MPP Pauli product is anti-Hermitian",
        )),
    }
}

fn validate_selected_mpp_detector_parity_determined(groups: &[&[Target]]) -> CircuitResult<()> {
    let mut terms = Vec::new();
    let mut phase = 0u8;
    for group in groups {
        for target in *group {
            if target.is_combiner() {
                continue;
            }
            let pauli = target.pauli_type().ok_or_else(|| {
                inverse_qec_mpp_detector_error("MPP product targets must be Pauli targets")
            })?;
            let qubit = target
                .qubit_id()
                .ok_or_else(|| {
                    inverse_qec_mpp_detector_error("MPP product targets must have qubits")
                })?
                .get();
            if target.is_inverted_result_target() {
                phase = (phase + 2) % 4;
            }
            multiply_mpp_term(&mut terms, qubit, pauli, &mut phase);
        }
    }
    if !terms.is_empty() {
        return Err(inverse_qec_mpp_detector_error(
            "combined selected MPP detector parity must reduce to identity",
        ));
    }
    match phase {
        0 | 2 => Ok(()),
        _ => Err(inverse_qec_mpp_detector_error(
            "combined selected MPP detector parity is anti-Hermitian",
        )),
    }
}

fn multiply_mpp_term(terms: &mut Vec<(u32, Pauli)>, qubit: u32, incoming: Pauli, phase: &mut u8) {
    let Some(index) = terms
        .iter()
        .position(|(existing_qubit, _)| *existing_qubit == qubit)
    else {
        terms.push((qubit, incoming));
        return;
    };
    let (_, existing) = terms.remove(index);
    let (product, phase_delta) = multiply_pauli_bases(existing, incoming);
    *phase = (*phase + phase_delta) % 4;
    if let Some(product) = product {
        terms.insert(index, (qubit, product));
    }
}

fn multiply_pauli_bases(left: Pauli, right: Pauli) -> (Option<Pauli>, u8) {
    match (left, right) {
        (Pauli::X, Pauli::X) | (Pauli::Y, Pauli::Y) | (Pauli::Z, Pauli::Z) => (None, 0),
        (Pauli::X, Pauli::Y) => (Some(Pauli::Z), 1),
        (Pauli::Y, Pauli::Z) => (Some(Pauli::X), 1),
        (Pauli::Z, Pauli::X) => (Some(Pauli::Y), 1),
        (Pauli::Y, Pauli::X) => (Some(Pauli::Z), 3),
        (Pauli::Z, Pauli::Y) => (Some(Pauli::X), 3),
        (Pauli::X, Pauli::Z) => (Some(Pauli::Y), 3),
    }
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

fn inverse_qec_mpp_detector_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected MPP detector subset requires one noiseless MPP instruction with Hermitian Pauli products and one detector referencing exactly all selected MPP records; {reason}"
    ))
}

fn inverse_qec_noisy_measurement_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected noisy measurement subset requires only top-level M, MX, and MY instructions with qubit targets; {reason}"
    ))
}

fn inverse_qec_noisy_measure_reset_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected noisy measure-reset subset requires only top-level MR, MRX, and MRY instructions with qubit targets; {reason}"
    ))
}

fn inverse_qec_noisy_measure_reset_detector_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected noisy measure-reset detector subset requires one noisy single-target MR, MRX, or MRY instruction, one TICK, two matching noisy single-target measure-reset instructions, and one detector containing exactly rec[-1]; {reason}"
    ))
}

fn inverse_qec_two_to_one_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected two_to_one subset requires one noiseless plain two-target R instruction, one matching CX pair, one matching noiseless plain two-target M instruction, and one detector containing exactly rec[-1] rec[-2]; {reason}"
    ))
}
