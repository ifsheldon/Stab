use std::num::NonZeroUsize;

use stab_bits::{BitMatrix, BitSlice, BitWordsMut};

const WORD_BITS: usize = u64::BITS as usize;

use crate::{
    BitIndex, CorrectionWidth, DetectorWidth, FormatError, FormatErrorCode, MeasurementWidth,
    ObservableWidth, RecordResult, SampledErrorWidth, ShotIndex,
};

mod bit_planes;
mod prediction_view;

pub use bit_planes::{BitPlaneBatch, BitPlaneBatchView};
pub use prediction_view::ObservablePredictionBatchViewMut;

/// Owned shot-major packed records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedShotBatch {
    storage: BitMatrix,
}

impl PackedShotBatch {
    pub fn zeros(shot_count: usize, bits_per_shot: usize) -> RecordResult<Self> {
        Ok(Self {
            storage: BitMatrix::zeros(shot_count, bits_per_shot).map_err(bit_storage_error)?,
        })
    }

    /// Builds the same batch as [`Self::zeros`], zeroing its storage in chunks of at most
    /// `chunk_words` packed words and calling `before_chunk` with each chunk's word count before
    /// that chunk is written; see [`BitMatrix::zeros_chunked`].
    ///
    /// The storage is reserved fallibly once, up front. The first error `before_chunk` returns
    /// stops the zeroing and is returned unchanged; a shape or reservation failure is returned as
    /// the [`FormatError`] [`Self::zeros`] reports.
    pub fn zeros_chunked<E: From<FormatError>>(
        shot_count: usize,
        bits_per_shot: usize,
        chunk_words: NonZeroUsize,
        mut before_chunk: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        enum Stop<E> {
            Storage(stab_bits::BitError),
            Caller(E),
        }
        impl<E> From<stab_bits::BitError> for Stop<E> {
            fn from(error: stab_bits::BitError) -> Self {
                Self::Storage(error)
            }
        }
        let storage = BitMatrix::zeros_chunked(shot_count, bits_per_shot, chunk_words, |words| {
            before_chunk(words).map_err(Stop::Caller)
        })
        .map_err(|stop| match stop {
            Stop::Storage(error) => E::from(bit_storage_error(error)),
            Stop::Caller(error) => error,
        })?;
        Ok(Self { storage })
    }

    /// Bytes of packed words that [`Self::zeros`] reserves for `shot_count` shots of
    /// `bits_per_shot` bits: one `u64` per started 64 bits of each shot.
    pub fn storage_bytes(shot_count: usize, bits_per_shot: usize) -> u128 {
        let row_words = bits_per_shot.div_ceil(WORD_BITS) as u128;
        (shot_count as u128)
            .saturating_mul(row_words)
            .saturating_mul(size_of::<u64>() as u128)
    }

    pub fn from_records(records: &[Vec<bool>], bits_per_shot: usize) -> RecordResult<Self> {
        let mut batch = Self::zeros(records.len(), bits_per_shot)?;
        for (shot_index, record) in records.iter().enumerate() {
            if record.len() != bits_per_shot {
                return Err(FormatError::with_context(
                    FormatErrorCode::InvalidRecordWidth,
                    format!(
                        "record {shot_index} has {} bits but {bits_per_shot} were expected",
                        record.len()
                    ),
                    None,
                    crate::FormatErrorContext::RecordWidth {
                        actual_bits: record.len(),
                        expected_bits: bits_per_shot,
                    },
                ));
            }
            for (bit_index, bit) in record.iter().copied().enumerate() {
                if bit {
                    batch.set(ShotIndex(shot_index), BitIndex(bit_index), true)?;
                }
            }
        }
        Ok(batch)
    }

    pub fn from_bit_planes(batch: BitPlaneBatchView<'_>) -> RecordResult<Self> {
        let mut records = Self::zeros(batch.shot_count(), batch.bits_per_shot())?;
        records.copy_from_bit_planes(batch)?;
        Ok(records)
    }

    /// Replaces every record with the transpose of a borrowed bit-plane batch.
    pub fn copy_from_bit_planes(&mut self, batch: BitPlaneBatchView<'_>) -> RecordResult<()> {
        if self.shot_count() != batch.shot_count() || self.bits_per_shot() != batch.bits_per_shot()
        {
            return Err(FormatError::invalid_data(format!(
                "packed output is {}x{} but bit-plane input is {}x{}",
                self.shot_count(),
                self.bits_per_shot(),
                batch.shot_count(),
                batch.bits_per_shot()
            )));
        }
        if batch.shot_count() == 0 || batch.bits_per_shot() == 0 {
            return Ok(());
        }
        if let Some(words) = batch.single_word_planes() {
            return self
                .storage
                .copy_transpose_from_word_rows_at(0, words)
                .map_err(bit_storage_error);
        }
        let mut column = Vec::new();
        column
            .try_reserve_exact(batch.bits_per_shot())
            .map_err(|error| {
                FormatError::invalid_data(format!(
                    "bit-plane transpose could not reserve {} words: {error}",
                    batch.bits_per_shot()
                ))
            })?;
        column.resize(batch.bits_per_shot(), 0);
        for word_index in 0..batch.shot_count().div_ceil(WORD_BITS) {
            self.copy_block_from_bit_planes(batch, word_index, &mut column)?;
        }
        Ok(())
    }

    /// Replaces the 64 records of block `word_index` from word `word_index` of every plane,
    /// using `column` as caller-owned scratch of one word per result bit.
    pub(crate) fn copy_block_from_bit_planes(
        &mut self,
        batch: BitPlaneBatchView<'_>,
        word_index: usize,
        column: &mut [u64],
    ) -> RecordResult<()> {
        batch.copy_word_column_into(word_index, column)?;
        let row_start = word_index
            .checked_mul(WORD_BITS)
            .ok_or_else(|| FormatError::invalid_data("bit-plane block offset overflowed"))?;
        self.storage
            .copy_transpose_from_word_rows_at(row_start, column)
            .map_err(bit_storage_error)
    }

    /// Replaces the first 64 records (or all of them, when fewer) with the transpose of one word
    /// per result bit.
    pub(crate) fn copy_transpose_from_word_column(&mut self, column: &[u64]) -> RecordResult<()> {
        if self.shot_count() == 0 || self.bits_per_shot() == 0 {
            return Ok(());
        }
        self.storage
            .copy_transpose_from_word_rows_at(0, column)
            .map_err(bit_storage_error)
    }

    pub const fn shot_count(&self) -> usize {
        self.storage.rows()
    }

    pub const fn bits_per_shot(&self) -> usize {
        self.storage.cols()
    }

    pub fn get(
        &self,
        ShotIndex(shot_index): ShotIndex,
        BitIndex(bit_index): BitIndex,
    ) -> Option<bool> {
        self.storage.get(shot_index, bit_index)
    }

    pub fn set(
        &mut self,
        ShotIndex(shot_index): ShotIndex,
        BitIndex(bit_index): BitIndex,
        value: bool,
    ) -> RecordResult<()> {
        self.storage
            .set(shot_index, bit_index, value)
            .map_err(bit_storage_error)
    }

    /// Replaces one shot from a dense flag record of exactly this batch's width.
    ///
    /// Flags are packed eight per step, so the copy costs one pass over the record rather than
    /// one branch per flag.
    pub fn copy_shot_from_bools(&mut self, shot_index: usize, record: &[bool]) -> RecordResult<()> {
        if record.len() != self.bits_per_shot() {
            return Err(bit_storage_error(stab_bits::BitError::LengthMismatch {
                left: record.len(),
                right: self.bits_per_shot(),
            }));
        }
        let mut words = self.shot_words_mut(shot_index)?;
        let mut targets = words.iter_mut();
        // Zip the source first so exhausting it never consumes an extra target word.
        for (flags, target) in record.chunks(WORD_BITS).zip(targets.by_ref()) {
            *target = pack_flag_word(flags);
        }
        for target in targets {
            *target = 0;
        }
        Ok(())
    }

    /// Replaces one shot from a B8-format little-endian packed record.
    pub fn copy_shot_from_b8(&mut self, shot_index: usize, record: &[u8]) -> RecordResult<()> {
        self.storage
            .copy_row_from_le_bytes(shot_index, record)
            .map_err(bit_storage_error)
    }

    /// Replaces one shot with a packed bit slice of exactly this batch's width.
    pub fn copy_shot_from_bits(
        &mut self,
        shot_index: usize,
        bits: BitSlice<'_>,
    ) -> RecordResult<()> {
        if bits.len() != self.bits_per_shot() {
            return Err(FormatError::invalid_data(format!(
                "packed shot has {} bits but the batch expects {}",
                bits.len(),
                self.bits_per_shot()
            )));
        }
        let mut words = self.shot_words_mut(shot_index)?;
        let source = bits.words().get(..words.len()).ok_or_else(|| {
            FormatError::invalid_data("packed shot storage was shorter than its declared width")
        })?;
        words.copy_from_slice(source);
        Ok(())
    }

    /// Replaces every row of the first `shot_count` shots with the source bit columns
    /// `source_start..source_start + self.bits_per_shot()`.
    ///
    /// The copy moves whole words with shifts, so splitting a record into column groups costs
    /// one pass over its words rather than one step per bit.
    pub fn copy_columns_from(
        &mut self,
        source: PackedShotBatchView<'_>,
        source_start: usize,
        shot_count: usize,
    ) -> RecordResult<()> {
        let width = self.bits_per_shot();
        let source_end = source_start
            .checked_add(width)
            .ok_or_else(|| FormatError::invalid_data("packed column range overflowed"))?;
        if source_end > source.bits_per_shot()
            || shot_count > source.shot_count()
            || shot_count > self.shot_count()
        {
            return Err(FormatError::invalid_data(format!(
                "packed column range {source_start}..{source_end} of {shot_count} shots escapes a {}x{} source or {}x{width} target",
                source.shot_count(),
                source.bits_per_shot(),
                self.shot_count(),
            )));
        }
        for shot_index in 0..shot_count {
            let row = source.shot(shot_index)?;
            let mut words = self.shot_words_mut(shot_index)?;
            copy_bit_range(row.words(), source_start, &mut words);
        }
        Ok(())
    }

    /// Borrows one shot's storage words; bits beyond the declared width are cleared on drop.
    pub fn shot_words_mut(&mut self, shot_index: usize) -> RecordResult<BitWordsMut<'_>> {
        self.storage
            .row_words_mut(shot_index)
            .map_err(bit_storage_error)
    }

    pub fn shot(&self, shot_index: usize) -> RecordResult<BitSlice<'_>> {
        self.storage.row(shot_index).map_err(bit_storage_error)
    }

    pub const fn view(&self) -> PackedShotBatchView<'_> {
        PackedShotBatchView {
            storage: &self.storage,
            shot_count: self.storage.rows(),
        }
    }

    /// Borrows the first `shot_count` records without resizing the owned storage.
    pub fn view_prefix(&self, shot_count: usize) -> RecordResult<PackedShotBatchView<'_>> {
        if shot_count > self.shot_count() {
            return Err(FormatError::invalid_data(format!(
                "packed shot prefix has {shot_count} records but storage contains only {}",
                self.shot_count()
            )));
        }
        Ok(PackedShotBatchView {
            storage: &self.storage,
            shot_count,
        })
    }

    pub fn to_records(&self) -> RecordResult<Vec<Vec<bool>>> {
        (0..self.shot_count())
            .map(|shot_index| {
                let shot = self.shot(shot_index)?;
                (0..shot.len())
                    .map(|bit_index| {
                        shot.get(bit_index).ok_or_else(|| {
                            FormatError::invalid_data(
                                "packed shot bit index escaped the declared record width",
                            )
                        })
                    })
                    .collect()
            })
            .collect()
    }
}

/// Borrowed shot-major packed records.
#[derive(Clone, Copy, Debug)]
pub struct PackedShotBatchView<'a> {
    storage: &'a BitMatrix,
    shot_count: usize,
}

impl<'a> PackedShotBatchView<'a> {
    pub fn shot_count(self) -> usize {
        self.shot_count
    }

    pub fn bits_per_shot(self) -> usize {
        self.storage.cols()
    }

    pub fn get(
        self,
        ShotIndex(shot_index): ShotIndex,
        BitIndex(bit_index): BitIndex,
    ) -> Option<bool> {
        if shot_index >= self.shot_count {
            return None;
        }
        self.storage.get(shot_index, bit_index)
    }

    pub fn shot(self, shot_index: usize) -> RecordResult<BitSlice<'a>> {
        if shot_index >= self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "packed shot index {shot_index} is out of range for {} records",
                self.shot_count
            )));
        }
        self.storage.row(shot_index).map_err(bit_storage_error)
    }

    /// Borrows the first `shot_count` records of this view.
    pub fn prefix(self, shot_count: usize) -> RecordResult<Self> {
        if shot_count > self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "packed shot prefix has {shot_count} records but the view contains only {}",
                self.shot_count
            )));
        }
        Ok(Self {
            storage: self.storage,
            shot_count,
        })
    }

    /// Copies this shot-major batch into one packed word per result-bit plane.
    ///
    /// The view may contain at most 64 shots. Reused target words are fully replaced and inactive
    /// high lanes are cleared.
    pub fn copy_into_bit_plane_words(self, target: &mut [u64]) -> RecordResult<()> {
        self.storage
            .copy_transpose_prefix_into_word_rows(self.shot_count, target)
            .map_err(bit_storage_error)
    }

    /// Copies records `row_start..row_start + row_count` into one packed word per result bit,
    /// with record `row_start + i` at bit `i`.
    ///
    /// At most 64 records may be selected. Reused target words are fully replaced and inactive
    /// high lanes are cleared.
    pub fn copy_rows_into_bit_plane_words(
        self,
        row_start: usize,
        row_count: usize,
        target: &mut [u64],
    ) -> RecordResult<()> {
        if row_start
            .checked_add(row_count)
            .is_none_or(|end| end > self.shot_count)
        {
            return Err(FormatError::invalid_data(format!(
                "packed rows {row_start}..{} escape a view of {} records",
                row_start.saturating_add(row_count),
                self.shot_count
            )));
        }
        self.storage
            .copy_transpose_rows_into_word_rows(row_start, row_count, target)
            .map_err(bit_storage_error)
    }
}

/// Borrowed records of one width, stored either shot-major or as bit planes.
///
/// Producers deliver whichever layout they computed; consumers that need a specific layout
/// match on [`Self::layout`], and [`Self::get`] reads either.
#[derive(Clone, Copy, Debug)]
pub struct RecordBatchView<'a> {
    storage: RecordLayout<'a>,
}

/// The layout of a [`RecordBatchView`]'s records, borrowed.
#[derive(Clone, Copy, Debug)]
pub enum RecordLayout<'a> {
    /// One packed row of bits per shot.
    ShotMajor(PackedShotBatchView<'a>),
    /// One packed plane of shots per bit.
    BitPlanes(BitPlaneBatchView<'a>),
}

impl<'a> RecordBatchView<'a> {
    pub const fn from_shot_major(records: PackedShotBatchView<'a>) -> Self {
        Self {
            storage: RecordLayout::ShotMajor(records),
        }
    }

    pub const fn from_bit_planes(bit_planes: BitPlaneBatchView<'a>) -> Self {
        Self {
            storage: RecordLayout::BitPlanes(bit_planes),
        }
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        match self.storage {
            RecordLayout::ShotMajor(records) => records.shot_count(),
            RecordLayout::BitPlanes(planes) => planes.shot_count(),
        }
    }

    #[inline]
    pub fn bits_per_shot(self) -> usize {
        match self.storage {
            RecordLayout::ShotMajor(records) => records.bits_per_shot(),
            RecordLayout::BitPlanes(planes) => planes.bits_per_shot(),
        }
    }

    pub fn get(
        self,
        ShotIndex(shot_index): ShotIndex,
        BitIndex(bit_index): BitIndex,
    ) -> Option<bool> {
        match self.storage {
            RecordLayout::ShotMajor(records) => {
                records.get(ShotIndex(shot_index), BitIndex(bit_index))
            }
            RecordLayout::BitPlanes(planes) => {
                planes.get(ShotIndex(shot_index), BitIndex(bit_index))
            }
        }
    }

    /// Borrows the first `shot_count` records of this view.
    pub fn prefix(self, shot_count: usize) -> RecordResult<Self> {
        Ok(Self {
            storage: match self.storage {
                RecordLayout::ShotMajor(records) => {
                    RecordLayout::ShotMajor(records.prefix(shot_count)?)
                }
                RecordLayout::BitPlanes(planes) => {
                    RecordLayout::BitPlanes(planes.prefix(shot_count)?)
                }
            },
        })
    }

    /// The records in the layout their producer delivered.
    #[inline]
    pub const fn layout(self) -> RecordLayout<'a> {
        self.storage
    }
}

impl<'a> From<PackedShotBatchView<'a>> for RecordBatchView<'a> {
    fn from(records: PackedShotBatchView<'a>) -> Self {
        Self::from_shot_major(records)
    }
}

impl<'a> From<BitPlaneBatchView<'a>> for RecordBatchView<'a> {
    fn from(bit_planes: BitPlaneBatchView<'a>) -> Self {
        Self::from_bit_planes(bit_planes)
    }
}

/// Borrowed measurement records with explicit measurement semantics.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementBatchView<'a> {
    records: RecordBatchView<'a>,
    width: MeasurementWidth,
}

impl<'a> MeasurementBatchView<'a> {
    pub fn new(records: PackedShotBatchView<'a>) -> Self {
        Self::from_records(records.into())
    }

    #[inline]
    pub fn from_bit_planes(bit_planes: BitPlaneBatchView<'a>) -> Self {
        Self::from_records(bit_planes.into())
    }

    #[inline]
    pub fn from_records(records: RecordBatchView<'a>) -> Self {
        Self {
            records,
            width: MeasurementWidth::new(records.bits_per_shot()),
        }
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        self.records.shot_count()
    }

    pub fn get(
        self,
        ShotIndex(shot_index): ShotIndex,
        BitIndex(bit_index): BitIndex,
    ) -> Option<bool> {
        self.records.get(ShotIndex(shot_index), BitIndex(bit_index))
    }

    #[inline]
    pub const fn records(self) -> RecordBatchView<'a> {
        self.records
    }

    /// The records in the layout their producer delivered.
    #[inline]
    pub const fn layout(self) -> RecordLayout<'a> {
        self.records.layout()
    }

    #[inline]
    pub const fn width(self) -> MeasurementWidth {
        self.width
    }
}

/// Borrowed detector and observable records with independent planes.
#[derive(Clone, Copy, Debug)]
pub struct DetectionBatchView<'a> {
    detectors: RecordBatchView<'a>,
    observables: RecordBatchView<'a>,
    detector_width: DetectorWidth,
    observable_width: ObservableWidth,
}

impl<'a> DetectionBatchView<'a> {
    /// Pairs detector and observable records of the same shot count, in either layout.
    pub fn try_new(
        detectors: impl Into<RecordBatchView<'a>>,
        observables: impl Into<RecordBatchView<'a>>,
    ) -> RecordResult<Self> {
        let detectors = detectors.into();
        let observables = observables.into();
        ensure_same_shot_count(
            detectors.shot_count(),
            observables.shot_count(),
            "detector",
            "observable",
        )?;
        Ok(Self {
            detectors,
            observables,
            detector_width: DetectorWidth::new(detectors.bits_per_shot()),
            observable_width: ObservableWidth::new(observables.bits_per_shot()),
        })
    }

    pub const fn detectors(self) -> RecordBatchView<'a> {
        self.detectors
    }

    pub const fn observables(self) -> RecordBatchView<'a> {
        self.observables
    }

    pub fn shot_count(self) -> usize {
        self.detectors.shot_count()
    }

    pub const fn detector_width(self) -> DetectorWidth {
        self.detector_width
    }

    pub const fn observable_width(self) -> ObservableWidth {
        self.observable_width
    }
}

/// Borrowed DEM samples with optional sampled-error records.
#[derive(Clone, Copy, Debug)]
pub struct DemSampleBatchView<'a> {
    detection: DetectionBatchView<'a>,
    sampled_errors: Option<RecordBatchView<'a>>,
    sampled_error_width: Option<SampledErrorWidth>,
}

impl<'a> DemSampleBatchView<'a> {
    pub fn try_new(
        detection: DetectionBatchView<'a>,
        sampled_errors: Option<RecordBatchView<'a>>,
    ) -> RecordResult<Self> {
        if let Some(errors) = sampled_errors {
            ensure_same_shot_count(
                detection.shot_count(),
                errors.shot_count(),
                "detection",
                "sampled-error",
            )?;
        }
        Ok(Self {
            detection,
            sampled_errors,
            sampled_error_width: sampled_errors
                .map(|errors| SampledErrorWidth::new(errors.bits_per_shot())),
        })
    }

    pub const fn detection(self) -> DetectionBatchView<'a> {
        self.detection
    }

    pub const fn sampled_errors(self) -> Option<RecordBatchView<'a>> {
        self.sampled_errors
    }

    pub const fn sampled_error_width(self) -> Option<SampledErrorWidth> {
        self.sampled_error_width
    }
}

/// Owned observable predictions.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservablePredictionBatch {
    records: PackedShotBatch,
    width: CorrectionWidth,
}

impl ObservablePredictionBatch {
    pub fn zeros(shot_count: usize, width: CorrectionWidth) -> RecordResult<Self> {
        Ok(Self {
            records: PackedShotBatch::zeros(shot_count, width.get())?,
            width,
        })
    }

    pub const fn view(&self) -> PackedShotBatchView<'_> {
        self.records.view()
    }

    pub fn records(&self) -> &PackedShotBatch {
        &self.records
    }

    pub const fn correction_width(&self) -> CorrectionWidth {
        self.width
    }
}

fn ensure_same_shot_count(
    left: usize,
    right: usize,
    left_name: &str,
    right_name: &str,
) -> RecordResult<()> {
    if left != right {
        return Err(FormatError::invalid_data(format!(
            "{left_name} batch has {left} shots but {right_name} batch has {right}"
        )));
    }
    Ok(())
}

pub(super) fn batch_width_mismatch(label: &str, actual: usize, expected: usize) -> FormatError {
    FormatError::with_context(
        FormatErrorCode::InvalidRecordWidth,
        format!("{label} has {actual} bits but {expected} were expected"),
        None,
        crate::FormatErrorContext::RecordWidth {
            actual_bits: actual,
            expected_bits: expected,
        },
    )
}

/// Fills `target` with the source bits starting at `source_start`; source words past the end
/// read as zero and the caller's guard masks the target's tail.
fn copy_bit_range(source: &[u64], source_start: usize, target: &mut [u64]) {
    let word_offset = source_start / WORD_BITS;
    let shift = source_start % WORD_BITS;
    let word = |index: usize| source.get(index).copied().unwrap_or(0);
    for (index, target_word) in target.iter_mut().enumerate() {
        let low = word(word_offset + index);
        *target_word = if shift == 0 {
            low
        } else {
            (low >> shift) | (word(word_offset + index + 1) << (WORD_BITS - shift))
        };
    }
}

/// Packs up to 64 flags into one word, flag `i` becoming bit `i`.
fn pack_flag_word(flags: &[bool]) -> u64 {
    /// Multiplying eight 0-or-1 bytes by this gathers them into the top byte, byte `i`
    /// becoming bit `56 + i`.
    const GATHER_LOW_BITS: u64 = 0x0102_0408_1020_4080;
    let (octets, tail) = flags.as_chunks::<8>();
    let mut word = 0_u64;
    for (octet_index, octet) in octets.iter().enumerate() {
        let bytes = u64::from_le_bytes(octet.map(u8::from));
        word |= (bytes.wrapping_mul(GATHER_LOW_BITS) >> 56) << (octet_index * 8);
    }
    let tail_offset = octets.len() * 8;
    for (flag_index, flag) in tail.iter().enumerate() {
        word |= u64::from(*flag) << (tail_offset + flag_index);
    }
    word
}

pub(super) fn bit_storage_error(error: stab_bits::BitError) -> FormatError {
    FormatError::invalid_data(error.to_string())
}

#[cfg(test)]
mod tests;
