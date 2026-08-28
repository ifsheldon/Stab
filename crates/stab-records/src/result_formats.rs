use crate::{
    BitPlane64BatchView, FormatError, PackedShotBatch, PackedShotBatchView, RecordResult,
    result_packed::{
        b8_bytes_per_record, decode_next_r8_record, ptb64_prefix_layout,
        ptb64_record_count as packed_ptb64_record_count,
    },
    result_text::HitsEvent,
};
use stab_bits::BitSlice;

mod capabilities;
mod dets;

pub use capabilities::codec_capabilities;
pub use capabilities::{CodecCapability, RecordEncoding, RecordFormat};
pub use dets::{DetsLayout, DetsResultType, DetsToken, read_dets_records};

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

const ZERO_ONE_LINES_BY_BYTE: [[u8; 16]; 256] = zero_one_lines_by_byte();
const ZERO_ONE_BITS_BY_BYTE: [[u8; 8]; 256] = zero_one_bits_by_byte();

#[allow(
    clippy::indexing_slicing,
    reason = "const table indices are bounded by the byte and bit loop limits"
)]
const fn zero_one_bits_by_byte() -> [[u8; 8]; 256] {
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
    reason = "const table indices are bounded by the byte and bit loop limits"
)]
const fn zero_one_lines_by_byte() -> [[u8; 16]; 256] {
    let mut table = [[0_u8; 16]; 256];
    let mut byte = 0;
    while byte < 256 {
        let mut bit = 0;
        while bit < 8 {
            table[byte][bit * 2] = if byte & (1 << bit) == 0 { b'0' } else { b'1' };
            table[byte][bit * 2 + 1] = b'\n';
            bit += 1;
        }
        byte += 1;
    }
    table
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
        .chunks_exact_mut(64)
        .zip(input.chunks_exact(bytes_per_group))
        .take(shot_groups)
    {
        for (bit_index, word_chunk) in group_bytes.chunks_exact(8).enumerate() {
            let mut word_bytes = [0u8; 8];
            word_bytes.copy_from_slice(word_chunk);
            let word = u64::from_le_bytes(word_bytes);
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
        if lookback == 0 || lookback > self.max_lookback || lookback > self.storage.len() {
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
    ) -> RecordResult<()> {
        let unwritten = self.storage.get(self.unwritten_start..).ok_or_else(|| {
            FormatError::invalid_result_format("measure record unwritten cursor is out of range")
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
    written_count: usize,
    max_lookback: usize,
    shot_count: usize,
}

impl MeasureRecordBatch {
    pub fn new(shot_count: usize, max_lookback: usize) -> Self {
        Self {
            records: Vec::new(),
            unwritten_start: 0,
            written_count: 0,
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

    pub fn record_result(&mut self, shot_bits: Vec<bool>) -> RecordResult<()> {
        if shot_bits.len() != self.shot_count {
            return Err(FormatError::invalid_result_format(format!(
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
        if lookback == 0 || lookback > self.max_lookback || lookback > self.records.len() {
            return None;
        }
        self.records
            .get(self.records.len() - lookback)
            .map(Vec::as_slice)
    }

    pub fn intermediate_write_unwritten_results_to(
        &mut self,
        writer: &mut MeasureRecordBatchWriter,
        reference_sample: &[bool],
    ) -> RecordResult<()> {
        const WRITE_SIZE: usize = 256;
        self.validate_unwritten_cursor()?;
        while self.unwritten() >= WRITE_SIZE {
            let end = self
                .unwritten_start
                .checked_add(WRITE_SIZE)
                .ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "measure record batch write range overflowed",
                    )
                })?;
            self.write_range(writer, reference_sample, self.unwritten_start, end)?;
            self.unwritten_start = end;
            self.written_count = self.written_count.checked_add(WRITE_SIZE).ok_or_else(|| {
                FormatError::invalid_result_format("measure record batch written count overflowed")
            })?;
        }
        self.compact_written_prefix();
        Ok(())
    }

    pub fn final_write_unwritten_results_to(
        &mut self,
        writer: &mut MeasureRecordBatchWriter,
        reference_sample: &[bool],
    ) -> RecordResult<()> {
        self.validate_unwritten_cursor()?;
        let unwritten = self.unwritten();
        self.write_range(
            writer,
            reference_sample,
            self.unwritten_start,
            self.records.len(),
        )?;
        self.written_count = self.written_count.checked_add(unwritten).ok_or_else(|| {
            FormatError::invalid_result_format("measure record batch written count overflowed")
        })?;
        self.unwritten_start = self.records.len();
        self.compact_written_prefix();
        Ok(())
    }

    fn validate_unwritten_cursor(&self) -> RecordResult<()> {
        if self.unwritten_start > self.records.len() {
            return Err(FormatError::invalid_result_format(
                "measure record batch unwritten cursor is out of range",
            ));
        }
        Ok(())
    }

    fn write_range(
        &self,
        writer: &mut MeasureRecordBatchWriter,
        reference_sample: &[bool],
        start: usize,
        end: usize,
    ) -> RecordResult<()> {
        let records = self.records.get(start..end).ok_or_else(|| {
            FormatError::invalid_result_format("measure record batch write range is out of bounds")
        })?;
        let mut inverted = vec![false; self.shot_count];
        for (offset, record) in records.iter().enumerate() {
            let measurement_index = self.written_count.checked_add(offset).ok_or_else(|| {
                FormatError::invalid_result_format(
                    "measure record batch reference index overflowed",
                )
            })?;
            if reference_sample
                .get(measurement_index)
                .copied()
                .unwrap_or(false)
            {
                for (output, bit) in inverted.iter_mut().zip(record) {
                    *output = !*bit;
                }
                writer.batch_write_bit(&inverted)?;
            } else {
                writer.batch_write_bit(record)?;
            }
        }
        Ok(())
    }

    fn compact_written_prefix(&mut self) {
        let keep = self
            .max_lookback
            .max(self.unwritten())
            .min(self.records.len());
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
    format: PerRecordFormat,
    output: Vec<u8>,
    index: usize,
    b8_byte: u8,
    b8_bit_index: u8,
    r8_false_run: u8,
    hits_first: bool,
    dets_started: bool,
    dets_type: u8,
    b8_transpose: Option<PackedShotBatch>,
}

impl MeasureRecordWriter {
    pub fn try_new(format: RecordFormat) -> RecordResult<Self> {
        Self::try_with_capacity(format, 0)
    }

    pub fn try_with_capacity(format: RecordFormat, capacity: usize) -> RecordResult<Self> {
        let mut writer = Self {
            format: PerRecordFormat::try_from(format)?,
            output: Vec::new(),
            index: 0,
            b8_byte: 0,
            b8_bit_index: 0,
            r8_false_run: 0,
            hits_first: true,
            dets_started: false,
            dets_type: b'M',
            b8_transpose: None,
        };
        writer.reserve_output(capacity)?;
        Ok(writer)
    }

    #[inline]
    pub(crate) fn reserve_output(&mut self, additional: usize) -> RecordResult<()> {
        self.output.try_reserve(additional).map_err(|error| {
            FormatError::invalid_result_format(format!(
                "result writer could not reserve {additional} output bytes: {error}"
            ))
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

    pub fn write_packed_record(&mut self, record: BitSlice<'_>) -> RecordResult<()> {
        if self.format == PerRecordFormat::B8 && self.b8_bit_index == 0 {
            self.write_byte_aligned_b8_record(record)?;
            return Ok(());
        }
        for bit_index in 0..record.len() {
            let bit = record.get(bit_index).ok_or_else(|| {
                FormatError::invalid_result_format(
                    "packed record bit index escaped the declared width",
                )
            })?;
            self.write_bit(bit);
        }
        Ok(())
    }

    fn write_byte_aligned_b8_record(&mut self, record: BitSlice<'_>) -> RecordResult<()> {
        let full_bytes = record.len() / 8;
        let tail_bits = record.len() % 8;
        let mut remaining = full_bytes;
        for word in record.words() {
            let bytes = word.to_le_bytes();
            let copy_count = remaining.min(bytes.len());
            self.output
                .extend_from_slice(bytes.get(..copy_count).ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "packed B8 word escaped its fixed byte representation",
                    )
                })?);
            remaining -= copy_count;
            if remaining == 0 {
                break;
            }
        }
        if remaining != 0 {
            return Err(FormatError::invalid_result_format(
                "packed record omitted bytes inside its declared width",
            ));
        }
        if tail_bits != 0 {
            let word_index = full_bytes / size_of::<u64>();
            let byte_index = full_bytes % size_of::<u64>();
            let tail = record
                .words()
                .get(word_index)
                .map(|word| word.to_le_bytes())
                .and_then(|bytes| bytes.get(byte_index).copied())
                .ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "packed record omitted its declared tail byte",
                    )
                })?;
            self.b8_byte = tail & ((1_u8 << tail_bits) - 1);
            self.b8_bit_index = u8::try_from(tail_bits).map_err(|_| {
                FormatError::invalid_result_format("B8 tail width did not fit its writer state")
            })?;
        }
        self.index = self.index.checked_add(record.len()).ok_or_else(|| {
            FormatError::invalid_result_format("packed B8 record index overflowed")
        })?;
        Ok(())
    }

    pub fn write_packed_batch(&mut self, batch: PackedShotBatchView<'_>) -> RecordResult<()> {
        if self.format == PerRecordFormat::ZeroOne
            && batch.bits_per_shot() == 1
            && self.is_at_record_boundary()
        {
            for shot_index in 0..batch.shot_count() {
                let bit = batch.get(shot_index, 0).ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "packed record bit index escaped the declared width",
                    )
                })?;
                self.output
                    .extend_from_slice(if bit { b"1\n" } else { b"0\n" });
            }
            return Ok(());
        }
        for shot_index in 0..batch.shot_count() {
            self.write_packed_record(batch.shot(shot_index)?)?;
            self.write_end();
        }
        Ok(())
    }

    #[inline]
    pub fn write_bit_plane_batch(&mut self, batch: BitPlane64BatchView<'_>) -> RecordResult<()> {
        if self.format == PerRecordFormat::B8 && self.is_at_record_boundary() {
            let mut packed = match self.b8_transpose.take() {
                Some(packed)
                    if packed.shot_count() == batch.shot_count()
                        && packed.bits_per_shot() == batch.bits_per_shot() =>
                {
                    packed
                }
                _ => PackedShotBatch::zeros(batch.shot_count(), batch.bits_per_shot())?,
            };
            packed.copy_from_bit_planes(batch)?;
            let result = self.write_packed_batch(packed.view());
            self.b8_transpose = Some(packed);
            return result;
        }
        if self.format == PerRecordFormat::ZeroOne
            && batch.bits_per_shot() == 1
            && self.is_at_record_boundary()
        {
            let word = batch.plane(0)?.words().first().copied().unwrap_or_default();
            return self.write_zero_one_single_plane(word, batch.shot_count());
        }
        for shot_index in 0..batch.shot_count() {
            for bit_index in 0..batch.bits_per_shot() {
                let bit = batch.get(bit_index, shot_index).ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "bit-plane record index escaped the declared dimensions",
                    )
                })?;
                self.write_bit(bit);
            }
            self.write_end();
        }
        Ok(())
    }

    #[inline]
    fn write_zero_one_single_plane(&mut self, word: u64, shot_count: usize) -> RecordResult<()> {
        let mut encoded = [0_u8; 128];
        let mut encoded_len = 0_usize;
        let mut remaining_shots = shot_count;
        for input_byte in word.to_le_bytes() {
            if remaining_shots == 0 {
                break;
            }
            let shots_in_byte = remaining_shots.min(8);
            let bytes_in_chunk = shots_in_byte * 2;
            let pattern = ZERO_ONE_LINES_BY_BYTE
                .get(usize::from(input_byte))
                .and_then(|line| line.get(..bytes_in_chunk))
                .ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "single-plane 01 lookup escaped its fixed table",
                    )
                })?;
            let next_len = encoded_len.checked_add(bytes_in_chunk).ok_or_else(|| {
                FormatError::invalid_result_format("single-plane 01 output length overflowed")
            })?;
            encoded
                .get_mut(encoded_len..next_len)
                .ok_or_else(|| {
                    FormatError::invalid_result_format(
                        "single-plane 01 output escaped its fixed batch",
                    )
                })?
                .copy_from_slice(pattern);
            encoded_len = next_len;
            remaining_shots -= shots_in_byte;
        }
        self.output
            .extend_from_slice(encoded.get(..encoded_len).ok_or_else(|| {
                FormatError::invalid_result_format("single-plane 01 output escaped its fixed batch")
            })?);
        Ok(())
    }

    #[allow(
        clippy::indexing_slicing,
        reason = "every u8 value is a valid index into the 256-entry encoding table"
    )]
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        if self.format == PerRecordFormat::ZeroOne {
            for byte in bytes {
                self.output
                    .extend_from_slice(&ZERO_ONE_BITS_BY_BYTE[usize::from(*byte)]);
                self.index += 8;
            }
            return;
        }
        for byte in bytes {
            for bit_index in 0..8 {
                self.write_bit(byte & (1u8 << bit_index) != 0);
            }
        }
    }

    /// Returns encoded bytes currently retained by this compatibility writer.
    ///
    /// Streaming adapters should inspect this only at record boundaries.
    pub fn buffered_bytes(&self) -> &[u8] {
        &self.output
    }

    /// Clears emitted bytes while retaining allocation and completed-record encoding state.
    ///
    /// Clearing an incomplete record is rejected so streaming adapters cannot silently discard a
    /// prefix and continue with corrupt state.
    pub fn clear_buffered_bytes(&mut self) -> RecordResult<()> {
        if !self.is_at_record_boundary() {
            return Err(FormatError::invalid_result_format(
                "result writer bytes can be cleared only at a completed record boundary",
            ));
        }
        self.output.clear();
        Ok(())
    }

    fn is_at_record_boundary(&self) -> bool {
        self.index == 0
            && self.b8_byte == 0
            && self.b8_bit_index == 0
            && self.r8_false_run == 0
            && self.hits_first
            && !self.dets_started
            && self.dets_type == b'M'
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureRecordBatchWriter {
    format: RecordFormat,
    records: Vec<Vec<bool>>,
}

impl MeasureRecordBatchWriter {
    pub fn new(shots: usize, format: RecordFormat) -> Self {
        Self {
            format,
            records: vec![Vec::new(); shots],
        }
    }

    pub fn batch_write_bit(&mut self, shot_bits: &[bool]) -> RecordResult<()> {
        if shot_bits.len() != self.records.len() {
            return Err(FormatError::invalid_result_format(format!(
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

    pub fn write_end(&self) -> RecordResult<Vec<u8>> {
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
