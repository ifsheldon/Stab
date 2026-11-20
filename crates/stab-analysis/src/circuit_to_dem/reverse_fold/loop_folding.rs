//! `--fold_loops`: tortoise-and-hare period search over a repeat body, then one analyzed period
//! emitted as a `repeat` block.

use stab_model::{
    Circuit, DemInstruction, DemItem, DemRepeatCount, DetectorErrorModel, RepeatCount,
};

use super::ReverseFoldAnalyzer;
use crate::sparse_rev_frame_tracker::SparseReverseFrameTracker;
use crate::{AnalysisError, AnalysisResult};

/// Where a repeat body's reverse tracker state recurs, up to a measurement and detector shift.
struct Recurrence {
    /// Body iterations the hare had undone when it matched the analyzer's tracker.
    cycle_end_iterations: u64,
    /// Measurements one period removes from the history.
    measurements_per_period: usize,
    /// Detectors one period removes from the history.
    detectors_per_period: u64,
}

/// How a period search ended. In every case the analyzer itself has undone
/// `tortoise_iterations` body iterations, with their errors recorded.
struct SearchOutcome {
    tortoise_iterations: u64,
    recurrence: Option<Recurrence>,
}

impl<'c> ReverseFoldAnalyzer<'c> {
    pub(super) fn run_loop(
        &mut self,
        body: &'c Circuit,
        repeat_count: RepeatCount,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let iterations = repeat_count.get();
        if iterations == 0 {
            return Ok(());
        }
        if !self.options.fold_loops {
            return self.undo_loop_by_unrolling(body, iterations);
        }

        let SearchOutcome {
            mut tortoise_iterations,
            recurrence,
        } = self.search_recurrence(body, iterations)?;
        if let Some(recurrence) = recurrence
            && recurrence.cycle_end_iterations < iterations
        {
            let period = recurrence
                .cycle_end_iterations
                .checked_sub(tortoise_iterations)
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(
                        "folded analyzer recurrence period underflowed",
                    )
                })?;
            if period == 0 {
                return Err(AnalysisError::invalid_detector_error_model(
                    "folded analyzer recurrence period was zero",
                ));
            }
            let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer repeat remainder underflowed",
                )
            })?;
            let period_repetitions = remaining / period;
            if period_repetitions > 1 {
                self.capture_repeated_period(
                    body,
                    period,
                    period_repetitions,
                    &recurrence,
                    &mut tortoise_iterations,
                    tag,
                )?;
            }
        }

        let remaining = iterations.checked_sub(tortoise_iterations).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "folded analyzer repeat remainder underflowed",
            )
        })?;
        self.undo_loop_by_unrolling(body, remaining)
    }

    /// Pinned Stim's `run_loop` cycle finder: one hare tracker runs ahead undoing body iterations
    /// without recording errors, while the analyzer itself is the tortoise and undoes every
    /// second iteration for real, so no transient iteration is undone twice.
    ///
    /// The hare is charged against the budget beside everything the analyzer holds, and the
    /// analyzer's own steps are charged with the hare beside them. When the hare does not fit, or
    /// fails in any other way, the search stops and the caller unrolls the rest, so a genuine
    /// shortage or error resurfaces from the analyzer itself; cancellation propagates at once.
    fn search_recurrence(
        &mut self,
        body: &'c Circuit,
        iterations: u64,
    ) -> AnalysisResult<SearchOutcome> {
        let mut tortoise_iterations = 0_u64;
        let exhausted = |tortoise_iterations| SearchOutcome {
            tortoise_iterations,
            recurrence: None,
        };
        self.sync_error_budget_base();
        if !hare_step_succeeded(
            self.tracker
                .check_live_budget_with_extra(self.tracker.live_bytes()),
        )? {
            return Ok(exhausted(tortoise_iterations));
        }
        let mut hare = self.tracker.clone();
        let mut hare_iterations = 0_u64;
        while hare_iterations < iterations {
            hare.set_base_bytes(
                self.retained_bytes_beside_tracker()
                    .saturating_add(self.tracker.live_bytes()),
            );
            let step = hare
                .undo_circuit_for_analyzer_probe(body)
                .and_then(|()| hare.check_live_budget());
            if !hare_step_succeeded(step)? {
                return Ok(exhausted(tortoise_iterations));
            }
            hare_iterations = checked_add(hare_iterations, 1, "hare iteration")?;
            if hare.is_shifted_copy(&self.tracker) {
                return found(&hare, &self.tracker, tortoise_iterations, hare_iterations);
            }

            if hare_iterations.is_multiple_of(2) {
                self.undo_circuit_beside(body, &hare)?;
                tortoise_iterations = checked_add(tortoise_iterations, 1, "tortoise iteration")?;
                if hare.is_shifted_copy(&self.tracker) {
                    return found(&hare, &self.tracker, tortoise_iterations, hare_iterations);
                }
            }
        }
        Ok(exhausted(tortoise_iterations))
    }

    /// Undoes one body iteration on the analyzer while `hare` is held beside it.
    fn undo_circuit_beside(
        &mut self,
        body: &'c Circuit,
        hare: &SparseReverseFrameTracker,
    ) -> AnalysisResult<()> {
        let outer = self.held_probe_bytes;
        self.held_probe_bytes = outer.saturating_add(hare.live_bytes());
        let result = self.undo_circuit(body);
        self.held_probe_bytes = outer;
        self.sync_error_budget_base();
        result
    }

    fn capture_repeated_period(
        &mut self,
        body: &'c Circuit,
        period: u64,
        period_repetitions: u64,
        recurrence: &Recurrence,
        tortoise_iterations: &mut u64,
        tag: Option<&[u8]>,
    ) -> AnalysisResult<()> {
        let Recurrence {
            measurements_per_period,
            detectors_per_period,
            ..
        } = *recurrence;

        self.flush()?;
        let outer_reversed = std::mem::take(&mut self.reversed_model);
        let skipped_periods = period_repetitions.checked_sub(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "folded analyzer skipped period count underflowed",
            )
        })?;
        let folded_iterations =
            checked_product_u64(period, skipped_periods, "folded repeat iteration")?;
        let skipped_measurements =
            checked_product_usize(measurements_per_period, skipped_periods, "measurement skip")?;
        let skipped_detectors =
            checked_product_u64(detectors_per_period, skipped_periods, "detector skip")?;
        self.tracker.shift_counts(
            -i128::try_from(skipped_measurements).map_err(|_| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer measurement skip does not fit i128",
                )
            })?,
            -i128::from(skipped_detectors),
        )?;
        *tortoise_iterations = checked_add(
            *tortoise_iterations,
            folded_iterations,
            "tortoise skipped iteration",
        )?;

        for _ in 0..period {
            self.undo_circuit(body)?;
            *tortoise_iterations =
                checked_add(*tortoise_iterations, 1, "captured period iteration")?;
        }
        self.flush()?;
        let mut period_body = std::mem::take(&mut self.reversed_model);
        let nested_shift = period_body.total_detector_shift()?;
        let remaining_shift = detectors_per_period
            .checked_sub(nested_shift)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "folded analyzer nested detector shifts exceed the period delta",
                )
            })?;
        if remaining_shift > 0 {
            prepend_detector_shift(&mut period_body, remaining_shift)?;
        }

        self.reversed_model = outer_reversed;
        self.charge_output(size_of::<DemItem>() as u64)?;
        self.reversed_model.push_repeat_block(
            stab_model::advanced::dem_repeat_block_with_tag_bytes(
                DemRepeatCount::new(period_repetitions),
                period_body,
                tag,
            ),
        );
        Ok(())
    }

    pub(super) fn undo_loop_by_unrolling(
        &mut self,
        body: &'c Circuit,
        iterations: u64,
    ) -> AnalysisResult<()> {
        for _ in 0..iterations {
            self.undo_circuit(body)?;
        }
        Ok(())
    }
}

/// Whether a hare step succeeded; any failure but cancellation abandons the search, as pinned
/// Stim's `run_loop` abandons it when its hare throws.
fn hare_step_succeeded(step: AnalysisResult<()>) -> AnalysisResult<bool> {
    match step {
        Ok(()) => Ok(true),
        Err(error @ AnalysisError::Cancelled(_)) => Err(error),
        Err(_) => Ok(false),
    }
}

fn found(
    hare: &SparseReverseFrameTracker,
    tortoise: &SparseReverseFrameTracker,
    tortoise_iterations: u64,
    cycle_end_iterations: u64,
) -> AnalysisResult<SearchOutcome> {
    let measurements_per_period = tortoise
        .measurement_count()
        .checked_sub(hare.measurement_count())
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "folded analyzer measurement period underflowed",
            )
        })?;
    let detectors_per_period = tortoise
        .detector_count()
        .checked_sub(hare.detector_count())
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "folded analyzer detector period underflowed",
            )
        })?;
    Ok(SearchOutcome {
        tortoise_iterations,
        recurrence: Some(Recurrence {
            cycle_end_iterations,
            measurements_per_period,
            detectors_per_period,
        }),
    })
}

/// Adds `detector_shift` to the body's leading `shift_detectors`, or inserts one.
fn prepend_detector_shift(
    model: &mut DetectorErrorModel,
    detector_shift: u64,
) -> AnalysisResult<()> {
    let items = stab_model::advanced::dem_model_items_mut(model);
    if let Some(DemItem::Instruction(instruction)) = items.first_mut()
        && let Some(existing_shift) = instruction.detector_shift()
    {
        let combined = existing_shift.checked_add(detector_shift).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model("folded analyzer detector shift overflowed")
        })?;
        *instruction = stab_model::advanced::dem_shift_detectors_instruction_with_tag_bytes(
            instruction.args().to_vec(),
            combined,
            instruction.tag_bytes(),
        )?;
        return Ok(());
    }
    items.insert(
        0,
        DemItem::Instruction(DemInstruction::shift_detectors(
            Vec::new(),
            detector_shift,
            None,
        )?),
    );
    Ok(())
}

fn checked_add(left: u64, right: u64, context: &str) -> AnalysisResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_u64(left: u64, right: u64, context: &str) -> AnalysisResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}

fn checked_product_usize(left: usize, right: u64, context: &str) -> AnalysisResult<usize> {
    left.checked_mul(usize::try_from(right).map_err(|_| {
        AnalysisError::invalid_detector_error_model(format!(
            "folded analyzer {context} does not fit usize"
        ))
    })?)
    .ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("folded analyzer {context} overflowed"))
    })
}
