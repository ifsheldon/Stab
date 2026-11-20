//! Incremental run-length `r8` record decoding for [`RecordStreamReader`].

use std::io::Read;

use super::{
    RecordStreamReadError, RecordStreamReader, StreamDecoder, decode, internal_decoder_mismatch,
};
use crate::{FormatError, RecordResult};

impl<R: Read> RecordStreamReader<R> {
    /// Decodes the next `r8` record into batch row `row` as its bytes arrive, so a record of any
    /// length spans refills of the fixed transport buffer without being buffered whole.
    ///
    /// The first record decodes into a row that grows only with its decoded hits, and the batch
    /// is allocated once that record completes. A failure after part of a record was consumed is
    /// reported again by every later read.
    pub(super) fn next_r8_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        let StreamDecoder::R8(state) = &self.decoder else {
            return Err(internal_decoder_mismatch().into());
        };
        if let Some(error) = &state.failed {
            return Err(error.clone().into());
        }
        let record_start = self.record_offset();
        let result = self.decode_r8_record(row, capacity);
        if let Err(error) = &result
            && self.record_offset() != record_start
            && let StreamDecoder::R8(state) = &mut self.decoder
        {
            state.failed = Some(match error {
                RecordStreamReadError::Format(error) => error.clone(),
                _ => FormatError::invalid_data(
                    "r8 record stream cannot resume a record whose read failed partway",
                ),
            });
        }
        result
    }

    fn decode_r8_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        let width = self.bits_per_record;
        self.fill_at_least(1)?;
        if self.available() == 0 {
            return Ok(false);
        }
        let proven = self.batch.is_some();
        if proven {
            // A later read may request more records than the batch holds; regrow it like the
            // other formats before decoding into the requested row.
            self.ensure_batch(capacity)?;
            self.r8_batch_row(row)?.fill(0);
        } else if let StreamDecoder::R8(state) = &mut self.decoder {
            state.first_row = decode::FirstR8Row::default();
        }
        let mut bit_index = 0_usize;
        loop {
            let slice_offset = self.record_offset().ok_or_else(|| {
                FormatError::invalid_data("streamed record byte offset overflowed")
            })?;
            let input = self.buffer.get(self.start..self.end).unwrap_or_default();
            let mut consumed = 0_usize;
            let step = match (&mut self.decoder, self.batch.as_mut()) {
                (StreamDecoder::R8(_), Some(batch)) if proven => decode::r8_row_bytes(
                    input,
                    width,
                    &mut bit_index,
                    &mut consumed,
                    &mut batch.shot_words_mut(row)?,
                ),
                (StreamDecoder::R8(state), _) => decode::r8_first_row_bytes(
                    input,
                    width,
                    &mut bit_index,
                    &mut consumed,
                    &mut state.first_row,
                ),
                _ => return Err(internal_decoder_mismatch().into()),
            };
            let complete = step.map_err(|error| error.with_span_offset(slice_offset))?;
            self.consume(consumed)?;
            if complete {
                break;
            }
            if self.eof {
                let offset = self.record_offset().ok_or_else(|| {
                    FormatError::invalid_data("streamed record byte offset overflowed")
                })?;
                return Err(decode::r8_truncated(offset, bit_index, width).into());
            }
            self.refill()?;
        }
        if !proven {
            self.ensure_batch(capacity)?;
            let StreamDecoder::R8(state) = &mut self.decoder else {
                return Err(internal_decoder_mismatch().into());
            };
            let mut first_row = std::mem::take(&mut state.first_row);
            let mut words = self.r8_batch_row(row)?;
            words.fill(0);
            first_row.take_into(&mut words, width)?;
        }
        Ok(true)
    }

    fn r8_batch_row(&mut self, row: usize) -> RecordResult<stab_bits::BitWordsMut<'_>> {
        self.batch
            .as_mut()
            .ok_or_else(|| FormatError::invalid_data("r8 record lost its reusable batch"))?
            .shot_words_mut(row)
    }
}
