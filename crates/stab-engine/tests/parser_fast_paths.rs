#![allow(
    clippy::expect_used,
    reason = "parser fast-path tests use direct fixture assertions for compact diagnostics"
)]

mod support;

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_engine::{
    DetectionCompileError, DetectionError, MeasurementToDetectionCompiler, ReferenceSampleMode,
    SamplingCompiler,
};
use stab_model::{Circuit, MeasureRecordOffset, Target};
use support::sample_detections;

#[test]
fn stim_negative_zero_target_preserves_boundary_semantics() {
    assert!(MeasureRecordOffset::try_new(0).is_err());
    let target = "rec[-0]".parse::<Target>().expect("parse Stim text target");
    assert_eq!(target.to_string(), "rec[-0]");
    assert_eq!(
        target
            .measurement_record_offset()
            .map(|offset| offset.get()),
        Some(0)
    );

    let exact =
        Circuit::from_stim_str("M 0\nDETECTOR rec[-0]\n").expect("parse uppercase exact path");
    let generic =
        Circuit::from_stim_str("M 0\ndetector rec[-0]\n").expect("parse lowercase generic path");
    assert_eq!(exact, generic);
    assert_eq!(exact.to_stim_string(), "M 0\nDETECTOR rec[-0]\n");

    let conversion_error = MeasurementToDetectionCompiler::new()
        .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
        .compile(&exact)
        .expect_err("zero lookback must not compile for detection conversion");
    assert!(matches!(
        conversion_error,
        DetectionCompileError::InvalidCircuit(DetectionError::InvalidResultFormat { .. })
    ));
    assert!(conversion_error.to_string().contains("rec[-0]"));

    let detector_model = circuit_to_detector_error_model(&exact, ErrorAnalyzerOptions::default())
        .expect("Stim analyzer treats negative zero as an unused future record target");
    assert_eq!(detector_model.to_dem_string(), "detector D0\n");

    let observable = Circuit::from_stim_str("M 0\nOBSERVABLE_INCLUDE(2) rec[-0]\n")
        .expect("parse negative-zero observable target");
    let observable_model =
        circuit_to_detector_error_model(&observable, ErrorAnalyzerOptions::default())
            .expect("analyze negative-zero observable target");
    assert_eq!(observable_model.to_dem_string(), "logical_observable L2\n");

    let feedback = Circuit::from_stim_str("M 0\nCX rec[-0] 1\n")
        .expect("parse Stim feedback with text-only zero lookback");
    let feedback_model =
        circuit_to_detector_error_model(&feedback, ErrorAnalyzerOptions::default())
            .expect("Stim analyzer treats negative-zero feedback as having no effect");
    assert_eq!(feedback_model.to_dem_string(), "");
    let sampling_error = SamplingCompiler::new()
        .compile(&feedback)
        .expect_err("zero lookback must not compile for sampling");
    assert!(matches!(
        sampling_error,
        stab_engine::SamplingCompileError::InvalidCircuit { .. }
    ));
    assert!(sampling_error.to_string().contains("rec[-0]"));

    let detection_feedback = Circuit::from_stim_str("M 0\nCX rec[-0] 1\nM 1\nDETECTOR rec[-1]\n")
        .expect("parse negative-zero frame-detection feedback");
    let detection_sampling_error = sample_detections(&detection_feedback, 1, Some(5))
        .expect_err("zero lookback must fail frame detection through a controlled error");
    assert!(detection_sampling_error.to_string().contains("rec[-0]"));
}
