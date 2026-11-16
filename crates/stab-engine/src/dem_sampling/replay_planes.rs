//! Error-major replay of up to 64 recorded sampled-error records at once.
//!
//! The records of one batch are transposed into one 64-shot word per error, and the compiled
//! program is walked once, XORing each nonzero error word into the words of its targets. The
//! work per batch is proportional to the unrolled errors whose words are nonzero plus a scan of
//! the error words, independent of how many errors each record sets, so it replaces the per-shot
//! locator for records dense enough that per-error toggling would dominate.

use stab_records::PackedShotBatchView;

use super::DemResult;
use super::plan::DemSamplingPlan;
use super::program::{
    DemPoll, DemSampleBlock, DemSampleError, DemSampleOperation, detector_index_overflow_error,
    detector_shift_overflow_error, usize_from_u64,
};

/// Relative cost, in tenths of a nanosecond, of one unrolled error in error-major replay: its
/// share of the record transpose plus the scan or XOR of its word.
const ERROR_MAJOR_COST_PER_ERROR: u128 = 70;
/// Relative cost of one record in per-record replay, independent of its width.
const RECORD_COST_PER_RECORD: u128 = 600;
/// Relative cost of one detector or observable bit of one record in per-record replay, which
/// resets and repacks that bit.
const RECORD_COST_PER_TARGET_BIT: u128 = 10;
/// Relative cost of one 64-bit error-record word of one record in per-record replay.
const RECORD_COST_PER_ERROR_WORD: u128 = 10;
/// Relative cost of one set error in per-record replay, which locates and applies it.
const RECORD_COST_PER_SET_ERROR: u128 = 60;

/// Whether replaying records `first..first + shots` of `records` error-major is expected to
/// cost less than locating each set error of each record. Both paths produce identical records.
///
/// The set-error count is estimated from record `first` alone, so the choice costs one record
/// scan rather than a scan of the whole batch. The constants were measured on one host and only
/// steer speed: a wrong choice costs time, never output.
pub(super) fn prefers_error_major(
    plan: &DemSamplingPlan,
    records: PackedShotBatchView<'_>,
    first: usize,
    shots: usize,
) -> bool {
    let first_record_errors = records.shot(first).map_or(0, |record| {
        let words = record.words();
        let full_words = record.len() / u64::BITS as usize;
        let tail_bits = record.len() % u64::BITS as usize;
        let tail = words.get(full_words).map_or(0, |word| {
            (word & ((1_u64 << tail_bits).wrapping_sub(1))).count_ones()
        });
        words
            .get(..full_words)
            .unwrap_or(words)
            .iter()
            .map(|word| word.count_ones())
            .sum::<u32>()
            .saturating_add(tail)
    });
    let target_bits =
        (plan.detector_count() as u128).saturating_add(plan.observable_count() as u128);
    let error_words = plan.error_count().div_ceil(u64::BITS as usize) as u128;
    let error_major = (plan.error_count() as u128).saturating_mul(ERROR_MAJOR_COST_PER_ERROR);
    let per_record = RECORD_COST_PER_RECORD
        .saturating_add(target_bits.saturating_mul(RECORD_COST_PER_TARGET_BIT))
        .saturating_add(error_words.saturating_mul(RECORD_COST_PER_ERROR_WORD))
        .saturating_add(u128::from(first_record_errors).saturating_mul(RECORD_COST_PER_SET_ERROR))
        .saturating_mul(shots as u128);
    error_major < per_record
}

/// Replays `error_words`, one 64-shot word per unrolled error of `block`, into one word per
/// detector and observable. The target words are fully overwritten.
pub(super) fn replay_error_words(
    block: &DemSampleBlock,
    error_words: &[u64],
    detector_words: &mut [u64],
    observable_words: &mut [u64],
    poll: &mut DemPoll<'_>,
) -> DemResult<()> {
    if error_words.len() != block.error_count {
        return Err(super::DemError::invalid_sampler_compilation(format!(
            "DEM error-major replay received {} error words for {} compiled errors",
            error_words.len(),
            block.error_count
        )));
    }
    detector_words.fill(0);
    observable_words.fill(0);
    let mut targets = Targets {
        detectors: detector_words,
        observables: observable_words,
    };
    replay_block(block, 0, error_words, &mut targets, poll)
}

struct Targets<'a> {
    detectors: &'a mut [u64],
    observables: &'a mut [u64],
}

/// Replays `block`, whose errors own exactly `error_words`, under `detector_shift`.
fn replay_block(
    block: &DemSampleBlock,
    detector_shift: u64,
    error_words: &[u64],
    targets: &mut Targets<'_>,
    poll: &mut DemPoll<'_>,
) -> DemResult<()> {
    let mut rest = error_words;
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                let (word, tail) = rest.split_first().ok_or_else(error_words_escape_error)?;
                rest = tail;
                if *word != 0 {
                    poll.tick()?;
                    apply_error_word(error, detector_shift, *word, targets)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                let body_errors = repeat.body.error_count;
                let repeat_errors = usize_from_u64(repeat.repeat_count, "DEM repeat count")
                    .ok()
                    .and_then(|count| count.checked_mul(body_errors))
                    .ok_or_else(error_words_escape_error)?;
                let (repeat_words, tail) = rest
                    .split_at_checked(repeat_errors)
                    .ok_or_else(error_words_escape_error)?;
                rest = tail;
                // Errors of an iteration are contiguous, so an iteration with no error in any of
                // the replayed shots is skipped by one scan of its words.
                if body_errors == 0 || all_zero(repeat_words) {
                    continue;
                }
                let start_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                if let Some(errors) = flat_errors(&repeat.body) {
                    replay_flat_repeat(
                        &errors,
                        repeat.body.detector_shift,
                        start_shift,
                        repeat_words,
                        targets,
                        poll,
                    )?;
                    continue;
                }
                for (iteration, iteration_words) in
                    (0_u64..).zip(repeat_words.chunks_exact(body_errors))
                {
                    poll.tick()?;
                    if all_zero(iteration_words) {
                        continue;
                    }
                    let shift = iteration
                        .checked_mul(repeat.body.detector_shift)
                        .and_then(|offset| start_shift.checked_add(offset))
                        .ok_or_else(detector_shift_overflow_error)?;
                    replay_block(&repeat.body, shift, iteration_words, targets, poll)?;
                }
            }
        }
    }
    if rest.is_empty() {
        Ok(())
    } else {
        Err(error_words_escape_error())
    }
}

/// The errors of a block made only of errors, in order, or `None` when it holds a repeat.
fn flat_errors(block: &DemSampleBlock) -> Option<Vec<&DemSampleError>> {
    if block.operations.len() != block.error_count {
        return None;
    }
    block
        .operations
        .iter()
        .map(|operation| match operation {
            DemSampleOperation::Error(error) => Some(error),
            DemSampleOperation::Repeat(_) => None,
        })
        .collect()
}

/// Replays every iteration of a repeat whose body is the flat error list `errors`, without
/// descending into the body block once per iteration.
fn replay_flat_repeat(
    errors: &[&DemSampleError],
    body_detector_shift: u64,
    start_shift: u64,
    repeat_words: &[u64],
    targets: &mut Targets<'_>,
    poll: &mut DemPoll<'_>,
) -> DemResult<()> {
    for (iteration, iteration_words) in (0_u64..).zip(repeat_words.chunks_exact(errors.len())) {
        poll.tick()?;
        let mut shift = None;
        for (error, word) in errors.iter().zip(iteration_words) {
            if *word == 0 {
                continue;
            }
            let shift = match shift {
                Some(shift) => shift,
                None => *shift.insert(
                    iteration
                        .checked_mul(body_detector_shift)
                        .and_then(|offset| start_shift.checked_add(offset))
                        .ok_or_else(detector_shift_overflow_error)?,
                ),
            };
            poll.tick()?;
            apply_error_word(error, shift, *word, targets)?;
        }
    }
    Ok(())
}

fn apply_error_word(
    error: &DemSampleError,
    detector_shift: u64,
    word: u64,
    targets: &mut Targets<'_>,
) -> DemResult<()> {
    for detector in &error.detectors {
        let shifted = detector_shift
            .checked_add(*detector)
            .ok_or_else(|| detector_index_overflow_error("detector"))?;
        let index = usize_from_u64(shifted, "detector index")?;
        *targets
            .detectors
            .get_mut(index)
            .ok_or_else(|| target_range_error("detector", index))? ^= word;
    }
    for observable in &error.observables {
        *targets
            .observables
            .get_mut(*observable)
            .ok_or_else(|| target_range_error("observable", *observable))? ^= word;
    }
    Ok(())
}

fn all_zero(words: &[u64]) -> bool {
    words.iter().all(|word| *word == 0)
}

fn target_range_error(kind: &'static str, index: usize) -> super::DemError {
    super::DemError::invalid_sampler_compilation(format!("{kind} index {index} is out of range"))
}

fn error_words_escape_error() -> super::DemError {
    super::DemError::invalid_sampler_compilation(
        "DEM error-major replay walked outside its error words",
    )
}
