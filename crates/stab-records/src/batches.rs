use stab_bits::{BitMatrix, BitSlice, BitWordsMut};

const WORD_BITS: usize = u64::BITS as usize;

use crate::{
    CorrectionWidth, DetectorWidth, FormatError, FormatErrorCode, MeasurementWidth,
    ObservableWidth, RecordResult, SampledErrorWidth,
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
                    batch.set(shot_index, bit_index, true)?;
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

    pub fn get(&self, shot_index: usize, bit_index: usize) -> Option<bool> {
        self.storage.get(shot_index, bit_index)
    }

    pub fn set(&mut self, shot_index: usize, bit_index: usize, value: bool) -> RecordResult<()> {
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

    pub fn get(self, shot_index: usize, bit_index: usize) -> Option<bool> {
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
/// Producers deliver whichever layout they computed; consumers that need a specific layout ask
/// for it through [`Self::shot_major_records`] or [`Self::bit_planes`], and [`Self::get`] reads
/// either.
#[derive(Clone, Copy, Debug)]
pub struct RecordBatchView<'a> {
    storage: RecordBatchStorage<'a>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum RecordBatchStorage<'a> {
    ShotMajor(PackedShotBatchView<'a>),
    BitPlanes(BitPlaneBatchView<'a>),
}

impl<'a> RecordBatchView<'a> {
    pub const fn from_shot_major(records: PackedShotBatchView<'a>) -> Self {
        Self {
            storage: RecordBatchStorage::ShotMajor(records),
        }
    }

    pub const fn from_bit_planes(bit_planes: BitPlaneBatchView<'a>) -> Self {
        Self {
            storage: RecordBatchStorage::BitPlanes(bit_planes),
        }
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        match self.storage {
            RecordBatchStorage::ShotMajor(records) => records.shot_count(),
            RecordBatchStorage::BitPlanes(planes) => planes.shot_count(),
        }
    }

    #[inline]
    pub fn bits_per_shot(self) -> usize {
        match self.storage {
            RecordBatchStorage::ShotMajor(records) => records.bits_per_shot(),
            RecordBatchStorage::BitPlanes(planes) => planes.bits_per_shot(),
        }
    }

    pub fn get(self, shot_index: usize, bit_index: usize) -> Option<bool> {
        match self.storage {
            RecordBatchStorage::ShotMajor(records) => records.get(shot_index, bit_index),
            RecordBatchStorage::BitPlanes(planes) => planes.get(bit_index, shot_index),
        }
    }

    /// Borrows the first `shot_count` records of this view.
    pub fn prefix(self, shot_count: usize) -> RecordResult<Self> {
        Ok(Self {
            storage: match self.storage {
                RecordBatchStorage::ShotMajor(records) => {
                    RecordBatchStorage::ShotMajor(records.prefix(shot_count)?)
                }
                RecordBatchStorage::BitPlanes(planes) => {
                    RecordBatchStorage::BitPlanes(planes.prefix(shot_count)?)
                }
            },
        })
    }

    pub const fn shot_major_records(self) -> Option<PackedShotBatchView<'a>> {
        match self.storage {
            RecordBatchStorage::ShotMajor(records) => Some(records),
            RecordBatchStorage::BitPlanes(_) => None,
        }
    }

    pub const fn bit_planes(self) -> Option<BitPlaneBatchView<'a>> {
        match self.storage {
            RecordBatchStorage::ShotMajor(_) => None,
            RecordBatchStorage::BitPlanes(planes) => Some(planes),
        }
    }

    pub(crate) const fn storage(self) -> RecordBatchStorage<'a> {
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

    pub fn get(self, shot_index: usize, bit_index: usize) -> Option<bool> {
        self.records.get(shot_index, bit_index)
    }

    #[inline]
    pub const fn records(self) -> RecordBatchView<'a> {
        self.records
    }

    pub const fn shot_major_records(self) -> Option<PackedShotBatchView<'a>> {
        self.records.shot_major_records()
    }

    #[inline]
    pub const fn bit_planes(self) -> Option<BitPlaneBatchView<'a>> {
        self.records.bit_planes()
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

    pub fn records_mut(&mut self) -> &mut PackedShotBatch {
        &mut self.records
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
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "batch tests use compact exact fixtures"
    )]

    use proptest::prelude::*;
    use proptest::test_runner::{Config as ProptestConfig, RngAlgorithm, RngSeed};

    use super::*;

    #[test]
    fn detection_and_dem_views_keep_planes_separate() {
        let detectors =
            PackedShotBatch::from_records(&[vec![true, false], vec![false, true]], 2).unwrap();
        let observables = PackedShotBatch::from_records(&[vec![true], vec![false]], 1).unwrap();
        let errors =
            PackedShotBatch::from_records(&[vec![false, true], vec![true, false]], 2).unwrap();

        let detection = DetectionBatchView::try_new(detectors.view(), observables.view()).unwrap();
        let dem = DemSampleBatchView::try_new(detection, Some(errors.view().into())).unwrap();

        assert_eq!(dem.detection().detectors().get(0, 0), Some(true));
        assert_eq!(dem.detection().observables().get(0, 0), Some(true));
        assert_eq!(dem.detection().shot_count(), 2);
        assert_eq!(dem.sampled_errors().unwrap().get(0, 1), Some(true));

        let measurement = MeasurementBatchView::new(detectors.view());
        assert_eq!(measurement.shot_count(), 2);
        assert_eq!(measurement.get(1, 1), Some(true));
        assert!(measurement.shot_major_records().is_some());
        assert!(measurement.bit_planes().is_none());

        let mismatched = PackedShotBatch::zeros(1, 1).unwrap();
        assert!(DetectionBatchView::try_new(detectors.view(), mismatched.view()).is_err());
        assert!(DemSampleBatchView::try_new(detection, Some(mismatched.view().into())).is_err());
        assert!(DemSampleBatchView::try_new(detection, None).is_ok());
    }

    #[test]
    fn semantic_widths_bind_each_record_plane_and_dets_namespace() {
        let measurements = PackedShotBatch::zeros(2, 7).unwrap();
        let detectors = PackedShotBatch::zeros(2, 5).unwrap();
        let observables = PackedShotBatch::zeros(2, 3).unwrap();
        let sampled_errors = PackedShotBatch::zeros(2, 11).unwrap();

        let measurement = MeasurementBatchView::new(measurements.view());
        let detection = DetectionBatchView::try_new(detectors.view(), observables.view()).unwrap();
        let dem =
            DemSampleBatchView::try_new(detection, Some(sampled_errors.view().into())).unwrap();
        let predictions = ObservablePredictionBatch::zeros(2, CorrectionWidth::new(3)).unwrap();
        let layout = crate::DetsLayout::from_widths(
            measurement.width(),
            detection.detector_width(),
            detection.observable_width(),
        )
        .unwrap();

        assert_eq!(measurement.width(), MeasurementWidth::new(7));
        assert_eq!(detection.detector_width(), DetectorWidth::new(5));
        assert_eq!(detection.observable_width(), ObservableWidth::new(3));
        assert_eq!(dem.sampled_error_width(), Some(SampledErrorWidth::new(11)));
        assert_eq!(predictions.correction_width(), CorrectionWidth::new(3));
        assert_eq!(layout.measurement_width(), MeasurementWidth::new(7));
        assert_eq!(layout.detector_width(), DetectorWidth::new(5));
        assert_eq!(layout.observable_width(), ObservableWidth::new(3));
        assert_eq!(layout.total_bits(), 15);
    }

    #[test]
    fn packed_and_prediction_batches_preserve_checked_dimensions_and_access() {
        let records = vec![vec![true, false, true], vec![false, true, false]];
        let mut packed = PackedShotBatch::from_records(&records, 3).unwrap();
        assert_eq!(packed.shot_count(), 2);
        assert_eq!(packed.bits_per_shot(), 3);
        assert_eq!(packed.get(0, 2), Some(true));
        assert_eq!(packed.get(2, 0), None);
        assert_eq!(packed.shot(1).unwrap().get(1), Some(true));
        assert_eq!(packed.to_records().unwrap(), records);
        assert!(PackedShotBatch::from_records(&[vec![true]], 2).is_err());

        packed.set(1, 0, true).unwrap();
        let view = packed.view();
        assert_eq!(view.shot_count(), 2);
        assert_eq!(view.bits_per_shot(), 3);
        assert_eq!(view.get(1, 0), Some(true));
        assert_eq!(view.shot(1).unwrap().get(0), Some(true));

        let mut planes = BitPlaneBatch::from_shot_major(view).unwrap();
        assert_eq!(planes.shot_count(), 2);
        assert_eq!(planes.bits_per_shot(), 3);
        assert_eq!(planes.get(0, 1), Some(true));
        assert_eq!(planes.plane(2).unwrap().get(0), Some(true));
        planes.set(2, 1, true).unwrap();
        let plane_view = planes.view();
        assert_eq!(plane_view.shot_count(), 2);
        assert_eq!(plane_view.bits_per_shot(), 3);
        assert_eq!(plane_view.get(2, 1), Some(true));
        assert_eq!(plane_view.plane(2).unwrap().get(1), Some(true));
        let measurement_planes = MeasurementBatchView::from_bit_planes(plane_view);
        assert_eq!(measurement_planes.shot_count(), 2);
        assert_eq!(measurement_planes.width(), MeasurementWidth::new(3));
        assert_eq!(measurement_planes.get(1, 2), Some(true));
        assert!(measurement_planes.shot_major_records().is_none());
        assert!(measurement_planes.bit_planes().is_some());

        let mut predictions = ObservablePredictionBatch::zeros(2, CorrectionWidth::new(3)).unwrap();
        predictions.records_mut().set(0, 1, true).unwrap();
        assert_eq!(predictions.records().get(0, 1), Some(true));
        assert_eq!(predictions.view().get(0, 1), Some(true));
        assert_eq!(predictions.correction_width(), CorrectionWidth::new(3));
    }

    #[test]
    fn packed_prefix_view_limits_access_without_reallocating_storage() {
        let mut packed = PackedShotBatch::zeros(64, 3).unwrap();
        packed.set(1, 2, true).unwrap();
        packed.set(2, 0, true).unwrap();

        let prefix = packed.view_prefix(2).unwrap();
        assert_eq!(prefix.shot_count(), 2);
        assert_eq!(prefix.bits_per_shot(), 3);
        assert_eq!(prefix.get(1, 2), Some(true));
        assert_eq!(prefix.get(2, 0), None);
        assert!(prefix.shot(2).is_err());
        assert!(packed.view_prefix(65).is_err());
        assert_eq!(packed.shot_count(), 64);
    }

    #[test]
    fn packed_batch_replaces_complete_shots_without_retaining_old_bits() {
        let mut packed = PackedShotBatch::zeros(2, 4).unwrap();
        packed
            .copy_shot_from_bools(1, &[true, true, false, true])
            .unwrap();
        packed
            .copy_shot_from_bools(1, &[false, true, true, false])
            .unwrap();

        assert_eq!(
            (0..4)
                .map(|bit| packed.get(1, bit).unwrap())
                .collect::<Vec<_>>(),
            [false, true, true, false]
        );
        assert!(
            packed
                .copy_shot_from_bools(1, &[true, false, true])
                .is_err()
        );
        assert!(
            packed
                .copy_shot_from_bools(2, &[false, true, true, false])
                .is_err()
        );

        let mut packed_bytes = PackedShotBatch::zeros(2, 10).unwrap();
        packed_bytes.copy_shot_from_b8(0, &[0xff, 0xff]).unwrap();
        assert!((0..10).all(|bit| packed_bytes.get(0, bit) == Some(true)));
        packed_bytes.copy_shot_from_b8(0, &[0x01, 0x02]).unwrap();
        assert_eq!(
            (0..10)
                .map(|bit| packed_bytes.get(0, bit).unwrap())
                .collect::<Vec<_>>(),
            [
                true, false, false, false, false, false, false, false, false, true
            ]
        );
        assert!(packed_bytes.copy_shot_from_b8(0, &[0]).is_err());
        assert!(packed_bytes.copy_shot_from_b8(2, &[0, 0]).is_err());
    }

    #[test]
    fn bit_plane_prefix_and_shot_copy_hide_inactive_storage() {
        let mut planes = BitPlaneBatch::zeros(64, 2).unwrap();
        planes.set(0, 0, true).unwrap();
        planes.set(0, 2, true).unwrap();
        planes
            .copy_shot_from_bools(1, &[false, true])
            .expect("replace one shot");

        let prefix = planes.view_prefix(2).unwrap();
        assert_eq!(prefix.shot_count(), 2);
        assert_eq!(prefix.get(0, 0), Some(true));
        assert_eq!(prefix.get(0, 1), Some(false));
        assert_eq!(prefix.get(0, 2), None);
        assert_eq!(prefix.get(1, 1), Some(true));
        assert_eq!(prefix.plane(0).unwrap().len(), 2);
        assert!(planes.view_prefix(65).is_err());
        assert!(planes.copy_shot_from_bools(1, &[true]).is_err());
        assert!(planes.copy_shot_from_bools(64, &[true, false]).is_err());
    }

    fn patterned_records(shots: usize, width: usize) -> PackedShotBatch {
        let mut packed = PackedShotBatch::zeros(shots, width).unwrap();
        for shot in 0..shots {
            for bit in 0..width {
                if (shot * 5 + bit * 3) % 11 < 4 {
                    packed.set(shot, bit, true).unwrap();
                }
            }
        }
        packed
    }

    #[test]
    fn wide_bit_planes_round_trip_partial_and_multi_word_batches() {
        for shots in [0, 1, 17, 63, 64, 65, 255, 256, 257, 1000] {
            for width in [0, 1, 7, 64, 65, 130] {
                let packed = patterned_records(shots, width);
                let planes = BitPlaneBatch::from_shot_major(packed.view()).unwrap();
                assert_eq!(planes.shot_count(), shots);
                assert_eq!(planes.bits_per_shot(), width);
                for shot in 0..shots {
                    for bit in 0..width {
                        assert_eq!(planes.get(bit, shot), packed.get(shot, bit));
                    }
                }
                assert_eq!(planes.to_shot_major().unwrap(), packed);
                let records = RecordBatchView::from(planes.view());
                assert_eq!(records.shot_count(), shots);
                assert_eq!(records.bits_per_shot(), width);
                if shots > 0 && width > 0 {
                    assert_eq!(
                        records.get(shots - 1, width - 1),
                        packed.get(shots - 1, width - 1)
                    );
                }
            }
        }
    }

    #[test]
    fn block_planes_hide_inactive_lanes_and_match_word_planes() {
        let shots = 200;
        let packed = patterned_records(256, 9);
        let words = BitPlaneBatch::from_shot_major(packed.view()).unwrap();
        let blocks = (0..9)
            .map(|bit| {
                let plane = words.plane(bit).unwrap().words().to_vec();
                stab_bits::BitBlock::from_words(std::array::from_fn(|index| plane[index]))
            })
            .collect::<Vec<_>>();
        let view = BitPlaneBatchView::try_from_blocks(&blocks, shots).unwrap();
        let rows = PackedShotBatch::from_bit_planes(view).unwrap();
        assert_eq!(rows.shot_count(), shots);
        for shot in 0..256 {
            for bit in 0..9 {
                let expected = (shot < shots).then(|| packed.get(shot, bit).unwrap());
                assert_eq!(view.get(bit, shot), expected);
                if shot < shots {
                    assert_eq!(rows.get(shot, bit), expected);
                }
            }
        }
        let mut column = vec![u64::MAX; 9];
        view.copy_word_column_into(3, &mut column).unwrap();
        assert!(column.iter().all(|word| *word >> (shots - 192) == 0));
        assert!(view.copy_word_column_into(4, &mut column).is_err());
        assert!(BitPlaneBatchView::try_from_blocks(&blocks, 257).is_err());
    }

    #[test]
    fn word_aligned_shot_windows_read_the_selected_shots_only() {
        let packed = patterned_records(1000, 5);
        let planes = BitPlaneBatch::from_shot_major(packed.view()).unwrap();
        for (first, count) in [(0, 256), (256, 256), (768, 232), (960, 40), (960, 0)] {
            let window = planes.view().shot_window(first, count).unwrap();
            assert_eq!(window.shot_count(), count);
            let rows = PackedShotBatch::from_bit_planes(window).unwrap();
            for shot in 0..count {
                for bit in 0..5 {
                    assert_eq!(window.get(bit, shot), packed.get(first + shot, bit));
                    assert_eq!(rows.get(shot, bit), packed.get(first + shot, bit));
                }
            }
            assert_eq!(window.get(0, count), None);
        }
        assert!(planes.view().shot_window(1, 2).is_err());
        assert!(planes.view().shot_window(960, 41).is_err());
    }

    #[test]
    fn borrowed_bit_plane_words_validate_layout_and_hide_tail_bits() {
        let words = [u64::MAX, 0b10];
        let view = BitPlaneBatchView::try_from_words(&words, 1, 2, 2).unwrap();

        assert_eq!(view.shot_count(), 2);
        assert_eq!(view.bits_per_shot(), 2);
        assert_eq!(view.get(0, 0), Some(true));
        assert_eq!(view.get(0, 1), Some(true));
        assert_eq!(view.get(0, 2), None);
        assert_eq!(view.get(1, 0), Some(false));
        assert_eq!(view.get(1, 1), Some(true));
        let plane = view.plane(0).unwrap();
        assert_eq!(plane.popcount(), 2);
        assert_eq!(plane.words(), &[u64::MAX]);
        let mut copied = [u64::MAX; 2];
        view.copy_word_column_into(0, &mut copied).unwrap();
        assert_eq!(copied, [0b11, 0b10]);

        assert!(BitPlaneBatchView::try_from_words(&words, 1, 65, 2).is_err());
        assert!(BitPlaneBatchView::try_from_words(&words, 1, 2, 1).is_err());
        assert!(BitPlaneBatchView::try_from_words(&words, usize::MAX, 2, 2).is_err());
        assert!(view.copy_word_column_into(0, &mut copied[..1]).is_err());
    }

    #[test]
    fn bit_planes_report_unaddressable_widths_without_panicking() {
        assert!(BitPlaneBatch::zeros(1, usize::MAX).is_err());
        assert!(BitPlaneBatch::zeros(65, usize::MAX).is_err());
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            rng_algorithm: RngAlgorithm::ChaCha,
            rng_seed: RngSeed::Fixed(0xba7c_4e5c_0dec_0001),
            ..ProptestConfig::default()
        })]

        #[test]
        fn shot_major_and_bit_plane_round_trip(
            shot_count in 0usize..=300,
            bits_per_shot in 0usize..=130,
            seed in any::<u64>(),
        ) {
            let mut source = PackedShotBatch::zeros(shot_count, bits_per_shot).unwrap();
            for shot in 0..shot_count {
                for bit in 0..bits_per_shot {
                    let value = (seed
                        .wrapping_add((shot as u64).wrapping_mul(17))
                        .wrapping_add((bit as u64).wrapping_mul(31)))
                        .count_ones()
                        .is_multiple_of(2);
                    source.set(shot, bit, value).unwrap();
                }
            }

            let planes = BitPlaneBatch::from_shot_major(source.view()).unwrap();
            let round_trip = planes.to_shot_major().unwrap();
            prop_assert_eq!(round_trip, source);
        }

        #[test]
        fn packed_flag_copy_matches_the_per_flag_reference(
            width in prop::sample::select(vec![0_usize, 1, 7, 8, 63, 64, 65, 1000]),
            length_delta in prop::sample::select(vec![0_isize, 0, 0, -1, 1]),
            shot_index in 0_usize..4,
            density in 0_u32..=8,
            seed in any::<u64>(),
            dirty in any::<bool>(),
        ) {
            let length = width.saturating_add_signed(length_delta);
            let record = (0..length)
                .map(|bit| (seed.wrapping_mul(bit as u64 | 1) >> 61) < u64::from(density))
                .collect::<Vec<_>>();
            let mut packed = PackedShotBatch::zeros(3, width).unwrap();
            let mut reference = BitMatrix::zeros(3, width).unwrap();
            if dirty {
                // A previously written row must be fully replaced, not merged.
                for bit in 0..width {
                    packed.set(shot_index.min(2), bit, true).unwrap();
                    reference.set(shot_index.min(2), bit, true).unwrap();
                }
            }
            let copied = packed.copy_shot_from_bools(shot_index, &record);
            let expected = reference
                .copy_row_from_bools(shot_index, &record)
                .map_err(bit_storage_error);
            prop_assert_eq!(copied, expected);
            prop_assert_eq!(packed.storage, reference);
        }
    }
}
