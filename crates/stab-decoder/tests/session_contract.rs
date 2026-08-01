#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "decoder contract tests use compact exact fixtures"
)]

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeBatchSummary, DecodeCancellation,
    DecodeContractError, DecodePreflightError, DecodeSessionFailure, DecoderInputBatchView,
    DecoderLayout, DecoderModelView, DecoderSession, ValidatedDecodeBatch, decode_batch,
};
use stab_model::DetectorErrorModel;
use stab_records::{
    CorrectionWidth, DetectionBatchView, DetectorWidth, ObservablePredictionBatch, ObservableWidth,
    PackedShotBatch,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FixtureError;

impl Display for FixtureError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("fixture decoder failed")
    }
}

impl Error for FixtureError {}

#[derive(Debug)]
struct FixtureSession {
    layout: DecoderLayout,
    calls: usize,
    cancel_after: Option<usize>,
    fail_after: Option<usize>,
    returned_summary: Option<DecodeBatchSummary>,
    replacement_layout: Option<DecoderLayout>,
}

impl FixtureSession {
    fn new(layout: DecoderLayout) -> Self {
        Self {
            layout,
            calls: 0,
            cancel_after: None,
            fail_after: None,
            returned_summary: None,
            replacement_layout: None,
        }
    }
}

impl DecoderSession for FixtureSession {
    type Error = FixtureError;

    fn layout(&self) -> DecoderLayout {
        self.layout
    }

    fn decode_validated_batch(
        &mut self,
        mut batch: ValidatedDecodeBatch<'_, '_>,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodeBatchSummary, DecodeSessionFailure<Self::Error>> {
        self.calls += 1;
        let requested = batch.shot_count();
        let mut completed = 0;
        while completed < requested {
            if cancellation.is_cancelled() {
                return Ok(DecodeBatchSummary::cancelled(requested, completed));
            }
            if self.fail_after == Some(completed) {
                return Err(DecodeSessionFailure::new(FixtureError, completed));
            }
            let prediction = batch.detector(completed, 0).unwrap_or(false);
            batch
                .set_prediction(completed, 0, prediction)
                .expect("validated prediction write");
            completed += 1;
            if self.cancel_after == Some(completed) {
                cancellation.cancel();
            }
        }
        if let Some(layout) = self.replacement_layout {
            self.layout = layout;
        }
        Ok(self
            .returned_summary
            .unwrap_or_else(|| DecodeBatchSummary::completed(requested)))
    }
}

fn layout(detectors: usize, observables: usize) -> DecoderLayout {
    DecoderLayout::new(
        DetectorWidth::new(detectors),
        ObservableWidth::new(observables),
    )
}

fn detector_records(records: &[Vec<bool>]) -> PackedShotBatch {
    let width = records.first().map_or(0, Vec::len);
    PackedShotBatch::from_records(records, width).expect("detector records")
}

fn prediction_bits(predictions: &ObservablePredictionBatch) -> Vec<Vec<bool>> {
    predictions
        .records()
        .to_records()
        .expect("prediction records")
}

#[test]
fn model_view_derives_exact_layout_and_fingerprint() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.125) D0 D2 L1\nshift_detectors 4\nerror(0.25) D0 L0\n",
    )
    .expect("model");
    let view = DecoderModelView::try_new(&model).expect("decoder model view");

    assert_eq!(view.model().to_dem_string(), model.to_dem_string());
    assert_eq!(view.layout(), layout(5, 2));
    assert_eq!(view.fingerprint(), model.fingerprint());
    assert_eq!(view.layout().correction_width(), CorrectionWidth::new(2));
}

#[test]
fn detection_input_drops_observable_truth() {
    let detectors = detector_records(&[vec![true, false], vec![false, true]]);
    let observables = detector_records(&[vec![true], vec![false]]);
    let detection =
        DetectionBatchView::try_new(detectors.view(), observables.view()).expect("detection");

    let input = DecoderInputBatchView::from_detection(detection);

    assert_eq!(input.detector_width(), DetectorWidth::new(2));
    assert_eq!(input.shot_count(), 2);
    assert_eq!(input.detector(0, 0), Some(true));
    assert_eq!(input.detector(1, 1), Some(true));
    assert_eq!(input.detectors().bits_per_shot(), 2);
}

#[test]
fn dimensional_failures_precede_dispatch_and_prediction_mutation() {
    let one_detector = detector_records(&[vec![true], vec![false]]);
    let two_detectors = detector_records(&[vec![true, false], vec![false, true]]);
    let mut session = FixtureSession::new(layout(1, 1));
    let cancellation = DecodeCancellation::new();

    let mut detector_mismatch =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("predictions");
    detector_mismatch
        .records_mut()
        .copy_shot_from_bools(0, &[true])
        .expect("sentinel");
    let before = detector_mismatch.clone();
    let error = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(two_detectors.view()),
        &mut detector_mismatch,
        &cancellation,
    )
    .expect_err("detector width mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::DetectorWidth {
            expected,
            actual
        }) if expected == DetectorWidth::new(1) && actual == DetectorWidth::new(2)
    ));
    assert_eq!(detector_mismatch, before);

    let mut correction_mismatch =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(2)).expect("predictions");
    let before = correction_mismatch.clone();
    let error = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut correction_mismatch,
        &cancellation,
    )
    .expect_err("correction width mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::CorrectionWidth {
            expected,
            actual
        }) if expected == CorrectionWidth::new(1) && actual == CorrectionWidth::new(2)
    ));
    assert_eq!(correction_mismatch, before);

    let mut too_short =
        ObservablePredictionBatch::zeros(1, CorrectionWidth::new(1)).expect("predictions");
    too_short
        .records_mut()
        .copy_shot_from_bools(0, &[true])
        .expect("sentinel");
    let before = too_short.clone();
    let error = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut too_short,
        &cancellation,
    )
    .expect_err("prediction shot capacity mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::PredictionShotCapacity {
            required: 2,
            available: 1
        })
    ));
    assert_eq!(too_short, before);
    assert_eq!(session.calls, 0);
}

#[test]
fn pre_cancelled_batch_returns_without_dispatch_or_mutation() {
    let detectors = detector_records(&[vec![true], vec![false]]);
    let mut predictions =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("predictions");
    predictions
        .records_mut()
        .copy_shot_from_bools(0, &[true])
        .expect("sentinel");
    let before = predictions.clone();
    let mut session = FixtureSession::new(layout(1, 1));
    let cancellation = DecodeCancellation::new();
    cancellation.cancel();

    let summary = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &cancellation,
    )
    .expect("cancelled summary");

    assert_eq!(summary.status(), DecodeBatchStatus::Cancelled);
    assert_eq!(summary.requested_shots(), 2);
    assert_eq!(summary.completed_shots(), 0);
    assert_eq!(predictions, before);
    assert_eq!(session.calls, 0);
}

#[test]
fn cancellation_commits_only_the_completed_prefix() {
    let detectors = detector_records(&[vec![true], vec![false], vec![true], vec![false]]);
    let mut predictions =
        ObservablePredictionBatch::zeros(6, CorrectionWidth::new(1)).expect("predictions");
    for shot in 0..6 {
        predictions
            .records_mut()
            .copy_shot_from_bools(shot, &[true])
            .expect("sentinel");
    }
    let mut session = FixtureSession::new(layout(1, 1));
    session.cancel_after = Some(2);
    let cancellation = DecodeCancellation::new();

    let summary = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &cancellation,
    )
    .expect("cancelled summary");

    assert_eq!(summary.status(), DecodeBatchStatus::Cancelled);
    assert_eq!(summary.requested_shots(), 4);
    assert_eq!(summary.completed_shots(), 2);
    assert_eq!(
        prediction_bits(&predictions),
        vec![
            vec![true],
            vec![false],
            vec![true],
            vec![true],
            vec![true],
            vec![true],
        ]
    );
}

#[test]
fn implementation_failure_preserves_exact_completed_progress() {
    let detectors = detector_records(&[vec![false], vec![true], vec![false]]);
    let mut predictions =
        ObservablePredictionBatch::zeros(3, CorrectionWidth::new(1)).expect("predictions");
    for shot in 0..3 {
        predictions
            .records_mut()
            .copy_shot_from_bools(shot, &[true])
            .expect("sentinel");
    }
    let mut session = FixtureSession::new(layout(1, 1));
    session.fail_after = Some(1);

    let error = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("fixture failure");

    assert!(matches!(&error, DecodeBatchError::Session(_)));
    if let DecodeBatchError::Session(failure) = error {
        assert_eq!(failure.completed_shots(), 1);
        assert_eq!(failure.source_ref(), &FixtureError);
    }
    assert_eq!(
        prediction_bits(&predictions),
        vec![vec![false], vec![true], vec![true]]
    );
}

#[test]
fn malformed_session_progress_and_layout_changes_are_rejected() {
    let detectors = detector_records(&[vec![true], vec![false]]);
    let mut predictions =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("predictions");
    let mut summary_session = FixtureSession::new(layout(1, 1));
    summary_session.returned_summary = Some(DecodeBatchSummary::cancelled(3, 1));

    let error = decode_batch(
        &mut summary_session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("wrong requested shot count");
    assert!(matches!(
        error,
        DecodeBatchError::Contract(DecodeContractError::RequestedShotCount {
            expected: 2,
            actual: 3
        })
    ));

    let mut layout_session = FixtureSession::new(layout(1, 1));
    layout_session.replacement_layout = Some(layout(2, 1));
    let error = decode_batch(
        &mut layout_session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("layout changed during dispatch");
    assert!(matches!(
        error,
        DecodeBatchError::Contract(DecodeContractError::SessionLayoutChanged { .. })
    ));
}

#[test]
fn zero_shot_batches_complete_without_touching_reusable_storage() {
    let detectors = PackedShotBatch::zeros(0, 1).expect("detectors");
    let mut predictions =
        ObservablePredictionBatch::zeros(3, CorrectionWidth::new(1)).expect("predictions");
    for shot in 0..3 {
        predictions
            .records_mut()
            .copy_shot_from_bools(shot, &[true])
            .expect("sentinel");
    }
    let before = predictions.clone();
    let mut session = FixtureSession::new(layout(1, 1));

    let summary = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect("zero-shot decode");

    assert_eq!(summary.status(), DecodeBatchStatus::Completed);
    assert_eq!(summary.requested_shots(), 0);
    assert_eq!(summary.completed_shots(), 0);
    assert_eq!(predictions, before);
    assert_eq!(session.calls, 1);
}
