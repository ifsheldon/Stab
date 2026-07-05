#![allow(
    clippy::unwrap_used,
    reason = "missing-detector parity tests use exact circuit text for compact diagnostics"
)]

use super::*;

fn missing(text: &str, ignore_non_deterministic_measurements: bool) -> String {
    let circuit = Circuit::from_stim_str(text).unwrap();
    missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements,
        },
    )
    .unwrap()
    .to_stim_string()
}

#[test]
fn missing_detectors_basic() {
    assert_eq!(missing("", false), "");
    assert_eq!(missing("R 0\nM 0\nDETECTOR rec[-1]\n", false), "");
    assert_eq!(
        missing("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false),
        ""
    );
    assert_eq!(missing("R 0\nM 0\n", false), "DETECTOR rec[-1]\n");
    assert_eq!(missing("M 0\n", false), "DETECTOR rec[-1]\n");
    assert_eq!(missing("M 0\n", true), "");
    assert_eq!(
        missing("R 0 1\nM 0 1\nDETECTOR rec[-1]\n", false),
        "DETECTOR rec[-2]\n"
    );
    assert_eq!(
        missing("M 0\nDETECTOR rec[-1] rec[-1]\n", false),
        "DETECTOR rec[-1]\n"
    );
    assert_eq!(missing("MX 0\n", false), "");
}

#[test]
fn missing_detectors_supports_mpp_stabilizer_products() {
    // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
    assert_eq!(
        missing(
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n",
            false,
        ),
        "DETECTOR rec[-4]\n"
    );
    assert_eq!(
        missing(
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n\
             DETECTOR rec[-1] rec[-3] rec[-2] rec[-4]\n",
            false,
        ),
        "DETECTOR rec[-3] rec[-2] rec[-1]\n"
    );
    assert_eq!(
        missing(
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n",
            true,
        ),
        ""
    );
}

#[test]
fn missing_detectors_basic_reset_and_measurement_aliases() {
    assert_eq!(missing("RX 0\nMX 0\n", false), "DETECTOR rec[-1]\n");
    assert_eq!(missing("RY 0\nMY 0\n", false), "DETECTOR rec[-1]\n");
    assert_eq!(missing("RX 0\nMY 0\n", false), "");
    assert_eq!(missing("RX 0\nMY 0\n", true), "");
    assert_eq!(missing("MR 0\n", false), "DETECTOR rec[-1]\n");
    assert_eq!(missing("MR 0\n", true), "");
}

#[test]
fn missing_detectors_reduces_multi_record_detector_rows() {
    assert_eq!(
        missing("R 0 1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n", false),
        "DETECTOR rec[-2]\n"
    );
}

#[test]
fn missing_detectors_supports_observable_interactions() {
    // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
    assert_eq!(
        missing(
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
        ""
    );
    assert_eq!(
        missing(
            "OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
        "DETECTOR rec[-3] rec[-1]\n"
    );
}

#[test]
fn missing_detectors_supports_toric_global_stabilizer_product() {
    // Adapted from Stim v1.16.0 src/stim/util_top/missing_detectors.test.cc.
    assert_eq!(
        missing(
            "R 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15\n\
             TICK\n\
             MPP X0*X4*X5*X1 X2*X6*X7*X3 X10*X14*X15*X11 X8*X12*X13*X9\n\
             TICK\n\
             MPP X5*X9*X10*X6 X1*X13*X14*X2 X0*X12*X15*X3 X4*X8*X11*X7\n\
             TICK\n\
             MPP Z4*Z8*Z9*Z5 Z6*Z10*Z11*Z7 Z2*Z14*Z15*Z3 Z0*Z12*Z13*Z1\n\
             TICK\n\
             MPP Z1*Z5*Z6*Z2 Z9*Z13*Z14*Z10 Z8*Z12*Z15*Z11 Z0*Z4*Z7*Z3\n\
             DETECTOR rec[-1]\n\
             DETECTOR rec[-2]\n\
             DETECTOR rec[-3]\n\
             DETECTOR rec[-4]\n\
             DETECTOR rec[-5]\n\
             DETECTOR rec[-6]\n\
             DETECTOR rec[-7]\n\
             DETECTOR rec[-8]\n",
            true,
        ),
        "DETECTOR rec[-16] rec[-15] rec[-14] rec[-13] rec[-12] rec[-11] rec[-10] rec[-9]\n"
    );
}

#[test]
fn missing_detectors_handles_bounded_repeat_blocks() {
    let repeat = Circuit::from_stim_str("REPEAT 2 {\n    M 0\n}\n").unwrap();
    let repeat_output = missing_detectors(
        &repeat,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    )
    .unwrap()
    .to_stim_string();
    assert_eq!(repeat_output, "DETECTOR rec[-2] rec[-1]\n");

    let excessive = Circuit::from_stim_str("REPEAT 1000001 {\n    M 0\n}\n").unwrap();
    let excessive_error = missing_detectors(
        &excessive,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: true,
        },
    )
    .unwrap_err();
    assert!(
        excessive_error
            .to_string()
            .contains("expanded repeat iterations")
    );

    let clifford = Circuit::from_stim_str("H 0\nM 0\n").unwrap();
    let clifford_output = missing_detectors(
        &clifford,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements: false,
        },
    )
    .unwrap()
    .to_stim_string();
    assert_eq!(clifford_output, "");
}
