use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate,
    MeasureRecordOffset, Target,
};

struct RecordTailOutput {
    gate: Gate,
    args: Vec<f64>,
    targets: Vec<Target>,
    tag: Option<String>,
}

pub(super) fn selected_mpad_record_tail_inverse(
    circuit: &Circuit,
) -> CircuitResult<Option<Circuit>> {
    let [CircuitItem::Instruction(mpad), tail @ ..] = circuit.items() else {
        return Ok(None);
    };

    if mpad.gate().canonical_name() != "MPAD" {
        return Ok(None);
    }

    Ok(Some(build_selected_mpad_record_tail_inverse(mpad, tail)?))
}

fn build_selected_mpad_record_tail_inverse(
    mpad: &CircuitInstruction,
    tail: &[CircuitItem],
) -> CircuitResult<Circuit> {
    validate_mpad_targets(mpad)?;
    let measurement_count = i64::try_from(mpad.targets().len())
        .map_err(|_| inverse_qec_mpad_error("MPAD target count exceeds supported range"))?;

    let mut detector_outputs = Vec::new();
    let mut observable_outputs = BTreeMap::new();
    for item in tail {
        let CircuitItem::Instruction(instruction) = item else {
            return Err(inverse_qec_mpad_error(
                "repeat blocks after MPAD are not supported",
            ));
        };
        match instruction.gate().canonical_name() {
            "DETECTOR" => {
                let targets = remapped_mpad_record_targets(instruction, measurement_count)?;
                if !targets.is_empty() {
                    detector_outputs.push(RecordTailOutput {
                        gate: instruction.gate(),
                        args: instruction.args().to_vec(),
                        targets,
                        tag: instruction.tag().map(str::to_owned),
                    });
                }
            }
            "OBSERVABLE_INCLUDE" => {
                let observable = instruction
                    .observable_id_argument()?
                    .ok_or_else(|| {
                        inverse_qec_mpad_error("OBSERVABLE_INCLUDE is missing an observable id")
                    })?
                    .get();
                if observable_outputs.contains_key(&observable) {
                    return Err(inverse_qec_mpad_error(
                        "duplicate OBSERVABLE_INCLUDE ids after MPAD are not selected",
                    ));
                }
                let targets = remapped_mpad_record_targets(instruction, measurement_count)?;
                if !targets.is_empty() {
                    observable_outputs.insert(
                        observable,
                        RecordTailOutput {
                            gate: instruction.gate(),
                            args: instruction.args().to_vec(),
                            targets,
                            tag: instruction.tag().map(str::to_owned),
                        },
                    );
                }
            }
            _ => {
                return Err(inverse_qec_mpad_error(
                    "only record-only DETECTOR or OBSERVABLE_INCLUDE tails are selected",
                ));
            }
        }
    }

    let mut result = Circuit::new();
    result.append_instruction(CircuitInstruction::new(
        mpad.gate(),
        mpad.args().to_vec(),
        mpad.targets().iter().rev().cloned().collect(),
        mpad.tag().map(str::to_owned),
    )?);
    for output in detector_outputs
        .into_iter()
        .chain(observable_outputs.into_values())
    {
        result.append_instruction(CircuitInstruction::new(
            output.gate,
            output.args,
            output.targets,
            output.tag,
        )?);
    }
    Ok(result)
}

fn validate_mpad_targets(mpad: &CircuitInstruction) -> CircuitResult<()> {
    for target in mpad.targets() {
        let Some(pad) = target.qubit_id() else {
            return Err(inverse_qec_mpad_error(
                "MPAD targets must be deterministic pads 0 or 1",
            ));
        };
        if pad.get() > 1 || target.is_inverted_result_target() {
            return Err(inverse_qec_mpad_error(
                "MPAD targets must be deterministic pads 0 or 1",
            ));
        }
    }
    Ok(())
}

fn remapped_mpad_record_targets(
    instruction: &CircuitInstruction,
    measurement_count: i64,
) -> CircuitResult<Vec<Target>> {
    let mut parity = BTreeSet::new();
    for target in instruction.targets() {
        let offset = target
            .measurement_record_offset()
            .ok_or_else(|| inverse_qec_mpad_error("tail targets must be measurement records"))?
            .get();
        let absolute_index = i64::from(-offset);
        if !(1..=measurement_count).contains(&absolute_index) {
            return Err(inverse_qec_mpad_error(
                "tail measurement record references must point inside the MPAD packet",
            ));
        }
        let remapped = -(measurement_count + 1 - absolute_index);
        let remapped = i32::try_from(remapped)
            .map_err(|_| inverse_qec_mpad_error("remapped record offset is out of range"))?;
        if !parity.insert(remapped) {
            parity.remove(&remapped);
        }
    }

    parity
        .into_iter()
        .map(|offset| MeasureRecordOffset::try_new(offset).map(Target::measurement_record))
        .collect()
}

fn inverse_qec_mpad_error(reason: &str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "inverse_qec selected MPAD record-tail subset requires a top-level MPAD followed only by record-only DETECTOR or unique-id OBSERVABLE_INCLUDE instructions referencing that MPAD packet; {reason}"
    ))
}
