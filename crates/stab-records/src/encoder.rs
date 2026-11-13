//! The single Records-owned result encoder.
//!
//! [`RecordEncoder`] turns packed shot data into the bytes of one Stim result format and drains
//! them into any [`Write`]. Every product output route uses it, so the format rules (record
//! separators, DETS namespaces, R8 run splitting, B8 tail masking, and PTB64 grouping) live in
//! exactly one place.

use std::io::{self, Write};

use stab_bits::BitSlice;
use thiserror::Error;

use crate::{
    BitPlane64BatchView, DetsResultType, FormatError, FormatErrorCode, FormatErrorContext,
    PackedShotBatch, PackedShotBatchView, RecordFormat, RecordResult,
};

/// Encoded bytes retained before they are drained into the destination writer mid-call.
const DRAIN_THRESHOLD_BYTES: usize = 64 * 1024;

const WORD_BITS: usize = u64::BITS as usize;

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

/// One column group of a shot-major batch together with its DETS namespace.
///
/// Every part of one [`RecordEncoder::write_batch`] call must hold the same number of shots; shot
/// `i` of the output record is the concatenation of shot `i` of every part.
#[derive(Clone, Copy, Debug)]
pub struct RecordBatchPart<'a> {
    namespace: DetsResultType,
    records: PackedShotBatchView<'a>,
}

impl<'a> RecordBatchPart<'a> {
    pub const fn new(namespace: DetsResultType, records: PackedShotBatchView<'a>) -> Self {
        Self { namespace, records }
    }

    pub const fn namespace(self) -> DetsResultType {
        self.namespace
    }

    pub const fn records(self) -> PackedShotBatchView<'a> {
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
/// [`Self::pending_group_records`]. Working storage is bounded by one record, one batch transpose,
/// and one PTB64 group, never by the number of records written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordEncoder {
    format: RecordFormat,
    width: usize,
    pending: Vec<u8>,
    record: RecordState,
    ptb64_planes: Vec<u64>,
    ptb64_records: usize,
    transpose: Option<PackedShotBatch>,
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
            transpose: None,
        })
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
            self.encode_part(part.namespace, part.bits.words(), part.bits.len())?;
        }
        self.end_record();
        self.drain(out)
    }

    /// Encodes every shot of a shot-major batch whose records are split into column groups.
    pub fn write_batch<W>(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        let shot_count = self.validate_batch(parts)?;
        if self.format == RecordFormat::Ptb64 && self.ptb64_records == 0 && shot_count == WORD_BITS
        {
            return self.write_aligned_ptb64_group(parts, out);
        }
        for shot_index in 0..shot_count {
            self.begin_record();
            for part in parts {
                let row = part.records.shot(shot_index)?;
                self.encode_part(part.namespace, row.words(), row.len())?;
            }
            self.end_record();
            self.drain_if_full(out)?;
        }
        self.drain(out)
    }

    /// Encodes a bit-plane batch whose records all belong to one namespace.
    pub fn write_bit_planes<W>(
        &mut self,
        planes: BitPlane64BatchView<'_>,
        namespace: DetsResultType,
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        self.check_width(planes.bits_per_shot())?;
        if self.format == RecordFormat::Ptb64
            && self.ptb64_records == 0
            && planes.shot_count() == WORD_BITS
        {
            for bit_index in 0..planes.bits_per_shot() {
                let plane = planes.plane(bit_index)?;
                let word = plane.words().first().copied().ok_or_else(|| {
                    FormatError::invalid_data(
                        "64-shot bit plane did not contain its required storage word",
                    )
                })?;
                self.pending.extend_from_slice(&word.to_le_bytes());
                self.drain_if_full(out)?;
            }
            return self.drain(out);
        }
        let mut transpose = match self.transpose.take() {
            Some(transpose)
                if transpose.shot_count() == planes.shot_count()
                    && transpose.bits_per_shot() == planes.bits_per_shot() =>
            {
                transpose
            }
            _ => PackedShotBatch::zeros(planes.shot_count(), planes.bits_per_shot())?,
        };
        transpose.copy_from_bit_planes(planes)?;
        let result = self.write_batch(&[RecordBatchPart::new(namespace, transpose.view())], out);
        self.transpose = Some(transpose);
        result
    }

    /// Transposes a batch of exactly 64 shots straight into one complete PTB64 group.
    fn write_aligned_ptb64_group<W>(
        &mut self,
        parts: &[RecordBatchPart<'_>],
        out: &mut W,
    ) -> Result<(), RecordWriteError>
    where
        W: Write + ?Sized,
    {
        let mut offset = 0_usize;
        for part in parts {
            let end = offset + part.records.bits_per_shot();
            let planes = self.ptb64_planes.get_mut(offset..end).ok_or_else(|| {
                FormatError::invalid_data("ptb64 column group escaped the encoder's declared width")
            })?;
            part.records.copy_into_bit_plane_words(planes)?;
            offset = end;
        }
        for plane in &mut self.ptb64_planes {
            self.pending.extend_from_slice(&plane.to_le_bytes());
            *plane = 0;
        }
        self.drain(out)
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

    fn encode_part(
        &mut self,
        namespace: DetsResultType,
        words: &[u64],
        len: usize,
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
                    push_decimal(pending, index);
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
        while bits != 0 {
            visit(base + bits.trailing_zeros() as usize);
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
