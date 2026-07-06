use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, MeasureRecordOffset,
    Target,
};

pub(super) fn selected_m_det_inverse(circuit: &Circuit) -> CircuitResult<Option<Circuit>> {
    let [
        CircuitItem::Instruction(reset),
        CircuitItem::Instruction(first_tick),
        CircuitItem::Instruction(middle),
        CircuitItem::Instruction(second_tick),
        CircuitItem::Instruction(last),
        CircuitItem::Instruction(first_detector),
        CircuitItem::Instruction(second_detector),
    ] = circuit.items()
    else {
        return Ok(None);
    };

    if reset.gate().canonical_name() != "R"
        || first_tick.gate().canonical_name() != "TICK"
        || middle.gate().canonical_name() != "M"
        || second_tick.gate().canonical_name() != "TICK"
        || last.gate().canonical_name() != "M"
        || first_detector.gate().canonical_name() != "DETECTOR"
        || second_detector.gate().canonical_name() != "DETECTOR"
    {
        return Ok(None);
    }

    Ok(Some(build_selected_m_det_inverse(
        reset,
        first_tick,
        middle,
        second_tick,
        last,
        first_detector,
        second_detector,
    )?))
}

fn build_selected_m_det_inverse(
    reset: &CircuitInstruction,
    first_tick: &CircuitInstruction,
    middle: &CircuitInstruction,
    second_tick: &CircuitInstruction,
    last: &CircuitInstruction,
    first_detector: &CircuitInstruction,
    second_detector: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    if !reset.args().is_empty() || !middle.args().is_empty() || !last.args().is_empty() {
        return Err(inverse_qec_m_det_error(
            "reset and measurement instructions must be noiseless",
        ));
    }
    if reset.tag().is_some() || middle.tag().is_some() || last.tag().is_some() {
        return Err(inverse_qec_m_det_error(
            "reset and measurement instruction tags are outside this selected packet",
        ));
    }
    if !first_tick.args().is_empty()
        || !first_tick.targets().is_empty()
        || !second_tick.args().is_empty()
        || !second_tick.targets().is_empty()
    {
        return Err(inverse_qec_m_det_error(
            "TICK instructions must not have arguments or targets",
        ));
    }

    let reset_targets = exact_m_det_targets(reset)?;
    let middle_targets = exact_m_det_targets(middle)?;
    let last_targets = exact_m_det_targets(last)?;
    if reset_targets != middle_targets || reset_targets != last_targets {
        return Err(inverse_qec_m_det_error(
            "reset and measurement targets must match exactly",
        ));
    }

    let first_detector_offsets = super::detector_offsets(first_detector, inverse_qec_m_det_error)?;
    if first_detector_offsets.as_slice() != [-1] {
        return Err(inverse_qec_m_det_error(
            "first detector must reference exactly rec[-1]",
        ));
    }
    let second_detector_offsets =
        super::detector_offsets(second_detector, inverse_qec_m_det_error)?;
    if second_detector_offsets.as_slice() != [-2] {
        return Err(inverse_qec_m_det_error(
            "second detector must reference exactly rec[-2]",
        ));
    }

    let [target0, target1, target2] = reset_targets;
    let reversed_targets = vec![target2.clone(), target1.clone(), target0.clone()];

    let mut result = Circuit::new();
    super::append_target_instruction(
        &mut result,
        reset.gate(),
        reset.args(),
        vec![target2.clone(), target1.clone()],
        reset.tag(),
    )?;
    super::append_one_target_instruction(
        &mut result,
        middle.gate(),
        middle.args(),
        target0,
        middle.tag(),
    )?;
    result.append_instruction(first_tick.clone());
    super::append_target_instruction(
        &mut result,
        last.gate(),
        last.args(),
        reversed_targets.clone(),
        last.tag(),
    )?;
    result.append_instruction(second_tick.clone());
    super::append_target_instruction(
        &mut result,
        middle.gate(),
        middle.args(),
        reversed_targets,
        middle.tag(),
    )?;
    super::append_target_instruction(
        &mut result,
        first_detector.gate(),
        first_detector.args(),
        vec![Target::measurement_record(MeasureRecordOffset::try_new(
            -3,
        )?)],
        first_detector.tag(),
    )?;
    super::append_target_instruction(
        &mut result,
        second_detector.gate(),
        second_detector.args(),
        vec![Target::measurement_record(MeasureRecordOffset::try_new(
            -2,
        )?)],
        second_detector.tag(),
    )?;
    Ok(result)
}

fn exact_m_det_targets(instruction: &CircuitInstruction) -> CircuitResult<[Target; 3]> {
    let targets = super::plain_unique_single_qubit_targets(instruction)
        .ok_or_else(|| inverse_qec_m_det_error("targets must be plain unique qubits"))?;
    let [target0, target1, target2] = targets.as_slice() else {
        return Err(inverse_qec_m_det_error("target list must be exactly 0 1 2"));
    };
    let ids = [
        target0.qubit_id().map(|id| id.get()),
        target1.qubit_id().map(|id| id.get()),
        target2.qubit_id().map(|id| id.get()),
    ];
    if ids != [Some(0), Some(1), Some(2)] {
        return Err(inverse_qec_m_det_error("target list must be exactly 0 1 2"));
    }
    Ok([target0.clone(), target1.clone(), target2.clone()])
}

fn inverse_qec_m_det_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected m_det subset requires exact top-level R 0 1 2, TICK, M 0 1 2, TICK, M 0 1 2, DETECTOR rec[-1], and DETECTOR rec[-2]; {reason}"
    ))
}
