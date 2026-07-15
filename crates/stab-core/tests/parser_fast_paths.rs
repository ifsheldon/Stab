#![allow(
    clippy::expect_used,
    reason = "parser fast-path tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    Circuit, CircuitError, CircuitItem, CompiledDetectionConverter, CompiledSampler,
    DetectionConversionOptions, ErrorAnalyzerOptions, MeasureRecordOffset, Target,
    circuit_to_detector_error_model, sample_detection_events,
};

#[test]
fn common_phase_and_annotation_paths_preserve_public_semantics() {
    let exact = Circuit::from_stim_str("S 1\nTICK\nDETECTOR rec[-1]\n")
        .expect("parse exact common instructions");
    let generic = Circuit::from_stim_str("s    1\n tick\n detector  rec[-1]\n")
        .expect("parse generic common instructions");

    assert_eq!(exact, generic);
    assert_eq!(exact.to_stim_string(), "S 1\nTICK\nDETECTOR rec[-1]\n");

    let decorated =
        Circuit::from_stim_str("S[tag] 1\nTICK[tag]\nDETECTOR[tag](1, 2) rec[-1] rec[-2]\n")
            .expect("parse decorated generic fallbacks");
    assert_eq!(
        decorated.to_stim_string(),
        "S[tag] 1\nTICK[tag]\nDETECTOR[tag](1, 2) rec[-1] rec[-2]\n"
    );
}

#[test]
fn detector_fast_path_preserves_generic_unicode_whitespace() {
    for separator in ['\u{a0}', '\u{2003}'] {
        let exact = Circuit::from_stim_str(&format!("DETECTOR rec[-1]{separator}rec[-2]\n"))
            .expect("parse uppercase detector with Unicode whitespace");
        let generic = Circuit::from_stim_str(&format!("detector rec[-1]{separator}rec[-2]\n"))
            .expect("parse lowercase detector with Unicode whitespace");

        assert_eq!(exact, generic);
        assert_eq!(exact.to_stim_string(), "DETECTOR rec[-1] rec[-2]\n");
    }
}

#[test]
fn qualification_cycle_uses_one_bounded_item_allocation() {
    const INSTRUCTION_COUNT: usize = 4_096;
    const CYCLE: [&str; 6] = [
        "H 0\n",
        "S 1\n",
        "CX 0 1\n",
        "M 0\n",
        "DETECTOR rec[-1]\n",
        "TICK\n",
    ];

    let mut input = String::with_capacity(INSTRUCTION_COUNT * 12);
    for instruction in CYCLE.iter().cycle().take(INSTRUCTION_COUNT) {
        input.push_str(instruction);
    }
    let parsed = Circuit::from_stim_str(&input).expect("warm qualification-cycle parse");
    assert_eq!(parsed.items().len(), INSTRUCTION_COUNT);
    std::hint::black_box(parsed);

    let allocations = allocation_counter::measure(|| {
        let parsed = Circuit::from_stim_str(&input).expect("measured qualification-cycle parse");
        std::hint::black_box(parsed.items().len());
    });
    let expected_bytes = u64::try_from(std::mem::size_of::<CircuitItem>() * INSTRUCTION_COUNT)
        .expect("qualification-cycle allocation size fits u64");
    assert_eq!(allocations.count_total, 1, "{allocations:?}");
    assert_eq!(allocations.count_max, 1, "{allocations:?}");
    assert_eq!(allocations.bytes_total, expected_bytes, "{allocations:?}");
    assert_eq!(allocations.bytes_max, expected_bytes, "{allocations:?}");
}

#[test]
fn exact_detector_fast_candidates_preserve_target_boundaries() {
    for invalid in [
        "DETECTOR rec[-16777216]\n",
        "DETECTOR rec[-999999999999999999999]\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}

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

    let conversion_error = CompiledDetectionConverter::compile(
        &exact,
        DetectionConversionOptions {
            skip_reference_sample: true,
        },
    )
    .expect_err("zero lookback must not compile for detection conversion");
    assert!(matches!(
        conversion_error,
        CircuitError::InvalidResultFormat { .. }
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
    let sampling_error = CompiledSampler::compile(&feedback)
        .expect_err("zero lookback must not compile for sampling");
    assert!(matches!(
        sampling_error,
        CircuitError::InvalidSamplerCompilation { .. }
    ));
    assert!(sampling_error.to_string().contains("rec[-0]"));

    let detection_feedback = Circuit::from_stim_str("M 0\nCX rec[-0] 1\nM 1\nDETECTOR rec[-1]\n")
        .expect("parse negative-zero frame-detection feedback");
    let detection_sampling_error = sample_detection_events(&detection_feedback, 1, Some(5))
        .expect_err("zero lookback must fail frame detection through a controlled error");
    assert!(
        matches!(
            detection_sampling_error,
            CircuitError::InvalidSamplerCompilation { .. }
        ),
        "{detection_sampling_error:?}"
    );
    assert!(detection_sampling_error.to_string().contains("rec[-0]"));
}
