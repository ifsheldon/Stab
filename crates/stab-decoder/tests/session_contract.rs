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
    reported_failure_progress: Option<usize>,
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
            reported_failure_progress: None,
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
        if let Some(completed_shots) = self.reported_failure_progress {
            return Err(DecodeSessionFailure::new(FixtureError, completed_shots));
        }
        let mut completed = 0;
        while completed < requested {
            if cancellation.is_cancelled() {
                return Ok(DecodeBatchSummary::cancelled(requested, completed));
            }
            if self.fail_after == Some(completed) {
                return Err(DecodeSessionFailure::new(FixtureError, completed));
            }
            let prediction = batch
                .detector(completed, 0)
                .expect("validated detector read");
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

fn filled_predictions(
    shot_capacity: usize,
    correction_width: usize,
    value: bool,
) -> ObservablePredictionBatch {
    let mut predictions =
        ObservablePredictionBatch::zeros(shot_capacity, CorrectionWidth::new(correction_width))
            .expect("prediction batch");
    let record = vec![value; correction_width];
    for shot in 0..shot_capacity {
        predictions
            .records_mut()
            .copy_shot_from_bools(shot, &record)
            .expect("prediction sentinel");
    }
    predictions
}

fn prediction_bits(predictions: &ObservablePredictionBatch) -> Vec<Vec<bool>> {
    predictions
        .records()
        .to_records()
        .expect("prediction records")
}

fn decode_fixture(session: &mut FixtureSession, records: &[Vec<bool>]) -> Vec<Vec<bool>> {
    let detectors = detector_records(records);
    let mut predictions = filled_predictions(records.len(), 1, false);
    decode_batch(
        session,
        DecoderInputBatchView::from_detectors(detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect("fixture decode");
    prediction_bits(&predictions)
}

#[test]
fn decoder_session_layout_projection_reuse_and_partitioning() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.125) D0 D2 L1\nshift_detectors 4\nerror(0.25) D0 L0\n",
    )
    .expect("model");
    let model_view = DecoderModelView::try_new(&model).expect("decoder model view");
    assert_eq!(model_view.layout(), layout(5, 2));
    assert_eq!(model_view.fingerprint(), model.fingerprint());

    let detection_detectors = detector_records(&[vec![true, false], vec![false, true]]);
    let observable_truth = detector_records(&[vec![true], vec![false]]);
    let detection =
        DetectionBatchView::try_new(detection_detectors.view(), observable_truth.view())
            .expect("detection view");
    let decoder_input = DecoderInputBatchView::from_detection(detection);
    assert_eq!(decoder_input.detector_width(), DetectorWidth::new(2));
    assert_eq!(decoder_input.shot_count(), 2);
    assert_eq!(decoder_input.detector(0, 0), Some(true));
    assert_eq!(decoder_input.detector(1, 1), Some(true));

    let all = vec![vec![false], vec![true], vec![true], vec![false]];
    let mut whole_session = FixtureSession::new(layout(1, 1));
    let whole = decode_fixture(&mut whole_session, &all);
    let mut reused_session = FixtureSession::new(layout(1, 1));
    let mut partitioned = decode_fixture(&mut reused_session, &all[..2]);
    partitioned.extend(decode_fixture(&mut reused_session, &all[2..]));
    assert_eq!(partitioned, whole);
    assert_eq!(whole_session.calls, 1);
    assert_eq!(reused_session.calls, 2);
    assert_eq!(reused_session.layout(), layout(1, 1));
}

#[test]
fn decoder_session_preflight_is_atomic() {
    let one_detector = detector_records(&[vec![true], vec![false]]);
    let two_detectors = detector_records(&[vec![true, false], vec![false, true]]);
    let mut preflight_session = FixtureSession::new(layout(1, 1));
    let mut predictions = filled_predictions(2, 1, true);
    let before = predictions.clone();
    let error = decode_batch(
        &mut preflight_session,
        DecoderInputBatchView::from_detectors(two_detectors.view()),
        &mut predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("detector width mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::DetectorWidth {
            expected,
            actual
        }) if expected == DetectorWidth::new(1) && actual == DetectorWidth::new(2)
    ));
    assert_eq!(predictions, before);

    let mut wrong_correction_width = filled_predictions(2, 2, true);
    let before = wrong_correction_width.clone();
    let error = decode_batch(
        &mut preflight_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut wrong_correction_width,
        &DecodeCancellation::new(),
    )
    .expect_err("correction width mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::CorrectionWidth {
            expected,
            actual
        }) if expected == CorrectionWidth::new(1) && actual == CorrectionWidth::new(2)
    ));
    assert_eq!(wrong_correction_width, before);

    let mut too_short = filled_predictions(1, 1, true);
    let before = too_short.clone();
    let error = decode_batch(
        &mut preflight_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut too_short,
        &DecodeCancellation::new(),
    )
    .expect_err("prediction capacity mismatch");
    assert!(matches!(
        error,
        DecodeBatchError::Preflight(DecodePreflightError::PredictionShotCapacity {
            required: 2,
            available: 1
        })
    ));
    assert_eq!(too_short, before);
    assert_eq!(preflight_session.calls, 0);
}

#[test]
fn decoder_session_cancellation_and_failure_preserve_progress() {
    let one_detector = detector_records(&[vec![true], vec![false]]);
    let mut pre_cancelled_session = FixtureSession::new(layout(1, 1));
    let cancellation = DecodeCancellation::new();
    cancellation.cancel();
    let mut pre_cancelled_predictions = filled_predictions(2, 1, true);
    let before = pre_cancelled_predictions.clone();
    let summary = decode_batch(
        &mut pre_cancelled_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut pre_cancelled_predictions,
        &cancellation,
    )
    .expect("pre-cancelled summary");
    assert_eq!(summary.status(), DecodeBatchStatus::Cancelled);
    assert_eq!(summary.requested_shots(), 2);
    assert_eq!(summary.completed_shots(), 0);
    assert_eq!(pre_cancelled_predictions, before);
    assert_eq!(pre_cancelled_session.calls, 0);

    let cancellation_records =
        detector_records(&[vec![true], vec![false], vec![true], vec![false]]);
    let mut cancellation_predictions = filled_predictions(6, 1, true);
    let mut cancellation_session = FixtureSession::new(layout(1, 1));
    cancellation_session.cancel_after = Some(2);
    let summary = decode_batch(
        &mut cancellation_session,
        DecoderInputBatchView::from_detectors(cancellation_records.view()),
        &mut cancellation_predictions,
        &DecodeCancellation::new(),
    )
    .expect("mid-batch cancellation");
    assert_eq!(summary.status(), DecodeBatchStatus::Cancelled);
    assert_eq!(summary.requested_shots(), 4);
    assert_eq!(summary.completed_shots(), 2);
    assert_eq!(
        prediction_bits(&cancellation_predictions),
        vec![
            vec![true],
            vec![false],
            vec![true],
            vec![true],
            vec![true],
            vec![true],
        ]
    );

    let failure_records = detector_records(&[vec![false], vec![true], vec![false]]);
    let mut failure_predictions = filled_predictions(3, 1, true);
    let mut failure_session = FixtureSession::new(layout(1, 1));
    failure_session.fail_after = Some(1);
    let error = decode_batch(
        &mut failure_session,
        DecoderInputBatchView::from_detectors(failure_records.view()),
        &mut failure_predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("session failure");
    assert!(matches!(&error, DecodeBatchError::Session(_)));
    if let DecodeBatchError::Session(failure) = error {
        assert_eq!(failure.completed_shots(), 1);
        assert_eq!(failure.source_ref(), &FixtureError);
    }
    assert_eq!(
        prediction_bits(&failure_predictions),
        vec![vec![false], vec![true], vec![true]]
    );
}

#[test]
fn decoder_session_protocol_and_zero_shot_contracts() {
    let failure_records = detector_records(&[vec![false], vec![true], vec![false]]);
    let mut failure_predictions = filled_predictions(3, 1, true);
    let mut invalid_failure_session = FixtureSession::new(layout(1, 1));
    invalid_failure_session.reported_failure_progress = Some(4);
    let error = decode_batch(
        &mut invalid_failure_session,
        DecoderInputBatchView::from_detectors(failure_records.view()),
        &mut failure_predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("failure progress beyond request");
    assert!(matches!(
        error,
        DecodeBatchError::Contract(DecodeContractError::FailureProgress {
            requested: 3,
            actual: 4
        })
    ));

    let one_detector = detector_records(&[vec![true], vec![false]]);
    let mut invalid_summary_session = FixtureSession::new(layout(1, 1));
    invalid_summary_session.returned_summary = Some(DecodeBatchSummary::cancelled(3, 1));
    let mut two_predictions = filled_predictions(2, 1, false);
    let error = decode_batch(
        &mut invalid_summary_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut two_predictions,
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

    let mut overflow_summary_session = FixtureSession::new(layout(1, 1));
    overflow_summary_session.returned_summary = Some(DecodeBatchSummary::cancelled(2, 3));
    let error = decode_batch(
        &mut overflow_summary_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut two_predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("completed progress beyond request");
    assert!(matches!(
        error,
        DecodeBatchError::Contract(DecodeContractError::CompletedShotCount {
            requested: 2,
            actual: 3
        })
    ));

    let mut changing_layout_session = FixtureSession::new(layout(1, 1));
    changing_layout_session.replacement_layout = Some(layout(2, 1));
    let error = decode_batch(
        &mut changing_layout_session,
        DecoderInputBatchView::from_detectors(one_detector.view()),
        &mut two_predictions,
        &DecodeCancellation::new(),
    )
    .expect_err("layout changed during dispatch");
    assert!(matches!(
        error,
        DecodeBatchError::Contract(DecodeContractError::SessionLayoutChanged { .. })
    ));

    let empty_detectors = PackedShotBatch::zeros(0, 1).expect("empty detectors");
    let mut reusable_predictions = filled_predictions(3, 1, true);
    let before = reusable_predictions.clone();
    let mut empty_session = FixtureSession::new(layout(1, 1));
    let summary = decode_batch(
        &mut empty_session,
        DecoderInputBatchView::from_detectors(empty_detectors.view()),
        &mut reusable_predictions,
        &DecodeCancellation::new(),
    )
    .expect("zero-shot batch");
    assert_eq!(summary.status(), DecodeBatchStatus::Completed);
    assert_eq!(summary.requested_shots(), 0);
    assert_eq!(summary.completed_shots(), 0);
    assert_eq!(reusable_predictions, before);
    assert_eq!(empty_session.calls, 1);
}
