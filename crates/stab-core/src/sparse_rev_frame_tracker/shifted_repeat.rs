use std::collections::{BTreeMap, BTreeSet};

use crate::{Circuit, CircuitError, CircuitResult, DemTarget};

use super::SparseReverseFrameTracker;

pub(super) fn undo_loop(
    tracker: &mut SparseReverseFrameTracker,
    body: &Circuit,
    repetitions: u64,
) -> CircuitResult<()> {
    if repetitions < 5 {
        return undo_loop_by_unrolling(tracker, body, repetitions);
    }

    let mut tortoise = tracker.clone();
    let mut hare_steps = 0_u64;
    let mut tortoise_steps = 0_u64;
    loop {
        tracker.undo_circuit(body)?;
        hare_steps = hare_steps.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse repeat step count overflowed",
            )
        })?;
        if is_shifted_copy(tracker, &tortoise) {
            break;
        }

        if hare_steps > repetitions.saturating_sub(hare_steps) {
            return undo_loop_by_unrolling(tracker, body, repetitions - hare_steps);
        }

        if hare_steps & 1 == 0 {
            tortoise.undo_circuit(body)?;
            tortoise_steps = tortoise_steps.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "sparse reverse repeat step count overflowed",
                )
            })?;
            if is_shifted_copy(tracker, &tortoise) {
                break;
            }
        }
    }

    let period = hare_steps.checked_sub(tortoise_steps).ok_or_else(|| {
        CircuitError::invalid_detector_error_model("sparse reverse repeat period underflowed")
    })?;
    if period == 0 {
        return Err(CircuitError::invalid_detector_error_model(
            "sparse reverse repeat period was zero",
        ));
    }
    let skipped_iterations = (repetitions - hare_steps) / period;
    let measurements_per_period = tortoise
        .measurement_count
        .checked_sub(tracker.measurement_count)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse repeat measurement period underflowed",
            )
        })?;
    let detectors_per_period = tortoise
        .detector_count
        .checked_sub(tracker.detector_count)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse repeat detector period underflowed",
            )
        })?;

    let skipped_measurements =
        signed_product_usize(measurements_per_period, skipped_iterations, "measurement")?;
    let skipped_detectors =
        signed_product_u64(detectors_per_period, skipped_iterations, "detector")?;
    shift(tracker, -skipped_measurements, -skipped_detectors)?;
    hare_steps = hare_steps
        .checked_add(skipped_iterations.checked_mul(period).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse repeat skipped step count overflowed",
            )
        })?)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sparse reverse repeat step count overflowed",
            )
        })?;

    undo_loop_by_unrolling(tracker, body, repetitions - hare_steps)
}

pub(super) fn undo_loop_by_unrolling(
    tracker: &mut SparseReverseFrameTracker,
    body: &Circuit,
    repetitions: u64,
) -> CircuitResult<()> {
    for _ in 0..repetitions {
        tracker.undo_circuit(body)?;
    }
    Ok(())
}

pub(super) fn is_shifted_copy(
    tracker: &SparseReverseFrameTracker,
    other: &SparseReverseFrameTracker,
) -> bool {
    let measurement_offset =
        match offset_between_usize(tracker.measurement_count, other.measurement_count) {
            Ok(offset) => offset,
            Err(_) => return false,
        };
    let detector_offset = i128::from(other.detector_count) - i128::from(tracker.detector_count);
    rec_bits_match_shifted(
        &tracker.rec_bits,
        &other.rec_bits,
        measurement_offset,
        detector_offset,
    ) && tracker.qubit_count == other.qubit_count
        && maps_match_shifted(&tracker.xs, &other.xs, detector_offset)
        && maps_match_shifted(&tracker.zs, &other.zs, detector_offset)
}

pub(super) fn shift(
    tracker: &mut SparseReverseFrameTracker,
    measurement_offset: i128,
    detector_offset: i128,
) -> CircuitResult<()> {
    tracker.measurement_count = add_signed_usize(
        tracker.measurement_count,
        measurement_offset,
        "measurement count",
    )?;
    tracker.detector_count =
        add_signed_u64(tracker.detector_count, detector_offset, "detector count")?;

    let mut shifted_records = BTreeMap::new();
    for (index, targets) in &tracker.rec_bits {
        let shifted_index = add_signed_usize(*index, measurement_offset, "record index")?;
        let shifted_targets = shift_target_set(targets, detector_offset)?;
        if shifted_records
            .insert(shifted_index, shifted_targets)
            .is_some()
        {
            return Err(CircuitError::invalid_detector_error_model(
                "sparse reverse repeat shift merged measurement records unexpectedly",
            ));
        }
    }
    tracker.rec_bits = shifted_records;
    shift_target_sets(&mut tracker.xs, detector_offset)?;
    shift_target_sets(&mut tracker.zs, detector_offset)
}

fn rec_bits_match_shifted(
    unshifted: &BTreeMap<usize, BTreeSet<DemTarget>>,
    expected: &BTreeMap<usize, BTreeSet<DemTarget>>,
    measurement_offset: i128,
    detector_offset: i128,
) -> bool {
    if unshifted.len() != expected.len() {
        return false;
    }
    unshifted.iter().all(|(index, targets)| {
        let Ok(shifted_index) = add_signed_usize(*index, measurement_offset, "record index") else {
            return false;
        };
        expected
            .get(&shifted_index)
            .is_some_and(|expected_targets| {
                target_set_matches_shifted(targets, expected_targets, detector_offset)
            })
    })
}

fn maps_match_shifted(
    unshifted: &BTreeMap<crate::QubitId, BTreeSet<DemTarget>>,
    expected: &BTreeMap<crate::QubitId, BTreeSet<DemTarget>>,
    detector_offset: i128,
) -> bool {
    unshifted.len() == expected.len()
        && unshifted.iter().all(|(qubit, targets)| {
            expected.get(qubit).is_some_and(|expected_targets| {
                target_set_matches_shifted(targets, expected_targets, detector_offset)
            })
        })
}

fn target_set_matches_shifted(
    unshifted: &BTreeSet<DemTarget>,
    expected: &BTreeSet<DemTarget>,
    detector_offset: i128,
) -> bool {
    if unshifted.len() != expected.len() {
        return false;
    }
    unshifted.iter().all(|target| {
        shifted_detector_target(*target, detector_offset)
            .is_ok_and(|shifted| expected.contains(&shifted))
    })
}

fn shift_target_sets(
    sets: &mut BTreeMap<crate::QubitId, BTreeSet<DemTarget>>,
    detector_offset: i128,
) -> CircuitResult<()> {
    for targets in sets.values_mut() {
        *targets = shift_target_set(targets, detector_offset)?;
    }
    Ok(())
}

fn shift_target_set(
    targets: &BTreeSet<DemTarget>,
    detector_offset: i128,
) -> CircuitResult<BTreeSet<DemTarget>> {
    targets
        .iter()
        .map(|target| shifted_detector_target(*target, detector_offset))
        .collect()
}

fn shifted_detector_target(target: DemTarget, detector_offset: i128) -> CircuitResult<DemTarget> {
    match target {
        DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(add_signed_u64(
            detector.get(),
            detector_offset,
            "detector id",
        )?),
        DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
            Ok(target)
        }
    }
}

fn offset_between_usize(left: usize, right: usize) -> CircuitResult<i128> {
    let left = i128::try_from(left).map_err(|_| {
        CircuitError::invalid_detector_error_model("sparse reverse tracker count does not fit i128")
    })?;
    let right = i128::try_from(right).map_err(|_| {
        CircuitError::invalid_detector_error_model("sparse reverse tracker count does not fit i128")
    })?;
    Ok(right - left)
}

fn signed_product_usize(value: usize, multiplier: u64, label: &'static str) -> CircuitResult<i128> {
    let value = i128::try_from(value).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse repeat {label} value does not fit i128"
        ))
    })?;
    let multiplier = i128::from(multiplier);
    value.checked_mul(multiplier).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse repeat skipped {label} count overflowed"
        ))
    })
}

fn signed_product_u64(value: u64, multiplier: u64, label: &'static str) -> CircuitResult<i128> {
    let value = i128::from(value);
    let multiplier = i128::from(multiplier);
    value.checked_mul(multiplier).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse repeat skipped {label} count overflowed"
        ))
    })
}

fn add_signed_usize(value: usize, offset: i128, label: &'static str) -> CircuitResult<usize> {
    let value = i128::try_from(value).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse {label} does not fit i128"
        ))
    })?;
    let shifted = value.checked_add(offset).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("sparse reverse {label} overflowed"))
    })?;
    usize::try_from(shifted).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse {label} shifted out of range"
        ))
    })
}

fn add_signed_u64(value: u64, offset: i128, label: &'static str) -> CircuitResult<u64> {
    let value = i128::from(value);
    let shifted = value.checked_add(offset).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("sparse reverse {label} overflowed"))
    })?;
    u64::try_from(shifted).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "sparse reverse {label} shifted out of range"
        ))
    })
}
