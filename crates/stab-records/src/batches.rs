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
        Ok(Self {
            storage: batch.storage.transpose().map_err(bit_storage_error)?,
        })
    }

    pub fn shot_count(&self) -> usize {
        self.storage.rows()
    }

    pub fn bits_per_shot(&self) -> usize {
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

    pub fn shot(&self, shot_index: usize) -> RecordResult<BitSlice<'_>> {
        self.storage.row(shot_index).map_err(bit_storage_error)
    }

    pub const fn view(&self) -> PackedShotBatchView<'_> {
        PackedShotBatchView {
            storage: &self.storage,
        }
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
}

impl<'a> PackedShotBatchView<'a> {
    pub fn shot_count(self) -> usize {
        self.storage.rows()
    }

    pub fn bits_per_shot(self) -> usize {
        self.storage.cols()
    }

    pub fn get(self, shot_index: usize, bit_index: usize) -> Option<bool> {
        self.storage.get(shot_index, bit_index)
    }

    pub fn shot(self, shot_index: usize) -> RecordResult<BitSlice<'a>> {
        self.storage.row(shot_index).map_err(bit_storage_error)
    }
}

/// Owned bit planes for at most 64 shots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BitPlane64Batch {
    storage: BitMatrix,
}

impl BitPlane64Batch {
    pub fn zeros(shot_count: usize, bits_per_shot: usize) -> RecordResult<Self> {
        validate_bit_plane_shots(shot_count)?;
        Ok(Self {
            storage: BitMatrix::zeros(bits_per_shot, shot_count).map_err(bit_storage_error)?,
        })
    }

    pub fn from_shot_major(batch: PackedShotBatchView<'_>) -> RecordResult<Self> {
        validate_bit_plane_shots(batch.shot_count())?;
        Ok(Self {
            storage: batch.storage.transpose().map_err(bit_storage_error)?,
        })
    }

    pub fn shot_count(&self) -> usize {
        self.storage.cols()
    }

    pub fn bits_per_shot(&self) -> usize {
        self.storage.rows()
    }

    pub fn get(&self, bit_index: usize, shot_index: usize) -> Option<bool> {
        self.storage.get(bit_index, shot_index)
    }

    pub fn set(&mut self, bit_index: usize, shot_index: usize, value: bool) -> RecordResult<()> {
        self.storage
            .set(bit_index, shot_index, value)
            .map_err(bit_storage_error)
    }

    pub fn plane(&self, bit_index: usize) -> RecordResult<BitSlice<'_>> {
        self.storage.row(bit_index).map_err(bit_storage_error)
    }

    pub const fn view(&self) -> BitPlane64BatchView<'_> {
        BitPlane64BatchView {
            storage: &self.storage,
        }
    }

    pub fn to_shot_major(&self) -> RecordResult<PackedShotBatch> {
        PackedShotBatch::from_bit_planes(self.view())
    }
}

/// Borrowed bit planes for at most 64 shots.
#[derive(Clone, Copy, Debug)]
pub struct BitPlane64BatchView<'a> {
    storage: &'a BitMatrix,
}

impl<'a> BitPlane64BatchView<'a> {
    pub fn shot_count(self) -> usize {
        self.storage.cols()
    }

    pub fn bits_per_shot(self) -> usize {
        self.storage.rows()
    }

    pub fn get(self, bit_index: usize, shot_index: usize) -> Option<bool> {
        self.storage.get(bit_index, shot_index)
    }

    pub fn plane(self, bit_index: usize) -> RecordResult<BitSlice<'a>> {
        self.storage.row(bit_index).map_err(bit_storage_error)
    }
}

/// Borrowed measurement records with explicit measurement semantics.
#[derive(Clone, Copy, Debug)]
pub struct MeasurementBatchView<'a> {
    records: PackedShotBatchView<'a>,
    width: MeasurementWidth,
}

impl<'a> MeasurementBatchView<'a> {
    pub fn new(records: PackedShotBatchView<'a>) -> Self {
        Self {
            records,
            width: MeasurementWidth::new(records.bits_per_shot()),
        }
    }

    pub const fn records(self) -> PackedShotBatchView<'a> {
        self.records
    }

    pub const fn width(self) -> MeasurementWidth {
        self.width
    }
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

fn validate_bit_plane_shots(shot_count: usize) -> RecordResult<()> {
    if shot_count > 64 {
        return Err(FormatError::invalid_data(format!(
            "bit-plane batch supports at most 64 shots, got {shot_count}"
        )));
    }
    Ok(())
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
        assert_eq!(measurement.records().shot_count(), 2);

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

        let mut predictions = ObservablePredictionBatch::zeros(2, CorrectionWidth::new(3)).unwrap();
        predictions.records_mut().set(0, 1, true).unwrap();
        assert_eq!(predictions.records().get(0, 1), Some(true));
        assert_eq!(predictions.view().get(0, 1), Some(true));
        assert_eq!(predictions.correction_width(), CorrectionWidth::new(3));
    }

    #[test]
    fn bit_planes_reject_more_than_64_shots_before_storage() {
        let records = PackedShotBatch::zeros(65, 1).unwrap();
        assert!(BitPlane64Batch::from_shot_major(records.view()).is_err());
        assert!(BitPlane64Batch::zeros(65, 1).is_err());
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
