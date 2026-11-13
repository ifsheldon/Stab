//! Per-format decoders that write one record straight into packed row words.
//!
//! Every decoder clears or overwrites the whole row; the caller's row guard masks bits beyond the
//! declared width. Text grammar is delegated to the shared text decoders, so diagnostics match the
//! whole-buffer readers exactly; the `01` fast path only accepts lines that grammar would accept
//! unchanged and defers everything else to it.

use super::{HitsDuplicates, Ptb64Group};
use crate::{
    BitPlane64BatchView, DetsLayout, FormatError, PackedShotBatch, RecordFormat, RecordResult,
    result_packed::{
        b8_length_multiple_error, b8_record_byte_width, decode_next_r8_record,
        ptb64_bytes_per_group, ptb64_length_multiple_error, ptb64_zero_width_count_error,
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

/// Validates one complete `r8` frame without materializing it and returns its byte length.
pub(super) fn r8_frame_len(frame: &[u8], width: usize) -> RecordResult<usize> {
    let mut consumed = 0_usize;
    decode_next_r8_record(frame, width, &mut consumed, |_| Ok(()))?;
    Ok(consumed)
}

/// Decodes one `r8` frame into row words, setting only hit bits, and returns its byte length.
pub(super) fn r8_record(frame: &[u8], width: usize, words: &mut [u64]) -> RecordResult<usize> {
    words.fill(0);
    let mut consumed = 0_usize;
    decode_next_r8_record(frame, width, &mut consumed, |bit_index| {
        set_bit(words, bit_index, width, "r8 hit")
    })?;
    Ok(consumed)
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

/// Reads one PTB64 group and transposes it into 64 shot-major rows.
pub(super) fn ptb64_group(frame: &[u8], width: usize, group: &mut Ptb64Group) -> RecordResult<()> {
    group.plane_words.clear();
    group
        .plane_words
        .try_reserve_exact(width)
        .map_err(|error| {
            FormatError::invalid_data(format!(
                "ptb64 reader could not reserve {width} result planes: {error}"
            ))
        })?;
    group.plane_words.extend(
        frame
            .as_chunks::<WORD_BYTES>()
            .0
            .iter()
            .map(|word_bytes| u64::from_le_bytes(*word_bytes)),
    );
    if group.rows.is_none() {
        group.rows = Some(PackedShotBatch::zeros(WORD_BITS, width)?);
    }
    let rows = group
        .rows
        .as_mut()
        .ok_or_else(|| FormatError::invalid_data("ptb64 group lost its transposed rows"))?;
    rows.copy_from_bit_planes(BitPlane64BatchView::try_from_words(
        &group.plane_words,
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
