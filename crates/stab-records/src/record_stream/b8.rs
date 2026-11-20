//! Fixed-width `b8` record decoding for [`RecordStreamReader`].

use std::io::Read;

use super::{RecordFraming, RecordStreamReadError, RecordStreamReader, decode};
use crate::FormatError;

impl<R: Read> RecordStreamReader<R> {
    pub(super) fn frame_b8_record(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
        let record_bytes = match self.options.framing {
            RecordFraming::Stream => decode::b8_record_bytes(self.bits_per_record)?,
            RecordFraming::Records if self.bits_per_record == 0 => {
                return Err(RecordStreamReadError::ZeroWidthRecords);
            }
            RecordFraming::Records => self.bits_per_record.div_ceil(8),
        };
        self.fill_at_least(record_bytes)?;
        let available = self.available();
        if available == 0 {
            return Ok(None);
        }
        if available < record_bytes {
            return Err(match self.options.framing {
                RecordFraming::Stream => {
                    decode::b8_truncated_error(self.bytes_read(), record_bytes).into()
                }
                RecordFraming::Records => RecordStreamReadError::TruncatedRecord {
                    bytes_read: available,
                    record_bytes,
                },
            });
        }
        Ok(Some(record_bytes))
    }

    pub(super) fn next_b8_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        let Some(record_bytes) = self.frame_b8_record()? else {
            return Ok(false);
        };
        self.ensure_batch(capacity)?;
        let frame = self
            .buffer
            .get(self.start..self.start.saturating_add(record_bytes))
            .ok_or_else(|| {
                FormatError::invalid_data("b8 record frame escaped the stream buffer")
            })?;
        let batch = self
            .batch
            .as_mut()
            .ok_or_else(|| FormatError::invalid_data("b8 record lost its reusable batch"))?;
        decode::b8_record(frame, &mut batch.shot_words_mut(row)?);
        self.consume(record_bytes)?;
        Ok(true)
    }
}
