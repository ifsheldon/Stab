//! Whole-buffer `Vec<bool>` reference codec for tests.
//!
//! One byte per bit, no memory budget, and the whole input or output held at once: this is the
//! simple shape the streaming [`crate::RecordStreamReader`] and [`crate::RecordEncoder`] are
//! checked against, not a product route. Encoding mirrors Stim's bit-at-a-time
//! `MeasureRecordWriter` (`vendor/stim/src/stim/io/measure_record_writer.cc`) and shares no code
//! with the encoder; decoding drives the shared text grammars over the whole buffer and frames
//! the packed formats on its own, so the stream reader's fast paths, transport refills, and
//! packed framing are checked against an independent framing of the same bytes.

use std::fmt::NumBuffer;

use crate::{
    DetsLayout, FormatError, RecordFormat, RecordResult,
    result_packed::{
        b8_length_multiple_error, b8_record_byte_width, ptb64_bytes_per_group,
        ptb64_length_multiple_error, ptb64_zero_width_count_error, r8_truncated_error,
        resume_r8_record,
    },
    result_text::{DetsEvent, HitsEvent},
};

/// Encodes whole records; PTB64 requires a multiple of 64 records of one width.
pub(crate) fn write_records(records: &[Vec<bool>], format: RecordFormat) -> RecordResult<Vec<u8>> {
    let format = match format {
        RecordFormat::ZeroOne => PerRecordFormat::ZeroOne,
        RecordFormat::B8 => PerRecordFormat::B8,
        RecordFormat::R8 => PerRecordFormat::R8,
        RecordFormat::Hits => PerRecordFormat::Hits,
        RecordFormat::Dets => PerRecordFormat::Dets,
        RecordFormat::Ptb64 => return write_ptb64_records(records),
    };
    let mut writer = BitWriter::new(format);
    for record in records {
        for bit in record {
            writer.write_bit(*bit);
        }
        writer.write_end();
    }
    Ok(writer.output)
}

fn write_ptb64_records(records: &[Vec<bool>]) -> RecordResult<Vec<u8>> {
    if !records.len().is_multiple_of(64) {
        return Err(FormatError::invalid_data(
            "shots must be a multiple of 64 to use ptb64 format",
        ));
    }
    let width = records.first().map_or(0, Vec::len);
    if let Some(index) = records.iter().position(|record| record.len() != width) {
        return Err(FormatError::invalid_result_format(format!(
            "ptb64 record {index} expected {width} bits"
        )));
    }
    let mut output = Vec::new();
    for shot_group in records.as_chunks::<64>().0 {
        for measurement_index in 0..width {
            let mut word = 0u64;
            for (shot_index, shot) in shot_group.iter().enumerate() {
                if shot.get(measurement_index).copied().unwrap_or(false) {
                    word |= 1u64 << shot_index;
                }
            }
            output.extend_from_slice(&word.to_le_bytes());
        }
    }
    Ok(output)
}

/// The formats Stim's writer emits one record at a time.
#[derive(Clone, Copy)]
enum PerRecordFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
}

/// Stim's per-record writer state for one measurement-namespace record at a time.
struct BitWriter {
    format: PerRecordFormat,
    output: Vec<u8>,
    index: usize,
    b8_byte: u8,
    b8_bit_index: u8,
    r8_false_run: u8,
    hits_first: bool,
    dets_started: bool,
}

impl BitWriter {
    const fn new(format: PerRecordFormat) -> Self {
        Self {
            format,
            output: Vec::new(),
            index: 0,
            b8_byte: 0,
            b8_bit_index: 0,
            r8_false_run: 0,
            hits_first: true,
            dets_started: false,
        }
    }

    fn write_bit(&mut self, bit: bool) {
        match self.format {
            PerRecordFormat::ZeroOne => self.output.push(if bit { b'1' } else { b'0' }),
            PerRecordFormat::B8 => {
                if bit {
                    self.b8_byte |= 1u8 << self.b8_bit_index;
                }
                self.b8_bit_index += 1;
                if self.b8_bit_index == 8 {
                    self.output.push(self.b8_byte);
                    self.b8_byte = 0;
                    self.b8_bit_index = 0;
                }
            }
            PerRecordFormat::R8 => {
                if self.r8_false_run == u8::MAX {
                    self.output.push(u8::MAX);
                    self.r8_false_run = 0;
                }
                if bit {
                    self.output.push(self.r8_false_run);
                    self.r8_false_run = 0;
                } else {
                    self.r8_false_run += 1;
                }
            }
            PerRecordFormat::Hits => {
                if bit {
                    if !self.hits_first {
                        self.output.push(b',');
                    }
                    self.hits_first = false;
                    self.push_index();
                }
            }
            PerRecordFormat::Dets => {
                self.ensure_dets_started();
                if bit {
                    self.output.extend_from_slice(b" M");
                    self.push_index();
                }
            }
        }
        self.index += 1;
    }

    fn write_end(&mut self) {
        match self.format {
            PerRecordFormat::ZeroOne | PerRecordFormat::Hits => self.output.push(b'\n'),
            PerRecordFormat::Dets => {
                self.ensure_dets_started();
                self.output.push(b'\n');
            }
            PerRecordFormat::B8 => {
                if self.b8_bit_index != 0 {
                    self.output.push(self.b8_byte);
                }
            }
            PerRecordFormat::R8 => {
                if self.r8_false_run == u8::MAX {
                    self.output.push(u8::MAX);
                    self.r8_false_run = 0;
                }
                self.output.push(self.r8_false_run);
            }
        }
        *self = Self {
            output: std::mem::take(&mut self.output),
            ..Self::new(self.format)
        };
    }

    fn push_index(&mut self) {
        self.output
            .extend_from_slice(self.index.format_into(&mut NumBuffer::new()).as_bytes());
    }

    fn ensure_dets_started(&mut self) {
        if !self.dets_started {
            self.output.extend_from_slice(b"shot");
            self.dets_started = true;
        }
    }
}

/// Decodes a whole buffer of measurement-shaped records.
pub(crate) fn read_records(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
) -> RecordResult<Vec<Vec<bool>>> {
    match format {
        RecordFormat::ZeroOne => {
            let mut records = Vec::new();
            crate::result_text::for_each_zero_one_line(input, bits_per_record, |line| {
                records.push(line.iter().map(|byte| *byte == b'1').collect());
                Ok(())
            })?;
            Ok(records)
        }
        RecordFormat::B8 => read_b8_records(input, bits_per_record),
        RecordFormat::R8 => read_r8_records(input, bits_per_record),
        RecordFormat::Hits => read_hits_records(input, bits_per_record),
        RecordFormat::Dets => {
            read_dets_records(input, DetsLayout::measurement_only(bits_per_record))
        }
        RecordFormat::Ptb64 => read_ptb64_records(input, bits_per_record),
    }
}

/// Decodes a whole buffer of DETS records resolved against a typed namespace layout.
pub(crate) fn read_dets_records(input: &[u8], layout: DetsLayout) -> RecordResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut record = None;
    crate::result_text::for_each_dets_event(input, layout, |event| match event {
        DetsEvent::RecordStart => {
            record = Some(vec![false; layout.total_bits()]);
            Ok(())
        }
        DetsEvent::Token(token) => {
            let index = layout.resolve(token.result_type(), token.index())?;
            let bit = record
                .as_mut()
                .and_then(|record| record.get_mut(index))
                .ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "DETS token resolved beyond the layout's total width",
                    )
                })?;
            *bit = true;
            Ok(())
        }
        DetsEvent::RecordEnd => {
            records.push(record.take().ok_or_else(|| {
                FormatError::invalid_result_format("DETS record ended before it started")
            })?);
            Ok(())
        }
    })?;
    Ok(records)
}

fn read_b8_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    let bytes_per_record = b8_record_byte_width(bits_per_record)?;
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(b8_length_multiple_error(input.len(), bytes_per_record));
    }
    Ok(input
        .chunks_exact(bytes_per_record)
        .map(|chunk| {
            (0..bits_per_record)
                .map(|bit| chunk.get(bit / 8).copied().unwrap_or(0) & (1u8 << (bit % 8)) != 0)
                .collect()
        })
        .collect())
}

/// Decodes the `r8` record starting at `*offset`, reporting each hit; returns `false` only when
/// `*offset` is already at the end of `input`.
pub(crate) fn decode_next_r8_record<F>(
    input: &[u8],
    bits_per_record: usize,
    offset: &mut usize,
    record_hit: F,
) -> RecordResult<bool>
where
    F: FnMut(usize) -> RecordResult<()>,
{
    if *offset == input.len() {
        return Ok(false);
    }
    let mut bit_index = 0usize;
    if resume_r8_record(input, bits_per_record, &mut bit_index, offset, record_hit)? {
        return Ok(true);
    }
    Err(r8_truncated_error(*offset, bit_index, bits_per_record))
}

fn read_r8_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    while offset < input.len() {
        let mut record = vec![false; bits_per_record];
        decode_next_r8_record(input, bits_per_record, &mut offset, |bit_index| {
            let Some(bit) = record.get_mut(bit_index) else {
                return Err(FormatError::invalid_result_format(format!(
                    "r8 hit index {bit_index} exceeds record width {bits_per_record}"
                )));
            };
            *bit = true;
            Ok(())
        })?;
        records.push(record);
    }
    Ok(records)
}

fn read_hits_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut record = None;
    crate::result_text::for_each_hits_event(input, bits_per_record, |event| match event {
        HitsEvent::RecordStart => {
            record = Some(vec![false; bits_per_record]);
            Ok(())
        }
        HitsEvent::Hit(index) => {
            let bit = usize::try_from(index)
                .ok()
                .and_then(|index| record.as_mut()?.get_mut(index))
                .ok_or_else(|| {
                    FormatError::invalid_result_format(format!(
                        "HITS index {index} exceeds record width {bits_per_record}"
                    ))
                })?;
            *bit = !*bit;
            Ok(())
        }
        HitsEvent::RecordEnd => {
            records.push(record.take().ok_or_else(|| {
                FormatError::invalid_result_format("HITS record ended before it started")
            })?);
            Ok(())
        }
    })?;
    Ok(records)
}

fn read_ptb64_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    if bits_per_record == 0 {
        return Err(ptb64_zero_width_count_error());
    }
    let bytes_per_group = ptb64_bytes_per_group(bits_per_record)?;
    if !input.len().is_multiple_of(bytes_per_group) {
        return Err(ptb64_length_multiple_error(input.len(), bytes_per_group));
    }
    let mut records = Vec::new();
    for group in input.chunks_exact(bytes_per_group) {
        let words = group.as_chunks::<8>().0;
        for shot in 0..64 {
            records.push(
                words
                    .iter()
                    .map(|word| u64::from_le_bytes(*word) & (1u64 << shot) != 0)
                    .collect(),
            );
        }
    }
    Ok(records)
}
