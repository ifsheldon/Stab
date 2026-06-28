#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{Circuit, DetectorErrorModel, ExplainedError, explain_errors_from_circuit};

fn explain(text: &str) -> Vec<ExplainedError> {
    let circuit = Circuit::from_stim_str(text).expect("circuit");
    explain_errors_from_circuit(&circuit, None, false).expect("explain errors")
}

fn only_error(actual: &[ExplainedError]) -> &ExplainedError {
    assert_eq!(actual.len(), 1);
    actual.first().expect("one explained error")
}

#[test]
fn error_matcher_x_error_matches_upstream_subset() {
    let actual = explain(
        "
        QUBIT_COORDS(5, 6) 0
        X_ERROR(0.25) 0
        M 0
        DETECTOR(2, 3) rec[-1]
        ",
    );

    assert_eq!(
        only_error(&actual).to_string(),
        "ExplainedError {\n    dem_error_terms: D0[coords 2,3]\n    CircuitErrorLocation {\n        flipped_pauli_product: X0[coords 5,6]\n        Circuit location stack trace:\n            (after 0 TICKs)\n            at instruction #2 (X_ERROR) in the circuit\n            at target #1 of the instruction\n            resolving to X_ERROR(0.25) 0[coords 5,6]\n    }\n}"
    );
}

#[test]
fn error_matcher_correlated_error_matches_upstream_subset() {
    let actual = explain(
        "
        SHIFT_COORDS(10, 20)
        QUBIT_COORDS(5, 6) 0
        SHIFT_COORDS(100, 200)
        CORRELATED_ERROR(0.25) X0 Y1
        M 0
        DETECTOR(2, 3) rec[-1]
        ",
    );

    assert_eq!(
        only_error(&actual).to_string(),
        "ExplainedError {\n    dem_error_terms: D0[coords 112,223]\n    CircuitErrorLocation {\n        flipped_pauli_product: X0[coords 15,26]*Y1\n        Circuit location stack trace:\n            (after 0 TICKs)\n            at instruction #4 (E) in the circuit\n            at targets #1 to #2 of the instruction\n            resolving to E(0.25) X0[coords 15,26] Y1\n    }\n}"
    );
}

#[test]
fn error_matcher_mx_error_matches_upstream_subset() {
    let actual = explain(
        "
        QUBIT_COORDS(5, 6) 0
        RX 0
        REPEAT 10 {
            TICK
        }
        MX(0.125) 1 2 3 0 4
        DETECTOR(2, 3) rec[-2]
        ",
    );

    assert_eq!(
        only_error(&actual).to_string(),
        "ExplainedError {\n    dem_error_terms: D0[coords 2,3]\n    CircuitErrorLocation {\n        flipped_measurement.measurement_record_index: 3\n        flipped_measurement.measured_observable: X0[coords 5,6]\n        Circuit location stack trace:\n            (after 10 TICKs)\n            at instruction #4 (MX) in the circuit\n            at target #4 of the instruction\n            resolving to MX(0.125) 0[coords 5,6]\n    }\n}"
    );
}

#[test]
fn error_matcher_mxx_error_matches_upstream_subset() {
    let actual = explain(
        "
        QUBIT_COORDS(5, 6) 0
        RX 0
        CX 0 1
        MXX(0.125) 0 1
        DETECTOR(2, 3) rec[-1]
        ",
    );

    assert_eq!(
        only_error(&actual).to_string(),
        "ExplainedError {\n    dem_error_terms: D0[coords 2,3]\n    CircuitErrorLocation {\n        flipped_measurement.measurement_record_index: 0\n        flipped_measurement.measured_observable: X0[coords 5,6]*X1\n        Circuit location stack trace:\n            (after 0 TICKs)\n            at instruction #4 (MXX) in the circuit\n            at targets #1 to #2 of the instruction\n            resolving to MXX(0.125) 0[coords 5,6] 1\n    }\n}"
    );
}

#[test]
fn error_matcher_filter_keeps_unmatched_errors() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");
    let filter = DetectorErrorModel::from_dem_str("error(1) D0\n").expect("filter DEM");
    let actual =
        explain_errors_from_circuit(&circuit, Some(&filter), false).expect("explain errors");

    assert_eq!(
        only_error(&actual).to_string(),
        "ExplainedError {\n    dem_error_terms: D0\n    [no single circuit error had these exact symptoms]\n}"
    );
}

#[test]
fn error_matcher_rejects_repeat_contained_noise_until_recursive_matching_exists() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 2 {
            X_ERROR(0.125) 0
        }
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");

    let error =
        explain_errors_from_circuit(&circuit, None, false).expect_err("reject repeated noise");

    assert!(error.to_string().contains("repeat-contained noise"));
}

#[test]
fn error_matcher_rejects_repeat_contained_mpp_noise_until_recursive_matching_exists() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 2 {
            MPP(0.125) Z0
        }
        ",
    )
    .expect("circuit");

    let error =
        explain_errors_from_circuit(&circuit, None, false).expect_err("reject repeated MPP noise");

    assert!(error.to_string().contains("repeat-contained noise"));
}

#[test]
fn error_matcher_rejects_nested_repeat_expansion_budget() {
    let circuit = Circuit::from_stim_str(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                TICK
            }
        }
        ",
    )
    .expect("circuit");

    let error =
        explain_errors_from_circuit(&circuit, None, false).expect_err("reject nested expansion");

    assert!(error.to_string().contains("expanded repeat iterations"));
}

#[test]
fn error_matcher_rejects_filter_dem_repeat_expansion_budget() {
    let circuit = Circuit::from_stim_str(
        "
        M 0
        DETECTOR rec[-1]
        ",
    )
    .expect("circuit");
    let filter = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            error(0.1) D0
            shift_detectors 1
        }
        ",
    )
    .expect("filter DEM");

    let error =
        explain_errors_from_circuit(&circuit, Some(&filter), false).expect_err("reject filter");

    assert!(
        error
            .to_string()
            .contains("DEM ErrorMatcher filter currently supports repeat counts")
    );
}
