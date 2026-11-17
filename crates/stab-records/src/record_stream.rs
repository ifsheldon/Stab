//! Packed record streaming over byte transports.
//!
//! [`RecordStreamReader`] frames records out of an [`std::io::Read`] transport and decodes them
//! straight into the words of a reusable [`PackedShotBatch`]: sparse formats set only their hit
//! bits, `01` packs eight bytes per step, `b8` copies bytes, and PTB64 transposes whole groups.
//! Grammar diagnostics come from the same component decoders the whole-buffer readers use, so
//! streaming and materialized decoding cannot drift.
//!
//! Input is read through one reusable buffer that grows only while a single record frame spans
//! it, and record storage is allocated fallibly, and only once input has proven that a record
//! frame exists, so a declared width far beyond the available input is reported as a typed error
//! instead of aborting the process.

use std::io::Read;

use thiserror::Error;

use crate::{
    BitPlaneBatchView, DetsLayout, FormatError, FormatErrorCode, PackedShotBatch,
    PackedShotBatchView, RecordFormat, RecordResult,
};

mod decode;
mod fast_text;
mod storage;

pub use storage::RecordStreamStorage;

#[cfg(test)]
mod tests;

/// Initial transport buffer size; the buffer doubles only while one record frame spans it.
const INITIAL_BUFFER_BYTES: usize = 64 * 1024;

/// Records decoded per internal step when a caller skips records without receiving them.
const SKIP_CHUNK_RECORDS: usize = 64;

/// Failure surfaced by [`RecordStreamReader`].
#[derive(Debug, Error)]
pub enum RecordStreamReadError {
    /// The underlying byte transport failed.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The streamed bytes violated the record-format contract; spans are absolute input offsets.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// Record framing: one line-framed text record violated its grammar.
    ///
    /// `source` carries a span relative to the line, which starts at absolute `byte_offset`.
    #[error("{source}")]
    Record {
        byte_offset: usize,
        #[source]
        source: FormatError,
    },
    /// Record framing: an `01` or HITS line reached end of input without its newline.
    ///
    /// `source` is the grammar diagnostic for the unterminated line, relative to the line.
    #[error("text record at byte {byte_offset} ended without a newline")]
    UnterminatedRecord {
        byte_offset: usize,
        line_len: usize,
        #[source]
        source: FormatError,
    },
    /// Record framing: one text line, or a run of blank DETS lines when that budget is enabled,
    /// exceeded the reader's per-record byte limit.
    #[error("text record exceeded the {limit}-byte record limit")]
    RecordTooLarge { limit: usize },
    /// Record framing: the absolute input offset of a text record overflowed.
    #[error("streamed record byte offset overflowed")]
    ByteOffsetOverflow,
    /// Record framing: a `b8` record was cut short by the end of input.
    #[error("input ended after {bytes_read} bytes of a {record_bytes}-byte record")]
    TruncatedRecord {
        bytes_read: usize,
        record_bytes: usize,
    },
    /// Record framing: `b8` and PTB64 cannot frame zero-width records.
    #[error("zero-width records cannot be framed in this format")]
    ZeroWidthRecords,
    /// A zero-width `b8` stream that admits unbounded empty records contained bytes.
    #[error("zero-width record input must be empty")]
    NonEmptyZeroWidthInput,
}

/// How record frames are delimited and how their failures are reported.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RecordFraming {
    /// Records are decoded as one continuous stream, and every grammar or length violation is a
    /// [`RecordStreamReadError::Format`] whose span is an absolute input offset.
    #[default]
    Stream,
    /// Each text line and each `b8` record is framed on its own: text grammar failures report
    /// the line start with a line-relative span, `01` and HITS lines must end with a newline,
    /// and `b8` and PTB64 report truncated records and zero widths structurally.
    Records,
}

/// How a repeated HITS index within one record is applied.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum HitsDuplicates {
    /// Each occurrence flips the bit, so an index listed twice cancels.
    #[default]
    Toggle,
    /// Every occurrence sets the bit.
    Set,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReaderOptions {
    framing: RecordFraming,
    hits_duplicates: HitsDuplicates,
    empty_zero_width_input: bool,
    blank_dets_line_budget: bool,
    /// Test-only reference mode: decode every text line through the full grammar alone.
    #[cfg(test)]
    grammar_only: bool,
}

#[derive(Debug)]
enum StreamDecoder {
    /// Line-framed text formats: `01`, HITS, and DETS.
    Text {
        format: RecordFormat,
        layout: DetsLayout,
    },
    /// Fixed-width byte-packed records.
    B8,
    /// Run-length records with data-dependent frame lengths.
    R8,
    /// 64-record measurement-major bit-plane groups.
    Ptb64(Ptb64Group),
}

/// One decoded PTB64 group, transposed to shot-major rows and served one row at a time.
#[derive(Debug, Default)]
struct Ptb64Group {
    plane_words: Vec<u64>,
    rows: Option<PackedShotBatch>,
    served: usize,
}

/// Decodes records into packed shot-major batches from a byte transport.
///
/// Memory is bounded by one transport buffer that holds the largest record frame seen, one
/// reusable record batch of the largest requested size, and one PTB64 group; it never grows with
/// the number of records.
pub struct RecordStreamReader<R> {
    source: R,
    /// Initialized transport storage; `start..end` holds unconsumed input bytes.
    buffer: Vec<u8>,
    start: usize,
    end: usize,
    /// Absolute input offset of `buffer[0]`.
    absolute_base: usize,
    eof: bool,
    bits_per_record: usize,
    max_text_record_bytes: usize,
    decoder: StreamDecoder,
    options: ReaderOptions,
    batch: Option<PackedShotBatch>,
    /// A failure found after earlier records of the same batch decoded; returned by the next call.
    pending_error: Option<RecordStreamReadError>,
    zero_width_input_checked: bool,
    /// Bytes of consecutive blank DETS lines since the last record, under the blank-line budget.
    blank_line_bytes: usize,
}

impl<R> std::fmt::Debug for RecordStreamReader<R> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RecordStreamReader")
            .field("decoder", &self.decoder)
            .field("options", &self.options)
            .field("bits_per_record", &self.bits_per_record)
            .field("bytes_read", &self.bytes_read())
            .field("eof", &self.eof)
            .finish_non_exhaustive()
    }
}

impl<R> RecordStreamReader<R> {
    /// Total bytes fetched from the transport so far.
    pub fn bytes_read(&self) -> usize {
        self.absolute_base.saturating_add(self.end)
    }

    /// Declared width of every decoded record.
    pub const fn bits_per_record(&self) -> usize {
        self.bits_per_record
    }

    fn available(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    fn unconsumed(&self) -> &[u8] {
        self.buffer.get(self.start..self.end).unwrap_or_default()
    }

    /// Absolute input offset of the next unconsumed byte, where the current record frame starts.
    fn record_offset(&self) -> Option<usize> {
        self.absolute_base.checked_add(self.start)
    }

    fn consume(&mut self, bytes: usize) -> RecordResult<()> {
        let start = self
            .start
            .checked_add(bytes)
            .filter(|start| *start <= self.end)
            .ok_or_else(|| FormatError::invalid_data("record frame escaped the stream buffer"))?;
        self.start = start;
        Ok(())
    }
}

impl<R: Read> RecordStreamReader<R> {
    /// Streams measurement-shaped records in the given per-record sample format.
    ///
    /// [`RecordFormat::Dets`] input uses a measurement-only layout. `max_text_record_bytes`
    /// bounds one text record's framed bytes (including its newline) and is ignored by the packed
    /// formats. Construction reads and allocates nothing.
    pub fn measurements(
        source: R,
        format: RecordFormat,
        bits_per_record: usize,
        max_text_record_bytes: usize,
    ) -> Self {
        let decoder = match format {
            RecordFormat::B8 => StreamDecoder::B8,
            RecordFormat::R8 => StreamDecoder::R8,
            RecordFormat::Ptb64 => StreamDecoder::Ptb64(Ptb64Group::default()),
            RecordFormat::ZeroOne | RecordFormat::Hits | RecordFormat::Dets => {
                StreamDecoder::Text {
                    format,
                    layout: DetsLayout::measurement_only(bits_per_record),
                }
            }
        };
        Self::with_decoder(source, decoder, bits_per_record, max_text_record_bytes)
    }

    /// Streams DETS records resolved against an explicit typed namespace layout.
    pub fn dets(source: R, layout: DetsLayout, max_text_record_bytes: usize) -> Self {
        Self::with_decoder(
            source,
            StreamDecoder::Text {
                format: RecordFormat::Dets,
                layout,
            },
            layout.total_bits(),
            max_text_record_bytes,
        )
    }

    fn with_decoder(
        source: R,
        decoder: StreamDecoder,
        bits_per_record: usize,
        max_text_record_bytes: usize,
    ) -> Self {
        Self {
            source,
            buffer: Vec::new(),
            start: 0,
            end: 0,
            absolute_base: 0,
            eof: false,
            bits_per_record,
            max_text_record_bytes,
            decoder,
            options: ReaderOptions::default(),
            batch: None,
            pending_error: None,
            zero_width_input_checked: false,
            blank_line_bytes: 0,
        }
    }

    /// Selects how record frames are delimited and how their failures are reported.
    #[must_use]
    pub const fn with_framing(mut self, framing: RecordFraming) -> Self {
        self.options.framing = framing;
        self
    }

    /// Selects how a HITS index repeated within one record is applied.
    #[must_use]
    pub const fn with_hits_duplicates(mut self, duplicates: HitsDuplicates) -> Self {
        self.options.hits_duplicates = duplicates;
        self
    }

    /// Lets a zero-width `b8` stream supply any number of empty records, provided its input is
    /// empty; [`Self::has_more_records`] then reports no further records.
    #[must_use]
    pub const fn with_empty_zero_width_input(mut self) -> Self {
        self.options.empty_zero_width_input = true;
        self
    }

    /// Under record framing, charges consecutive blank DETS lines against the per-record byte
    /// limit, so whitespace between records is bounded like a record.
    #[must_use]
    pub const fn with_blank_dets_line_budget(mut self) -> Self {
        self.options.blank_dets_line_budget = true;
        self
    }

    /// Test-only reference mode that decodes every text line through the full grammar alone.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn with_grammar_only(mut self) -> Self {
        self.options.grammar_only = true;
        self
    }

    /// Decodes up to `max_records` records into the reader's reusable batch.
    ///
    /// Returns `None` at a clean end of input. Fewer than `max_records` records are returned
    /// only at end of input or when a later record failed; that failure is returned by the next
    /// call, so every record before a malformed one is delivered first. The batch borrows the
    /// reader and is valid until the next call.
    pub fn read_batch(
        &mut self,
        max_records: usize,
    ) -> Result<Option<PackedShotBatchView<'_>>, RecordStreamReadError> {
        if let Some(error) = self.pending_error.take() {
            return Err(error);
        }
        if max_records == 0 {
            return Err(FormatError::invalid_data(
                "a record batch read requires a positive record count",
            )
            .into());
        }
        let (count, error) = self.fill(max_records);
        match (count, error) {
            (0, Some(error)) => Err(error),
            (0, None) => Ok(None),
            (count, error) => {
                self.pending_error = error;
                let batch = self.batch.as_ref().ok_or_else(|| {
                    FormatError::invalid_data("decoded records lost their reusable batch")
                })?;
                Ok(Some(batch.view_prefix(count)?))
            }
        }
    }

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

    /// Decodes and discards up to `max_records` records, returning how many existed.
    ///
    /// Fewer than `max_records` are returned only at a clean end of input; the first failure is
    /// returned immediately.
    pub fn skip_records(&mut self, max_records: usize) -> Result<usize, RecordStreamReadError> {
        if let Some(error) = self.pending_error.take() {
            return Err(error);
        }
        if max_records == 0 {
            return Ok(0);
        }
        let mut skipped = 0_usize;
        if matches!(self.decoder, StreamDecoder::B8) && !self.admits_unbounded_empty_records() {
            while skipped < max_records {
                let Some(record_bytes) = self.frame_b8_record()? else {
                    return Ok(skipped);
                };
                self.consume(record_bytes)?;
                skipped += 1;
            }
            return Ok(skipped);
        }
        if let StreamDecoder::Ptb64(group) = &mut self.decoder
            && group.rows.is_some()
            && group.served < 64
        {
            let buffered = (64 - group.served).min(max_records);
            group.served += buffered;
            skipped += buffered;
        }
        if matches!(self.decoder, StreamDecoder::Ptb64(_)) {
            while max_records - skipped >= 64 {
                let Some(group_bytes) = self.frame_ptb64_group()? else {
                    return Ok(skipped);
                };
                self.consume(group_bytes)?;
                skipped += 64;
            }
        }
        while skipped < max_records {
            let wanted = (max_records - skipped).min(SKIP_CHUNK_RECORDS);
            let (count, error) = self.fill(wanted);
            if let Some(error) = error {
                return Err(error);
            }
            skipped += count;
            if count < wanted {
                break;
            }
        }
        Ok(skipped)
    }

    /// Consumes the next record and reports whether one existed.
    ///
    /// A zero-width stream that admits unbounded empty records has no records beyond those
    /// already requested; for it this checks that the input is empty and returns `false`.
    pub fn has_more_records(&mut self) -> Result<bool, RecordStreamReadError> {
        if let Some(error) = self.pending_error.take() {
            return Err(error);
        }
        if self.admits_unbounded_empty_records() {
            self.check_empty_zero_width_input()?;
            return Ok(false);
        }
        Ok(self.skip_records(1)? == 1)
    }

    fn admits_unbounded_empty_records(&self) -> bool {
        self.options.empty_zero_width_input
            && self.bits_per_record == 0
            && matches!(self.decoder, StreamDecoder::B8)
    }

    /// Decodes up to `max_records` records into batch rows `0..count`.
    fn fill(&mut self, max_records: usize) -> (usize, Option<RecordStreamReadError>) {
        if self.admits_unbounded_empty_records() {
            let result = self
                .check_empty_zero_width_input()
                .and_then(|()| self.ensure_batch(max_records));
            return match result {
                Ok(()) => (max_records, None),
                Err(error) => (0, Some(error)),
            };
        }
        let mut count = 0_usize;
        while count < max_records {
            match self.decode_next(count, max_records) {
                Ok(true) => count += 1,
                Ok(false) => return (count, None),
                Err(error) => return (count, Some(error)),
            }
        }
        (count, None)
    }

    /// Allocates the reusable batch with at least `capacity` rows, fallibly.
    ///
    /// Callers invoke this only after input has proven that a record frame exists.
    fn ensure_batch(&mut self, capacity: usize) -> Result<(), RecordStreamReadError> {
        if self
            .batch
            .as_ref()
            .is_some_and(|batch| batch.shot_count() >= capacity)
        {
            return Ok(());
        }
        self.batch = None;
        self.batch = Some(PackedShotBatch::zeros(capacity, self.bits_per_record)?);
        Ok(())
    }

    fn check_empty_zero_width_input(&mut self) -> Result<(), RecordStreamReadError> {
        if self.zero_width_input_checked {
            return Ok(());
        }
        self.fill_at_least(1)?;
        if self.available() > 0 {
            return Err(RecordStreamReadError::NonEmptyZeroWidthInput);
        }
        self.zero_width_input_checked = true;
        Ok(())
    }

    /// Ensures `wanted` unconsumed bytes are buffered, or that the transport has ended.
    fn fill_at_least(&mut self, wanted: usize) -> Result<(), RecordStreamReadError> {
        while self.available() < wanted && !self.eof {
            self.refill()?;
        }
        Ok(())
    }

    /// Moves unconsumed bytes to the front of the buffer and reads once from the transport.
    ///
    /// The buffer doubles (zero-filled once per growth) only when unconsumed input already fills
    /// at least half of it, so its size follows bytes actually received rather than any declared
    /// record width, a record spanning many refills costs amortized linear work, and ordinary
    /// reads never re-initialize storage.
    fn refill(&mut self) -> Result<(), RecordStreamReadError> {
        if self.eof {
            return Ok(());
        }
        if self.start > 0 {
            self.absolute_base = self.absolute_base.checked_add(self.start).ok_or_else(|| {
                FormatError::invalid_data("streamed input byte offset overflowed")
            })?;
            self.buffer.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }
        let free = self.buffer.len() - self.end;
        if free == 0 || free < self.buffer.len() / 2 {
            let grown = self
                .buffer
                .len()
                .saturating_mul(2)
                .max(INITIAL_BUFFER_BYTES);
            let additional = grown - self.buffer.len();
            self.buffer.try_reserve_exact(additional).map_err(|error| {
                FormatError::invalid_data(format!(
                    "record stream could not reserve {additional} buffered input bytes: {error}"
                ))
            })?;
            self.buffer.resize(grown, 0);
        }
        let read = loop {
            let window = self.buffer.get_mut(self.end..).unwrap_or_default();
            match self.source.read(window) {
                Ok(read) => break read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error.into()),
            }
        };
        self.end = self.end.saturating_add(read).min(self.buffer.len());
        if read == 0 {
            self.eof = true;
        }
        Ok(())
    }

    /// Decodes the next record into batch row `row`, allocating a batch of `capacity` rows once
    /// the first frame is proven. Returns `false` at a clean end of input.
    fn decode_next(&mut self, row: usize, capacity: usize) -> Result<bool, RecordStreamReadError> {
        match &self.decoder {
            StreamDecoder::Text { .. } => match self.options.framing {
                RecordFraming::Stream => self.next_stream_text_record(row, capacity),
                RecordFraming::Records => self.next_line_text_record(row, capacity),
            },
            StreamDecoder::B8 => self.next_b8_record(row, capacity),
            StreamDecoder::R8 => self.next_r8_record(row, capacity),
            StreamDecoder::Ptb64(_) => self.next_ptb64_record(row, capacity),
        }
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

    fn next_stream_text_record(
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

    fn next_line_text_record(
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

    fn frame_b8_record(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
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

    fn next_b8_record(
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

    fn next_r8_record(
        &mut self,
        row: usize,
        capacity: usize,
    ) -> Result<bool, RecordStreamReadError> {
        let width = self.bits_per_record;
        loop {
            if self.available() == 0 {
                if self.eof {
                    return Ok(false);
                }
                self.refill()?;
                continue;
            }
            let record_offset = self.record_offset().ok_or_else(|| {
                FormatError::invalid_data("streamed record byte offset overflowed")
            })?;
            let decoded = if self.batch.is_none() {
                // Prove the first frame before allocating storage proportional to the width.
                decode::r8_frame_len(self.unconsumed(), width).map(|_| None)
            } else {
                // A later read may request more records than the batch holds; regrow it like
                // the other formats before decoding into the requested row.
                self.ensure_batch(capacity)?;
                let frame = self.buffer.get(self.start..self.end).unwrap_or_default();
                let batch = self.batch.as_mut().ok_or_else(|| {
                    FormatError::invalid_data("r8 record lost its reusable batch")
                })?;
                decode::r8_record(frame, width, &mut batch.shot_words_mut(row)?).map(Some)
            };
            match decoded {
                Ok(Some(consumed)) => {
                    self.consume(consumed)?;
                    return Ok(true);
                }
                Ok(None) => self.ensure_batch(capacity)?,
                Err(error)
                    if error.code() == FormatErrorCode::UnexpectedEndOfInput && !self.eof =>
                {
                    // The record spans the buffered window; fetch more bytes and redecode it
                    // from its start. The buffer grows geometrically, bounding total redecode
                    // work by a constant factor of the record length.
                    self.refill()?;
                }
                Err(error) => return Err(error.with_span_offset(record_offset).into()),
            }
        }
    }

    fn frame_ptb64_group(&mut self) -> Result<Option<usize>, RecordStreamReadError> {
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

    fn next_ptb64_record(
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

fn internal_decoder_mismatch() -> FormatError {
    FormatError::invalid_data("record stream decoder state disagreed with its selected format")
}
