use std::collections::{BTreeMap, BTreeSet};

use stab_model::{Circuit, CircuitItem, DemTarget};

use crate::target_xor_set::TargetXorSet;
use crate::{AnalysisError, AnalysisResult};

use super::unitary_repeat::UnitaryFold;
use super::{GaugeOutputPolicy, SparseReverseFrameTracker, TargetSets};

#[derive(Clone, Copy)]
enum ProbeTarget {
    Hare,
    Tortoise,
}

#[derive(Clone, Copy)]
enum ProbePhase {
    StartHare,
    FinishHare,
    FinishTortoise,
}

struct RecurrenceProbe<'a> {
    body: &'a Circuit,
    repetitions: u64,
    max_iterations: u64,
    tortoise: SparseReverseFrameTracker,
    hare: SparseReverseFrameTracker,
    tortoise_iterations: u64,
    hare_iterations: u64,
    target: ProbeTarget,
    phase: ProbePhase,
}

impl RecurrenceProbe<'_> {
    fn active_mut(&mut self) -> &mut SparseReverseFrameTracker {
        match self.target {
            ProbeTarget::Hare => &mut self.hare,
            ProbeTarget::Tortoise => &mut self.tortoise,
        }
    }
}

enum UndoFrame<'a> {
    Items(std::slice::Iter<'a, CircuitItem>),
    Unroll {
        body: &'a Circuit,
        remaining: u64,
    },
    Probe(Box<RecurrenceProbe<'a>>),
    /// A unitary repeat whose body is being undone once on the fold's basis tracker; the body's
    /// items frame sits directly above it.
    UnitaryFold(Box<UnitaryFold<'a>>),
}

#[cfg(test)]
pub(super) fn undo_loop(
    tracker: &mut SparseReverseFrameTracker,
    body: &Circuit,
    repetitions: u64,
) -> AnalysisResult<()> {
    undo_loop_with_gauge_output(tracker, body, repetitions, GaugeOutputPolicy::Preserve)
}

pub(super) fn undo_circuit_with_gauge_output(
    tracker: &mut SparseReverseFrameTracker,
    circuit: &Circuit,
    gauge_output: GaugeOutputPolicy,
) -> AnalysisResult<()> {
    run_undo_stack(
        tracker,
        vec![UndoFrame::Items(circuit.items().iter())],
        gauge_output,
    )
}

pub(super) fn undo_loop_with_gauge_output(
    tracker: &mut SparseReverseFrameTracker,
    body: &Circuit,
    repetitions: u64,
    gauge_output: GaugeOutputPolicy,
) -> AnalysisResult<()> {
    tracker.meter.sibling_tracker_bytes = 0;
    let initial = prepare_loop_frame(tracker, body, repetitions)?;
    run_undo_stack(tracker, vec![initial], gauge_output)
}

#[cfg(test)]
pub(super) fn undo_unitary_fold(
    tracker: &mut SparseReverseFrameTracker,
    fold: UnitaryFold<'_>,
) -> AnalysisResult<()> {
    let body = fold.body();
    run_undo_stack(
        tracker,
        vec![
            UndoFrame::UnitaryFold(Box::new(fold)),
            UndoFrame::Items(body.items().iter()),
        ],
        GaugeOutputPolicy::Preserve,
    )
}

#[cfg(test)]
pub(super) fn undo_loop_by_unrolling(
    tracker: &mut SparseReverseFrameTracker,
    body: &Circuit,
    repetitions: u64,
) -> AnalysisResult<()> {
    run_undo_stack(
        tracker,
        vec![UndoFrame::Unroll {
            body,
            remaining: repetitions,
        }],
        GaugeOutputPolicy::Preserve,
    )
}

fn prepare_loop_frame<'a>(
    active: &SparseReverseFrameTracker,
    body: &'a Circuit,
    repetitions: u64,
) -> AnalysisResult<UndoFrame<'a>> {
    if repetitions < 5 {
        return Ok(UndoFrame::Unroll {
            body,
            remaining: repetitions,
        });
    }
    active.check_live_budget_with_extra(active.live_bytes().saturating_mul(2))?;
    Ok(UndoFrame::Probe(Box::new(RecurrenceProbe {
        body,
        repetitions,
        max_iterations: repetitions / 2 + 1,
        tortoise: active.clone(),
        hare: active.clone(),
        tortoise_iterations: 0,
        hare_iterations: 0,
        target: ProbeTarget::Hare,
        phase: ProbePhase::StartHare,
    })))
}

fn active_tracker_mut<'t>(
    root: &'t mut SparseReverseFrameTracker,
    stack: &'t mut [UndoFrame<'_>],
) -> &'t mut SparseReverseFrameTracker {
    let total_bytes = stack
        .iter()
        .fold(root.live_bytes(), |acc, frame| match frame {
            UndoFrame::Probe(probe) => acc
                .saturating_add(probe.tortoise.live_bytes())
                .saturating_add(probe.hare.live_bytes()),
            UndoFrame::UnitaryFold(fold) => acc.saturating_add(fold.basis().live_bytes()),
            UndoFrame::Items(_) | UndoFrame::Unroll { .. } => acc,
        });
    let active = stack
        .iter_mut()
        .rev()
        .find_map(|frame| match frame {
            UndoFrame::Probe(probe) => Some(probe.active_mut()),
            UndoFrame::UnitaryFold(fold) => Some(fold.basis_mut()),
            UndoFrame::Items(_) | UndoFrame::Unroll { .. } => None,
        })
        .unwrap_or(root);
    active.meter.sibling_tracker_bytes = total_bytes.saturating_sub(active.live_bytes());
    active
}

fn replace_active_tracker<'t>(
    root: &'t mut SparseReverseFrameTracker,
    stack: &'t mut [UndoFrame<'_>],
    replacement: SparseReverseFrameTracker,
) -> &'t mut SparseReverseFrameTracker {
    let target = active_tracker_mut(root, stack);
    let sibling_bytes = target.meter.sibling_tracker_bytes;
    *target = replacement;
    target.meter.sibling_tracker_bytes = sibling_bytes;
    target
}

fn run_undo_stack<'a>(
    root: &mut SparseReverseFrameTracker,
    mut stack: Vec<UndoFrame<'a>>,
    gauge_output: GaugeOutputPolicy,
) -> AnalysisResult<()> {
    while let Some(top_index) = stack.len().checked_sub(1) {
        let (below, top_slice) = stack.split_at_mut(top_index);
        let Some(top) = top_slice.first_mut() else {
            break;
        };
        match top {
            UndoFrame::Items(items) => {
                let Some(item) = items.next_back() else {
                    stack.pop();
                    continue;
                };
                let active = active_tracker_mut(root, below);
                match item {
                    CircuitItem::Instruction(instruction) => {
                        active.undo_instruction(instruction)?;
                        if gauge_output == GaugeOutputPolicy::Discard {
                            active.take_gauge_errors();
                        }
                    }
                    CircuitItem::RepeatBlock(repeat) => {
                        if gauge_output == GaugeOutputPolicy::Preserve
                            && let Some(fold) = UnitaryFold::start(active, repeat)?
                        {
                            let body = fold.body();
                            stack.push(UndoFrame::UnitaryFold(Box::new(fold)));
                            stack.push(UndoFrame::Items(body.items().iter()));
                            continue;
                        }
                        let next_frame =
                            prepare_loop_frame(active, repeat.body(), repeat.repeat_count().get())?;
                        stack.push(next_frame);
                    }
                }
            }
            UndoFrame::Unroll { body, remaining } => {
                if *remaining == 0 {
                    stack.pop();
                    continue;
                }
                *remaining -= 1;
                let body = *body;
                active_tracker_mut(root, below).poll_cancellation()?;
                stack.push(UndoFrame::Items(body.items().iter()));
            }
            UndoFrame::UnitaryFold(_) => {
                let Some(UndoFrame::UnitaryFold(fold)) = stack.pop() else {
                    return Err(AnalysisError::invalid_detector_error_model(
                        "sparse reverse unitary fold frame disappeared from the undo stack",
                    ));
                };
                fold.finish(active_tracker_mut(root, &mut stack))?;
            }
            UndoFrame::Probe(probe) => match probe.phase {
                ProbePhase::StartHare => {
                    if probe.hare_iterations >= probe.max_iterations {
                        let Some(UndoFrame::Probe(probe)) = stack.pop() else {
                            break;
                        };
                        let probe = *probe;
                        let remainder = probe
                            .repetitions
                            .checked_sub(probe.hare_iterations)
                            .ok_or_else(|| {
                                AnalysisError::invalid_detector_error_model(
                                    "sparse reverse repeat remainder underflowed",
                                )
                            })?;
                        replace_active_tracker(root, &mut stack, probe.hare);
                        stack.push(UndoFrame::Unroll {
                            body: probe.body,
                            remaining: remainder,
                        });
                        continue;
                    }
                    probe.target = ProbeTarget::Hare;
                    probe.phase = ProbePhase::FinishHare;
                    let body = probe.body;
                    stack.push(UndoFrame::Unroll { body, remaining: 1 });
                }
                ProbePhase::FinishHare => {
                    probe.hare_iterations =
                        probe.hare_iterations.checked_add(1).ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "shifted recurrence probe step count overflowed",
                            )
                        })?;
                    if probe.hare.is_shifted_copy(&probe.tortoise) {
                        let Some(UndoFrame::Probe(probe)) = stack.pop() else {
                            break;
                        };
                        let next = finish_found_recurrence(root, &mut stack, *probe)?;
                        stack.push(next);
                        continue;
                    }
                    if probe.hare_iterations.is_multiple_of(2) {
                        probe.target = ProbeTarget::Tortoise;
                        probe.phase = ProbePhase::FinishTortoise;
                        let body = probe.body;
                        stack.push(UndoFrame::Unroll { body, remaining: 1 });
                    } else {
                        probe.phase = ProbePhase::StartHare;
                    }
                }
                ProbePhase::FinishTortoise => {
                    probe.tortoise_iterations =
                        probe.tortoise_iterations.checked_add(1).ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "shifted recurrence tortoise step count overflowed",
                            )
                        })?;
                    if probe.hare.is_shifted_copy(&probe.tortoise) {
                        let Some(UndoFrame::Probe(probe)) = stack.pop() else {
                            break;
                        };
                        let next = finish_found_recurrence(root, &mut stack, *probe)?;
                        stack.push(next);
                    } else {
                        probe.phase = ProbePhase::StartHare;
                    }
                }
            },
        }
    }
    root.meter.sibling_tracker_bytes = 0;
    Ok(())
}

fn finish_found_recurrence<'a>(
    root: &mut SparseReverseFrameTracker,
    stack: &mut [UndoFrame<'_>],
    probe: RecurrenceProbe<'a>,
) -> AnalysisResult<UndoFrame<'a>> {
    let period = probe
        .hare_iterations
        .checked_sub(probe.tortoise_iterations)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("shifted recurrence period underflowed")
        })?;
    if period == 0 {
        return Err(AnalysisError::invalid_detector_error_model(
            "shifted recurrence period was zero",
        ));
    }
    let remaining_after_cycle = probe
        .repetitions
        .checked_sub(probe.hare_iterations)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat cycle end exceeded its repeat count",
            )
        })?;
    let skipped_iterations = remaining_after_cycle / period;
    let measurements_per_period = probe
        .tortoise
        .measurement_count
        .checked_sub(probe.hare.measurement_count)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat measurement period underflowed",
            )
        })?;
    let detectors_per_period = probe
        .tortoise
        .detector_count
        .checked_sub(probe.hare.detector_count)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat detector period underflowed",
            )
        })?;

    let skipped_measurements =
        signed_product_usize(measurements_per_period, skipped_iterations, "measurement")?;
    let skipped_detectors =
        signed_product_u64(detectors_per_period, skipped_iterations, "detector")?;
    let target_tracker = replace_active_tracker(root, stack, probe.hare);
    shift(target_tracker, -skipped_measurements, -skipped_detectors)?;
    let completed_iterations = probe
        .hare_iterations
        .checked_add(skipped_iterations.checked_mul(period).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat skipped step count overflowed",
            )
        })?)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat step count overflowed",
            )
        })?;
    let remainder = probe
        .repetitions
        .checked_sub(completed_iterations)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat remainder underflowed",
            )
        })?;
    Ok(UndoFrame::Unroll {
        body: probe.body,
        remaining: remainder,
    })
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
        && maps_match_shifted(
            &tracker.observable_effects,
            &other.observable_effects,
            detector_offset,
        )
        && target_sets_match_shifted(&tracker.gauge_errors, &other.gauge_errors, detector_offset)
        && anticommutations_match_shifted(
            &tracker.anticommutations,
            &other.anticommutations,
            detector_offset,
        )
        && tracker.anticommutation_policy == other.anticommutation_policy
        && tracker.error_analysis_mode == other.error_analysis_mode
        && tracker.eliminate_detector_gauges == other.eliminate_detector_gauges
}

pub(super) fn shift(
    tracker: &mut SparseReverseFrameTracker,
    measurement_offset: i128,
    detector_offset: i128,
) -> AnalysisResult<()> {
    tracker.measurement_count = add_signed_usize(
        tracker.measurement_count,
        measurement_offset,
        "measurement count",
    )?;
    tracker.detector_count =
        add_signed_u64(tracker.detector_count, detector_offset, "detector count")?;

    let mut shifted_records = BTreeMap::new();
    for (index, mut targets) in std::mem::take(&mut tracker.rec_bits) {
        let shifted_index = add_signed_usize(index, measurement_offset, "record index")?;
        shift_target_set(&mut targets, detector_offset)?;
        if shifted_records.insert(shifted_index, targets).is_some() {
            return Err(AnalysisError::invalid_detector_error_model(
                "sparse reverse repeat shift merged measurement records unexpectedly",
            ));
        }
    }
    tracker.rec_bits = shifted_records;
    shift_target_sets(&mut tracker.xs, detector_offset)?;
    shift_target_sets(&mut tracker.zs, detector_offset)?;
    shift_target_sets(&mut tracker.observable_effects, detector_offset)?;
    for gauge in &mut tracker.gauge_errors {
        shift_target_set(gauge, detector_offset)?;
    }
    tracker.anticommutations = tracker
        .anticommutations
        .iter()
        .map(|anticommutation| {
            Ok(super::Anticommutation {
                target: shifted_detector_target(anticommutation.target, detector_offset)?,
                location: anticommutation.location,
            })
        })
        .collect::<AnalysisResult<_>>()?;
    Ok(())
}

fn rec_bits_match_shifted(
    unshifted: &TargetSets<usize>,
    expected: &TargetSets<usize>,
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

fn maps_match_shifted<K: Ord>(
    unshifted: &TargetSets<K>,
    expected: &TargetSets<K>,
    detector_offset: i128,
) -> bool {
    unshifted.len() == expected.len()
        && unshifted.iter().all(|(qubit, targets)| {
            expected.get(qubit).is_some_and(|expected_targets| {
                target_set_matches_shifted(targets, expected_targets, detector_offset)
            })
        })
}

fn target_sets_match_shifted(
    unshifted: &[TargetXorSet],
    expected: &[TargetXorSet],
    detector_offset: i128,
) -> bool {
    unshifted.len() == expected.len()
        && unshifted
            .iter()
            .zip(expected)
            .all(|(targets, expected_targets)| {
                target_set_matches_shifted(targets, expected_targets, detector_offset)
            })
}

fn anticommutations_match_shifted(
    unshifted: &BTreeSet<super::Anticommutation>,
    expected: &BTreeSet<super::Anticommutation>,
    detector_offset: i128,
) -> bool {
    unshifted.len() == expected.len()
        && unshifted.iter().all(|anticommutation| {
            shifted_detector_target(anticommutation.target, detector_offset).is_ok_and(|target| {
                expected.contains(&super::Anticommutation {
                    target,
                    location: anticommutation.location,
                })
            })
        })
}

/// A uniform detector shift preserves target order, so sorted sets match elementwise.
fn target_set_matches_shifted(
    unshifted: &TargetXorSet,
    expected: &TargetXorSet,
    detector_offset: i128,
) -> bool {
    unshifted.len() == expected.len()
        && unshifted.iter().zip(expected).all(|(target, expected)| {
            shifted_detector_target(*target, detector_offset)
                .is_ok_and(|shifted| shifted == *expected)
        })
}

fn shift_target_sets<K>(sets: &mut TargetSets<K>, detector_offset: i128) -> AnalysisResult<()> {
    for targets in sets.values_mut() {
        shift_target_set(targets, detector_offset)?;
    }
    Ok(())
}

fn shift_target_set(targets: &mut TargetXorSet, detector_offset: i128) -> AnalysisResult<()> {
    targets.try_map_monotone(|target| shifted_detector_target(target, detector_offset))
}

fn shifted_detector_target(target: DemTarget, detector_offset: i128) -> AnalysisResult<DemTarget> {
    match target {
        DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(add_signed_u64(
            detector.get(),
            detector_offset,
            "detector id",
        )?)
        .map_err(Into::into),
        DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
            Ok(target)
        }
    }
}

fn offset_between_usize(left: usize, right: usize) -> AnalysisResult<i128> {
    let left = i128::try_from(left).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "sparse reverse tracker count does not fit i128",
        )
    })?;
    let right = i128::try_from(right).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "sparse reverse tracker count does not fit i128",
        )
    })?;
    Ok(right - left)
}

fn signed_product_usize(
    value: usize,
    multiplier: u64,
    label: &'static str,
) -> AnalysisResult<i128> {
    let value = i128::try_from(value).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse repeat {label} value does not fit i128"
        ))
    })?;
    let multiplier = i128::from(multiplier);
    value.checked_mul(multiplier).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse repeat skipped {label} count overflowed"
        ))
    })
}

fn signed_product_u64(value: u64, multiplier: u64, label: &'static str) -> AnalysisResult<i128> {
    let value = i128::from(value);
    let multiplier = i128::from(multiplier);
    value.checked_mul(multiplier).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse repeat skipped {label} count overflowed"
        ))
    })
}

fn add_signed_usize(value: usize, offset: i128, label: &'static str) -> AnalysisResult<usize> {
    let value = i128::try_from(value).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse {label} does not fit i128"
        ))
    })?;
    let shifted = value.checked_add(offset).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("sparse reverse {label} overflowed"))
    })?;
    usize::try_from(shifted).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse {label} shifted out of range"
        ))
    })
}

fn add_signed_u64(value: u64, offset: i128, label: &'static str) -> AnalysisResult<u64> {
    let value = i128::from(value);
    let shifted = value.checked_add(offset).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("sparse reverse {label} overflowed"))
    })?;
    u64::try_from(shifted).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "sparse reverse {label} shifted out of range"
        ))
    })
}
