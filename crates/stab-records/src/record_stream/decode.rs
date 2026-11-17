//! Per-format decoders that write one record straight into packed row words.
//!
//! Every decoder clears or overwrites the whole row; the caller's row guard masks bits beyond the
//! declared width. Text grammar is delegated to the shared text decoders, so diagnostics match the
//! whole-buffer readers exactly; the `01` fast path only accepts lines that grammar would accept
//! unchanged and defers everything else to it.

use super::{HitsDuplicates, Ptb64Group};
use crate::{
    BitPlaneBatchView, DetsLayout, FormatError, PackedShotBatch, RecordFormat, RecordResult,
    result_packed::{
        b8_length_multiple_error, b8_record_byte_width, ptb64_bytes_per_group,
        ptb64_length_multiple_error, ptb64_zero_width_count_error, r8_truncated_error,
        resume_r8_record,
    },
    result_text::{
        DetsEvent, HitsEvent, for_each_dets_event, for_each_hits_event, for_each_zero_one_line,
    },
};

const WORD_BITS: usize = u64::BITS as usize;
const WORD_BYTES: usize = size_of::<u64>();
/// Every byte of an eight-byte chunk of `01` text is `b'0'` or `b'1'` exactly when its bits
/// outside the low bit equal this pattern.
const ASCII_BIT_HIGH: u64 = 0x3030_3030_3030_3030;
const BYTE_HIGH_MASK: u64 = 0xFEFE_FEFE_FEFE_FEFE;
const BYTE_LOW_BITS: u64 = 0x0101_0101_0101_0101;
/// Multiplying the low bits of eight bytes by this gathers them into the top byte, byte `i`
/// becoming bit `56 + i`.
const GATHER_LOW_BITS: u64 = 0x0102_0408_1020_4080;

pub(super) fn b8_record_bytes(width: usize) -> RecordResult<usize> {
    b8_record_byte_width(width)
}

pub(super) fn b8_truncated_error(bytes_read: usize, record_bytes: usize) -> FormatError {
    b8_length_multiple_error(bytes_read, record_bytes)
}

pub(super) fn ptb64_group_bytes(width: usize) -> RecordResult<usize> {
    ptb64_bytes_per_group(width)
}

pub(super) fn ptb64_truncated_error(bytes_read: usize, group_bytes: usize) -> FormatError {
    ptb64_length_multiple_error(bytes_read, group_bytes)
}

pub(super) fn ptb64_zero_width_error() -> FormatError {
    ptb64_zero_width_count_error()
}

/// Copies one little-endian `b8` frame into row words.
pub(super) fn b8_record(frame: &[u8], words: &mut [u64]) {
    let (chunks, tail) = frame.as_chunks::<WORD_BYTES>();
    let mut targets = words.iter_mut();
    // Zip the source first so exhausting it never consumes an extra target word.
    for (chunk, target) in chunks.iter().zip(targets.by_ref()) {
        *target = u64::from_le_bytes(*chunk);
    }
    if let Some(target) = targets.next() {
        let mut bytes = [0_u8; WORD_BYTES];
        for (byte, source) in bytes.iter_mut().zip(tail) {
            *byte = *source;
        }
        *target = u64::from_le_bytes(bytes);
    }
}

/// Decodes the buffered run bytes of one `r8` record into its row words, setting only hit bits.
///
/// The row must be cleared before the record's first bytes. Decoding resumes at `*bit_index` and
/// advances it and `*consumed` past every byte read, returning whether the record completed;
/// otherwise every byte of `input` was consumed and the record continues in later input.
pub(super) fn r8_row_bytes(
    input: &[u8],
    width: usize,
    bit_index: &mut usize,
    consumed: &mut usize,
    words: &mut [u64],
) -> RecordResult<bool> {
    // Apply ordinary runs here; the shared decoder takes over at the first byte that ends the
    // record, overshoots, or needs any check, so it owns termination and every diagnostic.
    let mut bits = *bit_index;
    let mut position = *consumed;
    while let Some(&byte) = input.get(position) {
        let Some(decoded) = bits.checked_add(usize::from(byte)) else {
            break;
        };
        if decoded >= width {
            break;
        }
        if byte == u8::MAX {
            bits = decoded;
        } else {
            let Some(word) = words.get_mut(decoded / WORD_BITS) else {
                break;
            };
            *word |= 1_u64 << (decoded % WORD_BITS);
            bits = decoded + 1;
        }
        position += 1;
    }
    *bit_index = bits;
    *consumed = position;
    resume_r8_record(input, width, bit_index, consumed, |bit| {
        set_bit(words, bit, width, "r8 hit")
    })
}

/// The hits of the first `r8` record, decoded before any batch exists.
///
/// Its storage follows the input received rather than the declared width: a sparse record keeps
/// the indexes of its hits, at most a quarter row of them, and a denser one switches to the row
/// words up to its last hit, grown geometrically up to the whole row. Either way it holds less
/// than two rows at its peak and at most one row once the record completes.
#[derive(Debug)]
pub(super) enum FirstR8Row {
    Hits(Vec<usize>),
    Words(Vec<u64>),
}

impl Default for FirstR8Row {
    fn default() -> Self {
        Self::Hits(Vec::new())
    }
}

impl FirstR8Row {
    fn record_hit(&mut self, bit: usize, width: usize) -> RecordResult<()> {
        let row_words = width.div_ceil(WORD_BITS);
        if let Self::Hits(hits) = self {
            if hits.len() < hits.capacity() {
                hits.push(bit);
                return Ok(());
            }
            let grown = hits.capacity().saturating_mul(2).max(4);
            if grown <= row_words / 4 {
                hits.try_reserve_exact(grown - hits.len())
                    .map_err(|error| first_row_error(grown, error))?;
                hits.push(bit);
                return Ok(());
            }
            // Hits arrive in increasing order, so sizing the words for this one up front means
            // the indexes and the words never coexist with a word growth.
            let needed = bit / WORD_BITS + 1;
            let mut words = Vec::new();
            words
                .try_reserve_exact(needed)
                .map_err(|error| first_row_error(needed, error))?;
            for &hit in hits.iter() {
                set_grown_bit(&mut words, hit, width, row_words)?;
            }
            *self = Self::Words(words);
        }
        match self {
            Self::Words(words) => set_grown_bit(words, bit, width, row_words),
            Self::Hits(_) => Err(FormatError::invalid_data(
                "r8 first record kept hit indexes past its switch to row words",
            )),
        }
    }

    /// Writes the decoded hits into a cleared batch row and releases this storage.
    pub(super) fn take_into(&mut self, words: &mut [u64], width: usize) -> RecordResult<()> {
        match std::mem::take(self) {
            Self::Hits(hits) => {
                for hit in hits {
                    set_bit(words, hit, width, "r8 hit")?;
                }
            }
            Self::Words(source) => {
                for (target, word) in words.iter_mut().zip(source) {
                    *target = word;
                }
            }
        }
        Ok(())
    }
}

/// Sets `bit` in row words that hold only the words up to the last hit, growing them fallibly
/// and geometrically, capped at the `row_words`-word row.
fn set_grown_bit(
    words: &mut Vec<u64>,
    bit: usize,
    width: usize,
    row_words: usize,
) -> RecordResult<()> {
    let needed = bit / WORD_BITS + 1;
    if needed > words.len() {
        if needed > words.capacity() {
            let target = words
                .capacity()
                .saturating_mul(2)
                .clamp(needed, row_words.max(needed));
            words
                .try_reserve_exact(target - words.len())
                .map_err(|error| first_row_error(target, error))?;
        }
        words.resize(needed, 0);
    }
    set_bit(words, bit, width, "r8 hit")
}

fn first_row_error(entries: usize, error: std::collections::TryReserveError) -> FormatError {
    FormatError::invalid_data(format!(
        "r8 reader could not reserve {entries} words for its first record: {error}"
    ))
}

/// Like [`r8_row_bytes`], for the first record, decoded before any batch exists.
pub(super) fn r8_first_row_bytes(
    input: &[u8],
    width: usize,
    bit_index: &mut usize,
    consumed: &mut usize,
    row: &mut FirstR8Row,
) -> RecordResult<bool> {
    resume_r8_record(input, width, bit_index, consumed, |bit| {
        row.record_hit(bit, width)
    })
}

/// The diagnostic for an `r8` record that input ended inside, at absolute input offset `offset`
/// after `bit_index` decoded bits.
pub(super) fn r8_truncated(offset: usize, bit_index: usize, width: usize) -> FormatError {
    r8_truncated_error(offset, bit_index, width)
}

/// Decodes one framed text line into row words; returns whether the line held a record.
///
/// Without `words` the line is only checked against its grammar, which already bounds every
/// index by the declared width.
pub(super) fn text_line(
    line: &[u8],
    format: RecordFormat,
    layout: DetsLayout,
    duplicates: HitsDuplicates,
    mut words: Option<&mut [u64]>,
) -> RecordResult<bool> {
    let width = layout.total_bits();
    let mut produced = 0_usize;
    match format {
        RecordFormat::ZeroOne => {
            if let Some(words) = words.as_deref_mut()
                && pack_zero_one_line(line, width, words)
            {
                return Ok(true);
            }
            for_each_zero_one_line(line, width, |bits| {
                produced += 1;
                if let Some(words) = words.as_deref_mut() {
                    pack_zero_one_bytes(bits, words);
                }
                Ok(())
            })?;
        }
        RecordFormat::Hits => for_each_hits_event(line, width, |event| match event {
            HitsEvent::RecordStart => {
                if let Some(words) = words.as_deref_mut() {
                    words.fill(0);
                }
                Ok(())
            }
            HitsEvent::Hit(index) => {
                let index = usize::try_from(index).map_err(|_| {
                    FormatError::invalid_result_format(format!(
                        "HITS index {index} does not fit usize"
                    ))
                })?;
                let Some(words) = words.as_deref_mut() else {
                    return Ok(());
                };
                match duplicates {
                    HitsDuplicates::Toggle => toggle_bit(words, index, width, "HITS index"),
                    HitsDuplicates::Set => set_bit(words, index, width, "HITS index"),
                }
            }
            HitsEvent::RecordEnd => {
                produced += 1;
                Ok(())
            }
        })?,
        RecordFormat::Dets => for_each_dets_event(line, layout, |event| match event {
            DetsEvent::RecordStart => {
                if let Some(words) = words.as_deref_mut() {
                    words.fill(0);
                }
                Ok(())
            }
            DetsEvent::Token(token) => {
                let index = layout.resolve(token.result_type(), token.index())?;
                match words.as_deref_mut() {
                    Some(words) => set_bit(words, index, width, "DETS token"),
                    None => Ok(()),
                }
            }
            DetsEvent::RecordEnd => {
                produced += 1;
                Ok(())
            }
        })?,
        RecordFormat::B8 | RecordFormat::R8 | RecordFormat::Ptb64 => {
            return Err(FormatError::invalid_result_format(
                "text line decoder requires a text format",
            ));
        }
    }
    match produced {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(FormatError::invalid_data(
            "text record frame decoded into multiple records",
        )),
    }
}

/// Packs a canonical `01` line (exactly `width` bit bytes then `\n` or `\r\n`) eight bytes per
/// step. Returns `false`, leaving the row unspecified, for any line the full grammar must judge.
fn pack_zero_one_line(line: &[u8], width: usize, words: &mut [u64]) -> bool {
    let terminator = match line.len().checked_sub(width) {
        Some(1) => line.get(width..) == Some(b"\n".as_slice()),
        Some(2) => line.get(width..) == Some(b"\r\n".as_slice()),
        _ => false,
    };
    let Some(bits) = line.get(..width) else {
        return false;
    };
    terminator && pack_zero_one_bytes(bits, words)
}

/// Packs `01` bit bytes into row words, returning `false` if any byte is not `0` or `1`.
fn pack_zero_one_bytes(bits: &[u8], words: &mut [u64]) -> bool {
    let mut valid = true;
    let mut targets = words.iter_mut();
    // Zip the source first so exhausting it never consumes an extra target word.
    for (word_bytes, target) in bits.chunks(WORD_BITS).zip(targets.by_ref()) {
        let (chunks, tail) = word_bytes.as_chunks::<WORD_BYTES>();
        let mut word = 0_u64;
        for (byte_index, chunk) in chunks.iter().enumerate() {
            let value = u64::from_le_bytes(*chunk);
            valid &= value & BYTE_HIGH_MASK == ASCII_BIT_HIGH;
            let packed = (value & BYTE_LOW_BITS).wrapping_mul(GATHER_LOW_BITS) >> 56;
            word |= packed << (byte_index * WORD_BYTES);
        }
        let tail_offset = chunks.len() * WORD_BYTES;
        for (bit_index, byte) in tail.iter().enumerate() {
            valid &= matches!(byte, b'0' | b'1');
            word |= u64::from(byte & 1) << (tail_offset + bit_index);
        }
        *target = word;
    }
    for target in targets {
        *target = 0;
    }
    valid
}

/// Reads one PTB64 group into one 64-shot word per result bit without transposing rows.
pub(super) fn ptb64_plane_words(
    frame: &[u8],
    width: usize,
    plane_words: &mut Vec<u64>,
) -> RecordResult<()> {
    plane_words.clear();
    plane_words.try_reserve_exact(width).map_err(|error| {
        FormatError::invalid_data(format!(
            "ptb64 reader could not reserve {width} result planes: {error}"
        ))
    })?;
    plane_words.extend(
        frame
            .as_chunks::<WORD_BYTES>()
            .0
            .iter()
            .map(|word_bytes| u64::from_le_bytes(*word_bytes)),
    );
    Ok(())
}

/// Reads one PTB64 group and transposes it into 64 shot-major rows.
pub(super) fn ptb64_group(frame: &[u8], width: usize, group: &mut Ptb64Group) -> RecordResult<()> {
    ptb64_plane_words(frame, width, &mut group.plane_words)?;
    if group.rows.is_none() {
        group.rows = Some(PackedShotBatch::zeros(WORD_BITS, width)?);
    }
    let rows = group
        .rows
        .as_mut()
        .ok_or_else(|| FormatError::invalid_data("ptb64 group lost its transposed rows"))?;
    rows.copy_from_bit_planes(BitPlaneBatchView::try_from_words(
        &group.plane_words,
        1,
        WORD_BITS,
        width,
    )?)?;
    group.served = 0;
    Ok(())
}

fn set_bit(words: &mut [u64], index: usize, width: usize, kind: &str) -> RecordResult<()> {
    let word = bit_word(words, index, width, kind)?;
    *word |= 1_u64 << (index % WORD_BITS);
    Ok(())
}

fn toggle_bit(words: &mut [u64], index: usize, width: usize, kind: &str) -> RecordResult<()> {
    let word = bit_word(words, index, width, kind)?;
    *word ^= 1_u64 << (index % WORD_BITS);
    Ok(())
}

fn bit_word<'a>(
    words: &'a mut [u64],
    index: usize,
    width: usize,
    kind: &str,
) -> RecordResult<&'a mut u64> {
    if index >= width {
        return Err(FormatError::invalid_result_format(format!(
            "{kind} {index} exceeds record width {width}"
        )));
    }
    words.get_mut(index / WORD_BITS).ok_or_else(|| {
        FormatError::invalid_result_format(format!("{kind} {index} escaped its row"))
    })
}
