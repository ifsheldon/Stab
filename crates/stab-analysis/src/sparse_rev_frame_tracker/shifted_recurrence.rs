use crate::{AnalysisError, AnalysisResult};

use super::SparseReverseFrameTracker;

#[derive(Clone, Debug)]
pub(crate) struct ShiftedRecurrence {
    pub(crate) cycle_start_state: SparseReverseFrameTracker,
    pub(crate) cycle_end_state: SparseReverseFrameTracker,
    pub(crate) transient_iterations: u64,
    pub(crate) cycle_end_iterations: u64,
    pub(crate) period: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum ShiftedRecurrenceSearch {
    Found {
        recurrence: ShiftedRecurrence,
    },
    Exhausted {
        state: SparseReverseFrameTracker,
        iterations: u64,
    },
}

pub(crate) fn search_shifted_recurrence<F>(
    initial: &SparseReverseFrameTracker,
    max_iterations: u64,
    mut step: F,
) -> AnalysisResult<ShiftedRecurrenceSearch>
where
    F: FnMut(&mut SparseReverseFrameTracker) -> AnalysisResult<()>,
{
    let mut tortoise = initial.clone();
    let mut hare = initial.clone();
    let mut hare_iterations = 0_u64;
    let mut tortoise_iterations = 0_u64;
    while hare_iterations < max_iterations {
        step(&mut hare)?;
        hare_iterations = hare_iterations.checked_add(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "shifted recurrence probe step count overflowed",
            )
        })?;
        if hare.is_shifted_copy(&tortoise) {
            return found(tortoise, hare, tortoise_iterations, hare_iterations);
        }

        if hare_iterations.is_multiple_of(2) {
            step(&mut tortoise)?;
            tortoise_iterations = tortoise_iterations.checked_add(1).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "shifted recurrence tortoise step count overflowed",
                )
            })?;
            if hare.is_shifted_copy(&tortoise) {
                return found(tortoise, hare, tortoise_iterations, hare_iterations);
            }
        }
    }

    Ok(ShiftedRecurrenceSearch::Exhausted {
        state: hare,
        iterations: hare_iterations,
    })
}

fn found(
    cycle_start_state: SparseReverseFrameTracker,
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
        recurrence: ShiftedRecurrence {
            cycle_start_state,
            cycle_end_state,
            transient_iterations,
            cycle_end_iterations,
            period,
        },
    })
}
