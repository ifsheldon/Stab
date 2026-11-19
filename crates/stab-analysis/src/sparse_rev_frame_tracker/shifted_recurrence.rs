use crate::{AnalysisError, AnalysisResult};

use super::SparseReverseFrameTracker;

#[derive(Clone, Debug)]
pub(crate) struct ShiftedRecurrence {
    pub(crate) cycle_end_state: SparseReverseFrameTracker,
    pub(crate) transient_iterations: u64,
    pub(crate) cycle_end_iterations: u64,
    pub(crate) period: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum ShiftedRecurrenceSearch {
    Found { recurrence: Box<ShiftedRecurrence> },
    Exhausted,
}

/// Searches for a state after which every further `step` reproduces an earlier state shifted
/// by a fixed number of measurements and detectors.
///
/// The search holds two clones of `initial` beside it. When they do not fit the tracker's
/// memory budget the search gives up with [`ShiftedRecurrenceSearch::Exhausted`] after dropping
/// both, so the caller unrolls instead and any genuine shortage resurfaces there with its real
/// numbers. Cancellation and every other error propagate.
pub(crate) fn search_shifted_recurrence<F>(
    initial: &SparseReverseFrameTracker,
    max_iterations: u64,
    step: F,
) -> AnalysisResult<ShiftedRecurrenceSearch>
where
    F: FnMut(&mut SparseReverseFrameTracker) -> AnalysisResult<()>,
{
    match run_search(initial, max_iterations, step) {
        Err(AnalysisError::ResourceLimit(error)) if error.is_memory_budget() => {
            Ok(ShiftedRecurrenceSearch::Exhausted)
        }
        result => result,
    }
}

fn run_search<F>(
    initial: &SparseReverseFrameTracker,
    max_iterations: u64,
    mut step: F,
) -> AnalysisResult<ShiftedRecurrenceSearch>
where
    F: FnMut(&mut SparseReverseFrameTracker) -> AnalysisResult<()>,
{
    initial.check_live_budget_with_extra(initial.live_bytes().saturating_mul(2))?;
    let mut tortoise = initial.clone();
    let mut hare = initial.clone();
    let mut hare_iterations = 0_u64;
    let mut tortoise_iterations = 0_u64;
    while hare_iterations < max_iterations {
        step(&mut hare)?;
        initial.check_live_budget_with_extra(
            tortoise.live_bytes().saturating_add(hare.live_bytes()),
        )?;
        hare_iterations = hare_iterations.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "shifted recurrence probe step count overflowed",
            )
        })?;
        if hare.is_shifted_copy(&tortoise) {
            return found(hare, tortoise_iterations, hare_iterations);
        }

        if hare_iterations.is_multiple_of(2) {
            step(&mut tortoise)?;
            initial.check_live_budget_with_extra(
                tortoise.live_bytes().saturating_add(hare.live_bytes()),
            )?;
            tortoise_iterations = tortoise_iterations.checked_add(1).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "shifted recurrence tortoise step count overflowed",
                )
            })?;
            if hare.is_shifted_copy(&tortoise) {
                return found(hare, tortoise_iterations, hare_iterations);
            }
        }
    }

    Ok(ShiftedRecurrenceSearch::Exhausted)
}

fn found(
    cycle_end_state: SparseReverseFrameTracker,
    transient_iterations: u64,
    cycle_end_iterations: u64,
) -> AnalysisResult<ShiftedRecurrenceSearch> {
    let period = cycle_end_iterations
        .checked_sub(transient_iterations)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("shifted recurrence period underflowed")
        })?;
    if period == 0 {
        return Err(AnalysisError::invalid_detector_error_model(
            "shifted recurrence period was zero",
        ));
    }
    Ok(ShiftedRecurrenceSearch::Found {
        recurrence: Box::new(ShiftedRecurrence {
            cycle_end_state,
            transient_iterations,
            cycle_end_iterations,
            period,
        }),
    })
}
