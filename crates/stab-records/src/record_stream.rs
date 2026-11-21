//! Packed record streaming over byte transports.
//!
//! [`RecordStreamReader`] frames records out of an [`std::io::Read`] transport and decodes them
//! straight into the words of a reusable [`PackedShotBatch`]: sparse formats set only their hit
//! bits, `01` packs eight bytes per step, `b8` copies bytes, and PTB64 transposes whole groups.
//! Grammar diagnostics come from the shared text decoders, which the test-only whole-buffer
//! reference codec also drives, so the streaming diagnostics are checked against an independent
//! framing of the same grammar.
//!
//! Input is read through one reusable buffer that grows only while a single record frame spans
//! it, and record storage is allocated fallibly, and only once input has proven that a record
//! frame exists, so a declared width far beyond the available input is reported as a typed error
//! instead of aborting the process.

use std::io::Read;

use thiserror::Error;

use crate::{
    DetsLayout, FormatError, PackedShotBatch, PackedShotBatchView, RecordFormat, RecordResult,
};

mod b8;
mod decode;
mod fast_text;
mod ptb64;
mod r8;
mod storage;
mod text;

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
    /// Run-length records with data-dependent frame lengths, decoded as their bytes arrive.
    R8(R8State),
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

/// Resumable `r8` decoding state that outlives one record.
#[derive(Debug, Default)]
struct R8State {
    /// Hits of the first record, decoded before any batch exists.
    first_row: decode::FirstR8Row,
    /// The failure of a record whose earlier bytes were already consumed. Decoding cannot
    /// resume inside it, so every later read reports it again, as rereading the record would.
    failed: Option<FormatError>,
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

    /// Absolute input offset of the first byte no decoded record has consumed: after a read that
    /// returned whole records, where the next record frame starts.
    ///
    /// A caller that seeks a fresh transport to this offset and resumes there with
    /// [`Self::starting_at_offset`] decodes the remaining records exactly as this reader would,
    /// provided no failure is pending and no PTB64 group is partly served.
    pub fn consumed_offset(&self) -> Result<usize, RecordStreamReadError> {
        self.record_offset()
            .ok_or(RecordStreamReadError::ByteOffsetOverflow)
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
            RecordFormat::R8 => StreamDecoder::R8(R8State::default()),
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

    /// Declares that the transport's first byte is input offset `offset`, a record boundary, so
    /// every diagnostic offset and [`Self::bytes_read`] count from the start of the whole input.
    ///
    /// Use it on a transport positioned at an offset another reader's
    /// [`Self::consumed_offset`] returned.
    #[must_use]
    pub const fn starting_at_offset(mut self, offset: usize) -> Self {
        self.absolute_base = offset;
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
        let skips_zero_one_lines = self.skips_canonical_zero_one_lines();
        while skipped < max_records {
            if skips_zero_one_lines {
                skipped += self.skip_canonical_zero_one_lines(max_records - skipped)?;
                if skipped == max_records {
                    break;
                }
            }
            // Under `01` line skipping, only the one line the fast check declined goes through
            // the full grammar, which owns its framing, refills, and diagnostics.
            let wanted = if skips_zero_one_lines {
                1
            } else {
                (max_records - skipped).min(SKIP_CHUNK_RECORDS)
            };
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

    /// The most bytes the transport buffer may hold: unlimited for the packed formats, and for
    /// line-framed text one byte past the record limit, which is all framing ever needs.
    fn buffer_limit(&self) -> usize {
        match self.decoder {
            StreamDecoder::Text { .. } => storage::text_buffer_limit(self.max_text_record_bytes),
            StreamDecoder::B8 | StreamDecoder::R8(_) | StreamDecoder::Ptb64(_) => usize::MAX,
        }
    }

    /// Moves unconsumed bytes to the front of the buffer and reads once from the transport.
    ///
    /// The buffer doubles (zero-filled once per growth), up to [`Self::buffer_limit`], only when
    /// unconsumed input already fills at least half of it, so its size follows bytes actually
    /// received rather than any declared record width, a record spanning many refills costs
    /// amortized linear work, and ordinary reads never re-initialize storage. Text framing
    /// refills only while fewer than the limit's bytes are unconsumed, so a buffer at its limit
    /// always has room to read.
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
                .max(INITIAL_BUFFER_BYTES)
                .min(self.buffer_limit());
            if grown <= self.buffer.len() && free == 0 {
                return Err(FormatError::invalid_data(
                    "record stream buffer filled at its limit before framing a record",
                )
                .into());
            }
            if grown > self.buffer.len() {
                let additional = grown - self.buffer.len();
                self.buffer.try_reserve_exact(additional).map_err(|error| {
                    FormatError::invalid_data(format!(
                        "record stream could not reserve {additional} buffered input bytes: {error}"
                    ))
                })?;
                self.buffer.resize(grown, 0);
            }
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
            StreamDecoder::R8(_) => self.next_r8_record(row, capacity),
            StreamDecoder::Ptb64(_) => self.next_ptb64_record(row, capacity),
        }
    }
}

fn internal_decoder_mismatch() -> FormatError {
    FormatError::invalid_data("record stream decoder state disagreed with its selected format")
}
