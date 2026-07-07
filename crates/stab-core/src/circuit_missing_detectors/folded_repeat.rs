use crate::{Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, RepeatBlock};

use super::{
    MAX_MISSING_DETECTOR_EXPANDED_WORK_UNITS, MAX_MISSING_DETECTOR_REPEAT_ITERATIONS,
    MissingDetectorFinder, MissingDetectorOptions, expanded_circuit_work_units,
    validate_repeat_budget,
};

pub(super) fn try_missing_detectors_folded_final_repeat(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> CircuitResult<Option<Circuit>> {
    let Some((prefix, repeat)) = final_repeat_with_prefix(circuit) else {
        return Ok(None);
    };

    validate_repeat_budget(&prefix)?;
    validate_repeat_budget(repeat.body())?;

    if !repeat_body_record_targets_are_local(repeat.body())? {
        return Ok(None);
    }
    if !repeat_exceeds_materialized_budget(repeat)? {
        return Ok(None);
    }

    let mut finder = MissingDetectorFinder::new(circuit.count_qubits(), options)?;
    if finder.process_circuit(&prefix).is_err() {
        return Ok(None);
    }
    if !matches!(finder.build_output(), Ok(output) if output.is_empty()) {
        return Ok(None);
    }

    let tracker_before_repeat = finder.tracker.clone();
    if finder.process_circuit(repeat.body()).is_err() {
        return Ok(None);
    }
    if finder.tracker != tracker_before_repeat {
        return Ok(None);
    }
    if !matches!(finder.build_output(), Ok(output) if output.is_empty()) {
        return Ok(None);
    }

    Ok(Some(Circuit::new()))
}

fn final_repeat_with_prefix(circuit: &Circuit) -> Option<(Circuit, &RepeatBlock)> {
    let (last, prefix_items) = circuit.items().split_last()?;
    let CircuitItem::RepeatBlock(repeat) = last else {
        return None;
    };
    Some((Circuit::from_unfused_items(prefix_items.to_vec()), repeat))
}

fn repeat_exceeds_materialized_budget(repeat: &RepeatBlock) -> CircuitResult<bool> {
    let repeat_count = repeat.repeat_count().get();
    if repeat_count > MAX_MISSING_DETECTOR_REPEAT_ITERATIONS {
        return Ok(true);
    }
    let body_work_units = expanded_circuit_work_units(repeat.body())?;
    let expanded_work_units = body_work_units.checked_mul(repeat_count).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "missing-detector repeat work-unit expansion count overflowed",
        )
    })?;
    Ok(expanded_work_units > MAX_MISSING_DETECTOR_EXPANDED_WORK_UNITS)
}

fn repeat_body_record_targets_are_local(circuit: &Circuit) -> CircuitResult<bool> {
    let mut measurements_seen = 0_i64;
    circuit_record_targets_are_local(circuit, &mut measurements_seen)
}

fn circuit_record_targets_are_local(
    circuit: &Circuit,
    measurements_seen: &mut i64,
) -> CircuitResult<bool> {
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
                let repeat_count = repeat.repeat_count().get();
                let Ok(repeat_count) = i64::try_from(repeat_count) else {
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
        measurements_seen
            .checked_add(i64::from(offset.get()))
            .is_some_and(|absolute_index| absolute_index >= 0 && absolute_index < measurements_seen)
    })
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
