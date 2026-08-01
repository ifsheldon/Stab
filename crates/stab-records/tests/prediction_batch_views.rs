#![allow(
    clippy::expect_used,
    reason = "prediction-view tests use compact checked fixtures"
)]

use stab_records::{CorrectionWidth, ObservablePredictionBatch};

#[test]
fn mutable_prediction_prefix_updates_only_admitted_records() {
    let mut predictions =
        ObservablePredictionBatch::zeros(4, CorrectionWidth::new(2)).expect("prediction storage");
    predictions
        .records_mut()
        .copy_shot_from_bools(3, &[true, true])
        .expect("suffix sentinel");

    {
        let mut prefix = predictions
            .view_prefix_mut(3)
            .expect("three-shot mutable prefix");
        assert_eq!(prefix.shot_count(), 3);
        assert_eq!(prefix.correction_width(), CorrectionWidth::new(2));
        assert_eq!(prefix.get(0, 0), Some(false));

        prefix.set(1, 0, true).expect("set admitted bit");
        prefix
            .copy_shot_from_bools(2, &[false, true])
            .expect("replace admitted prediction");

        assert!(prefix.set(3, 0, false).is_err());
        assert!(prefix.copy_shot_from_bools(0, &[true]).is_err());
        assert_eq!(prefix.get(3, 0), None);
        assert_eq!(prefix.view().get(2, 1), Some(true));
    }

    let prefix = predictions.view_prefix(3).expect("immutable prefix");
    assert_eq!(prefix.shot_count(), 3);
    assert_eq!(prefix.get(1, 0), Some(true));
    assert_eq!(prefix.get(2, 1), Some(true));
    assert_eq!(prefix.get(3, 0), None);
    assert_eq!(predictions.records().get(3, 0), Some(true));
    assert_eq!(predictions.records().get(3, 1), Some(true));
    assert!(predictions.view_prefix(5).is_err());
    assert!(predictions.view_prefix_mut(5).is_err());
}

#[test]
fn zero_width_and_zero_length_prediction_prefixes_remain_valid() {
    let mut predictions =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(0)).expect("zero-width storage");

    let mut empty = predictions
        .view_prefix_mut(0)
        .expect("empty mutable prefix");
    assert_eq!(empty.shot_count(), 0);
    assert_eq!(empty.correction_width(), CorrectionWidth::new(0));
    assert!(empty.copy_shot_from_bools(0, &[]).is_err());

    let mut full = predictions
        .view_prefix_mut(2)
        .expect("zero-width full prefix");
    full.copy_shot_from_bools(1, &[])
        .expect("empty prediction record");
    assert_eq!(full.view().shot_count(), 2);
    assert_eq!(full.view().bits_per_shot(), 0);
}

#[test]
fn mutable_prediction_prefix_reuse_allocates_nothing() {
    let mut predictions = ObservablePredictionBatch::zeros(64, CorrectionWidth::new(3))
        .expect("reusable prediction storage");
    let allocations = allocation_counter::measure(|| {
        for shot_count in [1, 17, 64, 3, 0] {
            let mut prefix = predictions
                .view_prefix_mut(shot_count)
                .expect("bounded mutable prefix");
            for shot_index in 0..shot_count {
                prefix
                    .copy_shot_from_bools(shot_index, &[true, false, true])
                    .expect("replace prediction");
            }
            std::hint::black_box(prefix.view());
        }
    });

    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");
}
