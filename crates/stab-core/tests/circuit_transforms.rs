#![allow(
    clippy::expect_used,
    reason = "RPF2 transform parity tests use compact exact-output assertions"
)]

use stab_core::{
    Circuit, CircuitError, ErrorAnalyzerOptions, circuit_to_detector_error_model,
    circuit_with_inlined_feedback,
};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

#[test]
fn flattened_matches_pinned_stim_basic_cases() {
    assert_eq!(
        Circuit::new()
            .flattened()
            .expect("flatten")
            .to_stim_string(),
        ""
    );
    assert_eq!(
        circuit("SHIFT_COORDS(1, 2)\n")
            .flattened()
            .expect("flatten")
            .to_stim_string(),
        ""
    );
    assert_eq!(
        circuit("H 1\n")
            .flattened()
            .expect("flatten")
            .to_stim_string(),
        "H 1\n"
    );
    assert_eq!(
        circuit("REPEAT 100 {\n}\n")
            .flattened()
            .expect("flatten")
            .to_stim_string(),
        ""
    );
    assert_eq!(
        circuit("REPEAT 3 {\nH 0\n}\n")
            .flattened()
            .expect("flatten")
            .to_stim_string(),
        "H 0 0 0\n"
    );
}

#[test]
fn flattened_applies_coordinate_shifts_through_repeats() {
    let flattened = circuit(
        "
        SHIFT_COORDS(5, 0)
        QUBIT_COORDS(1, 2, 3) 0
        REPEAT 3 {
            MR 0 1
            DETECTOR(0, 0) rec[-2]
            DETECTOR(1, 0) rec[-1]
            SHIFT_COORDS(0, 1)
        }
        OBSERVABLE_INCLUDE(2) rec[-1]
    ",
    )
    .flattened()
    .expect("flatten");

    assert_eq!(
        flattened.to_stim_string(),
        "\
QUBIT_COORDS(6, 2, 3) 0
MR 0 1
DETECTOR(5, 0) rec[-2]
DETECTOR(6, 0) rec[-1]
MR 0 1
DETECTOR(5, 1) rec[-2]
DETECTOR(6, 1) rec[-1]
MR 0 1
DETECTOR(5, 2) rec[-2]
DETECTOR(6, 2) rec[-1]
OBSERVABLE_INCLUDE(2) rec[-1]
"
    );
}

#[test]
fn flattened_preserves_instruction_tags_and_drops_repeat_tags() {
    let flattened = circuit(
        "
        R[test1] 0
        REPEAT[test1.5] 2 {
            H[test2] 0
        }
    ",
    )
    .flattened()
    .expect("flatten");

    assert_eq!(
        flattened.to_stim_string(),
        "\
R[test1] 0
H[test2] 0 0
"
    );
}

#[test]
fn flattened_operations_unrolls_without_fusing() {
    let operations = circuit(
        "
        H 0
        REPEAT 3 {
            X_ERROR(0.125) 1
        }
        CORRELATED_ERROR(0.25) X3 Y4 Z5
        M 0 !1
        DETECTOR rec[-1]
    ",
    )
    .flattened_operations()
    .expect("flatten operations");

    let lines = operations
        .into_iter()
        .map(|instruction| {
            let mut single = Circuit::new();
            single.append_instruction(instruction);
            single.to_stim_string().trim_end().to_string()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        lines,
        [
            "H 0",
            "X_ERROR(0.125) 1",
            "X_ERROR(0.125) 1",
            "X_ERROR(0.125) 1",
            "E(0.25) X3 Y4 Z5",
            "M 0 !1",
            "DETECTOR rec[-1]",
        ]
    );
}

#[test]
fn flattened_rejects_excessive_materialized_expansion() {
    let circuit = circuit("REPEAT 1000001 {\nH 0\n}\n");
    let error = circuit.flattened().expect_err("reject large flatten");

    assert!(error.to_string().contains("materialized limit"), "{error}");
}

#[test]
fn flattened_folds_shift_only_large_repeats() {
    let flattened = circuit(
        "
        REPEAT 1000000000 {
            SHIFT_COORDS(1, 2)
        }
        DETECTOR(3, 4) rec[-1]
    ",
    )
    .flattened()
    .expect("flatten shift-only repeat");

    assert_eq!(
        flattened.to_stim_string(),
        "DETECTOR(1000000003, 2000000004) rec[-1]\n"
    );
}

#[test]
fn without_noise_matches_pinned_stim_basic_cases() {
    let noiseless = circuit(
        "
        H 0
        X_ERROR(0.1) 0
        M(0.05) 0
        M(0.15) 1
        REPEAT 100 {
            CNOT 0 1
            DEPOLARIZE2(0.1) 0 1
            MPP(0.1) X0*X1 Z0 Z1
        }
    ",
    )
    .without_noise()
    .expect("without noise");

    assert_eq!(
        noiseless.to_stim_string(),
        "\
H 0
M 0 1
REPEAT 100 {
    CX 0 1
    MPP X0*X1 Z0 Z1
}
"
    );

    assert_eq!(
        circuit("H 0\nX_ERROR(0.01) 0\nH 0\n")
            .without_noise()
            .expect("without noise")
            .to_stim_string(),
        "H 0 0\n"
    );
}

#[test]
fn without_noise_replaces_heralded_noise_with_measurement_pads() {
    let noiseless = circuit(
        "
        M 0 1
        MPAD 1
        HERALDED_ERASE(0.01) 2 3 0 1
        MPAD 1
        M 2 0
        DETECTOR rec[-1] rec[-8]
    ",
    )
    .without_noise()
    .expect("without noise");

    assert_eq!(
        noiseless.to_stim_string(),
        "\
M 0 1
MPAD 1 0 0 0 0 1
M 2 0
DETECTOR rec[-1] rec[-8]
"
    );
}

#[test]
fn without_noise_preserves_tags_annotations_and_records() {
    let noiseless = circuit(
        "
        R[test1] 0
        X_ERROR[test2](0.25) 0 1
        M[test3](0.25) 0
        DETECTOR[test4](1, 2) rec[-1]
        OBSERVABLE_INCLUDE[test5](3) rec[-1]
        TICK[test6]
        SHIFT_COORDS[test7](5)
    ",
    )
    .without_noise()
    .expect("without noise");

    assert_eq!(
        noiseless.to_stim_string(),
        "\
R[test1] 0
M[test3] 0
DETECTOR[test4](1, 2) rec[-1]
OBSERVABLE_INCLUDE[test5](3) rec[-1]
TICK[test6]
SHIFT_COORDS[test7](5)
"
    );
}

#[test]
fn decomposed_exposes_current_simplified_circuit_subset() {
    let circuit = circuit(
        "
        H_XY 0
        CZ 0 1
        CY 1 2
        SWAP 0 2
    ",
    );

    let decomposed = circuit.decomposed().expect("decompose");

    assert_eq!(decomposed, circuit.simplified().expect("simplify"));
    assert!(!decomposed.to_stim_string().contains("H_XY"));
    assert!(!decomposed.to_stim_string().contains("CZ"));
    assert!(!decomposed.to_stim_string().contains("CY"));
    assert!(!decomposed.to_stim_string().contains("SWAP"));
}

#[test]
fn decomposed_preserves_unowned_mpp_spp_and_pair_phasing_families() {
    let circuit = circuit(
        "
        MPP X0*X1
        SPP X0
        SPP_DAG !Z1
        SQRT_XX 0 1
    ",
    );

    assert_eq!(circuit.decomposed().expect("decompose"), circuit);
}

#[test]
fn with_inlined_feedback_exposes_supported_transform_subset() {
    let circuit = circuit(
        "
        MR 0
        H 0
        CX sweep[5] 0
        CY rec[-1] 0 rec[-1] 0 2 3 rec[-1] 0
        H 0
        M 0
        DETECTOR rec[-1]
        OBSERVABLE_INCLUDE(2) rec[-1]
    ",
    );

    let method_output = circuit.with_inlined_feedback().expect("inline feedback");
    let helper_output = circuit_with_inlined_feedback(&circuit).expect("inline feedback");

    assert_eq!(method_output, helper_output);
    assert_eq!(
        method_output.to_stim_string(),
        "\
MR 0
H 0
CX sweep[5] 0
OBSERVABLE_INCLUDE(2) rec[-1]
CY 2 3
H 0
M 0
DETECTOR rec[-2] rec[-1]
OBSERVABLE_INCLUDE(2) rec[-1]
"
    );
}

#[test]
fn with_inlined_feedback_preserves_mpp_detector_error_model() {
    let input = circuit(
        "
        RX 0
        RY 1
        RZ 2
        MPP X0*Y1*Z2 Z5
        CX rec[-2] 3
        M 3
        DETECTOR rec[-1]
    ",
    );

    let inlined = input.with_inlined_feedback().expect("inline feedback");

    assert_eq!(
        inlined.to_stim_string(),
        "\
RX 0
RY 1
R 2
MPP X0*Y1*Z2 Z5
M 3
DETECTOR rec[-3] rec[-1]
"
    );
    let expected_dem = circuit_to_detector_error_model(&input, ErrorAnalyzerOptions::default())
        .expect("input DEM")
        .to_dem_string();
    let actual_dem = circuit_to_detector_error_model(&inlined, ErrorAnalyzerOptions::default())
        .expect("inlined DEM")
        .to_dem_string();
    assert_eq!(actual_dem, expected_dem);
}

#[test]
fn with_inlined_feedback_rejects_unimplemented_repeat_and_control_shapes() {
    let repeat_error = circuit(
        "
        REPEAT 2 {
            M 0
            CX rec[-1] 0
        }
    ",
    )
    .with_inlined_feedback()
    .expect_err("reject repeat feedback");
    assert!(
        repeat_error
            .to_string()
            .contains("does not support repeat blocks"),
        "{repeat_error}"
    );

    let gate_error = circuit(
        "
        M 0
        XCZ rec[-1] 1
        M 1
        DETECTOR rec[-1]
    ",
    )
    .with_inlined_feedback()
    .expect_err("reject unsupported feedback gate");
    assert!(
        gate_error.to_string().contains("does not support XCZ"),
        "{gate_error}"
    );
}

#[test]
fn flattened_rejects_coordinate_overflow() {
    let error = circuit("SHIFT_COORDS(1e308)\nSHIFT_COORDS(1e308)\nDETECTOR(0) rec[-1]\n")
        .flattened()
        .expect_err("reject coordinate overflow");

    assert!(matches!(error, CircuitError::InvalidResultFormat { .. }));
    assert!(error.to_string().contains("coordinate shift overflowed"));
}
