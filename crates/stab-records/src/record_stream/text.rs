//! Line-framed text decoding (`01`, HITS, and DETS) for [`RecordStreamReader`].

use std::io::Read;

use super::{
    HitsDuplicates, RecordFraming, RecordStreamReadError, RecordStreamReader, StreamDecoder,
    decode, fast_text, internal_decoder_mismatch,
};
use crate::{DetsLayout, FormatError, PackedShotBatch, RecordFormat, RecordResult};

impl<R: Read> RecordStreamReader<R> {
    /// Whether skipped records are line-framed `01` text, whose canonical lines are checked
    /// without decoding them.
    pub(super) fn skips_canonical_zero_one_lines(&self) -> bool {
        #[cfg(test)]
        if self.options.grammar_only {
            return false;
        }
        self.options.framing == RecordFraming::Records
            && matches!(
                self.decoder,
                StreamDecoder::Text {
                    format: RecordFormat::ZeroOne,
                    ..
                }
            )
    }

    /// Consumes up to `max_records` wholly buffered canonical `01` lines within the record byte
    /// limit, which the full grammar would accept, and returns how many it consumed. It stops,
    /// without reading, at the first line it cannot prove from buffered bytes alone.
    pub(super) fn skip_canonical_zero_one_lines(
        &mut self,
        max_records: usize,
    ) -> Result<usize, RecordStreamReadError> {
        let width = self.bits_per_record;
        let mut skipped = 0_usize;
        while skipped < max_records {
            let Some(line_len) = decode::canonical_zero_one_line_len(self.unconsumed(), width)
                .filter(|line_len| *line_len <= self.max_text_record_bytes)
            else {
                break;
            };
            self.consume(line_len)?;
            skipped += 1;
        }
        Ok(skipped)
    }

    /// Frames the next line (through its newline, or the unterminated tail at end of input) and
    /// returns its length without consuming it, or `None` at end of input.
    fn frame_text_line(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
        let mut scanned = 0_usize;
        loop {
            let unscanned = self.unconsumed().get(scanned..).unwrap_or_default();
            if let Some(position) = unscanned.iter().position(|byte| *byte == b'\n') {
                let line_len = scanned.saturating_add(position).saturating_add(1);
                if line_len > self.max_text_record_bytes {
                    return Err(self.text_record_limit_error());
                }
                return Ok(Some(line_len));
            }
            scanned = self.available();
            if scanned > self.max_text_record_bytes {
                return Err(self.text_record_limit_error());
            }
            if self.eof {
                return Ok((scanned > 0).then_some(scanned));
            }
            self.refill()?;
        }
    }

    fn text_record_limit_error(&self) -> RecordStreamReadError {
        match self.options.framing {
            RecordFraming::Stream => FormatError::invalid_data(format!(
                "text record exceeded the {}-byte streaming record limit before its newline",
                self.max_text_record_bytes
            ))
            .into(),
            RecordFraming::Records => RecordStreamReadError::RecordTooLarge {
                limit: self.max_text_record_bytes,
            },
        }
    }

    fn text_decoder(&self) -> RecordResult<(RecordFormat, DetsLayout)> {
        match &self.decoder {
            StreamDecoder::Text { format, layout } => Ok((*format, *layout)),
            _ => Err(internal_decoder_mismatch()),
        }
    }

    /// Decodes one framed text line into batch row `row`, returning whether it held a record.
    fn decode_text_line(
        &mut self,
        line_len: usize,
        row: usize,
        capacity: usize,
    ) -> Result<RecordResult<bool>, RecordStreamReadError> {
        let (format, layout) = self.text_decoder()?;
        let duplicates = self.options.hits_duplicates;
        if self.batch.is_none() {
            // Prove the first record against its grammar before allocating storage proportional
            // to the declared width.
            let line = self.unconsumed().get(..line_len).ok_or_else(|| {
                FormatError::invalid_data("text record frame escaped the stream buffer")
            })?;
            match decode::text_line(line, format, layout, duplicates, None) {
                Ok(true) => {}
                other => return Ok(other),
            }
        }
        self.ensure_batch(capacity)?;
        let line = self
            .buffer
            .get(self.start..self.start.saturating_add(line_len))
            .ok_or_else(|| {
                FormatError::invalid_data("text record frame escaped the stream buffer")
            })?;
        let batch = self
            .batch
            .as_mut()
            .ok_or_else(|| FormatError::invalid_data("text record lost its reusable batch"))?;
        let mut words = batch.shot_words_mut(row)?;
        Ok(decode::text_line(
            line,
            format,
            layout,
            duplicates,
            Some(&mut words),
        ))
    }

    /// Decodes a canonical HITS or DETS line at the record boundary in one pass, framing and
    /// parsing together, once the first record has proven its frame and allocated the batch.
    ///
    /// A line that outruns the buffered bytes resumes after each refill where it stopped. While
    /// no newline is buffered this refills exactly when line framing would, so transport
    /// failures surface identically. Returns `false`, leaving the row unspecified and nothing
    /// consumed, whenever the full grammar must judge the line (including at end of input and at
    /// the record byte limit); the caller then frames and decodes it normally, which overwrites
    /// the row and owns every diagnostic.
    fn fast_text_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        #[cfg(test)]
        if self.options.grammar_only {
            return Ok(false);
        }
        if self.batch.is_none() {
            return Ok(false);
        }
        self.ensure_batch(capacity)?;
        let mut resume = None;
        loop {
            let decoded = fast_text_line(
                &self.decoder,
                self.options.hits_duplicates,
                self.batch.as_mut(),
                self.buffer.get(self.start..self.end).unwrap_or_default(),
                self.max_text_record_bytes,
                row,
                resume,
            )?;
            match decoded {
                fast_text::FastLine::Record(line_len) => {
                    self.consume(line_len)?;
                    return Ok(true);
                }
                fast_text::FastLine::Incomplete(offset)
                    if !self.eof && self.available() < self.max_text_record_bytes =>
                {
                    self.refill()?;
                    resume = Some(offset);
                }
                fast_text::FastLine::Incomplete(_) | fast_text::FastLine::Declined => {
                    return Ok(false);
                }
            }
        }
    }

    pub(super) fn next_stream_text_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        if self.fast_text_record(row, capacity)? {
            return Ok(true);
        }
        loop {
            let Some(line_len) = self.frame_text_line()? else {
                return Ok(false);
            };
            let record_offset = self.record_offset().ok_or_else(|| {
                FormatError::invalid_data("streamed record byte offset overflowed")
            })?;
            let produced = self
                .decode_text_line(line_len, row, capacity)?
                .map_err(|error| error.with_span_offset(record_offset))?;
            self.consume(line_len)?;
            if produced {
                return Ok(true);
            }
            // A whitespace-only DETS line frames zero records; keep scanning.
        }
    }

    pub(super) fn next_line_text_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        if self.fast_text_record(row, capacity)? {
            self.blank_line_bytes = 0;
            return Ok(true);
        }
        loop {
            let Some(line_len) = self.frame_text_line()? else {
                return Ok(false);
            };
            let byte_offset = self
                .record_offset()
                .ok_or(RecordStreamReadError::ByteOffsetOverflow)?;
            let (format, _) = self.text_decoder()?;
            let terminated = self
                .unconsumed()
                .get(..line_len)
                .is_some_and(|line| line.ends_with(b"\n"));
            let decoded = self.decode_text_line(line_len, row, capacity)?;
            if !terminated && matches!(format, RecordFormat::ZeroOne | RecordFormat::Hits) {
                let source = decoded.err().ok_or_else(|| {
                    FormatError::invalid_data(
                        "an unterminated text record unexpectedly satisfied its grammar",
                    )
                })?;
                return Err(RecordStreamReadError::UnterminatedRecord {
                    byte_offset,
                    line_len,
                    source,
                });
            }
            let produced = decoded.map_err(|source| RecordStreamReadError::Record {
                byte_offset,
                source,
            })?;
            self.consume(line_len)?;
            if produced {
                self.blank_line_bytes = 0;
                return Ok(true);
            }
            if self.options.blank_dets_line_budget {
                self.blank_line_bytes = self.blank_line_bytes.saturating_add(line_len);
                if self.blank_line_bytes > self.max_text_record_bytes {
                    return Err(RecordStreamReadError::RecordTooLarge {
                        limit: self.max_text_record_bytes,
                    });
                }
            }
        }
    }
}

/// Runs the HITS or DETS single-pass decoder over `input` into batch row `row`, resuming at
/// `resume` when set; every other format, and a missing batch, declines.
fn fast_text_line(
    decoder: &StreamDecoder,
    duplicates: HitsDuplicates,
    batch: Option<&mut PackedShotBatch>,
    input: &[u8],
    max_line_bytes: usize,
    row: usize,
    resume: Option<usize>,
) -> RecordResult<fast_text::FastLine> {
    let (StreamDecoder::Text { format, layout }, Some(batch)) = (decoder, batch) else {
        return Ok(fast_text::FastLine::Declined);
    };
    let mut words = batch.shot_words_mut(row)?;
    Ok(match format {
        RecordFormat::Hits => fast_text::hits_line(
            input,
            layout.total_bits(),
            duplicates,
            max_line_bytes,
            &mut words,
            resume,
        ),
        RecordFormat::Dets => {
            fast_text::dets_line(input, *layout, max_line_bytes, &mut words, resume)
        }
        RecordFormat::ZeroOne | RecordFormat::B8 | RecordFormat::R8 | RecordFormat::Ptb64 => {
            fast_text::FastLine::Declined
        }
    })
}
