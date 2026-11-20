//! 64-record bit-plane group (PTB64) decoding for [`RecordStreamReader`].

use std::io::Read;

use super::{
    RecordFraming, RecordStreamReadError, RecordStreamReader, StreamDecoder, decode,
    internal_decoder_mismatch,
};
#[cfg(doc)]
use crate::RecordFormat;
use crate::{BitPlaneBatchView, FormatError};

impl<R: Read> RecordStreamReader<R> {
    /// Decodes the next 64-record [`RecordFormat::Ptb64`] group directly into bit planes without
    /// transposing it into shot-major rows.
    ///
    /// Returns `None` at a clean end of input. Requires a reader configured for
    /// [`RecordFormat::Ptb64`] positioned at a 64-record group boundary; the returned view borrows
    /// the reader and is valid until the next call.
    pub fn read_ptb64_group(
        &mut self,
    ) -> Result<Option<BitPlaneBatchView<'_>>, RecordStreamReadError> {
        if let Some(error) = self.pending_error.take() {
            return Err(error);
        }
        match &self.decoder {
            StreamDecoder::Ptb64(group) if group.rows.is_some() && group.served < 64 => {
                return Err(FormatError::invalid_data(
                    "ptb64 group read requires a 64-record group boundary",
                )
                .into());
            }
            StreamDecoder::Ptb64(_) => {}
            _ => {
                return Err(FormatError::invalid_data(
                    "ptb64 group read requires RecordFormat::Ptb64",
                )
                .into());
            }
        }
        let Some(group_bytes) = self.frame_ptb64_group()? else {
            return Ok(None);
        };
        let width = self.bits_per_record;
        let frame = self
            .buffer
            .get(self.start..self.start.saturating_add(group_bytes))
            .ok_or_else(|| {
                FormatError::invalid_data("ptb64 group frame escaped the stream buffer")
            })?;
        let StreamDecoder::Ptb64(group) = &mut self.decoder else {
            return Err(internal_decoder_mismatch().into());
        };
        decode::ptb64_plane_words(frame, width, &mut group.plane_words)?;
        group.served = 64;
        self.consume(group_bytes)?;
        let StreamDecoder::Ptb64(group) = &self.decoder else {
            return Err(internal_decoder_mismatch().into());
        };
        Ok(Some(BitPlaneBatchView::try_from_words(
            &group.plane_words,
            1,
            64,
            width,
        )?))
    }

    pub(super) fn frame_ptb64_group(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
        let width = self.bits_per_record;
        if width == 0 {
            return Err(match self.options.framing {
                RecordFraming::Stream => decode::ptb64_zero_width_error().into(),
                RecordFraming::Records => RecordStreamReadError::ZeroWidthRecords,
            });
        }
        let group_bytes = decode::ptb64_group_bytes(width)?;
        self.fill_at_least(group_bytes)?;
        if self.available() == 0 {
            return Ok(None);
        }
        if self.available() < group_bytes {
            return Err(decode::ptb64_truncated_error(self.bytes_read(), group_bytes).into());
        }
        Ok(Some(group_bytes))
    }

    pub(super) fn next_ptb64_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        let width = self.bits_per_record;
        let needs_group = match &self.decoder {
            StreamDecoder::Ptb64(group) => group.rows.is_none() || group.served >= 64,
            _ => return Err(internal_decoder_mismatch().into()),
        };
        if needs_group {
            let Some(group_bytes) = self.frame_ptb64_group()? else {
                return Ok(false);
            };
            let frame = self
                .buffer
                .get(self.start..self.start.saturating_add(group_bytes))
                .ok_or_else(|| {
                    FormatError::invalid_data("ptb64 group frame escaped the stream buffer")
                })?;
            let StreamDecoder::Ptb64(group) = &mut self.decoder else {
                return Err(internal_decoder_mismatch().into());
            };
            decode::ptb64_group(frame, width, group)?;
            self.consume(group_bytes)?;
        }
        self.ensure_batch(capacity)?;
        let StreamDecoder::Ptb64(group) = &mut self.decoder else {
            return Err(internal_decoder_mismatch().into());
        };
        let rows = group
            .rows
            .as_ref()
            .ok_or_else(|| FormatError::invalid_data("ptb64 group lost its transposed rows"))?;
        let shot = rows.shot(group.served)?;
        let batch = self
            .batch
            .as_mut()
            .ok_or_else(|| FormatError::invalid_data("ptb64 record lost its reusable batch"))?;
        batch.copy_shot_from_bits(row, shot)?;
        group.served += 1;
        Ok(true)
    }
}
