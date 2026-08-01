#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "exact decoder tests use compact admitted fixtures"
)]

use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeCancellation, DecoderInputBatchView, decode_batch,
};
use stab_model::{DemDetectorId, DemInstruction, DemTarget, DetectorErrorModel, Probability};
use stab_records::{CorrectionWidth, ObservablePredictionBatch, PackedShotBatch};
use stab_reference_decoder::{ExactMlCompileError, ExactMlDecodeError, ExactMlDecoderSession};

fn records(records: &[Vec<bool>]) -> PackedShotBatch {
    let width = records.first().map_or(0, Vec::len);
    PackedShotBatch::from_records(records, width).expect("records")
}

fn predictions(batch: &ObservablePredictionBatch) -> Vec<Vec<bool>> {
    batch.records().to_records().expect("predictions")
}

#[test]
fn exact_predictions_cover_certain_tied_duplicate_and_zero_observable_models() {
    let correlated = DetectorErrorModel::from_dem_str("error(0.75) D0 L0\n").expect("DEM");
    let session = ExactMlDecoderSession::try_compile_model(&correlated).expect("compile");
    assert!(!session.prediction_for_syndrome(0).expect("D0=0"));
    assert!(session.prediction_for_syndrome(1).expect("D0=1"));

    let tie = DetectorErrorModel::from_dem_str("error(0.5) L0\n").expect("tie DEM");
    let session = ExactMlDecoderSession::try_compile_model(&tie).expect("compile tie");
    assert!(!session.prediction_for_syndrome(0).expect("tie picks zero"));

    let duplicate =
        DetectorErrorModel::from_dem_str("error(1) D0 D0 L0 L0\n").expect("duplicate DEM");
    let session = ExactMlDecoderSession::try_compile_model(&duplicate).expect("compile duplicate");
    assert!(!session.prediction_for_syndrome(0).expect("zero effect"));
    assert!(matches!(
        session.prediction_for_syndrome(1),
        Err(ExactMlDecodeError::ImpossibleSyndrome { syndrome: 1 })
    ));

    let no_observable =
        DetectorErrorModel::from_dem_str("error(0.25) D0\n").expect("zero-observable DEM");
    let mut session =
        ExactMlDecoderSession::try_compile_model(&no_observable).expect("compile zero observable");
    let detector_records = records(&[vec![false], vec![true]]);
    let mut output =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(0)).expect("empty predictions");
    let summary = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detector_records.view()),
        &mut output,
        &DecodeCancellation::new(),
    )
    .expect("decode without observables");
    assert_eq!(summary.status(), DecodeBatchStatus::Completed);
    assert_eq!(predictions(&output), vec![Vec::<bool>::new(), Vec::new()]);
}

#[test]
fn impossible_batch_syndrome_fails_before_any_prediction_mutation() {
    let model = DetectorErrorModel::from_dem_str("error(1) D0 L0\n").expect("DEM");
    let mut session = ExactMlDecoderSession::try_compile_model(&model).expect("compile");
    let detector_records = records(&[vec![true], vec![false]]);
    let mut output =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("predictions");
    output
        .records_mut()
        .copy_shot_from_bools(0, &[false])
        .expect("sentinel zero");
    output
        .records_mut()
        .copy_shot_from_bools(1, &[true])
        .expect("sentinel one");
    let before = output.clone();

    let error = decode_batch(
        &mut session,
        DecoderInputBatchView::from_detectors(detector_records.view()),
        &mut output,
        &DecodeCancellation::new(),
    )
    .expect_err("second syndrome is impossible");

    assert!(matches!(
        error,
        DecodeBatchError::Session(failure)
            if failure.completed_shots() == 0
                && matches!(
                    failure.source_ref(),
                    ExactMlDecodeError::ImpossibleBatchSyndrome {
                        shot_index: 1,
                        syndrome: 0
                    }
                )
    ));
    assert_eq!(output, before);
}

#[test]
fn reused_decode_is_allocation_free_and_partition_equivalent() {
    let model = DetectorErrorModel::from_dem_str(
        "error(0.125) D0 L0\nerror(0.25) D1\nerror(0.375) D0 D1 L0\n",
    )
    .expect("DEM");
    let mut whole_session = ExactMlDecoderSession::try_compile_model(&model).expect("compile");
    let whole_records = records(&[
        vec![false, false],
        vec![true, false],
        vec![false, true],
        vec![true, true],
    ]);
    let mut whole_output =
        ObservablePredictionBatch::zeros(4, CorrectionWidth::new(1)).expect("predictions");
    let cancellation = DecodeCancellation::new();

    decode_batch(
        &mut whole_session,
        DecoderInputBatchView::from_detectors(whole_records.view()),
        &mut whole_output,
        &cancellation,
    )
    .expect("warm decode");
    let allocation = allocation_counter::measure(|| {
        decode_batch(
            &mut whole_session,
            DecoderInputBatchView::from_detectors(whole_records.view()),
            &mut whole_output,
            &cancellation,
        )
        .expect("allocation-measured decode");
    });
    assert_eq!(allocation.count_total, 0, "{allocation:?}");
    assert_eq!(allocation.bytes_total, 0, "{allocation:?}");

    let left_records = records(&[vec![false, false], vec![true, false]]);
    let right_records = records(&[vec![false, true], vec![true, true]]);
    let mut partitioned_session =
        ExactMlDecoderSession::try_compile_model(&model).expect("compile partitioned");
    let mut left_output =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("left predictions");
    let mut right_output =
        ObservablePredictionBatch::zeros(2, CorrectionWidth::new(1)).expect("right predictions");
    decode_batch(
        &mut partitioned_session,
        DecoderInputBatchView::from_detectors(left_records.view()),
        &mut left_output,
        &DecodeCancellation::new(),
    )
    .expect("left decode");
    decode_batch(
        &mut partitioned_session,
        DecoderInputBatchView::from_detectors(right_records.view()),
        &mut right_output,
        &DecodeCancellation::new(),
    )
    .expect("right decode");
    let mut partitioned = predictions(&left_output);
    partitioned.extend(predictions(&right_output));
    assert_eq!(partitioned, predictions(&whole_output));
}

#[test]
fn retained_table_is_exactly_one_byte_per_detector_syndrome() {
    for detector_count in [0, 1, 5, 12] {
        let model = if detector_count == 0 {
            DetectorErrorModel::from_dem_str("error(0.25) L0\n").expect("no-detector DEM")
        } else {
            DetectorErrorModel::from_dem_str(&format!("error(0.25) D{} L0\n", detector_count - 1))
                .expect("detector DEM")
        };
        let session = ExactMlDecoderSession::try_compile_model(&model).expect("compile");
        assert_eq!(session.syndrome_count(), 1 << detector_count);
        assert_eq!(session.retained_prediction_bytes(), 1 << detector_count);
    }
}

#[test]
fn fixed_width_and_mechanism_limits_accept_maxima_and_reject_first_excess() {
    let detector_max =
        DetectorErrorModel::from_dem_str("error(0) D19 L0\n").expect("20-detector DEM");
    assert!(ExactMlDecoderSession::try_compile_model(&detector_max).is_ok());

    let detector_over =
        DetectorErrorModel::from_dem_str("error(0) D20 L0\n").expect("21-detector DEM");
    assert!(matches!(
        ExactMlDecoderSession::try_compile_model(&detector_over),
        Err(ExactMlCompileError::DetectorWidth {
            actual: 21,
            limit: 20
        })
    ));

    let observable_over =
        DetectorErrorModel::from_dem_str("error(0) L1\n").expect("two-observable DEM");
    assert!(matches!(
        ExactMlDecoderSession::try_compile_model(&observable_over),
        Err(ExactMlCompileError::ObservableWidth {
            actual: 2,
            limit: 1
        })
    ));

    let mechanism_max = DetectorErrorModel::from_dem_str("repeat 256 {\n    error(0) D0\n}\n")
        .expect("256-mechanism DEM");
    assert!(ExactMlDecoderSession::try_compile_model(&mechanism_max).is_ok());

    let mechanism_over = DetectorErrorModel::from_dem_str("repeat 257 {\n    error(0) D0\n}\n")
        .expect("257-mechanism DEM");
    assert!(matches!(
        ExactMlDecoderSession::try_compile_model(&mechanism_over),
        Err(ExactMlCompileError::MechanismLimit {
            actual: 257,
            limit: 256
        })
    ));
}

#[test]
fn represented_instruction_limit_accepts_exact_maximum_and_rejects_next() {
    let at_limit = model_with_instruction_visits(65_536);
    assert!(ExactMlDecoderSession::try_compile_model(&at_limit).is_ok());

    let over_limit = model_with_instruction_visits(65_537);
    assert!(matches!(
        ExactMlDecoderSession::try_compile_model(&over_limit),
        Err(ExactMlCompileError::InstructionVisitLimit {
            actual_at_least: 65_537,
            limit: 65_536
        })
    ));
}

fn model_with_instruction_visits(instruction_count: usize) -> DetectorErrorModel {
    let detector = DemInstruction::detector(
        Vec::new(),
        DemTarget::RelativeDetector(DemDetectorId::try_new(0).expect("D0")),
        None,
    )
    .expect("detector annotation");
    let error = DemInstruction::error(
        Probability::try_new(0.0).expect("zero probability"),
        vec![DemTarget::RelativeDetector(
            DemDetectorId::try_new(0).expect("D0"),
        )],
        None,
    )
    .expect("error instruction");
    let mut model = DetectorErrorModel::new();
    for _ in 1..instruction_count {
        model.push_instruction(detector.clone());
    }
    model.push_instruction(error);
    model
}
