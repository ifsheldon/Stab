use stab_model::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock, advanced::CircuitBuilder};

use super::{
    MissingDetectorOptions, contains_measurement_row_instruction, missing_detectors_bounded,
    terminal_state_signature,
};
use crate::AnalysisResult;

pub(super) fn try_missing_detectors_folded_final_repeat(
    circuit: &Circuit,
    options: MissingDetectorOptions,
    qubit_count: usize,
) -> AnalysisResult<Option<Circuit>> {
    let Some((prefix, repeat)) = final_repeat_with_prefix(circuit) else {
        return Ok(None);
    };
    let Some(proof_body) = repeat_body_proof_circuit(repeat.body())? else {
        return Ok(None);
    };
    if !contains_measurement_row_instruction(&proof_body) || proof_body.count_measurements()? == 0 {
        return Ok(None);
    }

    let prefix_missing = missing_detectors_bounded(&prefix, options, qubit_count)?;
    if !prefix_missing.is_empty() {
        return Ok(None);
    }
    let Some(prefix_state) = terminal_state_signature(&prefix, options, qubit_count)? else {
        return Ok(None);
    };

    let mut one_iteration = prefix;
    one_iteration.append_circuit(&proof_body);
    let iteration_missing = missing_detectors_bounded(&one_iteration, options, qubit_count)?;
    if !iteration_missing.is_empty() {
        return Ok(None);
    }
    let Some(iteration_state) = terminal_state_signature(&one_iteration, options, qubit_count)?
    else {
        return Ok(None);
    };
    if iteration_state != prefix_state {
        return Ok(None);
    }

    Ok(Some(Circuit::new()))
}

fn final_repeat_with_prefix(circuit: &Circuit) -> Option<(Circuit, &RepeatBlock)> {
    let (last, prefix_items) = circuit.items().split_last()?;
    let CircuitItem::RepeatBlock(repeat) = last else {
        return None;
    };
    Some((
        CircuitBuilder::from_unfused_items(prefix_items.to_vec()).finish(),
        repeat,
    ))
}

fn repeat_body_proof_circuit(circuit: &Circuit) -> AnalysisResult<Option<Circuit>> {
    let mut measurements_seen = 0_i64;
    let mut proof_items = Vec::with_capacity(circuit.items().len());
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE" {
                    if !observable_record_targets_are_local(instruction, measurements_seen) {
                        return Ok(None);
                    }
                    continue;
                }
                if !instruction_record_targets_are_local(instruction, measurements_seen) {
                    return Ok(None);
                }
                let Some(produced) = instruction_measurement_result_count(instruction) else {
                    return Ok(None);
                };
                if !add_measurement_count(&mut measurements_seen, produced) {
                    return Ok(None);
                }
                proof_items.push(item.clone());
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body_measurements = 0_i64;
                if !circuit_record_targets_are_local(repeat.body(), &mut body_measurements)? {
                    return Ok(None);
                }
                let Ok(repeat_count) = i64::try_from(repeat.repeat_count().get()) else {
                    return Ok(None);
                };
                let Some(produced) = body_measurements.checked_mul(repeat_count) else {
                    return Ok(None);
                };
                if !add_measurement_count(&mut measurements_seen, produced) {
                    return Ok(None);
                }
                proof_items.push(item.clone());
            }
        }
    }
    Ok(Some(
        CircuitBuilder::from_unfused_items(proof_items).finish(),
    ))
}

fn circuit_record_targets_are_local(
    circuit: &Circuit,
    measurements_seen: &mut i64,
) -> AnalysisResult<bool> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if !instruction_record_targets_are_local(instruction, *measurements_seen) {
                    return Ok(false);
                }
                let Some(produced) = instruction_measurement_result_count(instruction) else {
                    return Ok(false);
                };
                if !add_measurement_count(measurements_seen, produced) {
                    return Ok(false);
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body_measurements = 0_i64;
                if !circuit_record_targets_are_local(repeat.body(), &mut body_measurements)? {
                    return Ok(false);
                }
                let Ok(repeat_count) = i64::try_from(repeat.repeat_count().get()) else {
                    return Ok(false);
                };
                let Some(produced) = body_measurements.checked_mul(repeat_count) else {
                    return Ok(false);
                };
                if !add_measurement_count(measurements_seen, produced) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn instruction_record_targets_are_local(
    instruction: &CircuitInstruction,
    measurements_seen: i64,
) -> bool {
    if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE" {
        return false;
    }
    instruction.targets().iter().all(|target| {
        let Some(offset) = target.measurement_record_offset() else {
            return true;
        };
        measurement_record_offset_is_local(offset.get(), measurements_seen)
    })
}

fn observable_record_targets_are_local(
    instruction: &CircuitInstruction,
    measurements_seen: i64,
) -> bool {
    instruction.targets().iter().all(|target| {
        let Some(offset) = target.measurement_record_offset() else {
            return false;
        };
        measurement_record_offset_is_local(offset.get(), measurements_seen)
    })
}

fn measurement_record_offset_is_local(offset: i32, measurements_seen: i64) -> bool {
    measurements_seen
        .checked_add(i64::from(offset))
        .is_some_and(|absolute_index| absolute_index >= 0 && absolute_index < measurements_seen)
}

fn instruction_measurement_result_count(instruction: &CircuitInstruction) -> Option<i64> {
    if instruction.gate().produces_measurements() {
        i64::try_from(instruction.target_groups().len()).ok()
    } else {
        Some(0)
    }
}

fn add_measurement_count(measurements_seen: &mut i64, produced: i64) -> bool {
    let Some(next) = measurements_seen.checked_add(produced) else {
        return false;
    };
    *measurements_seen = next;
    true
}
