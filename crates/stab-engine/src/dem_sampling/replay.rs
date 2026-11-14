//! Replay of recorded sampled errors into detection records.
//!
//! A replayed record is scanned for its set bits only, and each set error ordinal is located in
//! the compiled program through the per-block [`ErrorSpan`] index, so the work per record is
//! proportional to its scanned words and set errors rather than to the unrolled error count.

use stab_bits::BitSlice;

use super::program::{
    DemPoll, DemSampleBlock, DemSampleError, DemSampleOperation, detector_index_overflow_error,
    detector_shift_overflow_error, usize_from_u64,
};
use super::{DemError, DemResult};
use crate::DetectionRecordBuffer;

const RECORD_WORD_BITS: usize = u64::BITS as usize;

/// Scanned record words between cancellation ticks.
const WORDS_PER_TICK: usize = 64;

/// One replayed sampled-error record, scanned for set bits in DEM error order.
pub(super) trait ErrorRecordBits {
    fn bit_len(&self) -> usize;

    /// Calls `visit(word_start, bits)` with the bits of record positions `word_start..` packed
    /// low-to-high for every 64-bit word of the first `limit` bits that may hold a set bit.
    ///
    /// Positions at or beyond `limit` are cleared, words are visited in increasing order, and
    /// `poll` ticks at least once per [`WORDS_PER_TICK`] scanned words.
    fn try_for_each_word(
        &self,
        limit: usize,
        poll: &mut DemPoll<'_>,
        visit: impl FnMut(&mut DemPoll<'_>, usize, u64) -> DemResult<()>,
    ) -> DemResult<()>;

    #[cfg(test)]
    fn bit(&self, index: usize) -> Option<bool>;
}

impl ErrorRecordBits for [bool] {
    fn bit_len(&self) -> usize {
        self.len()
    }

    fn try_for_each_word(
        &self,
        limit: usize,
        poll: &mut DemPoll<'_>,
        mut visit: impl FnMut(&mut DemPoll<'_>, usize, u64) -> DemResult<()>,
    ) -> DemResult<()> {
        let bits = self.get(..limit).ok_or_else(record_scan_error)?;
        for (word_index, word_bits) in bits.chunks(RECORD_WORD_BITS).enumerate() {
            if word_index % WORDS_PER_TICK == 0 {
                poll.tick()?;
            }
            let mut word = 0_u64;
            // Sparse records are mostly false, so whole 8-flag chunks are skipped at once.
            for (chunk_index, chunk) in word_bits.chunks(8).enumerate() {
                if chunk.iter().fold(false, |any, bit| any | *bit) {
                    for (bit_index, bit) in chunk.iter().enumerate() {
                        word |= u64::from(*bit) << (chunk_index * 8 + bit_index);
                    }
                }
            }
            if word != 0 {
                visit(poll, word_index * RECORD_WORD_BITS, word)?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn bit(&self, index: usize) -> Option<bool> {
        self.get(index).copied()
    }
}

impl ErrorRecordBits for BitSlice<'_> {
    fn bit_len(&self) -> usize {
        self.len()
    }

    fn try_for_each_word(
        &self,
        limit: usize,
        poll: &mut DemPoll<'_>,
        mut visit: impl FnMut(&mut DemPoll<'_>, usize, u64) -> DemResult<()>,
    ) -> DemResult<()> {
        if limit > self.len() {
            return Err(record_scan_error());
        }
        let words = self
            .words()
            .get(..limit.div_ceil(RECORD_WORD_BITS))
            .ok_or_else(record_scan_error)?;
        // Storage bits at or beyond the logical length are unspecified and must not replay.
        let partial_word = limit / RECORD_WORD_BITS;
        let partial_mask = (1_u64 << (limit % RECORD_WORD_BITS)).wrapping_sub(1);
        for (chunk_index, chunk) in words.chunks(WORDS_PER_TICK).enumerate() {
            poll.tick()?;
            for (offset, word) in chunk.iter().enumerate() {
                if *word == 0 {
                    continue;
                }
                let word_index = chunk_index * WORDS_PER_TICK + offset;
                let bits = if word_index == partial_word {
                    word & partial_mask
                } else {
                    *word
                };
                visit(poll, word_index * RECORD_WORD_BITS, bits)?;
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn bit(&self, index: usize) -> Option<bool> {
        self.get(index)
    }
}

/// Applies every error set in `error_record` to `record`, visiting only the set bits.
///
/// Returns the number of record bits consumed, which is the block's error count. Errors are
/// applied in increasing ordinal order, so the first failure matches a sequential walk.
pub(super) fn apply_error_record<B: ErrorRecordBits + ?Sized>(
    block: &DemSampleBlock,
    error_record: &B,
    record: &mut DetectionRecordBuffer,
    poll: &mut DemPoll<'_>,
) -> DemResult<usize> {
    let scanned_bits = error_record.bit_len().min(block.error_count);
    let mut pending = PendingErrors::new();
    error_record.try_for_each_word(scanned_bits, poll, |poll, word_start, mut bits| {
        while bits != 0 {
            let ordinal = word_start
                .checked_add(bits.trailing_zeros() as usize)
                .ok_or_else(error_record_cursor_overflow_error)?;
            bits &= bits - 1;
            if pending.push(ordinal)? {
                pending.apply(block, record, poll)?;
            }
        }
        Ok(())
    })?;
    pending.apply(block, record, poll)?;
    if scanned_bits < block.error_count {
        return Err(DemError::invalid_result_format(
            "DEM error record ended early",
        ));
    }
    Ok(block.error_count)
}

/// Set error ordinals awaiting application, in increasing order.
///
/// Locating a batch before applying it lets the independent program and target loads of
/// nearby errors overlap instead of serialising one cache miss per replayed error.
struct PendingErrors {
    ordinals: [usize; Self::CAPACITY],
    len: usize,
}

impl PendingErrors {
    const CAPACITY: usize = 32;

    const fn new() -> Self {
        Self {
            ordinals: [0; Self::CAPACITY],
            len: 0,
        }
    }

    /// Queues `ordinal`, returning whether the batch is now full and must be applied.
    fn push(&mut self, ordinal: usize) -> DemResult<bool> {
        let slot = self
            .ordinals
            .get_mut(self.len)
            .ok_or_else(pending_errors_overflow_error)?;
        *slot = ordinal;
        self.len += 1;
        Ok(self.len == Self::CAPACITY)
    }

    /// Applies and clears the queued errors, reporting the first failure in ordinal order.
    fn apply(
        &mut self,
        block: &DemSampleBlock,
        record: &mut DetectionRecordBuffer,
        poll: &mut DemPoll<'_>,
    ) -> DemResult<()> {
        let ordinals = self
            .ordinals
            .get(..self.len)
            .ok_or_else(pending_errors_overflow_error)?;
        self.len = 0;
        let mut located: [Option<(&DemSampleError, u64)>; Self::CAPACITY] = [None; Self::CAPACITY];
        let mut locate_failure = None;
        for (slot, ordinal) in located.iter_mut().zip(ordinals) {
            match locate_error(block, *ordinal) {
                Ok(error) => *slot = Some(error),
                Err(failure) => {
                    locate_failure = Some(failure);
                    break;
                }
            }
        }
        for (error, detector_shift) in located.into_iter().map_while(|error| error) {
            poll.tick()?;
            apply_error_to_record(error, detector_shift, record)?;
        }
        locate_failure.map_or(Ok(()), Err)
    }
}

/// Finds the error with unrolled ordinal `ordinal` and its detector shift, descending through
/// at most the repeat nesting depth of compiled blocks.
fn locate_error(block: &DemSampleBlock, ordinal: usize) -> DemResult<(&DemSampleError, u64)> {
    let mut block = block;
    let mut ordinal = ordinal;
    let mut detector_shift = 0_u64;
    loop {
        if ordinal >= block.error_count {
            return Err(error_locator_error());
        }
        // Spans are nonempty and strictly increasing, so as many spans as errors means every
        // span holds exactly one error and the ordinal is its index; when every operation is
        // such a span too, the ordinal indexes the operations directly.
        let (operation_index, local) = if block.error_spans.len() == block.error_count {
            if block.operations.len() == block.error_count {
                (ordinal, 0)
            } else {
                let span = block
                    .error_spans
                    .get(ordinal)
                    .ok_or_else(error_locator_error)?;
                (span.operation, 0)
            }
        } else {
            let span_index = block
                .error_spans
                .partition_point(|span| span.first_error <= ordinal)
                .checked_sub(1)
                .ok_or_else(error_locator_error)?;
            let span = block
                .error_spans
                .get(span_index)
                .ok_or_else(error_locator_error)?;
            let local = ordinal
                .checked_sub(span.first_error)
                .ok_or_else(error_locator_error)?;
            (span.operation, local)
        };
        match block.operations.get(operation_index) {
            Some(DemSampleOperation::Error(error)) if local == 0 => {
                return Ok((error, detector_shift));
            }
            Some(DemSampleOperation::Repeat(repeat)) if repeat.body.error_count > 0 => {
                let iteration = u64::try_from(local / repeat.body.error_count)
                    .map_err(|_| error_locator_error())?;
                if iteration >= repeat.repeat_count {
                    return Err(error_locator_error());
                }
                let iteration_shift = iteration
                    .checked_mul(repeat.body.detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                detector_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .and_then(|shift| shift.checked_add(iteration_shift))
                    .ok_or_else(detector_shift_overflow_error)?;
                ordinal = local % repeat.body.error_count;
                block = &repeat.body;
            }
            _ => return Err(error_locator_error()),
        }
    }
}

/// Reference replay: walks every compiled error in order and reads its record bit.
#[cfg(test)]
pub(super) fn apply_error_record_block<B: ErrorRecordBits + ?Sized>(
    block: &DemSampleBlock,
    detector_shift: u64,
    error_record: &B,
    cursor: &mut usize,
    record: &mut DetectionRecordBuffer,
    poll: &mut DemPoll<'_>,
) -> DemResult<()> {
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                poll.tick()?;
                let occurred = error_record.bit(*cursor).ok_or_else(|| {
                    DemError::invalid_result_format("DEM error record ended early")
                })?;
                *cursor = cursor
                    .checked_add(1)
                    .ok_or_else(error_record_cursor_overflow_error)?;
                if occurred {
                    apply_error_to_record(error, detector_shift, record)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                let mut iteration_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    poll.tick()?;
                    apply_error_record_block(
                        &repeat.body,
                        iteration_shift,
                        error_record,
                        cursor,
                        record,
                        poll,
                    )?;
                    iteration_shift = iteration_shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_error_to_record(
    error: &DemSampleError,
    detector_shift: u64,
    record: &mut DetectionRecordBuffer,
) -> DemResult<()> {
    for detector in &error.detectors {
        let shifted = detector_shift
            .checked_add(*detector)
            .ok_or_else(|| detector_index_overflow_error("detector"))?;
        toggle_bit(
            &mut record.detectors,
            usize_from_u64(shifted, "detector index")?,
            "detector",
        )?;
    }
    for observable in &error.observables {
        toggle_bit(&mut record.observables, *observable, "observable")?;
    }
    Ok(())
}

pub(super) fn reset_detection_record(
    record: &mut DetectionRecordBuffer,
    detector_count: usize,
    observable_count: usize,
) {
    record.detectors.clear();
    record.detectors.resize(detector_count, false);
    record.observables.clear();
    record.observables.resize(observable_count, false);
}

fn toggle_bit(bits: &mut [bool], index: usize, kind: &'static str) -> DemResult<()> {
    let bit = bits.get_mut(index).ok_or_else(|| {
        DemError::invalid_sampler_compilation(format!("{kind} index {index} is out of range"))
    })?;
    *bit = !*bit;
    Ok(())
}

fn record_scan_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM error record scan escaped the record")
}

fn error_record_cursor_overflow_error() -> DemError {
    DemError::invalid_result_format("DEM error record cursor overflowed")
}

fn pending_errors_overflow_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM replay queued more errors than its batch holds")
}

fn error_locator_error() -> DemError {
    DemError::invalid_sampler_compilation("DEM error ordinal escaped the compiled error locator")
}
