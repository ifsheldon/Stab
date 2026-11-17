//! The single Records-owned result encoder.
//!
//! [`RecordEncoder`] turns packed shot data into the bytes of one Stim result format and drains
//! them into any [`Write`]. Every product output route uses it, so the format rules (record
//! separators, DETS namespaces, R8 run splitting, B8 tail masking, and PTB64 grouping) live in
//! exactly one place.

use std::io::{self, Write};

use stab_bits::BitSlice;
use thiserror::Error;

use crate::batches::RecordBatchStorage;
use crate::{
    DetsResultType, FormatError, FormatErrorCode, FormatErrorContext, PackedShotBatch,
    RecordBatchView, RecordFormat, RecordResult,
};

/// Encoded bytes retained before they are drained into the destination writer mid-call.
const DRAIN_THRESHOLD_BYTES: usize = 64 * 1024;

/// Bits of one wide record part encoded between two drain checks, so a single record wider than
/// this never holds its whole encoding (eight times its packed size for `01`) at once.
const SEGMENT_BITS: usize = 64 * 1024;

/// Encoded bytes retained inside one wide record before they are drained, larger than
/// [`DRAIN_THRESHOLD_BYTES`] so a wide record costs few writes.
const SEGMENT_DRAIN_BYTES: usize = 1 << 20;

const WORD_BITS: usize = u64::BITS as usize;

/// The smallest capacity a growing byte vector allocates.
const MIN_PENDING_ALLOCATION_BYTES: u128 = 8;

/// One contiguous run of a record's bits together with its DETS namespace.
///
/// Non-DETS formats concatenate parts in order and ignore their namespaces. DETS writes each
/// part's set bits as `<prefix><index>` tokens, where the index restarts at zero for every part.
#[derive(Clone, Copy, Debug)]
pub struct RecordPart<'a> {
    namespace: DetsResultType,
    bits: BitSlice<'a>,
}

impl<'a> RecordPart<'a> {
    pub const fn new(namespace: DetsResultType, bits: BitSlice<'a>) -> Self {
        Self { namespace, bits }
    }

    pub const fn namespace(self) -> DetsResultType {
        self.namespace
    }

    pub const fn bits(self) -> BitSlice<'a> {
        self.bits
    }
}

/// One column group of a batch together with its DETS namespace.
///
/// Every part of one [`RecordEncoder::write_batch`] call must hold the same number of shots; shot
/// `i` of the output record is the concatenation of shot `i` of every part. Each part may be
/// shot-major or bit planes of any shot count, independently of the others.
#[derive(Clone, Copy, Debug)]
pub struct RecordBatchPart<'a> {
    namespace: DetsResultType,
    records: RecordBatchView<'a>,
}

impl<'a> RecordBatchPart<'a> {
    pub fn new(namespace: DetsResultType, records: impl Into<RecordBatchView<'a>>) -> Self {
        Self {
            namespace,
            records: records.into(),
        }
    }

    pub const fn namespace(self) -> DetsResultType {
        self.namespace
    }

    pub const fn records(self) -> RecordBatchView<'a> {
        self.records
    }
}

/// A failure while encoding records or draining them into the destination writer.
#[derive(Debug, Error)]
pub enum RecordWriteError {
    /// The supplied records did not match the encoder's declared layout.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// The destination writer failed.
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Encodes packed records of one fixed width into one result format.
///
/// Each `write_*` call validates its input, encodes it, and drains every completed byte into the
/// destination before returning, so no encoded record is held back between calls. The only state
/// retained across calls is an incomplete PTB64 group of fewer than 64 records, reported by
/// [`Self::pending_group_records`]. Bit-plane input is encoded 64 shots at a time: PTB64 copies
/// whole plane words, and every other format transposes one 64-shot block per part. Working
/// storage is bounded by one record, one 64-shot block per part, and one PTB64 group, never by
/// the number of records or the size of a batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordEncoder {
    format: RecordFormat,
    width: usize,
    pending: Vec<u8>,
    record: RecordState,
    ptb64_planes: Vec<u64>,
    ptb64_records: usize,
    /// One transposed 64-shot block per bit-plane part of the current batch.
    plane_rows: Vec<PackedShotBatch>,
    /// One word per bit of the bit-plane part being transposed.
    plane_column: Vec<u64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RecordState {
    /// Bits of the current record consumed by earlier parts.
    offset: usize,
    /// Whether the current HITS record has emitted a hit yet.
    hits_started: bool,
    /// Index of the first bit after the last R8 hit of the current record.
    r8_next: usize,
    /// B8 bits not yet completing a byte, in the low `b8_bits` bits.
    b8_partial: u8,
    b8_bits: usize,
}

impl RecordEncoder {
    /// Creates an encoder for records of exactly `width` bits.
    pub fn try_new(format: RecordFormat, width: usize) -> RecordResult<Self> {
        let mut ptb64_planes = Vec::new();
        if format == RecordFormat::Ptb64 {
            ptb64_planes.try_reserve_exact(width).map_err(|error| {
                FormatError::invalid_data(format!(
                    "ptb64 encoder could not reserve {width} result planes: {error}"
                ))
            })?;
            ptb64_planes.resize(width, 0);
        }
        Ok(Self {
            format,
            width,
            pending: Vec::new(),
            record: RecordState::default(),
            ptb64_planes,
            ptb64_records: 0,
            plane_rows: Vec::new(),
            plane_column: Vec::new(),
        })
    }

    /// Upper bound on the bytes an encoder of `format` records of `width` bits retains while it
    /// encodes shot-major batches through [`Self::write_batch`] or single records through
    /// [`Self::write_record`]: its PTB64 group planes and its pending-output buffer.
    ///
    /// Output drains once it reaches a threshold between records, after each segment of a wide
    /// part, and after each part of a record wider than one segment, so the pending buffer holds
    /// at most that threshold plus one segment's (or one PTB64 group's) encoding. Amortized growth
    /// leaves its capacity at most twice that, and a growth briefly holds the old allocation as
    /// well, so the bound charges three times it. Bit-plane batches add one transposed 64-shot
    /// block per part, which this bound excludes.
    pub fn shot_major_scratch_bytes(format: RecordFormat, width: usize) -> u128 {
        Self::scratch_bytes_with_pending(format, width, u128::MAX)
    }

    /// [`Self::shot_major_scratch_bytes`] for an encoder that writes at most `records` records.
    ///
    /// The pending buffer never holds more than the whole output, so a short run is charged for
    /// at most its encoded size (a vector's smallest allocation of 8 bytes at least) instead of
    /// the drain threshold; long runs get the same bound as [`Self::shot_major_scratch_bytes`].
    pub fn shot_major_scratch_bytes_for_records(
        format: RecordFormat,
        width: usize,
        records: u64,
    ) -> u128 {
        let output = if format == RecordFormat::Ptb64 {
            u128::from(records.div_ceil(WORD_BITS as u64))
                .saturating_mul((width as u128).saturating_mul(size_of::<u64>() as u128))
        } else {
            u128::from(records).saturating_mul(record_encoded_bytes(format, width))
        };
        Self::scratch_bytes_with_pending(format, width, output.max(MIN_PENDING_ALLOCATION_BYTES))
    }

    /// The scratch bound when the pending buffer never holds more than `max_pending` bytes.
    fn scratch_bytes_with_pending(format: RecordFormat, width: usize, max_pending: u128) -> u128 {
        let width_bytes = width as u128;
        let (threshold, unit, planes) = if format == RecordFormat::Ptb64 {
            let group = width_bytes.saturating_mul(size_of::<u64>() as u128);
            (DRAIN_THRESHOLD_BYTES, group, group)
        } else {
            let threshold = if width > SEGMENT_BITS {
                SEGMENT_DRAIN_BYTES
            } else {
                DRAIN_THRESHOLD_BYTES
            };
            (threshold, segment_encoded_bytes(format, width), 0)
        };
        (threshold as u128)
            .saturating_add(unit)
            .min(max_pending)
            .saturating_mul(3)
            .saturating_add(planes)
    }

    /// Upper bound on the bytes an encoder of `format` additionally retains while it encodes
    /// bit-plane batches whose parts have `part_widths` bits, beyond
    /// [`Self::shot_major_scratch_bytes`]: one transposed 64-shot block per part, the column of
    /// plane words being transposed, and the list holding the blocks.
    ///
    /// The column grows to the widest part with amortized growth and briefly holds its old
    /// allocation as well, so the bound charges three times it, and a block replaced for a part
    /// of a new width briefly coexists with its replacement, so the widest block is charged
    /// twice. PTB64 output copies whole plane words of 64-shot aligned groups into the group
    /// planes the shot-major bound already covers, so it adds nothing here.
    pub fn bit_plane_scratch_bytes(format: RecordFormat, part_widths: &[usize]) -> u128 {
        if format == RecordFormat::Ptb64 || part_widths.is_empty() {
            return 0;
        }
        let blocks = part_widths
            .iter()
            .map(|width| PackedShotBatch::storage_bytes(WORD_BITS, *width))
            .fold(0_u128, u128::saturating_add);
        let widest = part_widths.iter().copied().max().unwrap_or(0);
        let column = (widest as u128)
            .saturating_mul(size_of::<u64>() as u128)
            .saturating_mul(3);
        // A vector of up to four entries is reserved on the first push.
        let list =
            (part_widths.len().max(4) as u128).saturating_mul(size_of::<PackedShotBatch>() as u128);
        blocks
            .saturating_add(PackedShotBatch::storage_bytes(WORD_BITS, widest))
            .saturating_add(column)
            .saturating_add(list)
    }

    pub const fn format(&self) -> RecordFormat {
        self.format
    }

    pub const fn width(&self) -> usize {
        self.width
    }

    /// Records buffered in an incomplete PTB64 group; always zero for other formats.
    ///
    /// A complete output requires this to be zero once the last record is written.
    pub const fn pending_group_records(&self) -> usize {
        self.ptb64_records
    }

    /// Encodes one record assembled from `parts` and drains it into `out`.
    pub fn write_record<W>(
        &mut self,
        parts: &[RecordPart<'_>],
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        let actual = parts.iter().try_fold(0_usize, |total, part| {
            total
                .checked_add(part.bits.len())
                .ok_or_else(|| FormatError::invalid_data("result record width overflowed"))
        })?;
        self.check_width(actual)?;
        self.begin_record();
        for part in parts {
            self.encode_part(part.namespace, part.bits.words(), part.bits.len(), 0)?;
        }
        self.end_record();
        self.drain(out)
    }

    /// Encodes every shot of a batch whose records are split into column groups.
    pub fn write_batch<W>(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        let shot_count = self.validate_batch(parts)?;
        let all_shot_major = parts
            .iter()
            .all(|part| part.records.shot_major_records().is_some());
        if all_shot_major && self.format != RecordFormat::Ptb64 {
            return self.write_shot_major_records(parts, shot_count, out);
        }
        let mut block_start = 0_usize;
        while block_start < shot_count {
            let block_shots = WORD_BITS.min(shot_count - block_start);
            if self.format == RecordFormat::Ptb64
                && self.ptb64_records == 0
                && block_shots == WORD_BITS
            {
                self.write_aligned_ptb64_group(parts, block_start)?;
            } else {
                self.write_block_records(parts, block_start, block_shots, out)?;
            }
            self.drain_if_full(out)?;
            block_start += block_shots;
        }
        self.drain(out)
    }

    /// Encodes a batch whose parts are all shot-major, record by record.
    fn write_shot_major_records<W>(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        shot_count: usize,
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        for shot_index in 0..shot_count {
            self.begin_record();
            for part in parts {
                let row = match part.records.storage() {
                    RecordBatchStorage::ShotMajor(records) => records.shot(shot_index)?,
                    RecordBatchStorage::BitPlanes(_) => {
                        return Err(FormatError::invalid_data(
                            "a shot-major batch part changed layout while it was encoded",
                        )
                        .into());
                    }
                };
                self.encode_part_draining(part.namespace, row.words(), row.len(), out)?;
            }
            self.end_record();
            self.drain_if_full(out)?;
        }
        self.drain(out)
    }

    /// Encodes the shots of one 64-shot block record by record.
    fn write_block_records<W>(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        block_start: usize,
        block_shots: usize,
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        let mut plane_rows = std::mem::take(&mut self.plane_rows);
        let result = self.transpose_plane_blocks(parts, block_start, &mut plane_rows);
        let result = result.and_then(|()| {
            for block_shot in 0..block_shots {
                self.begin_record();
                for (part, rows) in parts.iter().zip(&plane_rows) {
                    let row = match part.records.storage() {
                        RecordBatchStorage::ShotMajor(records) => {
                            records.shot(block_start + block_shot)?
                        }
                        RecordBatchStorage::BitPlanes(_) => rows.shot(block_shot)?,
                    };
                    self.encode_part_draining(part.namespace, row.words(), row.len(), out)?;
                }
                self.end_record();
                self.drain_if_full(out)?;
            }
            Ok(())
        });
        self.plane_rows = plane_rows;
        result
    }

    /// Transposes block `block_start / 64` of every bit-plane part into `plane_rows`, which
    /// receives one entry per part; shot-major parts keep an unused entry.
    fn transpose_plane_blocks(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        block_start: usize,
        plane_rows: &mut Vec<PackedShotBatch>,
    ) -> Result<(), RecordWriteError> {
        for (index, part) in parts.iter().enumerate() {
            let width = match part.records.storage() {
                RecordBatchStorage::BitPlanes(_) => part.records.bits_per_shot(),
                RecordBatchStorage::ShotMajor(_) => 0,
            };
            match plane_rows.get(index) {
                Some(rows) if rows.bits_per_shot() == width => {}
                Some(_) | None => {
                    let rows = PackedShotBatch::zeros(WORD_BITS, width)?;
                    if let Some(slot) = plane_rows.get_mut(index) {
                        *slot = rows;
                    } else {
                        plane_rows.push(rows);
                    }
                }
            }
            let RecordBatchStorage::BitPlanes(planes) = part.records.storage() else {
                continue;
            };
            let rows = plane_rows.get_mut(index).ok_or_else(|| {
                FormatError::invalid_data("bit-plane block scratch lost its part entry")
            })?;
            self.plane_column.resize(width, 0);
            planes.copy_word_column_into(block_start / WORD_BITS, &mut self.plane_column)?;
            rows.copy_transpose_from_word_column(&self.plane_column)?;
        }
        Ok(())
    }

    /// Writes one complete PTB64 group from the 64 shots starting at `block_start`.
    fn write_aligned_ptb64_group(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        block_start: usize,
    ) -> RecordResult<()> {
        let mut offset = 0_usize;
        for part in parts {
            let end = offset + part.records.bits_per_shot();
            let planes = self.ptb64_planes.get_mut(offset..end).ok_or_else(|| {
                FormatError::invalid_data("ptb64 column group escaped the encoder's declared width")
            })?;
            match part.records.storage() {
                RecordBatchStorage::ShotMajor(records) => {
                    records.copy_rows_into_bit_plane_words(block_start, WORD_BITS, planes)?;
                }
                RecordBatchStorage::BitPlanes(bit_planes) => {
                    bit_planes.copy_word_column_into(block_start / WORD_BITS, planes)?;
                }
            }
            offset = end;
        }
        for plane in &mut self.ptb64_planes {
            self.pending.extend_from_slice(&plane.to_le_bytes());
            *plane = 0;
        }
        Ok(())
    }

    fn check_width(&self, actual: usize) -> RecordResult<()> {
        if actual != self.width {
            return Err(FormatError::with_context(
                FormatErrorCode::InvalidRecordWidth,
                format!(
                    "result record has width {actual} but the encoder expects {}",
                    self.width
                ),
                None,
                FormatErrorContext::RecordWidth {
                    actual_bits: actual,
                    expected_bits: self.width,
                },
            ));
        }
        Ok(())
    }

    fn validate_batch(&self, parts: &[RecordBatchPart<'_>]) -> RecordResult<usize> {
        let Some(first) = parts.first() else {
            return Err(FormatError::invalid_data(
                "a record batch needs at least one column group to define its shot count",
            ));
        };
        let shot_count = first.records.shot_count();
        let mut width = 0_usize;
        for part in parts {
            if part.records.shot_count() != shot_count {
                return Err(FormatError::invalid_data(format!(
                    "record batch column groups hold {} and {shot_count} shots",
                    part.records.shot_count()
                )));
            }
            width = width
                .checked_add(part.records.bits_per_shot())
                .ok_or_else(|| FormatError::invalid_data("result record width overflowed"))?;
        }
        self.check_width(width)?;
        Ok(shot_count)
    }

    fn begin_record(&mut self) {
        self.record = RecordState::default();
        if self.format == RecordFormat::Dets {
            self.pending.extend_from_slice(b"shot");
        }
    }

    /// Encodes one part like [`Self::encode_part`], draining between segments of a wide part.
    ///
    /// A part split at word boundaries encodes exactly as the whole part: every format but DETS
    /// positions bits by the record offset, and DETS numbers each segment from its start within
    /// the part. PTB64 buffers whole groups, so it encodes the part at once.
    fn encode_part_draining<W>(
        &mut self,
        namespace: DetsResultType,
        words: &[u64],
        len: usize,
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        if self.format == RecordFormat::Ptb64 {
            return Ok(self.encode_part(namespace, words, len, 0)?);
        }
        if len <= SEGMENT_BITS {
            // Narrow parts of one wide record drain like its segments, so a record of many
            // parts never holds more than one segment past the drain threshold.
            self.encode_part(namespace, words, len, 0)?;
            if self.pending.len() >= SEGMENT_DRAIN_BYTES {
                self.drain(out)?;
            }
            return Ok(());
        }
        let mut start = 0_usize;
        while start < len {
            let segment = (len - start).min(SEGMENT_BITS);
            let first_word = start / WORD_BITS;
            let segment_words = words
                .get(first_word..first_word + segment.div_ceil(WORD_BITS))
                .ok_or_else(|| {
                    FormatError::invalid_data(
                        "packed record omitted words inside its declared width",
                    )
                })?;
            self.encode_part(namespace, segment_words, segment, start)?;
            if self.pending.len() >= SEGMENT_DRAIN_BYTES {
                self.drain(out)?;
            }
            start += segment;
        }
        Ok(())
    }

    /// Encodes `len` bits of one part; `part_start` is the index of the first of them within
    /// their part, which only DETS numbering reads.
    fn encode_part(
        &mut self,
        namespace: DetsResultType,
        words: &[u64],
        len: usize,
        part_start: usize,
    ) -> RecordResult<()> {
        if words.len() < len.div_ceil(WORD_BITS) {
            return Err(FormatError::invalid_data(
                "packed record omitted words inside its declared width",
            ));
        }
        let offset = self.record.offset;
        match self.format {
            RecordFormat::ZeroOne => encode_zero_one(&mut self.pending, words, len),
            RecordFormat::B8 => self.encode_b8(words, len),
            RecordFormat::R8 => {
                let pending = &mut self.pending;
                let next = &mut self.record.r8_next;
                for_each_set_bit(words, len, |index| {
                    let bit = offset + index;
                    push_r8_gap(pending, bit - *next);
                    *next = bit + 1;
                });
            }
            RecordFormat::Hits => {
                let pending = &mut self.pending;
                let started = &mut self.record.hits_started;
                for_each_set_bit(words, len, |index| {
                    if *started {
                        pending.push(b',');
                    }
                    *started = true;
                    push_decimal(pending, offset + index);
                });
            }
            RecordFormat::Dets => {
                let pending = &mut self.pending;
                let prefix = namespace.prefix();
                for_each_set_bit(words, len, |index| {
                    pending.extend_from_slice(&[b' ', prefix]);
                    push_decimal(pending, part_start + index);
                });
            }
            RecordFormat::Ptb64 => {
                let lane = 1_u64 << self.ptb64_records;
                let planes = &mut self.ptb64_planes;
                let mut escaped = false;
                for_each_set_bit(words, len, |index| match planes.get_mut(offset + index) {
                    Some(plane) => *plane |= lane,
                    None => escaped = true,
                });
                if escaped {
                    return Err(FormatError::invalid_data(
                        "ptb64 record bit escaped the encoder's declared width",
                    ));
                }
            }
        }
        self.record.offset = offset
            .checked_add(len)
            .ok_or_else(|| FormatError::invalid_data("result record width overflowed"))?;
        Ok(())
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "only the low byte of the bit accumulator is emitted"
    )]
    fn encode_b8(&mut self, words: &[u64], len: usize) {
        if self.record.b8_bits == 0 {
            let full_words = len / WORD_BITS;
            for word in words.iter().take(full_words) {
                self.pending.extend_from_slice(&word.to_le_bytes());
            }
            let tail_bits = len % WORD_BITS;
            if tail_bits != 0
                && let Some(word) = words.get(full_words)
            {
                let bytes = word.to_le_bytes();
                let full_bytes = tail_bits / 8;
                self.pending
                    .extend_from_slice(bytes.get(..full_bytes).unwrap_or_default());
                let partial_bits = tail_bits % 8;
                if partial_bits != 0 {
                    let byte = bytes.get(full_bytes).copied().unwrap_or_default();
                    self.record.b8_partial = byte & low_byte_mask(partial_bits);
                    self.record.b8_bits = partial_bits;
                }
            }
            return;
        }
        let mut remaining = len;
        for word in words {
            if remaining == 0 {
                break;
            }
            let chunk_bits = remaining.min(WORD_BITS);
            let chunk = u128::from(*word & low_word_mask(chunk_bits));
            let mut combined = u128::from(self.record.b8_partial) | (chunk << self.record.b8_bits);
            let mut combined_bits = self.record.b8_bits + chunk_bits;
            while combined_bits >= 8 {
                self.pending.push(combined as u8);
                combined >>= 8;
                combined_bits -= 8;
            }
            self.record.b8_partial = combined as u8;
            self.record.b8_bits = combined_bits;
            remaining -= chunk_bits;
        }
    }

    fn end_record(&mut self) {
        match self.format {
            RecordFormat::ZeroOne | RecordFormat::Hits | RecordFormat::Dets => {
                self.pending.push(b'\n');
            }
            RecordFormat::B8 => {
                if self.record.b8_bits != 0 {
                    self.pending.push(self.record.b8_partial);
                }
            }
            RecordFormat::R8 => {
                push_r8_gap(&mut self.pending, self.record.offset - self.record.r8_next);
            }
            RecordFormat::Ptb64 => {
                self.ptb64_records += 1;
                if self.ptb64_records == WORD_BITS {
                    for plane in &mut self.ptb64_planes {
                        self.pending.extend_from_slice(&plane.to_le_bytes());
                        *plane = 0;
                    }
                    self.ptb64_records = 0;
                }
            }
        }
    }

    fn drain_if_full<W>(&mut self, out: &mut W) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        if self.pending.len() >= DRAIN_THRESHOLD_BYTES {
            return self.drain(out);
        }
        Ok(())
    }

    fn drain<W>(&mut self, out: &mut W) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        if self.pending.is_empty() {
            return Ok(());
        }
        let result = out.write_all(&self.pending);
        self.pending.clear();
        result.map_err(RecordWriteError::Io)
    }
}

/// Upper bound on the bytes one segment of a `width`-bit record encodes to in a non-PTB64
/// `format`, including the record's framing and, for `r8`, every run byte the record's zero gaps
/// can emit at once.
fn segment_encoded_bytes(format: RecordFormat, width: usize) -> u128 {
    part_encoded_bytes(format, width, width.min(SEGMENT_BITS))
}

/// Upper bound on the encoded bytes of one whole record of `width` bits.
fn record_encoded_bytes(format: RecordFormat, width: usize) -> u128 {
    part_encoded_bytes(format, width, width)
}

/// Upper bound on the encoded bytes of `bits` bits of a record of `width` bits, plus framing.
fn part_encoded_bytes(format: RecordFormat, width: usize, bits: usize) -> u128 {
    let segment = bits as u128;
    let digits = decimal_digits(width) as u128;
    let framing = b"shot\n".len() as u128;
    let encoded = match format {
        RecordFormat::ZeroOne => segment,
        RecordFormat::B8 => segment.div_ceil(8),
        RecordFormat::R8 => segment.saturating_add((width / usize::from(u8::MAX)) as u128 + 1),
        RecordFormat::Hits => segment.saturating_mul(digits + 1),
        RecordFormat::Dets => segment.saturating_mul(digits + 2),
        RecordFormat::Ptb64 => segment.saturating_mul(size_of::<u64>() as u128),
    };
    encoded.saturating_add(framing)
}

/// Decimal digits of `value`, which bounds the digits of every index below it.
const fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

/// Calls `visit` with the index of every set bit below `len`, in increasing order.
#[inline]
fn for_each_set_bit(words: &[u64], len: usize, mut visit: impl FnMut(usize)) {
    let mut remaining = len;
    for (word_index, word) in words.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let chunk_bits = remaining.min(WORD_BITS);
        let mut bits = *word & low_word_mask(chunk_bits);
        let base = word_index * WORD_BITS;
        while let Some(bit) = bits.lowest_one() {
            visit(base + bit as usize);
            bits &= bits - 1;
        }
        remaining -= chunk_bits;
    }
}

const fn low_word_mask(bits: usize) -> u64 {
    if bits >= WORD_BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

const fn low_byte_mask(bits: usize) -> u8 {
    if bits >= 8 {
        u8::MAX
    } else {
        (1_u8 << bits) - 1
    }
}

const ZERO_ONE_BY_BYTE: [[u8; 8]; 256] = zero_one_by_byte();

#[allow(
    clippy::indexing_slicing,
    reason = "const table indices are bounded by the byte and bit loop limits"
)]
const fn zero_one_by_byte() -> [[u8; 8]; 256] {
    let mut table = [[0_u8; 8]; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut bit = 0;
        while bit < 8 {
            table[byte][bit] = if byte & (1 << bit) == 0 { b'0' } else { b'1' };
            bit += 1;
        }
        byte += 1;
    }
    table
}

#[allow(
    clippy::indexing_slicing,
    reason = "every u8 value indexes the 256-entry table"
)]
fn encode_zero_one(out: &mut Vec<u8>, words: &[u64], len: usize) {
    out.reserve(len);
    let mut remaining = len;
    for word in words {
        if remaining == 0 {
            break;
        }
        let chunk_bits = remaining.min(WORD_BITS);
        if chunk_bits == WORD_BITS {
            let mut text = [0_u8; WORD_BITS];
            for (target, byte) in text
                .as_chunks_mut::<8>()
                .0
                .iter_mut()
                .zip(word.to_le_bytes())
            {
                *target = ZERO_ONE_BY_BYTE[usize::from(byte)];
            }
            out.extend_from_slice(&text);
            remaining -= WORD_BITS;
            continue;
        }
        for (byte_index, byte) in word.to_le_bytes().into_iter().enumerate() {
            let start = byte_index * 8;
            if start >= chunk_bits {
                break;
            }
            let pattern = &ZERO_ONE_BY_BYTE[usize::from(byte)];
            let count = (chunk_bits - start).min(8);
            out.extend_from_slice(pattern.get(..count).unwrap_or_default());
        }
        remaining -= chunk_bits;
    }
}

/// Writes the R8 encoding of a run of `gap` zero bits followed by one terminator.
///
/// A run of 255 zeros is written as `0xFF` and continues, so the run ends with `gap % 255`.
#[allow(
    clippy::cast_possible_truncation,
    reason = "a value modulo 255 always fits in a byte"
)]
fn push_r8_gap(out: &mut Vec<u8>, gap: usize) {
    let full_runs = gap / usize::from(u8::MAX);
    out.resize(out.len() + full_runs, u8::MAX);
    out.push((gap % usize::from(u8::MAX)) as u8);
}

const DECIMAL_PAIRS: &[u8; 200] = b"\
0001020304050607080910111213141516171819\
2021222324252627282930313233343536373839\
4041424344454647484950515253545556575859\
6061626364656667686970717273747576777879\
8081828384858687888990919293949596979899";

#[allow(
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    reason = "digit values are below 100 and the buffer holds every usize decimal digit"
)]
fn push_decimal(out: &mut Vec<u8>, mut value: usize) {
    let mut digits = [0_u8; size_of::<usize>() * 3];
    let mut start = digits.len();
    while value >= 100 {
        let pair = (value % 100) * 2;
        value /= 100;
        start -= 2;
        digits[start..start + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    }
    if value >= 10 {
        let pair = value * 2;
        start -= 2;
        digits[start..start + 2].copy_from_slice(&DECIMAL_PAIRS[pair..pair + 2]);
    } else {
        start -= 1;
        digits[start] = b'0' + value as u8;
    }
    out.extend_from_slice(&digits[start..]);
}

#[cfg(test)]
mod tests;
