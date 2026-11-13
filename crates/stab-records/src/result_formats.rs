use crate::{
    BitPlane64BatchView, FormatError, RecordResult,
    result_packed::{
        b8_bytes_per_record, decode_next_r8_record, ptb64_prefix_layout,
        ptb64_record_count as packed_ptb64_record_count,
    },
    result_text::HitsEvent,
};

mod capabilities;
mod dets;

pub use capabilities::codec_capabilities;
pub use capabilities::{CodecCapability, RecordEncoding, RecordFormat};
pub(crate) use dets::DetsToken;
pub use dets::{DetsLayout, DetsResultType, read_dets_records};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PerRecordFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
}

impl TryFrom<RecordFormat> for PerRecordFormat {
    type Error = FormatError;

    fn try_from(format: RecordFormat) -> RecordResult<Self> {
        match format {
            RecordFormat::ZeroOne => Ok(Self::ZeroOne),
            RecordFormat::B8 => Ok(Self::B8),
            RecordFormat::R8 => Ok(Self::R8),
            RecordFormat::Hits => Ok(Self::Hits),
            RecordFormat::Dets => Ok(Self::Dets),
            RecordFormat::Ptb64 => Err(FormatError::invalid_data(
                "ptb64 requires a complete 64-record group",
            )),
        }
    }
}

pub fn write_records(records: &[Vec<bool>], format: RecordFormat) -> RecordResult<Vec<u8>> {
    if format == RecordFormat::Ptb64 {
        return write_ptb64_records_checked(records);
    }
    let mut writer = MeasureRecordWriter::try_new(format)?;
    for record in records {
        writer.write_bits(record);
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

fn write_ptb64_records(records: &[Vec<bool>]) -> Vec<u8> {
    let mut output = Vec::new();
    for shot_group in records.as_chunks::<64>().0 {
        let bits_per_shot = shot_group.first().map_or(0, Vec::len);
        for measurement_index in 0..bits_per_shot {
            let mut word = 0u64;
            for (shot_index, shot) in shot_group.iter().enumerate() {
                if shot.get(measurement_index).copied().unwrap_or(false) {
                    word |= 1u64 << shot_index;
                }
            }
            output.extend_from_slice(&word.to_le_bytes());
        }
    }
    output
}

pub fn write_ptb64_records_checked(records: &[Vec<bool>]) -> RecordResult<Vec<u8>> {
    validate_ptb64_shot_count(records.len())?;
    validate_uniform_record_width(records, "ptb64")?;
    Ok(write_ptb64_records(records))
}

pub fn write_bit_plane_64_batch(batch: BitPlane64BatchView<'_>) -> RecordResult<Vec<u8>> {
    validate_ptb64_shot_count(batch.shot_count())?;
    if batch.shot_count() == 0 {
        return Ok(Vec::new());
    }
    let byte_capacity = batch
        .bits_per_shot()
        .checked_mul(size_of::<u64>())
        .ok_or_else(|| FormatError::invalid_result_format("ptb64 output size overflowed"))?;
    let mut output = Vec::with_capacity(byte_capacity);
    for bit_index in 0..batch.bits_per_shot() {
        let plane = batch.plane(bit_index)?;
        let word = plane.words().first().copied().ok_or_else(|| {
            FormatError::invalid_result_format(
                "64-shot bit plane did not contain its required storage word",
            )
        })?;
        output.extend_from_slice(&word.to_le_bytes());
    }
    Ok(output)
}

pub fn read_ptb64_records(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
) -> RecordResult<Vec<Vec<bool>>> {
    validate_ptb64_shot_count(max_shots)?;
    if max_shots == 0 {
        return Ok(Vec::new());
    }
    let shot_groups = max_shots / 64;
    let (bytes_per_group, expected_bytes) =
        ptb64_prefix_layout(input.len(), bits_per_record, max_shots)?;

    let input = input.get(..expected_bytes).ok_or_else(|| {
        FormatError::invalid_result_format("ptb64 expected byte range was out of bounds")
    })?;
    let mut records = vec![vec![false; bits_per_record]; max_shots];
    for (group_records, group_bytes) in records
        .as_chunks_mut::<64>()
        .0
        .iter_mut()
        .zip(input.chunks_exact(bytes_per_group))
        .take(shot_groups)
    {
        for (bit_index, word_bytes) in group_bytes.as_chunks::<8>().0.iter().enumerate() {
            let word = u64::from_le_bytes(*word_bytes);
            for (shot_offset, record) in group_records.iter_mut().enumerate() {
                if word & (1u64 << shot_offset) != 0 {
                    let bit = record.get_mut(bit_index).ok_or_else(|| {
                        FormatError::invalid_result_format(
                            "ptb64 bit index was out of decoded record bounds",
                        )
                    })?;
                    *bit = true;
                }
            }
        }
    }
    Ok(records)
}

pub fn read_ptb64_records_all(
    input: &[u8],
    bits_per_record: usize,
) -> RecordResult<Vec<Vec<bool>>> {
    let shots = ptb64_record_count(input, bits_per_record)?;
    read_ptb64_records(input, bits_per_record, shots)
}

pub fn ptb64_record_count(input: &[u8], bits_per_record: usize) -> RecordResult<usize> {
    packed_ptb64_record_count(input.len(), bits_per_record)
}

pub fn validate_ptb64_shot_count(shots: usize) -> RecordResult<()> {
    if !shots.is_multiple_of(64) {
        return Err(FormatError::invalid_data(
            "shots must be a multiple of 64 to use ptb64 format",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecordWriter {
    format: PerRecordFormat,
    output: Vec<u8>,
    index: usize,
    b8_byte: u8,
    b8_bit_index: u8,
    r8_false_run: u8,
    hits_first: bool,
    dets_started: bool,
    dets_type: u8,
}

impl MeasureRecordWriter {
    pub fn try_new(format: RecordFormat) -> RecordResult<Self> {
        Ok(Self {
            format: PerRecordFormat::try_from(format)?,
            output: Vec::new(),
            index: 0,
            b8_byte: 0,
            b8_bit_index: 0,
            r8_false_run: 0,
            hits_first: true,
            dets_started: false,
            dets_type: b'M',
        })
    }

    pub fn begin_dets_result_type(&mut self, result_type: DetsResultType) {
        self.begin_result_type(result_type.prefix());
    }

    /// Selects a raw DETS namespace prefix for compatibility adapters.
    ///
    /// Mirroring upstream Stim's writer contract
    /// (`vendor/stim/src/stim/io/measure_record_writer.cc`), this is a no-op on every
    /// non-DETS writer: only the DETS writer overrides `begin_result_type`, so calling it on a
    /// HITS writer must not reset the in-record bit position.
    ///
    /// New component code should use [`Self::begin_dets_result_type`] so invalid prefixes are not
    /// representable.
    pub fn begin_result_type(&mut self, result_type: u8) {
        if self.format != PerRecordFormat::Dets {
            return;
        }
        self.dets_type = result_type;
        self.index = 0;
    }

    pub fn write_bits(&mut self, bits: &[bool]) {
        for bit in bits {
            self.write_bit(*bit);
        }
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            for bit_index in 0..8 {
                self.write_bit(byte & (1u8 << bit_index) != 0);
            }
        }
    }

    pub fn write_bit(&mut self, bit: bool) {
        match self.format {
            PerRecordFormat::ZeroOne => {
                self.output.push(if bit { b'1' } else { b'0' });
            }
            PerRecordFormat::B8 => self.write_b8_bit(bit),
            PerRecordFormat::R8 => self.write_r8_bit(bit),
            PerRecordFormat::Hits => self.write_hits_bit(bit),
            PerRecordFormat::Dets => self.write_dets_bit(bit),
        }
        self.index += 1;
    }

    pub fn write_end(&mut self) {
        match self.format {
            PerRecordFormat::ZeroOne | PerRecordFormat::Hits => {
                self.output.push(b'\n');
            }
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
        self.index = 0;
        self.b8_byte = 0;
        self.b8_bit_index = 0;
        self.r8_false_run = 0;
        self.hits_first = true;
        self.dets_started = false;
        self.dets_type = b'M';
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.output
    }

    fn write_b8_bit(&mut self, bit: bool) {
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

    fn write_r8_bit(&mut self, bit: bool) {
        if bit {
            if self.r8_false_run == u8::MAX {
                self.output.push(u8::MAX);
                self.r8_false_run = 0;
            }
            self.output.push(self.r8_false_run);
            self.r8_false_run = 0;
            return;
        }
        if self.r8_false_run == u8::MAX {
            self.output.push(u8::MAX);
            self.r8_false_run = 0;
        }
        self.r8_false_run += 1;
    }

    fn write_hits_bit(&mut self, bit: bool) {
        if !bit {
            return;
        }
        if !self.hits_first {
            self.output.push(b',');
        }
        self.hits_first = false;
        append_usize_decimal(&mut self.output, self.index);
    }

    fn write_dets_bit(&mut self, bit: bool) {
        self.ensure_dets_started();
        if !bit {
            return;
        }
        self.output.push(b' ');
        self.output.push(self.dets_type);
        append_usize_decimal(&mut self.output, self.index);
    }

    fn ensure_dets_started(&mut self) {
        if !self.dets_started {
            self.output.extend_from_slice(b"shot");
            self.dets_started = true;
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "value modulo 10 is always representable as u8"
)]
fn append_usize_decimal(output: &mut Vec<u8>, mut value: usize) {
    let mut digits = [0_u8; size_of::<usize>() * 3];
    let mut used = 0_usize;
    for digit in digits.iter_mut().rev() {
        *digit = b'0' + (value % 10) as u8;
        used += 1;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    let start = digits.len() - used;
    output.extend(digits.into_iter().skip(start));
}

pub fn read_records(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
) -> RecordResult<Vec<Vec<bool>>> {
    match format {
        RecordFormat::ZeroOne => read_zero_one_records(input, bits_per_record),
        RecordFormat::B8 => read_b8_records(input, bits_per_record),
        RecordFormat::R8 => read_r8_records(input, bits_per_record),
        RecordFormat::Hits => read_hits_records(input, bits_per_record),
        RecordFormat::Dets => {
            read_dets_records(input, DetsLayout::measurement_only(bits_per_record))
        }
        RecordFormat::Ptb64 => read_ptb64_records_all(input, bits_per_record),
    }
}

pub fn read_measurement_records(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
) -> RecordResult<Vec<Vec<bool>>> {
    read_records(input, format, bits_per_record)
}

fn read_zero_one_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    crate::result_text::for_each_zero_one_line(input, bits_per_record, |line| {
        let record = line.iter().map(|byte| *byte == b'1').collect();
        records.push(record);
        Ok(())
    })?;
    Ok(records)
}

fn read_b8_records(input: &[u8], bits_per_record: usize) -> RecordResult<Vec<Vec<bool>>> {
    let bytes_per_record = b8_bytes_per_record(input.len(), bits_per_record)?;
    input
        .chunks_exact(bytes_per_record)
        .map(|chunk| Ok(unpack_b8_chunk(chunk, bits_per_record)))
        .collect()
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
            let index = usize::try_from(index).map_err(|_| {
                FormatError::invalid_result_format(format!("HITS index {index} does not fit usize"))
            })?;
            let bit = record
                .as_mut()
                .and_then(|record| record.get_mut(index))
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

fn unpack_b8_chunk(chunk: &[u8], bits_per_record: usize) -> Vec<bool> {
    (0..bits_per_record)
        .map(|bit_index| {
            chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0
        })
        .collect()
}

fn validate_uniform_record_width(records: &[Vec<bool>], kind: &'static str) -> RecordResult<()> {
    let Some(first) = records.first() else {
        return Ok(());
    };
    let expected = first.len();
    for (index, record) in records.iter().enumerate() {
        if record.len() != expected {
            return Err(FormatError::invalid_result_format(format!(
                "{kind} record {index} expected {expected} bits, got {}",
                record.len()
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
