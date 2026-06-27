use crate::{CircuitError, CircuitResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
}

pub fn write_records(records: &[Vec<bool>], format: SampleFormat) -> Vec<u8> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in records {
        writer.write_bits(record);
        writer.write_end();
    }
    writer.into_bytes()
}

pub fn write_ptb64_records(records: &[Vec<bool>]) -> Vec<u8> {
    let mut output = Vec::new();
    for shot_group in records.chunks_exact(64) {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecord {
    storage: Vec<bool>,
    unwritten_start: usize,
    max_lookback: usize,
}

impl MeasureRecord {
    pub fn new(max_lookback: usize) -> Self {
        Self {
            storage: Vec::new(),
            unwritten_start: 0,
            max_lookback,
        }
    }

    pub fn record_result(&mut self, value: bool) {
        self.storage.push(value);
    }

    pub fn lookback(&self, lookback: usize) -> Option<bool> {
        if lookback == 0 || lookback > self.storage.len() {
            return None;
        }
        self.storage.get(self.storage.len() - lookback).copied()
    }

    pub fn storage_len(&self) -> usize {
        self.storage.len()
    }

    pub fn write_unwritten_results_to(
        &mut self,
        writer: &mut MeasureRecordWriter,
    ) -> CircuitResult<()> {
        let unwritten = self.storage.get(self.unwritten_start..).ok_or_else(|| {
            CircuitError::invalid_result_format("measure record unwritten cursor is out of range")
        })?;
        for bit in unwritten {
            writer.write_bit(*bit);
        }
        self.unwritten_start = self.storage.len();
        self.compact_written_prefix();
        Ok(())
    }

    fn compact_written_prefix(&mut self) {
        let keep = self.max_lookback.min(self.storage.len());
        let remove = self.storage.len() - keep;
        if remove == 0 {
            return;
        }
        self.storage.drain(..remove);
        self.unwritten_start = self.unwritten_start.saturating_sub(remove);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecordBatch {
    records: Vec<Vec<bool>>,
    unwritten_start: usize,
    max_lookback: usize,
    shot_count: usize,
}

impl MeasureRecordBatch {
    pub fn new(shot_count: usize, max_lookback: usize) -> Self {
        Self {
            records: Vec::new(),
            unwritten_start: 0,
            max_lookback,
            shot_count,
        }
    }

    pub fn stored(&self) -> usize {
        self.records.len()
    }

    pub fn unwritten(&self) -> usize {
        self.records.len() - self.unwritten_start
    }

    pub fn record_result(&mut self, shot_bits: Vec<bool>) -> CircuitResult<()> {
        if shot_bits.len() != self.shot_count {
            return Err(CircuitError::invalid_result_format(format!(
                "batch record expected {} shot bits, got {}",
                self.shot_count,
                shot_bits.len()
            )));
        }
        self.records.push(shot_bits);
        Ok(())
    }

    pub fn record_zero_result_to_edit(&mut self) -> &mut [bool] {
        self.records.push(vec![false; self.shot_count]);
        match self.records.last_mut() {
            Some(record) => record.as_mut_slice(),
            None => unreachable!("record_zero_result_to_edit just pushed a record"),
        }
    }

    pub fn lookback(&self, lookback: usize) -> Option<&[bool]> {
        if lookback == 0 || lookback > self.records.len() {
            return None;
        }
        self.records
            .get(self.records.len() - lookback)
            .map(Vec::as_slice)
    }

    pub fn intermediate_write_unwritten_results_to(
        &self,
        writer: &mut MeasureRecordBatchWriter,
        reference_sample: &[bool],
    ) -> CircuitResult<()> {
        let unwritten = self.records.get(self.unwritten_start..).ok_or_else(|| {
            CircuitError::invalid_result_format(
                "measure record batch unwritten cursor is out of range",
            )
        })?;
        for record in unwritten {
            writer.batch_write_bit(&xor_reference(record, reference_sample)?)?;
        }
        Ok(())
    }

    pub fn final_write_unwritten_results_to(
        &mut self,
        writer: &mut MeasureRecordBatchWriter,
        reference_sample: &[bool],
    ) -> CircuitResult<()> {
        self.intermediate_write_unwritten_results_to(writer, reference_sample)?;
        self.unwritten_start = self.records.len();
        self.compact_written_prefix();
        Ok(())
    }

    fn compact_written_prefix(&mut self) {
        let keep = self.max_lookback.min(self.records.len());
        let remove = self.records.len() - keep;
        if remove == 0 {
            return;
        }
        self.records.drain(..remove);
        self.unwritten_start = self.unwritten_start.saturating_sub(remove);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecordWriter {
    format: SampleFormat,
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
    pub fn new(format: SampleFormat) -> Self {
        Self {
            format,
            output: Vec::new(),
            index: 0,
            b8_byte: 0,
            b8_bit_index: 0,
            r8_false_run: 0,
            hits_first: true,
            dets_started: false,
            dets_type: b'M',
        }
    }

    pub fn begin_result_type(&mut self, result_type: u8) {
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
            SampleFormat::ZeroOne => {
                self.output.push(if bit { b'1' } else { b'0' });
            }
            SampleFormat::B8 => self.write_b8_bit(bit),
            SampleFormat::R8 => self.write_r8_bit(bit),
            SampleFormat::Hits => self.write_hits_bit(bit),
            SampleFormat::Dets => self.write_dets_bit(bit),
        }
        self.index += 1;
    }

    pub fn write_end(&mut self) {
        match self.format {
            SampleFormat::ZeroOne | SampleFormat::Hits => {
                self.output.push(b'\n');
            }
            SampleFormat::Dets => {
                self.ensure_dets_started();
                self.output.push(b'\n');
            }
            SampleFormat::B8 => {
                if self.b8_bit_index != 0 {
                    self.output.push(self.b8_byte);
                }
            }
            SampleFormat::R8 => {
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
        self.output
            .extend_from_slice(self.index.to_string().as_bytes());
    }

    fn write_dets_bit(&mut self, bit: bool) {
        self.ensure_dets_started();
        if !bit {
            return;
        }
        self.output.push(b' ');
        self.output.push(self.dets_type);
        self.output
            .extend_from_slice(self.index.to_string().as_bytes());
    }

    fn ensure_dets_started(&mut self) {
        if !self.dets_started {
            self.output.extend_from_slice(b"shot");
            self.dets_started = true;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecordBatchWriter {
    format: SampleFormat,
    records: Vec<Vec<bool>>,
}

impl MeasureRecordBatchWriter {
    pub fn new(shots: usize, format: SampleFormat) -> Self {
        Self {
            format,
            records: vec![Vec::new(); shots],
        }
    }

    pub fn batch_write_bit(&mut self, shot_bits: &[bool]) -> CircuitResult<()> {
        if shot_bits.len() != self.records.len() {
            return Err(CircuitError::invalid_result_format(format!(
                "batch writer expected {} shot bits, got {}",
                self.records.len(),
                shot_bits.len()
            )));
        }
        for (record, bit) in self.records.iter_mut().zip(shot_bits) {
            record.push(*bit);
        }
        Ok(())
    }

    pub fn write_end(&self) -> Vec<u8> {
        write_records(&self.records, self.format)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SparseShot {
    pub hits: Vec<u64>,
    pub obs_mask: Vec<bool>,
}

impl SparseShot {
    pub fn new(hits: Vec<u64>, obs_mask: Vec<bool>) -> Self {
        Self { hits, obs_mask }
    }

    pub fn obs_mask_as_u64(&self) -> u64 {
        self.obs_mask
            .iter()
            .take(64)
            .enumerate()
            .fold(
                0u64,
                |acc, (index, bit)| {
                    if *bit { acc | (1u64 << index) } else { acc }
                },
            )
    }

    pub fn stim_debug_string(&self) -> String {
        let hits = self
            .hits
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let obs_mask = self
            .obs_mask
            .iter()
            .map(|bit| if *bit { '1' } else { '_' })
            .collect::<String>();
        format!("SparseShot{{{{{hits}}}, {obs_mask}}}")
    }
}

pub fn read_records(
    input: &[u8],
    format: SampleFormat,
    bits_per_record: usize,
) -> CircuitResult<Vec<Vec<bool>>> {
    match format {
        SampleFormat::ZeroOne => read_zero_one_records(input, bits_per_record),
        SampleFormat::B8 => read_b8_records(input, bits_per_record),
        SampleFormat::R8 => read_r8_records(input, bits_per_record),
        SampleFormat::Hits => read_hits_records(input, bits_per_record),
        SampleFormat::Dets => read_dets_records(input, bits_per_record),
    }
}

fn read_zero_one_records(input: &[u8], bits_per_record: usize) -> CircuitResult<Vec<Vec<bool>>> {
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    text.split_terminator('\n')
        .map(|line| {
            if line.len() != bits_per_record {
                return Err(CircuitError::invalid_result_format(format!(
                    "01 record expected {bits_per_record} bits, got {}",
                    line.len()
                )));
            }
            line.bytes()
                .map(|byte| match byte {
                    b'0' => Ok(false),
                    b'1' => Ok(true),
                    _ => Err(CircuitError::invalid_result_format(format!(
                        "01 record contains non-bit byte {byte}"
                    ))),
                })
                .collect()
        })
        .collect()
}

fn read_b8_records(input: &[u8], bits_per_record: usize) -> CircuitResult<Vec<Vec<bool>>> {
    let bytes_per_record = bits_per_record.div_ceil(8);
    if bytes_per_record == 0 {
        return if input.is_empty() {
            Ok(Vec::new())
        } else {
            Err(CircuitError::invalid_result_format(
                "b8 input has bytes for zero-width records",
            ))
        };
    }
    if !input.len().is_multiple_of(bytes_per_record) {
        return Err(CircuitError::invalid_result_format(format!(
            "b8 input length {} is not a multiple of record byte width {bytes_per_record}",
            input.len()
        )));
    }
    input
        .chunks_exact(bytes_per_record)
        .map(|chunk| Ok(unpack_b8_chunk(chunk, bits_per_record)))
        .collect()
}

fn read_r8_records(input: &[u8], bits_per_record: usize) -> CircuitResult<Vec<Vec<bool>>> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    while offset < input.len() {
        let mut record = vec![false; bits_per_record];
        let mut bit_index = 0usize;
        loop {
            let byte = *input.get(offset).ok_or_else(|| {
                CircuitError::invalid_result_format("r8 input ended before record completed")
            })?;
            offset += 1;
            if byte == u8::MAX {
                bit_index += usize::from(u8::MAX);
                if bit_index > bits_per_record {
                    return Err(CircuitError::invalid_result_format(
                        "r8 run-length overshot record width",
                    ));
                }
                continue;
            }
            bit_index += usize::from(byte);
            if bit_index > bits_per_record {
                return Err(CircuitError::invalid_result_format(
                    "r8 run-length overshot record width",
                ));
            }
            if bit_index == bits_per_record {
                break;
            }
            let Some(bit) = record.get_mut(bit_index) else {
                return Err(CircuitError::invalid_result_format(format!(
                    "r8 hit index {bit_index} exceeds record width {bits_per_record}"
                )));
            };
            *bit = true;
            bit_index += 1;
        }
        records.push(record);
    }
    Ok(records)
}

fn read_hits_records(input: &[u8], bits_per_record: usize) -> CircuitResult<Vec<Vec<bool>>> {
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    text.split_terminator('\n')
        .map(|line| read_sparse_index_line(line, bits_per_record, false))
        .collect()
}

fn read_dets_records(input: &[u8], bits_per_record: usize) -> CircuitResult<Vec<Vec<bool>>> {
    let text = std::str::from_utf8(input)
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))?;
    text.split_terminator('\n')
        .map(|line| {
            let Some(rest) = line.strip_prefix("shot") else {
                return Err(CircuitError::invalid_result_format(format!(
                    "dets record does not start with shot: {line:?}"
                )));
            };
            read_sparse_index_line(rest.trim(), bits_per_record, true)
        })
        .collect()
}

fn read_sparse_index_line(
    line: &str,
    bits_per_record: usize,
    dets_tokens: bool,
) -> CircuitResult<Vec<bool>> {
    let mut record = vec![false; bits_per_record];
    if line.is_empty() {
        return Ok(record);
    }
    for token in line.split(if dets_tokens { ' ' } else { ',' }) {
        if token.is_empty() {
            continue;
        }
        let index = if dets_tokens {
            let mut chars = token.chars();
            if !matches!(chars.next(), Some('M' | 'D' | 'L')) {
                return Err(CircuitError::invalid_result_format(format!(
                    "invalid dets token {token:?}"
                )));
            }
            parse_sparse_index(chars.as_str())?
        } else {
            parse_sparse_index(token)?
        };
        let index = usize::try_from(index).map_err(|_| {
            CircuitError::invalid_result_format(format!("sparse index {index} does not fit usize"))
        })?;
        let Some(bit) = record.get_mut(index) else {
            return Err(CircuitError::invalid_result_format(format!(
                "sparse index {index} exceeds record width {bits_per_record}"
            )));
        };
        *bit = true;
    }
    Ok(record)
}

fn parse_sparse_index(text: &str) -> CircuitResult<u64> {
    text.parse::<u64>()
        .map_err(|error| CircuitError::invalid_result_format(error.to_string()))
}

fn unpack_b8_chunk(chunk: &[u8], bits_per_record: usize) -> Vec<bool> {
    (0..bits_per_record)
        .map(|bit_index| {
            chunk.get(bit_index / 8).copied().unwrap_or(0) & (1u8 << (bit_index % 8)) != 0
        })
        .collect()
}

fn xor_reference(record: &[bool], reference_sample: &[bool]) -> CircuitResult<Vec<bool>> {
    if record.len() != reference_sample.len() {
        return Err(CircuitError::invalid_result_format(format!(
            "reference sample length {} does not match record length {}",
            reference_sample.len(),
            record.len()
        )));
    }
    Ok(record
        .iter()
        .zip(reference_sample)
        .map(|(bit, reference)| bit ^ reference)
        .collect())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "result-format unit tests use direct fixture assertions for compact diagnostics"
    )]

    use super::*;

    #[test]
    fn measure_record_records_lookback_and_writes_unwritten_results() {
        let mut record = MeasureRecord::new(20);
        record.record_result(true);
        assert_eq!(record.lookback(1), Some(true));
        record.record_result(false);
        assert_eq!(record.lookback(1), Some(false));
        assert_eq!(record.lookback(2), Some(true));
        for _ in 0..50 {
            record.record_result(true);
            record.record_result(false);
        }
        assert_eq!(record.storage_len(), 102);

        let mut writer = MeasureRecordWriter::new(SampleFormat::ZeroOne);
        record
            .write_unwritten_results_to(&mut writer)
            .expect("write unwritten results");
        assert_eq!(
            writer.into_bytes(),
            (0..102)
                .map(|index| if index % 2 == 0 { b'1' } else { b'0' })
                .collect::<Vec<_>>()
        );
        assert!(record.storage_len() <= 40);
    }

    #[test]
    fn measure_record_writer_matches_stim_byte_layouts() {
        let bytes = [0xF8];

        let mut writer = MeasureRecordWriter::new(SampleFormat::ZeroOne);
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.write_bytes(&bytes);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(writer.into_bytes(), b"000111110000111111\n");

        let mut writer = MeasureRecordWriter::new(SampleFormat::B8);
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.write_bytes(&bytes);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(writer.into_bytes(), [0xF8, 0xF0, 0x03]);

        let mut writer = MeasureRecordWriter::new(SampleFormat::Hits);
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.write_bytes(&bytes);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(writer.into_bytes(), b"3,4,5,6,7,12,13,14,15,16,17\n");

        let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
        writer.begin_result_type(b'D');
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.write_bytes(&bytes);
        writer.begin_result_type(b'L');
        writer.write_bit(false);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(
            writer.into_bytes(),
            b"shot D3 D4 D5 D6 D7 D12 D13 D14 D15 D16 L1\n"
        );

        let mut writer = MeasureRecordWriter::new(SampleFormat::R8);
        writer.write_bytes(&bytes);
        writer.write_bit(false);
        writer.write_bytes(&bytes);
        writer.write_bit(true);
        writer.write_end();
        assert_eq!(writer.into_bytes(), [3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn measure_record_writer_handles_empty_dets_records_and_long_r8_gaps() {
        let mut writer = MeasureRecordWriter::new(SampleFormat::Dets);
        writer.write_end();
        writer.write_end();
        writer.write_end();
        assert_eq!(writer.into_bytes(), b"shot\nshot\nshot\n");

        let mut writer = MeasureRecordWriter::new(SampleFormat::R8);
        for _ in 0..(8 * 64) {
            writer.write_bit(false);
        }
        writer.write_bit(true);
        for _ in 0..32 {
            writer.write_bit(false);
        }
        writer.write_end();
        assert_eq!(writer.into_bytes(), [255, 255, 2, 32]);
    }

    #[test]
    fn measure_record_reader_loads_all_supported_record_formats() {
        let expected = [
            false, false, false, true, true, true, true, true, false, false, false, false, true,
            true, true, true, true, true,
        ]
        .to_vec();

        for (format, input) in [
            (SampleFormat::ZeroOne, b"000111110000111111\n".as_slice()),
            (SampleFormat::B8, &[0xF8, 0xF0, 0x03]),
            (
                SampleFormat::Hits,
                b"3,4,5,6,7,12,13,14,15,16,17\n".as_slice(),
            ),
            (SampleFormat::R8, &[3, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0]),
        ] {
            assert_eq!(
                read_records(input, format, 18).unwrap(),
                vec![expected.clone()]
            );
        }
    }

    #[test]
    fn measure_record_reader_round_trips_writer_output() {
        let source = [0, 1, 2, 3, 4, 0xFF, 0xBF, 0xFE, 80, 0, 0, 1, 20];
        let bits = unpack_b8_chunk(&source, source.len() * 8);
        for format in [
            SampleFormat::ZeroOne,
            SampleFormat::B8,
            SampleFormat::R8,
            SampleFormat::Hits,
            SampleFormat::Dets,
        ] {
            let encoded = write_records(std::slice::from_ref(&bits), format);
            let width = if matches!(format, SampleFormat::Hits | SampleFormat::Dets) {
                bits.len() - 1
            } else {
                bits.len()
            };
            assert_eq!(
                read_records(&encoded, format, width).unwrap(),
                vec![bits[..width].to_vec()]
            );
        }
    }

    #[test]
    fn measure_record_reader_handles_multiple_records() {
        let records = read_records(
            b"111011001\n010000000\n101100011\n",
            SampleFormat::ZeroOne,
            9,
        )
        .unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(
            read_records(b"shot M0\nshot M1\nshot M0\nshot\n", SampleFormat::Dets, 2).unwrap(),
            vec![
                vec![true, false],
                vec![false, true],
                vec![true, false],
                vec![false, false],
            ]
        );
    }

    #[test]
    fn measure_record_batch_writes_shot_major_01_records() {
        let s0 = vec![true, false, true, false, true];
        let s1 = vec![false, true, false, true, false];
        let mut batch = MeasureRecordBatch::new(5, 20);
        assert_eq!(batch.stored(), 0);
        batch.record_result(s0.clone()).unwrap();
        assert_eq!(batch.lookback(1), Some(s0.as_slice()));
        batch.record_result(s1.clone()).unwrap();
        assert_eq!(batch.lookback(1), Some(s1.as_slice()));
        assert_eq!(batch.lookback(2), Some(s0.as_slice()));
        for _ in 0..50 {
            batch.record_result(s0.clone()).unwrap();
            batch.record_result(s1.clone()).unwrap();
        }
        assert_eq!(batch.unwritten(), 102);

        let mut writer = MeasureRecordBatchWriter::new(5, SampleFormat::ZeroOne);
        batch
            .final_write_unwritten_results_to(&mut writer, &[false; 5])
            .unwrap();
        let output = writer.write_end();
        for shot_index in 0..5 {
            for sample_index in 0..102 {
                assert_eq!(
                    output[shot_index * 103 + sample_index],
                    b'0' + u8::from((shot_index + sample_index + 1) % 2 == 1)
                );
            }
            assert_eq!(output[shot_index * 103 + 102], b'\n');
        }
        assert!(batch.stored() <= 20);
    }

    #[test]
    fn measure_record_batch_records_zero_result_to_edit() {
        let mut batch = MeasureRecordBatch::new(5, 2);
        batch.record_zero_result_to_edit()[2] = true;
        assert_eq!(batch.stored(), 1);
        assert_eq!(
            batch.lookback(1),
            Some([false, false, true, false, false].as_slice())
        );
        batch.record_zero_result_to_edit()[3] = true;
        assert_eq!(
            batch.lookback(1),
            Some([false, false, false, true, false].as_slice())
        );
    }

    #[test]
    fn sparse_shot_matches_upstream_equality_string_and_mask_behavior() {
        assert_eq!(
            SparseShot::new(Vec::new(), vec![false; 64]),
            SparseShot::new(Vec::new(), vec![false; 64])
        );
        assert_ne!(
            SparseShot::new(Vec::new(), vec![false; 64]),
            SparseShot::new(vec![2], vec![false; 64])
        );
        let mut mask = vec![false; 64];
        mask[2] = true;
        let shot = SparseShot::new(vec![1, 2, 3], mask.clone());
        assert_eq!(
            shot.stim_debug_string(),
            "SparseShot{{1, 2, 3}, __1_____________________________________________________________}"
        );
        assert_eq!(shot.obs_mask_as_u64(), 4);

        let mut wide_mask = vec![false; 125];
        wide_mask[1] = true;
        wide_mask[64] = true;
        assert_eq!(SparseShot::new(Vec::new(), wide_mask).obs_mask_as_u64(), 2);
    }

    #[test]
    fn ptb64_records_are_measurement_major_over_64_shot_groups() {
        let mut records = vec![vec![false, false, false, false]; 64];
        for record in records.iter_mut().take(5) {
            record[1] = true;
        }
        assert_eq!(
            write_ptb64_records(&records),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0x1F, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ]
        );
    }
}
