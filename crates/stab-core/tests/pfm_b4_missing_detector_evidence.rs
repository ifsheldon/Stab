#![allow(
    clippy::expect_used,
    reason = "PFM-B4 exact parity tests use compact pinned Stim examples"
)]

use stab_core::{Circuit, MissingDetectorOptions, missing_detectors};

#[test]
fn pfm_b4_missing_circuit_empty() {
    assert_missing("", false, "");
}

#[test]
fn pfm_b4_missing_circuit_covered() {
    assert_missing("R 0\nM 0\nDETECTOR rec[-1]\n", false, "");
}

#[test]
fn pfm_b4_missing_circuit_duplicate_covered() {
    assert_missing("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false, "");
}

#[test]
fn pfm_b4_missing_circuit_reset_measurement() {
    assert_missing("R 0\nM 0\n", false, "DETECTOR rec[-1]\n");
}

#[test]
fn pfm_b4_missing_circuit_nondeterministic_known() {
    assert_missing("M 0\n", false, "DETECTOR rec[-1]\n");
}

#[test]
fn pfm_b4_missing_circuit_nondeterministic_ignored() {
    assert_missing("M 0\n", true, "");
}

#[test]
fn pfm_b4_missing_circuit_partial_multitarget() {
    assert_missing(
        "R 0 1\nM 0 1\nDETECTOR rec[-1]\n",
        false,
        "DETECTOR rec[-2]\n",
    );
}

#[test]
fn pfm_b4_missing_circuit_mpp_independent_product() {
    assert_missing(
        "MPP Z0*Z1 X0*X1\n\
         TICK\n\
         MPP Z0*Z1 X0*X1\n\
         DETECTOR rec[-1] rec[-3]\n\
         DETECTOR rec[-2] rec[-4]\n",
        false,
        "DETECTOR rec[-4]\n",
    );
}

#[test]
fn pfm_b4_missing_circuit_mpp_dependent_row() {
    assert_missing(
        "MPP Z0*Z1 X0*X1\n\
         TICK\n\
         MPP Z0*Z1 X0*X1\n\
         DETECTOR rec[-1] rec[-3]\n\
         DETECTOR rec[-2] rec[-4]\n\
         DETECTOR rec[-1] rec[-3] rec[-2] rec[-4]\n",
        false,
        "DETECTOR rec[-3] rec[-2] rec[-1]\n",
    );
}

#[test]
fn pfm_b4_missing_circuit_mpp_unknown_input() {
    assert_missing(
        "MPP Z0*Z1 X0*X1\n\
         TICK\n\
         MPP Z0*Z1 X0*X1\n\
         DETECTOR rec[-1] rec[-3]\n\
         DETECTOR rec[-2] rec[-4]\n",
        true,
        "",
    );
}

#[test]
fn pfm_b4_missing_circuit_record_observable() {
    assert_missing(
        "MPP Z0*Z1 X0*X1\n\
         TICK\n\
         MPP Z0*Z1 X0*X1\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n\
         DETECTOR rec[-2] rec[-4]\n\
         OBSERVABLE_INCLUDE(0) rec[-3]\n",
        true,
        "",
    );
}

#[test]
fn pfm_b4_missing_circuit_pauli_observable() {
    assert_missing(
        "OBSERVABLE_INCLUDE(0) Z0 Z1\n\
         MPP Z0*Z1 X0*X1\n\
         TICK\n\
         MPP Z0*Z1 X0*X1\n\
         OBSERVABLE_INCLUDE(0) Z0 Z1\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n\
         DETECTOR rec[-2] rec[-4]\n\
         OBSERVABLE_INCLUDE(0) rec[-3]\n",
        true,
        "DETECTOR rec[-3] rec[-1]\n",
    );
}

#[test]
fn pfm_b4_missing_honeycomb() {
    assert_missing(
        include_str!("fixtures/missing_detectors_honeycomb_missing_detector.stim"),
        true,
        "DETECTOR rec[-377] rec[-375] rec[-374] rec[-317] rec[-315] rec[-314]\n",
    );
}

#[test]
fn pfm_b4_missing_toric() {
    assert_missing(
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
        "DETECTOR rec[-16] rec[-15] rec[-14] rec[-13] rec[-12] rec[-11] rec[-10] rec[-9]\n",
    );
}

fn assert_missing(circuit_text: &str, ignore_non_deterministic_measurements: bool, expected: &str) {
    let circuit = Circuit::from_stim_str(circuit_text).expect("parse missing-detector circuit");
    let actual = missing_detectors(
        &circuit,
        MissingDetectorOptions {
            ignore_non_deterministic_measurements,
        },
    )
    .expect("compute missing detectors")
    .to_stim_string();
    assert_eq!(actual, expected);
}
