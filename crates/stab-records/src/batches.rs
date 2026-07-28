use stab_bits::{BitMatrix, BitSlice};

use crate::{
    CorrectionWidth, DetectorWidth, FormatError, FormatErrorCode, MeasurementWidth,
    ObservableWidth, RecordResult, SampledErrorWidth,
};

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

    pub fn from_bit_planes(batch: BitPlane64BatchView<'_>) -> RecordResult<Self> {
        let mut records = Self::zeros(batch.shot_count(), batch.bits_per_shot())?;
        for shot_index in 0..batch.shot_count() {
            for bit_index in 0..batch.bits_per_shot() {
                let value = batch.get(bit_index, shot_index).ok_or_else(|| {
                    FormatError::invalid_data(
                        "bit-plane record escaped its declared dimensions while transposing",
                    )
                })?;
                if value {
                    records.set(shot_index, bit_index, true)?;
                }
            }
        }
        Ok(records)
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

    pub fn copy_shot_from_bools(&mut self, shot_index: usize, record: &[bool]) -> RecordResult<()> {
        self.storage
            .copy_row_from_bools(shot_index, record)
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
}

/// Owned bit planes for at most 64 shots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitPlane64Batch {
    planes: Vec<u64>,
    shot_count: usize,
    bits_per_shot: usize,
}

impl BitPlane64Batch {
    pub fn zeros(shot_count: usize, bits_per_shot: usize) -> RecordResult<Self> {
        validate_bit_plane_shots(shot_count)?;
        let mut planes = Vec::new();
        planes.try_reserve_exact(bits_per_shot).map_err(|error| {
            FormatError::invalid_data(format!(
                "bit-plane batch could not reserve {bits_per_shot} result planes: {error}"
            ))
        })?;
        planes.resize(bits_per_shot, 0);
        Ok(Self {
            planes,
            shot_count,
            bits_per_shot,
        })
    }

    pub fn from_shot_major(batch: PackedShotBatchView<'_>) -> RecordResult<Self> {
        validate_bit_plane_shots(batch.shot_count())?;
        let mut planes = Self::zeros(batch.shot_count(), batch.bits_per_shot())?;
        for shot_index in 0..batch.shot_count() {
            for bit_index in 0..batch.bits_per_shot() {
                let value = batch.get(shot_index, bit_index).ok_or_else(|| {
                    FormatError::invalid_data("packed shot prefix escaped its declared dimensions")
                })?;
                if value {
                    planes.set(bit_index, shot_index, true)?;
                }
            }
        }
        Ok(planes)
    }

    #[inline]
    pub fn shot_count(&self) -> usize {
        self.shot_count
    }

    #[inline]
    pub fn bits_per_shot(&self) -> usize {
        self.bits_per_shot
    }

    pub fn get(&self, bit_index: usize, shot_index: usize) -> Option<bool> {
        if shot_index >= self.shot_count {
            return None;
        }
        self.planes
            .get(bit_index)
            .map(|word| word & (1_u64 << shot_index) != 0)
    }

    pub fn set(&mut self, bit_index: usize, shot_index: usize, value: bool) -> RecordResult<()> {
        if shot_index >= self.shot_count {
            return Err(FormatError::invalid_data(format!(
                "bit-plane shot index {shot_index} is out of range for {} records",
                self.shot_count
            )));
        }
        let word = self.planes.get_mut(bit_index).ok_or_else(|| {
            FormatError::invalid_data(format!(
                "bit-plane index {bit_index} is out of range for {} result bits",
                self.bits_per_shot
            ))
        })?;
        let mask = 1_u64 << shot_index;
        if value {
            *word |= mask;
        } else {
            *word &= !mask;
        }
        Ok(())
    }

    pub fn copy_shot_from_bools(&mut self, shot_index: usize, record: &[bool]) -> RecordResult<()> {
        if shot_index >= self.shot_count() {
            return Err(FormatError::invalid_data(format!(
                "bit-plane shot index {shot_index} is out of range for {} records",
                self.shot_count()
            )));
        }
        if record.len() != self.bits_per_shot() {
            return Err(batch_width_mismatch(
                "bit-plane record",
                record.len(),
                self.bits_per_shot(),
            ));
        }
        for (bit_index, value) in record.iter().copied().enumerate() {
            self.set(bit_index, shot_index, value)?;
        }
        Ok(())
    }

    #[inline]
    pub fn copy_plane_from_word(&mut self, bit_index: usize, word: u64) -> RecordResult<()> {
        let shot_count = self.shot_count;
        let plane = self.planes.get_mut(bit_index).ok_or_else(|| {
            FormatError::invalid_data(format!(
                "bit-plane index {bit_index} is out of range for {} result bits",
                self.bits_per_shot
            ))
        })?;
        *plane = word & low_bits_mask(shot_count);
        Ok(())
    }

    pub fn plane(&self, bit_index: usize) -> RecordResult<BitSlice<'_>> {
        self.view().plane(bit_index)
    }

    pub fn view(&self) -> BitPlane64BatchView<'_> {
        BitPlane64BatchView {
            planes: &self.planes,
            shot_count: self.shot_count,
            bits_per_shot: self.bits_per_shot,
        }
    }

    #[inline]
    pub fn view_prefix(&self, shot_count: usize) -> RecordResult<BitPlane64BatchView<'_>> {
        if shot_count > self.shot_count() {
            return Err(FormatError::invalid_data(format!(
                "bit-plane prefix has {shot_count} records but storage contains only {}",
                self.shot_count()
            )));
        }
        Ok(BitPlane64BatchView {
            planes: &self.planes,
            shot_count,
            bits_per_shot: self.bits_per_shot,
        })
    }

    pub fn to_shot_major(&self) -> RecordResult<PackedShotBatch> {
        PackedShotBatch::from_bit_planes(self.view())
    }
}

/// Borrowed bit planes for at most 64 shots.
///
/// Words supplied through [`Self::try_from_words`] may contain nonzero bits at shot indexes greater
/// than or equal to [`Self::shot_count`]. Logical accessors hide those inactive tail bits. A plane's
/// raw [`BitSlice::words`] retain the caller's storage and must be masked by consumers that inspect
/// words directly.
#[derive(Clone, Copy, Debug)]
pub struct BitPlane64BatchView<'a> {
    planes: &'a [u64],
    shot_count: usize,
    bits_per_shot: usize,
}

impl<'a> BitPlane64BatchView<'a> {
    #[inline(always)]
    pub fn try_from_words(
        planes: &'a [u64],
        shot_count: usize,
        bits_per_shot: usize,
    ) -> RecordResult<Self> {
        validate_bit_plane_shots(shot_count)?;
        if planes.len() != bits_per_shot {
            return Err(batch_width_mismatch(
                "bit-plane word storage",
                planes.len(),
                bits_per_shot,
            ));
        }
        Ok(Self {
            planes,
            shot_count,
            bits_per_shot,
        })
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        self.shot_count
    }

    #[inline]
    pub fn bits_per_shot(self) -> usize {
        self.bits_per_shot
    }

    #[inline]
    pub fn get(self, bit_index: usize, shot_index: usize) -> Option<bool> {
        if shot_index >= self.shot_count {
            return None;
        }
        self.planes
            .get(bit_index)
            .map(|word| word & (1_u64 << shot_index) != 0)
    }

    #[inline]
    pub fn plane(self, bit_index: usize) -> RecordResult<BitSlice<'a>> {
        let word = self.planes.get(bit_index).ok_or_else(|| {
            FormatError::invalid_data(format!(
                "bit-plane index {bit_index} is out of range for {} result bits",
                self.bits_per_shot
            ))
        })?;
        if self.shot_count == 0 {
            return BitSlice::new(&[], 0).map_err(bit_storage_error);
        }
        BitSlice::new(std::slice::from_ref(word), self.shot_count).map_err(bit_storage_error)
    }
}

/// Borrowed measurement records with explicit measurement semantics.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementBatchView<'a> {
    storage: MeasurementBatchStorageView<'a>,
    width: MeasurementWidth,
}

impl<'a> MeasurementBatchView<'a> {
    pub fn new(records: PackedShotBatchView<'a>) -> Self {
        Self {
            storage: MeasurementBatchStorageView::ShotMajor(records),
            width: MeasurementWidth::new(records.bits_per_shot()),
        }
    }

    #[inline]
    pub fn from_bit_planes(bit_planes: BitPlane64BatchView<'a>) -> Self {
        Self {
            storage: MeasurementBatchStorageView::BitPlanes(bit_planes),
            width: MeasurementWidth::new(bit_planes.bits_per_shot()),
        }
    }

    #[inline]
    pub fn shot_count(self) -> usize {
        match self.storage {
            MeasurementBatchStorageView::ShotMajor(records) => records.shot_count(),
            MeasurementBatchStorageView::BitPlanes(bit_planes) => bit_planes.shot_count(),
        }
    }

    pub fn get(self, shot_index: usize, bit_index: usize) -> Option<bool> {
        match self.storage {
            MeasurementBatchStorageView::ShotMajor(records) => records.get(shot_index, bit_index),
            MeasurementBatchStorageView::BitPlanes(bit_planes) => {
                bit_planes.get(bit_index, shot_index)
            }
        }
    }

    pub const fn shot_major_records(self) -> Option<PackedShotBatchView<'a>> {
        match self.storage {
            MeasurementBatchStorageView::ShotMajor(records) => Some(records),
            MeasurementBatchStorageView::BitPlanes(_) => None,
        }
    }

    #[inline]
    pub const fn bit_planes(self) -> Option<BitPlane64BatchView<'a>> {
        match self.storage {
            MeasurementBatchStorageView::ShotMajor(_) => None,
            MeasurementBatchStorageView::BitPlanes(bit_planes) => Some(bit_planes),
        }
    }

    #[inline]
    pub const fn width(self) -> MeasurementWidth {
        self.width
    }
}

#[derive(Clone, Copy, Debug)]
enum MeasurementBatchStorageView<'a> {
    ShotMajor(PackedShotBatchView<'a>),
    BitPlanes(BitPlane64BatchView<'a>),
}

/// Borrowed detector and observable records with independent planes.
#[derive(Clone, Copy, Debug)]
pub struct DetectionBatchView<'a> {
    detectors: PackedShotBatchView<'a>,
    observables: PackedShotBatchView<'a>,
    detector_width: DetectorWidth,
    observable_width: ObservableWidth,
}

impl<'a> DetectionBatchView<'a> {
    pub fn try_new(
        detectors: PackedShotBatchView<'a>,
        observables: PackedShotBatchView<'a>,
    ) -> RecordResult<Self> {
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

    pub const fn detectors(self) -> PackedShotBatchView<'a> {
        self.detectors
    }

    pub const fn observables(self) -> PackedShotBatchView<'a> {
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
    sampled_errors: Option<PackedShotBatchView<'a>>,
    sampled_error_width: Option<SampledErrorWidth>,
}

impl<'a> DemSampleBatchView<'a> {
    pub fn try_new(
        detection: DetectionBatchView<'a>,
        sampled_errors: Option<PackedShotBatchView<'a>>,
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

    pub const fn sampled_errors(self) -> Option<PackedShotBatchView<'a>> {
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

#[inline(always)]
fn validate_bit_plane_shots(shot_count: usize) -> RecordResult<()> {
    if shot_count > 64 {
        return Err(FormatError::invalid_data(format!(
            "bit-plane batch supports at most 64 shots, got {shot_count}"
        )));
    }
    Ok(())
}

const fn low_bits_mask(bit_count: usize) -> u64 {
    if bit_count == u64::BITS as usize {
        u64::MAX
    } else {
        (1_u64 << bit_count) - 1
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

fn batch_width_mismatch(label: &str, actual: usize, expected: usize) -> FormatError {
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

fn bit_storage_error(error: stab_bits::BitError) -> FormatError {
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

    use super::*;

    #[test]
    fn detection_and_dem_views_keep_planes_separate() {
        let detectors =
            PackedShotBatch::from_records(&[vec![true, false], vec![false, true]], 2).unwrap();
        let observables = PackedShotBatch::from_records(&[vec![true], vec![false]], 1).unwrap();
        let errors =
            PackedShotBatch::from_records(&[vec![false, true], vec![true, false]], 2).unwrap();

        let detection = DetectionBatchView::try_new(detectors.view(), observables.view()).unwrap();
        let dem = DemSampleBatchView::try_new(detection, Some(errors.view())).unwrap();

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
        assert!(DemSampleBatchView::try_new(detection, Some(mismatched.view())).is_err());
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
        let dem = DemSampleBatchView::try_new(detection, Some(sampled_errors.view())).unwrap();
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

        let mut planes = BitPlane64Batch::from_shot_major(view).unwrap();
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
    }

    #[test]
    fn bit_plane_prefix_and_word_copy_hide_inactive_storage() {
        let mut planes = BitPlane64Batch::zeros(64, 2).unwrap();
        planes
            .copy_plane_from_word(0, 0b101)
            .expect("copy packed plane");
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

    #[test]
    fn borrowed_bit_plane_words_validate_layout_and_hide_tail_bits() {
        let words = [u64::MAX, 0b10];
        let view = BitPlane64BatchView::try_from_words(&words, 2, 2).unwrap();

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

        assert!(BitPlane64BatchView::try_from_words(&words, 65, 2).is_err());
        assert!(BitPlane64BatchView::try_from_words(&words, 2, 1).is_err());
    }

    #[test]
    fn bit_planes_reject_more_than_64_shots_before_storage() {
        let records = PackedShotBatch::zeros(65, 1).unwrap();
        assert!(BitPlane64Batch::from_shot_major(records.view()).is_err());
        assert!(BitPlane64Batch::zeros(65, 1).is_err());
    }

    #[test]
    fn bit_planes_report_unaddressable_widths_without_panicking() {
        assert!(BitPlane64Batch::zeros(1, usize::MAX).is_err());
    }

    #[test]
    fn typed_writers_match_record_adapters() {
        let records = vec![
            vec![true, false, true, false, true],
            vec![false, true, false, true, false],
        ];
        let batch = PackedShotBatch::from_records(&records, 5).unwrap();
        for format in [
            crate::SampleFormat::ZeroOne,
            crate::SampleFormat::B8,
            crate::SampleFormat::R8,
            crate::SampleFormat::Hits,
            crate::SampleFormat::Dets,
        ] {
            let mut bits_writer = crate::MeasureRecordWriter::new(format);
            for record in &records {
                bits_writer.write_bits(record);
                bits_writer.write_end();
            }
            assert_eq!(
                bits_writer.into_bytes(),
                crate::write_records(&records, format)
            );

            let mut writer = crate::MeasureRecordWriter::new(format);
            writer
                .write_packed_record(batch.view().shot(0).unwrap())
                .unwrap();
            writer.write_end();
            writer
                .write_packed_batch(
                    PackedShotBatch::from_records(&records[1..], 5)
                        .unwrap()
                        .view(),
                )
                .unwrap();
            assert_eq!(writer.into_bytes(), crate::write_records(&records, format));
        }

        let records_64 = (0..64)
            .map(|shot| {
                (0..17)
                    .map(|bit| (shot * 7 + bit * 11) % 13 == 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let packed = PackedShotBatch::from_records(&records_64, 17).unwrap();
        let planes = BitPlane64Batch::from_shot_major(packed.view()).unwrap();
        assert_eq!(
            crate::write_bit_plane_64_batch(planes.view()).unwrap(),
            crate::write_ptb64_records_checked(&records_64).unwrap()
        );

        let empty_planes = BitPlane64Batch::zeros(0, 17).unwrap();
        assert_eq!(
            crate::write_bit_plane_64_batch(empty_planes.view()).unwrap(),
            crate::write_ptb64_records_checked(&[]).unwrap()
        );
    }

    proptest! {
        #[test]
        fn shot_major_and_bit_plane_round_trip(
            shot_count in 0usize..=64,
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

            let planes = BitPlane64Batch::from_shot_major(source.view()).unwrap();
            let round_trip = planes.to_shot_major().unwrap();
            prop_assert_eq!(round_trip, source);
        }
    }
}
