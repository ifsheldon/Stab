#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::Circuit;

fn analyze(text: &str) -> String {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string()
}

fn analyze_bytes(text: &[u8], options: ErrorAnalyzerOptions) -> Vec<u8> {
    let circuit = Circuit::from_stim_bytes(text).expect("circuit");
    circuit_to_detector_error_model(&circuit, options)
        .expect("analyze")
        .to_dem_bytes()
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

#[test]
fn dem_analyzer_keeps_distinct_opaque_error_tags_unmerged() {
    let dem = analyze_bytes(
        b"R 0\n\
          X_ERROR[\xff](0.125) 0\n\
          X_ERROR[\xfe](0.25) 0\n\
          M 0\n\
          DETECTOR rec[-1]\n",
        ErrorAnalyzerOptions::default(),
    );

    assert_eq!(
        dem,
        b"error[\xfe](0.25) D0\n\
          error[\xff](0.125) D0\n"
    );
}

#[test]
fn folded_dem_analyzer_keeps_distinct_opaque_error_tags_unmerged() {
    let dem = analyze_bytes(
        b"R 0\n\
          REPEAT 100 {\n\
              X_ERROR[\xff](0.125) 0\n\
              X_ERROR[\xfe](0.25) 0\n\
              MR 0\n\
              DETECTOR rec[-1]\n\
          }\n",
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    );
    let expected = [
        b"repeat 99 {\n".as_slice(),
        b"    error[\xfe](0.25) D0\n".as_slice(),
        b"    error[\xff](0.125) D0\n".as_slice(),
        b"    shift_detectors 1\n".as_slice(),
        b"}\n".as_slice(),
        b"error[\xfe](0.25) D0\n".as_slice(),
        b"error[\xff](0.125) D0\n".as_slice(),
    ]
    .concat();

    assert_eq!(dem, expected);
}
