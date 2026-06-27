#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string()
}

#[test]
fn dem_analyzer_preserves_instruction_tags_on_errors_and_declarations() {
    let dem = analyze(
        "
        R[test-tag-0] 0
        X_ERROR[test-tag-1](0.25) 0
        M[test-tag-2] 0
        DETECTOR[test-tag-3] rec[-1]
        OBSERVABLE_INCLUDE[test-tag-4](0) rec[-1]
        SHIFT_COORDS[test-tag-5](1)
        ",
    );
    assert_eq!(
        dem,
        "error[test-tag-1](0.25) D0 L0\n\
         detector[test-tag-3] D0\n\
         logical_observable[test-tag-4] L0\n\
         shift_detectors[test-tag-5](1) 0\n"
    );
}

#[test]
fn dem_analyzer_preserves_tagged_empty_observable_declarations() {
    let dem = analyze(
        "
        OBSERVABLE_INCLUDE[test-tag-1](0)
        OBSERVABLE_INCLUDE[test-tag-2](0)
        ",
    );
    assert_eq!(
        dem,
        "logical_observable[test-tag-1] L0\n\
         logical_observable[test-tag-2] L0\n"
    );
}

#[test]
fn dem_analyzer_preserves_tags_when_folding_prefixed_repeat() {
    let circuit = Circuit::from_stim_str(
        "
        R 0
        X_ERROR[test-tag-0](0.25) 0
        REPEAT[test-tag-1] 100 {
            X_ERROR[test-tag-2](0.125) 0
            MR 0
            DETECTOR[test-tag-3] rec[-1]
            OBSERVABLE_INCLUDE[test-tag-4](0) rec[-1]
        }
        ",
    )
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string();

    assert_eq!(
        dem,
        concat!(
            "error[test-tag-0](0.25) D0 L0\n",
            "repeat[test-tag-1] 99 {\n",
            "    error[test-tag-2](0.125) D0 L0\n",
            "    detector[test-tag-3] D0\n",
            "    logical_observable[test-tag-4] L0\n",
            "    shift_detectors 1\n",
            "}\n",
            "error[test-tag-2](0.125) D0 L0\n",
            "detector[test-tag-3] D0\n",
            "logical_observable[test-tag-4] L0\n",
        )
    );
}
