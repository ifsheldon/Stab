use crate::{CircuitError, CircuitResult};

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
        max_boundary_entries: usize,
    },
    Exhausted {
        state: SparseReverseFrameTracker,
        iterations: u64,
        max_boundary_entries: usize,
    },
}

pub(crate) fn search_shifted_recurrence<F>(
    initial: &SparseReverseFrameTracker,
    max_iterations: u64,
    mut step: F,
) -> CircuitResult<ShiftedRecurrenceSearch>
where
    F: FnMut(&mut SparseReverseFrameTracker) -> CircuitResult<()>,
{
    let mut tortoise = initial.clone();
    let mut hare = initial.clone();
    let mut hare_iterations = 0_u64;
    let mut tortoise_iterations = 0_u64;
    let mut max_boundary_entries = initial.boundary_entry_count();

    while hare_iterations < max_iterations {
        step(&mut hare)?;
        max_boundary_entries = max_boundary_entries.max(hare.boundary_entry_count());
        hare_iterations = hare_iterations.checked_add(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "shifted recurrence probe step count overflowed",
            )
        })?;
        if hare.is_shifted_copy(&tortoise) {
            return found(
                tortoise,
                hare,
                tortoise_iterations,
                hare_iterations,
                max_boundary_entries,
            );
        }

        if hare_iterations.is_multiple_of(2) {
            step(&mut tortoise)?;
            max_boundary_entries = max_boundary_entries.max(tortoise.boundary_entry_count());
            tortoise_iterations = tortoise_iterations.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "shifted recurrence tortoise step count overflowed",
                )
            })?;
            if hare.is_shifted_copy(&tortoise) {
                return found(
                    tortoise,
                    hare,
                    tortoise_iterations,
                    hare_iterations,
                    max_boundary_entries,
                );
            }
        }
    }

    Ok(ShiftedRecurrenceSearch::Exhausted {
        state: hare,
        iterations: hare_iterations,
        max_boundary_entries,
    })
}

fn found(
    cycle_start_state: SparseReverseFrameTracker,
    cycle_end_state: SparseReverseFrameTracker,
    transient_iterations: u64,
    cycle_end_iterations: u64,
    max_boundary_entries: usize,
) -> CircuitResult<ShiftedRecurrenceSearch> {
    let period = cycle_end_iterations
        .checked_sub(transient_iterations)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model("shifted recurrence period underflowed")
        })?;
    if period == 0 {
        return Err(CircuitError::invalid_detector_error_model(
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
        max_boundary_entries,
    })
}
