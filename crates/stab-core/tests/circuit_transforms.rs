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
fn decomposed_matches_public_stim_iswap_mpp_example() {
    let decomposed = circuit(
        "
        ISWAP 0 1 2 1
        TICK
        MPP X1*Z2*Y3
    ",
    )
    .decomposed()
    .expect("decompose");

    assert_eq!(
        decomposed.to_stim_string(),
        "\
H 0
CX 0 1 1 0
H 1
S 1 0
H 2
CX 2 1 1 2
H 1
S 1 2
TICK
H 1 3
S 3
H 3
S 3 3
CX 2 1 3 1
M 1
CX 2 1 3 1
H 3
S 3
H 3
S 3 3
H 1
"
    );
}

#[test]
fn decomposed_preserves_tags_noise_annotations_and_spp_shape() {
    let decomposed = circuit(
        "
        RX[test1] 0
        X_ERROR[test2](0.25) 0
        MPP[test3](0.25) X0*Z1
        DETECTOR[test4](1, 2) rec[-1]
        SPP[test5] Y0
    ",
    )
    .decomposed()
    .expect("decompose");

    assert_eq!(
        decomposed.to_stim_string(),
        "\
R[test1] 0
H[test1] 0
X_ERROR[test2](0.25) 0
H[test3] 0
CX[test3] 1 0
M[test3] 0
CX[test3] 1 0
H[test3] 0
DETECTOR[test4](1, 2) rec[-1]
H[test5] 0
S[test5] 0
H[test5] 0
S[test5] 0 0 0
H[test5] 0
S[test5] 0
H[test5] 0
S[test5] 0 0
"
    );
}

#[test]
fn decomposed_handles_constant_mpp_products_and_rejects_anti_hermitian_products() {
    assert_eq!(
        circuit("MPP X0*X0 X0*!X0\n")
            .decomposed()
            .expect("decompose")
            .to_stim_string(),
        "MPAD 0 1\n"
    );

    let mpp_error = circuit("MPP X0*Z0\n")
        .decomposed()
        .expect_err("reject anti-Hermitian MPP");
    assert!(
        mpp_error.to_string().contains("anti-Hermitian"),
        "{mpp_error}"
    );

    let spp_error = circuit("SPP X0*Z0\n")
        .decomposed()
        .expect_err("reject anti-Hermitian SPP");
    assert!(
        spp_error.to_string().contains("anti-Hermitian"),
        "{spp_error}"
    );
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
fn with_inlined_feedback_refolds_repeat_loop_like_upstream() {
    let input = circuit(
        "
        R 0 1
        X_ERROR(0.125) 0 1
        CX 0 1
        M 1
        CX rec[-1] 1
        DETECTOR rec[-1]
        REPEAT 30 {
            X_ERROR(0.125) 0 1
            CX 0 1
            M 1
            CX rec[-1] 1
            DETECTOR rec[-1] rec[-2]
        }
        M 0
        DETECTOR rec[-1] rec[-2]
    ",
    );
    let inlined = input.with_inlined_feedback().expect("inline feedback loop");

    assert_eq!(
        inlined.to_stim_string(),
        "\
R 0 1
X_ERROR(0.125) 0 1
CX 0 1
M 1
DETECTOR rec[-1]
X_ERROR(0.125) 0 1
CX 0 1
M 1
DETECTOR rec[-1]
REPEAT 29 {
    X_ERROR(0.125) 0 1
    CX 0 1
    M 1
    DETECTOR rec[-3] rec[-1]
}
M 0
DETECTOR rec[-3] rec[-2] rec[-1]
"
    );

    let expected_dem = circuit_to_detector_error_model(&input, ErrorAnalyzerOptions::default())
        .expect("input DEM")
        .flattened()
        .expect("flatten input DEM")
        .to_dem_string();
    let actual_dem = circuit_to_detector_error_model(&inlined, ErrorAnalyzerOptions::default())
        .expect("inlined DEM")
        .flattened()
        .expect("flatten inlined DEM")
        .to_dem_string();
    assert_eq!(actual_dem, expected_dem);
}

#[test]
fn with_inlined_feedback_preserves_repeat_body_target_group_order() {
    let input = circuit(
        "
        REPEAT 2 {
            CX 0 1 2 3 4 5
        }
    ",
    );
    let inlined = input
        .with_inlined_feedback()
        .expect("inline no-feedback repeat");

    assert_eq!(
        inlined.to_stim_string(),
        "\
REPEAT 2 {
    CX 0 1 2 3 4 5
}
"
    );
}

#[test]
fn with_inlined_feedback_rejects_unimplemented_control_shapes_and_excessive_repeat_work() {
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

    let repeat_error = circuit(
        "
        REPEAT 100001 {
            M 0
            CX rec[-1] 0
        }
    ",
    )
    .with_inlined_feedback()
    .expect_err("reject excessive repeat feedback work");
    assert!(
        repeat_error.to_string().contains("supports repeat counts"),
        "{repeat_error}"
    );

    let nested_repeat_error = circuit(
        "
        REPEAT 100000 {
            REPEAT 100000 {
                M 0
                CX rec[-1] 0
            }
        }
    ",
    )
    .with_inlined_feedback()
    .expect_err("reject nested excessive repeat feedback work before generic counting");
    assert!(
        nested_repeat_error
            .to_string()
            .contains("expanded repeat iterations"),
        "{nested_repeat_error}"
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
