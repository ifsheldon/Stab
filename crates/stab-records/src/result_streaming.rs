use crate::{
    FormatError, RecordResult, SampleFormat,
    result_formats::{
        DetsLayout, DetsResultType, DetsToken, SparseShot,
        ptb64_record_count as materialized_ptb64_record_count, validate_ptb64_shot_count,
    },
    result_packed::{
        b8_bytes_per_record, decode_next_r8_record, extend_ptb64_group_words,
        fill_record_from_ptb64_words, ptb64_prefix_layout, unpack_b8_chunk_into,
    },
    result_text::{DetsEvent, HitsEvent},
};
use stab_bits::{BitSlice, BitVec};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RecordStreamError<E> {
    /// The encoded input violated the selected record-format contract.
    #[error(transparent)]
    Format(#[from] FormatError),
    /// The visitor requested immediate cancellation by returning its own error.
    #[error("record visitor failed: {0}")]
    Visitor(E),
}

pub fn try_for_each_record<E, F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[bool]) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_record(input, format, bits_per_record, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

pub fn for_each_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_record(input, bits_per_record, visit),
        SampleFormat::Dets => {
            for_each_dets_record(input, DetsLayout::measurement_only(bits_per_record), visit)
        }
    }
}

pub fn for_each_packed_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_packed_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_packed_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_packed_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_packed_record(input, bits_per_record, visit),
        SampleFormat::Dets => {
            for_each_dets_packed_record(input, DetsLayout::measurement_only(bits_per_record), visit)
        }
    }
}

pub fn try_for_each_packed_record<E, F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(BitSlice<'_>) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_packed_record(input, format, bits_per_record, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

pub fn for_each_sparse_record<F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    match format {
        SampleFormat::ZeroOne => for_each_zero_one_sparse_record(input, bits_per_record, visit),
        SampleFormat::B8 => for_each_b8_sparse_record(input, bits_per_record, visit),
        SampleFormat::R8 => for_each_r8_sparse_record(input, bits_per_record, visit),
        SampleFormat::Hits => for_each_hits_sparse_record(input, bits_per_record, visit),
        SampleFormat::Dets => {
            for_each_dets_sparse_record(input, DetsLayout::measurement_only(bits_per_record), visit)
        }
    }
}

pub fn try_for_each_sparse_record<E, F>(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[u64]) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_sparse_record(input, format, bits_per_record, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

pub fn for_each_ptb64_record_all<F>(
    input: &[u8],
    bits_per_record: usize,
    visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let shots = ptb64_record_count(input, bits_per_record)?;
    for_each_ptb64_record(input, bits_per_record, shots, visit)
}

pub fn try_for_each_ptb64_record_all<E, F>(
    input: &[u8],
    bits_per_record: usize,
    visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[bool]) -> Result<(), E>,
{
    let shots = ptb64_record_count(input, bits_per_record)?;
    try_for_each_ptb64_record(input, bits_per_record, shots, visit)
}

pub fn for_each_ptb64_record<F>(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    validate_ptb64_shot_count(max_shots)?;
    if max_shots == 0 {
        return Ok(());
    }
    let (bytes_per_group, expected_bytes) =
        ptb64_prefix_layout(input.len(), bits_per_record, max_shots)?;

    let input = input.get(..expected_bytes).ok_or_else(|| {
        FormatError::invalid_result_format("ptb64 expected byte range was out of bounds")
    })?;
    let mut record = vec![false; bits_per_record];
    let mut words = Vec::with_capacity(bits_per_record);
    for group_bytes in input.chunks_exact(bytes_per_group) {
        extend_ptb64_group_words(group_bytes, &mut words);
        for shot_offset in 0..64 {
            fill_record_from_ptb64_words(&words, shot_offset, &mut record);
            visit(&record)?;
        }
    }
    Ok(())
}

pub fn try_for_each_ptb64_record<E, F>(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[bool]) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_ptb64_record(input, bits_per_record, max_shots, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

fn ptb64_record_count(input: &[u8], bits_per_record: usize) -> RecordResult<usize> {
    materialized_ptb64_record_count(input, bits_per_record)
}

fn for_each_zero_one_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let mut record = vec![false; bits_per_record];
    crate::result_text::for_each_zero_one_line(input, bits_per_record, |line| {
        record.fill(false);
        for (bit, byte) in record.iter_mut().zip(line) {
            *bit = *byte == b'1';
        }
        visit(&record)
    })
}

fn for_each_b8_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let bytes_per_record = b8_bytes_per_record(input.len(), bits_per_record)?;
    let mut record = vec![false; bits_per_record];
    for chunk in input.chunks_exact(bytes_per_record) {
        unpack_b8_chunk_into(chunk, &mut record);
        visit(&record)?;
    }
    Ok(())
}

fn for_each_r8_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let mut record = vec![false; bits_per_record];
    let mut offset = 0usize;
    while offset < input.len() {
        record.fill(false);
        decode_next_r8_record(input, bits_per_record, &mut offset, |bit_index| {
            let Some(bit) = record.get_mut(bit_index) else {
                return Err(FormatError::invalid_result_format(format!(
                    "r8 hit index {bit_index} exceeds record width {bits_per_record}"
                )));
            };
            *bit = true;
            Ok(())
        })?;
        visit(&record)?;
    }
    Ok(())
}

fn for_each_hits_record<F>(input: &[u8], bits_per_record: usize, mut visit: F) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let mut record = vec![false; bits_per_record];
    crate::result_text::for_each_hits_event(input, bits_per_record, |event| match event {
        HitsEvent::RecordStart => {
            record.fill(false);
            Ok(())
        }
        HitsEvent::Hit(index) => {
            let index = usize::try_from(index).map_err(|_| {
                FormatError::invalid_result_format(format!("HITS index {index} does not fit usize"))
            })?;
            let bit = record.get_mut(index).ok_or_else(|| {
                FormatError::invalid_result_format(format!(
                    "HITS index {index} exceeds record width {bits_per_record}"
                ))
            })?;
            *bit = !*bit;
            Ok(())
        }
        HitsEvent::RecordEnd => visit(&record),
    })
}

pub fn for_each_dets_record<F>(input: &[u8], layout: DetsLayout, mut visit: F) -> RecordResult<()>
where
    F: FnMut(&[bool]) -> RecordResult<()>,
{
    let mut record = vec![false; layout.total_bits()];
    crate::result_text::for_each_dets_event(input, layout, |event| match event {
        DetsEvent::RecordStart => {
            record.fill(false);
            Ok(())
        }
        DetsEvent::Token(token) => {
            let index = layout.resolve(token.result_type(), token.index())?;
            let bit = record.get_mut(index).ok_or_else(|| {
                FormatError::invalid_result_format(
                    "DETS token resolved beyond the layout's total width",
                )
            })?;
            *bit = true;
            Ok(())
        }
        DetsEvent::RecordEnd => visit(&record),
    })
}

pub fn try_for_each_dets_record<E, F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[bool]) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_dets_record(input, layout, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

fn for_each_zero_one_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    let mut record = BitVec::zeros(bits_per_record);
    crate::result_text::for_each_zero_one_line(input, bits_per_record, |line| {
        record.clear();
        for (index, byte) in line.iter().enumerate() {
            if *byte == b'1' {
                record.set(index, true).map_err(bit_error_to_format_error)?;
            }
        }
        visit(record.as_bitslice())
    })
}

fn for_each_b8_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    let bytes_per_record = b8_bytes_per_record(input.len(), bits_per_record)?;
    let mut record = BitVec::zeros(bits_per_record);
    for chunk in input.chunks_exact(bytes_per_record) {
        {
            let mut words = record.words_mut();
            unpack_b8_chunk_into_words(chunk, bits_per_record, &mut words);
        }
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_r8_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    let mut record = BitVec::zeros(bits_per_record);
    let mut offset = 0usize;
    while offset < input.len() {
        record.clear();
        decode_next_r8_record(input, bits_per_record, &mut offset, |bit_index| {
            record
                .set(bit_index, true)
                .map_err(bit_error_to_format_error)
        })?;
        visit(record.as_bitslice())?;
    }
    Ok(())
}

fn for_each_hits_packed_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    let mut record = BitVec::zeros(bits_per_record);
    crate::result_text::for_each_hits_event(input, bits_per_record, |event| match event {
        HitsEvent::RecordStart => {
            record.clear();
            Ok(())
        }
        HitsEvent::Hit(index) => {
            let index = usize::try_from(index).map_err(|_| {
                FormatError::invalid_result_format(format!("HITS index {index} does not fit usize"))
            })?;
            let value = record.get(index).ok_or_else(|| {
                FormatError::invalid_result_format(format!(
                    "HITS index {index} exceeds record width {bits_per_record}"
                ))
            })?;
            record
                .set(index, !value)
                .map_err(bit_error_to_format_error)?;
            Ok(())
        }
        HitsEvent::RecordEnd => visit(record.as_bitslice()),
    })
}

pub fn for_each_dets_packed_record<F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(BitSlice<'_>) -> RecordResult<()>,
{
    let mut record = BitVec::zeros(layout.total_bits());
    crate::result_text::for_each_dets_event(input, layout, |event| match event {
        DetsEvent::RecordStart => {
            record.clear();
            Ok(())
        }
        DetsEvent::Token(token) => {
            let index = layout.resolve(token.result_type(), token.index())?;
            record.set(index, true).map_err(bit_error_to_format_error)?;
            Ok(())
        }
        DetsEvent::RecordEnd => visit(record.as_bitslice()),
    })
}

pub fn try_for_each_dets_packed_record<E, F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(BitSlice<'_>) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_dets_packed_record(input, layout, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

fn for_each_zero_one_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    let mut hits = Vec::new();
    crate::result_text::for_each_zero_one_line(input, bits_per_record, |line| {
        hits.clear();
        for (index, byte) in line.iter().enumerate() {
            match byte {
                b'0' => {}
                b'1' => hits.push(u64::try_from(index).map_err(|_| {
                    FormatError::invalid_result_format(format!(
                        "01 hit index {index} does not fit u64"
                    ))
                })?),
                _ => {
                    return Err(FormatError::invalid_result_format(format!(
                        "01 record contains non-bit byte {byte}"
                    )));
                }
            }
        }
        visit(&hits)
    })
}

fn for_each_b8_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    let bytes_per_record = b8_bytes_per_record(input.len(), bits_per_record)?;
    let mut hits = Vec::new();
    for chunk in input.chunks_exact(bytes_per_record) {
        collect_b8_chunk_hits(chunk, bits_per_record, &mut hits)?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_r8_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    let mut hits = Vec::new();
    let mut offset = 0usize;
    while offset < input.len() {
        hits.clear();
        decode_next_r8_record(input, bits_per_record, &mut offset, |bit_index| {
            hits.push(u64::try_from(bit_index).map_err(|_| {
                FormatError::invalid_result_format(format!(
                    "r8 hit index {bit_index} does not fit u64"
                ))
            })?);
            Ok(())
        })?;
        visit(&hits)?;
    }
    Ok(())
}

fn for_each_hits_sparse_record<F>(
    input: &[u8],
    bits_per_record: usize,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    crate::result_text::for_each_hits(input, bits_per_record, |hits| visit(hits))
}

fn for_each_dets_sparse_record<F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[u64]) -> RecordResult<()>,
{
    let mut hits = Vec::new();
    crate::result_text::for_each_dets_event(input, layout, |event| match event {
        DetsEvent::RecordStart => {
            hits.clear();
            Ok(())
        }
        DetsEvent::Token(token) => {
            let index = layout.resolve(token.result_type(), token.index())?;
            hits.push(u64::try_from(index).map_err(|_| {
                FormatError::invalid_result_format(format!(
                    "DETS hit index {index} does not fit u64"
                ))
            })?);
            Ok(())
        }
        DetsEvent::RecordEnd => visit(&hits),
    })
}

pub fn for_each_dets_token_record<F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&[DetsToken]) -> RecordResult<()>,
{
    crate::result_text::for_each_dets_tokens(input, layout, |tokens| visit(tokens))
}

pub fn try_for_each_dets_token_record<E, F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&[DetsToken]) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_dets_token_record(input, layout, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

pub fn for_each_dets_sparse_shot<F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> RecordResult<()>
where
    F: FnMut(&SparseShot) -> RecordResult<()>,
{
    let mut shot = SparseShot::new(Vec::new(), vec![false; layout.observables()]);
    crate::result_text::for_each_dets_event(input, layout, |event| match event {
        DetsEvent::RecordStart => {
            shot.hits.clear();
            shot.obs_mask.fill(false);
            Ok(())
        }
        DetsEvent::Token(token) => {
            match token.result_type() {
                DetsResultType::Measurement | DetsResultType::Detector => {
                    let index = layout.resolve(token.result_type(), token.index())?;
                    shot.hits.push(u64::try_from(index).map_err(|_| {
                        FormatError::invalid_result_format(format!(
                            "DETS hit index {index} does not fit u64"
                        ))
                    })?);
                }
                DetsResultType::Observable => {
                    let bit = shot.obs_mask.get_mut(token.index()).ok_or_else(|| {
                        FormatError::invalid_result_format(
                            "DETS observable resolved beyond the observable mask",
                        )
                    })?;
                    *bit = !*bit;
                }
            }
            Ok(())
        }
        DetsEvent::RecordEnd => visit(&shot),
    })
}

pub fn try_for_each_dets_sparse_shot<E, F>(
    input: &[u8],
    layout: DetsLayout,
    mut visit: F,
) -> Result<(), RecordStreamError<E>>
where
    F: FnMut(&SparseShot) -> Result<(), E>,
{
    let mut visitor_error = None;
    let decode_result = for_each_dets_sparse_shot(input, layout, |record| {
        bridge_visit(&mut visitor_error, || visit(record))
    });
    finish_bridged_visit(decode_result, visitor_error)
}

fn bridge_visit<E>(
    visitor_error: &mut Option<E>,
    visit: impl FnOnce() -> Result<(), E>,
) -> RecordResult<()> {
    match visit() {
        Ok(()) => Ok(()),
        Err(error) => {
            *visitor_error = Some(error);
            Err(FormatError::invalid_result_format("record visitor stopped"))
        }
    }
}

fn finish_bridged_visit<E>(
    decode_result: RecordResult<()>,
    visitor_error: Option<E>,
) -> Result<(), RecordStreamError<E>> {
    match visitor_error {
        Some(error) => Err(RecordStreamError::Visitor(error)),
        None => decode_result.map_err(RecordStreamError::Format),
    }
}

fn unpack_b8_chunk_into_words(chunk: &[u8], bits_per_record: usize, words: &mut [u64]) {
    words.fill(0);
    let mut word_index = 0usize;
    let mut chunks = chunk.chunks_exact(8);
    for word_bytes in chunks.by_ref() {
        let Some(word) = words.get_mut(word_index) else {
            break;
        };
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(word_bytes);
        *word = u64::from_le_bytes(bytes);
        word_index += 1;
    }
    let remainder = chunks.remainder();
    if !remainder.is_empty()
        && let Some(word) = words.get_mut(word_index)
    {
        let mut tail = 0u64;
        for (byte_index, byte) in remainder.iter().enumerate() {
            tail |= u64::from(*byte) << (byte_index * 8);
        }
        *word = tail;
    }
    if let Some(last) = words.last_mut() {
        let tail = bits_per_record % u64::BITS as usize;
        if tail != 0 {
            *last &= (1_u64 << tail) - 1;
        }
    }
}

fn collect_b8_chunk_hits(
    chunk: &[u8],
    bits_per_record: usize,
    hits: &mut Vec<u64>,
) -> RecordResult<()> {
    hits.clear();
    for bit_index in 0..bits_per_record {
        if chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0 {
            hits.push(u64::try_from(bit_index).map_err(|_| {
                FormatError::invalid_result_format(format!(
                    "b8 hit index {bit_index} does not fit u64"
                ))
            })?);
        }
    }
    Ok(())
}

fn bit_error_to_format_error(error: stab_bits::BitError) -> FormatError {
    FormatError::invalid_result_format(error.to_string())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "streaming result-format tests use compact fixture assertions"
    )]

    use crate::result_formats::{
        read_ptb64_records_all, read_records, write_ptb64_records_checked, write_records,
    };

    use super::*;

    #[test]
    fn streaming_readers_match_materialized_readers() {
        let records = vec![
            vec![true, false, true, false, false, true, false, false, true],
            vec![false, true, false, true, false, false, true, false, false],
            vec![
                false, false, false, false, false, false, false, false, false,
            ],
        ];

        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let input = write_records(&records, format);
            let mut streamed = Vec::new();
            for_each_record(&input, format, 9, |record| {
                streamed.push(record.to_vec());
                Ok(())
            })
            .unwrap();
            assert_eq!(streamed, read_records(&input, format, 9).unwrap());
        }
    }

    #[test]
    fn packed_and_sparse_streaming_readers_match_materialized_readers() {
        let records = vec![
            vec![true, false, true, false, false, true, false, false, true],
            vec![false, true, false, true, false, false, true, false, false],
            vec![
                false, false, false, false, false, false, false, false, false,
            ],
        ];

        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let input = write_records(&records, format);
            let expected = read_records(&input, format, 9).unwrap();
            let mut packed = Vec::new();
            for_each_packed_record(&input, format, 9, |record| {
                packed.push(bitslice_to_vec(record));
                Ok(())
            })
            .unwrap();
            assert_eq!(packed, expected);

            let mut sparse = Vec::new();
            for_each_sparse_record(&input, format, 9, |hits| {
                sparse.push(sparse_hits_to_vec(hits, 9));
                Ok(())
            })
            .unwrap();
            assert_eq!(sparse, expected);
        }
    }

    #[test]
    fn streaming_ptb64_reader_matches_materialized_reader() {
        let records = (0..64)
            .map(|shot_index| {
                (0..17)
                    .map(|bit_index| (shot_index * 7 + bit_index * 11) % 13 == 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let input = write_ptb64_records_checked(&records).expect("write ptb64");
        let mut streamed = Vec::new();

        for_each_ptb64_record_all(&input, 17, |record| {
            streamed.push(record.to_vec());
            Ok(())
        })
        .unwrap();

        assert_eq!(streamed, read_ptb64_records_all(&input, 17).unwrap());

        let mut limited = Vec::new();
        for_each_ptb64_record(&input, 17, 64, |record| {
            limited.push(record.to_vec());
            Ok(())
        })
        .unwrap();
        assert_eq!(limited, streamed);
    }

    #[test]
    fn streaming_readers_reject_malformed_inputs() {
        assert!(for_each_record(b"0x\n", SampleFormat::ZeroOne, 2, ignore_record).is_err());
        assert!(for_each_record(&[0xff], SampleFormat::B8, 9, ignore_record).is_err());
        assert!(for_each_record(&[3], SampleFormat::R8, 2, ignore_record).is_err());
        assert!(for_each_record(b"3\n", SampleFormat::Hits, 3, ignore_record).is_err());
        assert!(for_each_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_record).is_err());
        assert!(
            for_each_packed_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_packed).is_err()
        );
        assert!(
            for_each_sparse_record(b"shot Q0\n", SampleFormat::Dets, 1, ignore_sparse).is_err()
        );
        assert!(for_each_ptb64_record_all(&[0; 7], 1, ignore_record).is_err());
        assert!(for_each_ptb64_record(&[], 0, 64, ignore_record).is_err());
    }

    #[test]
    fn streaming_readers_stop_on_visitor_error() {
        let records = vec![vec![true, false], vec![false, true]];
        let input = write_records(&records, SampleFormat::ZeroOne);
        let mut visited = 0usize;

        let result = for_each_record(&input, SampleFormat::ZeroOne, 2, |_| {
            visited += 1;
            Err(FormatError::invalid_result_format("visitor stopped"))
        });

        assert!(result.is_err());
        assert_eq!(visited, 1);
    }

    #[test]
    fn generic_streaming_preserves_the_first_visitor_error() {
        let dense = b"00\n11\n";
        let sparse = b"0\n1\n";
        let dets = b"shot M0 D0 L0\nshot M1 D1 L1\n";
        let layout = DetsLayout::try_new(2, 2, 2).expect("DETS layout");
        let ptb64_records = (0usize..64)
            .map(|shot| vec![shot.is_multiple_of(2), shot.is_multiple_of(3)])
            .collect::<Vec<_>>();
        let ptb64 = crate::write_ptb64_records_checked(&ptb64_records).expect("PTB64 fixture");

        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_record(dense, SampleFormat::ZeroOne, 2, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_packed_record(dense, SampleFormat::ZeroOne, 2, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_sparse_record(sparse, SampleFormat::Hits, 2, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_ptb64_record_all(&ptb64, 2, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_ptb64_record(&ptb64, 2, 64, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_dets_record(dets, layout, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_dets_packed_record(dets, layout, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_dets_token_record(dets, layout, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );
        let mut visited = 0usize;
        assert_visitor_error(
            try_for_each_dets_sparse_shot(dets, layout, |_| {
                visited += 1;
                Err("sink stopped")
            }),
            visited,
        );

        assert!(matches!(
            try_for_each_record(
                b"0x\n",
                SampleFormat::ZeroOne,
                2,
                |_| -> Result<(), &'static str> { Ok(()) }
            ),
            Err(RecordStreamError::Format(_))
        ));
    }

    fn assert_visitor_error(result: Result<(), RecordStreamError<&'static str>>, visited: usize) {
        assert!(matches!(
            result,
            Err(RecordStreamError::Visitor("sink stopped"))
        ));
        assert_eq!(visited, 1);
    }

    fn ignore_record(_: &[bool]) -> RecordResult<()> {
        Ok(())
    }

    fn ignore_packed(_: BitSlice<'_>) -> RecordResult<()> {
        Ok(())
    }

    fn ignore_sparse(_: &[u64]) -> RecordResult<()> {
        Ok(())
    }

    fn bitslice_to_vec(record: BitSlice<'_>) -> Vec<bool> {
        (0..record.len())
            .map(|index| record.get(index).unwrap())
            .collect()
    }

    fn sparse_hits_to_vec(hits: &[u64], bits_per_record: usize) -> Vec<bool> {
        let mut record = vec![false; bits_per_record];
        for hit in hits {
            *record
                .get_mut(usize::try_from(*hit).unwrap())
                .expect("sparse hit is in range") = true;
        }
        record
    }
}
