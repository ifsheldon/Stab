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
    let errors = PackedShotBatch::from_records(&[vec![false, true], vec![true, false]], 2).unwrap();

    let detection = DetectionBatchView::try_new(detectors.view(), observables.view()).unwrap();
    let dem = DemSampleBatchView::try_new(detection, Some(errors.view().into())).unwrap();

    assert_eq!(
        dem.detection().detectors().get(ShotIndex(0), BitIndex(0)),
        Some(true)
    );
    assert_eq!(
        dem.detection().observables().get(ShotIndex(0), BitIndex(0)),
        Some(true)
    );
    assert_eq!(dem.detection().shot_count(), 2);
    assert_eq!(
        dem.sampled_errors().unwrap().get(ShotIndex(0), BitIndex(1)),
        Some(true)
    );

    let measurement = MeasurementBatchView::new(detectors.view());
    assert_eq!(measurement.shot_count(), 2);
    assert_eq!(measurement.get(ShotIndex(1), BitIndex(1)), Some(true));
    assert!(matches!(measurement.layout(), RecordLayout::ShotMajor(_)));

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
    let dem = DemSampleBatchView::try_new(detection, Some(sampled_errors.view().into())).unwrap();
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
    assert_eq!(packed.get(ShotIndex(0), BitIndex(2)), Some(true));
    assert_eq!(packed.get(ShotIndex(2), BitIndex(0)), None);
    assert_eq!(packed.shot(1).unwrap().get(1), Some(true));
    assert_eq!(packed.to_records().unwrap(), records);
    assert!(PackedShotBatch::from_records(&[vec![true]], 2).is_err());

    packed.set(ShotIndex(1), BitIndex(0), true).unwrap();
    let view = packed.view();
    assert_eq!(view.shot_count(), 2);
    assert_eq!(view.bits_per_shot(), 3);
    assert_eq!(view.get(ShotIndex(1), BitIndex(0)), Some(true));
    assert_eq!(view.shot(1).unwrap().get(0), Some(true));

    let mut planes = BitPlaneBatch::from_shot_major(view).unwrap();
    assert_eq!(planes.shot_count(), 2);
    assert_eq!(planes.bits_per_shot(), 3);
    assert_eq!(planes.get(ShotIndex(1), BitIndex(0)), Some(true));
    assert_eq!(planes.plane(2).unwrap().get(0), Some(true));
    planes.set(ShotIndex(1), BitIndex(2), true).unwrap();
    let plane_view = planes.view();
    assert_eq!(plane_view.shot_count(), 2);
    assert_eq!(plane_view.bits_per_shot(), 3);
    assert_eq!(plane_view.get(ShotIndex(1), BitIndex(2)), Some(true));
    assert_eq!(plane_view.plane(2).unwrap().get(1), Some(true));
    let measurement_planes = MeasurementBatchView::from_bit_planes(plane_view);
    assert_eq!(measurement_planes.shot_count(), 2);
    assert_eq!(measurement_planes.width(), MeasurementWidth::new(3));
    assert_eq!(
        measurement_planes.get(ShotIndex(1), BitIndex(2)),
        Some(true)
    );
    assert!(matches!(
        measurement_planes.layout(),
        RecordLayout::BitPlanes(_)
    ));

    let mut predictions = ObservablePredictionBatch::zeros(2, CorrectionWidth::new(3)).unwrap();
    predictions
        .view_mut()
        .set(ShotIndex(0), BitIndex(1), true)
        .unwrap();
    assert_eq!(
        predictions.records().get(ShotIndex(0), BitIndex(1)),
        Some(true)
    );
    assert_eq!(
        predictions.view().get(ShotIndex(0), BitIndex(1)),
        Some(true)
    );
    assert_eq!(predictions.correction_width(), CorrectionWidth::new(3));
}

#[test]
fn packed_prefix_view_limits_access_without_reallocating_storage() {
    let mut packed = PackedShotBatch::zeros(64, 3).unwrap();
    packed.set(ShotIndex(1), BitIndex(2), true).unwrap();
    packed.set(ShotIndex(2), BitIndex(0), true).unwrap();

    let prefix = packed.view_prefix(2).unwrap();
    assert_eq!(prefix.shot_count(), 2);
    assert_eq!(prefix.bits_per_shot(), 3);
    assert_eq!(prefix.get(ShotIndex(1), BitIndex(2)), Some(true));
    assert_eq!(prefix.get(ShotIndex(2), BitIndex(0)), None);
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
            .map(|bit| packed.get(ShotIndex(1), BitIndex(bit)).unwrap())
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
    assert!((0..10).all(|bit| packed_bytes.get(ShotIndex(0), BitIndex(bit)) == Some(true)));
    packed_bytes.copy_shot_from_b8(0, &[0x01, 0x02]).unwrap();
    assert_eq!(
        (0..10)
            .map(|bit| packed_bytes.get(ShotIndex(0), BitIndex(bit)).unwrap())
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
    planes.set(ShotIndex(0), BitIndex(0), true).unwrap();
    planes.set(ShotIndex(2), BitIndex(0), true).unwrap();
    planes
        .copy_shot_from_bools(1, &[false, true])
        .expect("replace one shot");

    let prefix = planes.view_prefix(2).unwrap();
    assert_eq!(prefix.shot_count(), 2);
    assert_eq!(prefix.get(ShotIndex(0), BitIndex(0)), Some(true));
    assert_eq!(prefix.get(ShotIndex(1), BitIndex(0)), Some(false));
    assert_eq!(prefix.get(ShotIndex(2), BitIndex(0)), None);
    assert_eq!(prefix.get(ShotIndex(1), BitIndex(1)), Some(true));
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
                packed.set(ShotIndex(shot), BitIndex(bit), true).unwrap();
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
                    assert_eq!(
                        planes.get(ShotIndex(shot), BitIndex(bit)),
                        packed.get(ShotIndex(shot), BitIndex(bit))
                    );
                }
            }
            assert_eq!(planes.to_shot_major().unwrap(), packed);
            let records = RecordBatchView::from(planes.view());
            assert_eq!(records.shot_count(), shots);
            assert_eq!(records.bits_per_shot(), width);
            if shots > 0 && width > 0 {
                assert_eq!(
                    records.get(ShotIndex(shots - 1), BitIndex(width - 1)),
                    packed.get(ShotIndex(shots - 1), BitIndex(width - 1))
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
            let expected =
                (shot < shots).then(|| packed.get(ShotIndex(shot), BitIndex(bit)).unwrap());
            assert_eq!(view.get(ShotIndex(shot), BitIndex(bit)), expected);
            if shot < shots {
                assert_eq!(rows.get(ShotIndex(shot), BitIndex(bit)), expected);
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
                assert_eq!(
                    window.get(ShotIndex(shot), BitIndex(bit)),
                    packed.get(ShotIndex(first + shot), BitIndex(bit))
                );
                assert_eq!(
                    rows.get(ShotIndex(shot), BitIndex(bit)),
                    packed.get(ShotIndex(first + shot), BitIndex(bit))
                );
            }
        }
        assert_eq!(window.get(ShotIndex(count), BitIndex(0)), None);
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
    assert_eq!(view.get(ShotIndex(0), BitIndex(0)), Some(true));
    assert_eq!(view.get(ShotIndex(1), BitIndex(0)), Some(true));
    assert_eq!(view.get(ShotIndex(2), BitIndex(0)), None);
    assert_eq!(view.get(ShotIndex(0), BitIndex(1)), Some(false));
    assert_eq!(view.get(ShotIndex(1), BitIndex(1)), Some(true));
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
                source.set(ShotIndex(shot), BitIndex(bit), value).unwrap();
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
                packed.set(ShotIndex(shot_index.min(2)), BitIndex(bit), true).unwrap();
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
