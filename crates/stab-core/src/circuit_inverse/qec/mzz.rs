use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, Target,
};

pub(super) fn selected_mzz_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(first_mry),
        CircuitItem::Instruction(first_m),
        CircuitItem::Instruction(first_tick),
        CircuitItem::Instruction(mzz),
        CircuitItem::Instruction(second_tick),
        CircuitItem::Instruction(second_m),
        CircuitItem::Instruction(last_mry),
        CircuitItem::Instruction(detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if first_mry.gate().canonical_name() != "MRY"
        || first_m.gate().canonical_name() != "M"
        || first_tick.gate().canonical_name() != "TICK"
        || mzz.gate().canonical_name() != "MZZ"
        || second_tick.gate().canonical_name() != "TICK"
        || second_m.gate().canonical_name() != "M"
        || last_mry.gate().canonical_name() != "MRY"
        || detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_mzz_inverse(SelectedMzzPacket {
        first_mry,
        first_m,
        first_tick,
        mzz,
        second_tick,
        second_m,
        last_mry,
        detector,
    })?))
}

struct SelectedMzzPacket<'a> {
    first_mry: &'a CircuitInstruction,
    first_m: &'a CircuitInstruction,
    first_tick: &'a CircuitInstruction,
    mzz: &'a CircuitInstruction,
    second_tick: &'a CircuitInstruction,
    second_m: &'a CircuitInstruction,
    last_mry: &'a CircuitInstruction,
    detector: &'a CircuitInstruction,
}

fn build_selected_mzz_inverse(packet: SelectedMzzPacket<'_>) -> CircuitResult<Circuit> {
    let SelectedMzzPacket {
        first_mry,
        first_m,
        first_tick,
        mzz,
        second_tick,
        second_m,
        last_mry,
        detector,
    } = packet;

    if !first_mry.args().is_empty()
        || !first_m.args().is_empty()
        || !second_m.args().is_empty()
        || !last_mry.args().is_empty()
    {
        return Err(inverse_qec_mzz_error(
            "MRY and M instructions must be noiseless",
        ));
    }
    if mzz.args().len() != 1 {
        return Err(inverse_qec_mzz_error(
            "MZZ instruction must have exactly one probability argument",
        ));
    }
    if first_mry.tag().is_some()
        || first_m.tag().is_some()
        || mzz.tag().is_some()
        || second_m.tag().is_some()
        || last_mry.tag().is_some()
    {
        return Err(inverse_qec_mzz_error(
            "MRY, M, and MZZ instruction tags are outside this selected packet",
        ));
    }
    if !first_tick.args().is_empty()
        || !first_tick.targets().is_empty()
        || !second_tick.args().is_empty()
        || !second_tick.targets().is_empty()
    {
        return Err(inverse_qec_mzz_error(
            "TICK instructions must not have arguments or targets",
        ));
    }

    let first_mry_targets = exact_plain_targets(first_mry, [0, 1], "MRY")?;
    let last_mry_targets = exact_plain_targets(last_mry, [0, 1], "MRY")?;
    if first_mry_targets != last_mry_targets {
        return Err(inverse_qec_mzz_error("MRY target lists must match exactly"));
    }
    let [first_m_target] = exact_plain_targets(first_m, [0], "first M")?;
    let [second_m_target] = exact_plain_targets(second_m, [1], "second M")?;
    let mzz_targets = exact_mzz_targets(mzz)?;

    let detector_offsets = super::detector_offsets(detector, inverse_qec_mzz_error)?;
    if detector_offsets.as_slice() != [-3, -5, -6] {
        return Err(inverse_qec_mzz_error(
            "detector must reference exactly rec[-3] rec[-5] rec[-6]",
        ));
    }

    let [mry0, mry1] = first_mry_targets;
    let [mzz0, mzz1, mzz2, mzz3] = mzz_targets;
    let reset_gate = Gate::from_name("R")?;

    let mut result = Circuit::new();
    super::append_target_instruction(
        &mut result,
        first_mry.gate(),
        first_mry.args(),
        vec![mry1.clone(), mry0.clone()],
        first_mry.tag(),
    )?;
    super::append_one_target_instruction(&mut result, reset_gate, &[], second_m_target, None)?;
    result.append_instruction(first_tick.clone());
    super::append_target_instruction(
        &mut result,
        mzz.gate(),
        mzz.args(),
        vec![mzz2, mzz3, mzz0, mzz1],
        mzz.tag(),
    )?;
    result.append_instruction(second_tick.clone());
    super::append_one_target_instruction(
        &mut result,
        first_m.gate(),
        first_m.args(),
        first_m_target,
        first_m.tag(),
    )?;
    super::append_target_instruction(
        &mut result,
        detector.gate(),
        detector.args(),
        vec![
            Target::measurement_record(MeasureRecordOffset::try_new(-2)?),
            Target::measurement_record(MeasureRecordOffset::try_new(-1)?),
        ],
        detector.tag(),
    )?;
    super::append_target_instruction(
        &mut result,
        last_mry.gate(),
        last_mry.args(),
        vec![mry1, mry0],
        last_mry.tag(),
    )?;
    Ok(result)
}

fn exact_plain_targets<const N: usize>(
    instruction: &CircuitInstruction,
    expected_ids: [u32; N],
    label: &str,
) -> CircuitResult<[Target; N]> {
    let targets = super::plain_unique_single_qubit_targets(instruction)
        .ok_or_else(|| inverse_qec_mzz_error("targets must be plain unique qubits"))?;
    let Ok(targets) = <Vec<Target> as TryInto<[Target; N]>>::try_into(targets) else {
        return Err(inverse_qec_mzz_error(&format!(
            "{label} target list does not match the selected packet"
        )));
    };
    let ids = targets
        .iter()
        .filter_map(Target::qubit_id)
        .map(|id| id.get())
        .collect::<Vec<_>>();
    if ids.as_slice() != expected_ids {
        return Err(inverse_qec_mzz_error(&format!(
            "{label} target list does not match the selected packet"
        )));
    }
    Ok(targets)
}

fn exact_mzz_targets(instruction: &CircuitInstruction) -> CircuitResult<[Target; 4]> {
    if instruction.target_groups().len() != 2 {
        return Err(inverse_qec_mzz_error(
            "MZZ must contain exactly two target pairs",
        ));
    }
    let targets = instruction.targets();
    let [target0, target1, target2, target3] = targets else {
        return Err(inverse_qec_mzz_error(
            "MZZ target list must be exactly 0 1 2 3",
        ));
    };
    for target in targets {
        if !super::is_plain_qubit_target(target) {
            return Err(inverse_qec_mzz_error("MZZ targets must be plain qubits"));
        }
    }
    let ids = [
        target0.qubit_id().map(|id| id.get()),
        target1.qubit_id().map(|id| id.get()),
        target2.qubit_id().map(|id| id.get()),
        target3.qubit_id().map(|id| id.get()),
    ];
    if ids != [Some(0), Some(1), Some(2), Some(3)] {
        return Err(inverse_qec_mzz_error(
            "MZZ target list must be exactly 0 1 2 3",
        ));
    }
    Ok([
        target0.clone(),
        target1.clone(),
        target2.clone(),
        target3.clone(),
    ])
}

fn inverse_qec_mzz_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected MZZ detector subset requires exact top-level MRY 0 1, M 0, TICK, MZZ(p) 0 1 2 3, TICK, M 1, MRY 0 1, and DETECTOR rec[-3] rec[-5] rec[-6]; {reason}"
    ))
}
