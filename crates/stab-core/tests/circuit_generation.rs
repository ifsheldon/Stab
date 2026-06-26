#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        reason = "tests use direct assertions for compact compatibility diagnostics"
    )
)]

use stab_core::{
    CodeDistance, ColorCodeParams, ColorCodeTask, Probability, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};

#[test]
fn repetition_code_generator_matches_stim_noisy_reference() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_rep_code.test.cc.
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(5000).expect("rounds"),
        CodeDistance::try_new(4).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = generate_repetition_code_circuit(&params).expect("generate repetition code");

    assert_eq!(generated.layout_text(), "# L0 Z1 d2 Z3 d4 Z5 d6\n");
    assert_eq!(
        generated.circuit().to_stim_string(),
        concat!(
            "R 0 1 2 3 4 5 6\n",
            "X_ERROR(0.25) 0 1 2 3 4 5 6\n",
            "TICK\n",
            "DEPOLARIZE1(0.0625) 0 2 4 6\n",
            "CX 0 1 2 3 4 5\n",
            "DEPOLARIZE2(0.125) 0 1 2 3 4 5\n",
            "TICK\n",
            "CX 2 1 4 3 6 5\n",
            "DEPOLARIZE2(0.125) 2 1 4 3 6 5\n",
            "TICK\n",
            "X_ERROR(0.375) 1 3 5\n",
            "MR 1 3 5\n",
            "X_ERROR(0.25) 1 3 5\n",
            "DETECTOR(1, 0) rec[-3]\n",
            "DETECTOR(3, 0) rec[-2]\n",
            "DETECTOR(5, 0) rec[-1]\n",
            "REPEAT 4999 {\n",
            "    TICK\n",
            "    DEPOLARIZE1(0.0625) 0 2 4 6\n",
            "    CX 0 1 2 3 4 5\n",
            "    DEPOLARIZE2(0.125) 0 1 2 3 4 5\n",
            "    TICK\n",
            "    CX 2 1 4 3 6 5\n",
            "    DEPOLARIZE2(0.125) 2 1 4 3 6 5\n",
            "    TICK\n",
            "    X_ERROR(0.375) 1 3 5\n",
            "    MR 1 3 5\n",
            "    X_ERROR(0.25) 1 3 5\n",
            "    SHIFT_COORDS(0, 1)\n",
            "    DETECTOR(1, 0) rec[-3] rec[-6]\n",
            "    DETECTOR(3, 0) rec[-2] rec[-5]\n",
            "    DETECTOR(5, 0) rec[-1] rec[-4]\n",
            "}\n",
            "X_ERROR(0.375) 0 2 4 6\n",
            "M 0 2 4 6\n",
            "DETECTOR(1, 1) rec[-3] rec[-4] rec[-7]\n",
            "DETECTOR(3, 1) rec[-2] rec[-3] rec[-6]\n",
            "DETECTOR(5, 1) rec[-1] rec[-2] rec[-5]\n",
            "OBSERVABLE_INCLUDE(0) rec[-1]\n",
        )
    );
}

#[test]
fn repetition_code_generator_rejects_invalid_rounds_and_distance() {
    assert!(RoundCount::try_new(0).is_err());
    assert!(CodeDistance::try_new(1).is_err());
}

#[test]
fn surface_code_generator_matches_stim_unrotated_noisy_reference() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_surface_code.test.cc.
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(5).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::UnrotatedMemoryZ,
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = generate_surface_code_circuit(&params).expect("generate surface code");

    assert_eq!(
        generated.layout_text(),
        concat!("# d6 X7 d8\n", "# Z3 d4 Z5\n", "# L0 X1 L2\n")
    );
    assert_eq!(
        generated.circuit().to_stim_string(),
        concat!(
            "QUBIT_COORDS(0, 0) 0\n",
            "QUBIT_COORDS(1, 0) 1\n",
            "QUBIT_COORDS(2, 0) 2\n",
            "QUBIT_COORDS(0, 1) 3\n",
            "QUBIT_COORDS(1, 1) 4\n",
            "QUBIT_COORDS(2, 1) 5\n",
            "QUBIT_COORDS(0, 2) 6\n",
            "QUBIT_COORDS(1, 2) 7\n",
            "QUBIT_COORDS(2, 2) 8\n",
            "R 0 2 4 6 8\n",
            "X_ERROR(0.25) 0 2 4 6 8\n",
            "R 1 3 5 7\n",
            "X_ERROR(0.25) 1 3 5 7\n",
            "TICK\n",
            "DEPOLARIZE1(0.0625) 0 2 4 6 8\n",
            "H 1 7\n",
            "DEPOLARIZE1(0.125) 1 7\n",
            "TICK\n",
            "CX 1 2 7 8 4 3\n",
            "DEPOLARIZE2(0.125) 1 2 7 8 4 3\n",
            "TICK\n",
            "CX 1 4 6 3 8 5\n",
            "DEPOLARIZE2(0.125) 1 4 6 3 8 5\n",
            "TICK\n",
            "CX 7 4 0 3 2 5\n",
            "DEPOLARIZE2(0.125) 7 4 0 3 2 5\n",
            "TICK\n",
            "CX 1 0 7 6 4 5\n",
            "DEPOLARIZE2(0.125) 1 0 7 6 4 5\n",
            "TICK\n",
            "H 1 7\n",
            "DEPOLARIZE1(0.125) 1 7\n",
            "TICK\n",
            "X_ERROR(0.375) 1 3 5 7\n",
            "MR 1 3 5 7\n",
            "X_ERROR(0.25) 1 3 5 7\n",
            "DETECTOR(0, 1, 0) rec[-3]\n",
            "DETECTOR(2, 1, 0) rec[-2]\n",
            "REPEAT 4 {\n",
            "    TICK\n",
            "    DEPOLARIZE1(0.0625) 0 2 4 6 8\n",
            "    H 1 7\n",
            "    DEPOLARIZE1(0.125) 1 7\n",
            "    TICK\n",
            "    CX 1 2 7 8 4 3\n",
            "    DEPOLARIZE2(0.125) 1 2 7 8 4 3\n",
            "    TICK\n",
            "    CX 1 4 6 3 8 5\n",
            "    DEPOLARIZE2(0.125) 1 4 6 3 8 5\n",
            "    TICK\n",
            "    CX 7 4 0 3 2 5\n",
            "    DEPOLARIZE2(0.125) 7 4 0 3 2 5\n",
            "    TICK\n",
            "    CX 1 0 7 6 4 5\n",
            "    DEPOLARIZE2(0.125) 1 0 7 6 4 5\n",
            "    TICK\n",
            "    H 1 7\n",
            "    DEPOLARIZE1(0.125) 1 7\n",
            "    TICK\n",
            "    X_ERROR(0.375) 1 3 5 7\n",
            "    MR 1 3 5 7\n",
            "    X_ERROR(0.25) 1 3 5 7\n",
            "    SHIFT_COORDS(0, 0, 1)\n",
            "    DETECTOR(1, 0, 0) rec[-4] rec[-8]\n",
            "    DETECTOR(0, 1, 0) rec[-3] rec[-7]\n",
            "    DETECTOR(2, 1, 0) rec[-2] rec[-6]\n",
            "    DETECTOR(1, 2, 0) rec[-1] rec[-5]\n",
            "}\n",
            "X_ERROR(0.375) 0 2 4 6 8\n",
            "M 0 2 4 6 8\n",
            "DETECTOR(0, 1, 1) rec[-2] rec[-3] rec[-5] rec[-8]\n",
            "DETECTOR(2, 1, 1) rec[-1] rec[-3] rec[-4] rec[-7]\n",
            "OBSERVABLE_INCLUDE(0) rec[-4] rec[-5]\n",
        )
    );
}

#[test]
fn surface_code_generator_matches_stim_rotated_noisy_reference() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_surface_code.test.cc.
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(5).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = generate_surface_code_circuit(&params).expect("generate surface code");

    assert_eq!(
        generated.layout_text(),
        concat!(
            "#         X12\n",
            "#     d6      d8 \n",
            "#         Z7 \n",
            "#     L1      L3 \n",
            "#         X2 \n",
        )
    );
    assert_eq!(
        generated.circuit().to_stim_string(),
        concat!(
            "QUBIT_COORDS(1, 1) 1\n",
            "QUBIT_COORDS(2, 0) 2\n",
            "QUBIT_COORDS(3, 1) 3\n",
            "QUBIT_COORDS(1, 3) 6\n",
            "QUBIT_COORDS(2, 2) 7\n",
            "QUBIT_COORDS(3, 3) 8\n",
            "QUBIT_COORDS(2, 4) 12\n",
            "R 1 3 6 8\n",
            "X_ERROR(0.25) 1 3 6 8\n",
            "R 2 7 12\n",
            "X_ERROR(0.25) 2 7 12\n",
            "TICK\n",
            "DEPOLARIZE1(0.0625) 1 3 6 8\n",
            "H 2 12\n",
            "DEPOLARIZE1(0.125) 2 12\n",
            "TICK\n",
            "CX 2 3 8 7\n",
            "DEPOLARIZE2(0.125) 2 3 8 7\n",
            "TICK\n",
            "CX 2 1 3 7\n",
            "DEPOLARIZE2(0.125) 2 1 3 7\n",
            "TICK\n",
            "CX 12 8 6 7\n",
            "DEPOLARIZE2(0.125) 12 8 6 7\n",
            "TICK\n",
            "CX 12 6 1 7\n",
            "DEPOLARIZE2(0.125) 12 6 1 7\n",
            "TICK\n",
            "H 2 12\n",
            "DEPOLARIZE1(0.125) 2 12\n",
            "TICK\n",
            "X_ERROR(0.375) 2 7 12\n",
            "MR 2 7 12\n",
            "X_ERROR(0.25) 2 7 12\n",
            "DETECTOR(2, 2, 0) rec[-2]\n",
            "REPEAT 4 {\n",
            "    TICK\n",
            "    DEPOLARIZE1(0.0625) 1 3 6 8\n",
            "    H 2 12\n",
            "    DEPOLARIZE1(0.125) 2 12\n",
            "    TICK\n",
            "    CX 2 3 8 7\n",
            "    DEPOLARIZE2(0.125) 2 3 8 7\n",
            "    TICK\n",
            "    CX 2 1 3 7\n",
            "    DEPOLARIZE2(0.125) 2 1 3 7\n",
            "    TICK\n",
            "    CX 12 8 6 7\n",
            "    DEPOLARIZE2(0.125) 12 8 6 7\n",
            "    TICK\n",
            "    CX 12 6 1 7\n",
            "    DEPOLARIZE2(0.125) 12 6 1 7\n",
            "    TICK\n",
            "    H 2 12\n",
            "    DEPOLARIZE1(0.125) 2 12\n",
            "    TICK\n",
            "    X_ERROR(0.375) 2 7 12\n",
            "    MR 2 7 12\n",
            "    X_ERROR(0.25) 2 7 12\n",
            "    SHIFT_COORDS(0, 0, 1)\n",
            "    DETECTOR(2, 0, 0) rec[-3] rec[-6]\n",
            "    DETECTOR(2, 2, 0) rec[-2] rec[-5]\n",
            "    DETECTOR(2, 4, 0) rec[-1] rec[-4]\n",
            "}\n",
            "X_ERROR(0.375) 1 3 6 8\n",
            "M 1 3 6 8\n",
            "DETECTOR(2, 2, 1) rec[-1] rec[-2] rec[-3] rec[-4] rec[-6]\n",
            "OBSERVABLE_INCLUDE(0) rec[-3] rec[-4]\n",
        )
    );
}

#[test]
fn color_code_generator_matches_stim_noisy_reference() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_color_code.test.cc.
    let params = ColorCodeParams::new(
        RoundCount::try_new(100).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = generate_color_code_circuit(&params).expect("generate color code");

    assert_eq!(
        generated.layout_text(),
        concat!(
            "#          d9\n",
            "#       d7    R8\n",
            "#    B4    d5    d6\n",
            "# L0    L1    G2    L3\n",
        )
    );
    assert_eq!(
        generated.circuit().to_stim_string(),
        concat!(
            "QUBIT_COORDS(0, 0) 0\n",
            "QUBIT_COORDS(1, 0) 1\n",
            "QUBIT_COORDS(2, 0) 2\n",
            "QUBIT_COORDS(3, 0) 3\n",
            "QUBIT_COORDS(0.5, 1) 4\n",
            "QUBIT_COORDS(1.5, 1) 5\n",
            "QUBIT_COORDS(2.5, 1) 6\n",
            "QUBIT_COORDS(1, 2) 7\n",
            "QUBIT_COORDS(2, 2) 8\n",
            "QUBIT_COORDS(1.5, 3) 9\n",
            "R 0 1 2 3 4 5 6 7 8 9\n",
            "X_ERROR(0.25) 0 1 2 3 4 5 6 7 8 9\n",
            "REPEAT 2 {\n",
            "    TICK\n",
            "    DEPOLARIZE1(0.0625) 0 1 3 5 6 7 9\n",
            "    C_XYZ 0 1 3 5 6 7 9\n",
            "    DEPOLARIZE1(0.125) 0 1 3 5 6 7 9\n",
            "    TICK\n",
            "    CX 5 4 3 2\n",
            "    DEPOLARIZE2(0.125) 5 4 3 2\n",
            "    TICK\n",
            "    CX 7 4 6 2\n",
            "    DEPOLARIZE2(0.125) 7 4 6 2\n",
            "    TICK\n",
            "    CX 1 4 6 8\n",
            "    DEPOLARIZE2(0.125) 1 4 6 8\n",
            "    TICK\n",
            "    CX 1 2 7 8\n",
            "    DEPOLARIZE2(0.125) 1 2 7 8\n",
            "    TICK\n",
            "    CX 5 2 9 8\n",
            "    DEPOLARIZE2(0.125) 5 2 9 8\n",
            "    TICK\n",
            "    CX 0 4 5 8\n",
            "    DEPOLARIZE2(0.125) 0 4 5 8\n",
            "    TICK\n",
            "    X_ERROR(0.375) 2 4 8\n",
            "    MR 2 4 8\n",
            "    X_ERROR(0.25) 2 4 8\n",
            "}\n",
            "DETECTOR(2, 0, 0) rec[-3] rec[-6]\n",
            "DETECTOR(0.5, 1, 0) rec[-2] rec[-5]\n",
            "DETECTOR(2, 2, 0) rec[-1] rec[-4]\n",
            "REPEAT 98 {\n",
            "    TICK\n",
            "    DEPOLARIZE1(0.0625) 0 1 3 5 6 7 9\n",
            "    C_XYZ 0 1 3 5 6 7 9\n",
            "    DEPOLARIZE1(0.125) 0 1 3 5 6 7 9\n",
            "    TICK\n",
            "    CX 5 4 3 2\n",
            "    DEPOLARIZE2(0.125) 5 4 3 2\n",
            "    TICK\n",
            "    CX 7 4 6 2\n",
            "    DEPOLARIZE2(0.125) 7 4 6 2\n",
            "    TICK\n",
            "    CX 1 4 6 8\n",
            "    DEPOLARIZE2(0.125) 1 4 6 8\n",
            "    TICK\n",
            "    CX 1 2 7 8\n",
            "    DEPOLARIZE2(0.125) 1 2 7 8\n",
            "    TICK\n",
            "    CX 5 2 9 8\n",
            "    DEPOLARIZE2(0.125) 5 2 9 8\n",
            "    TICK\n",
            "    CX 0 4 5 8\n",
            "    DEPOLARIZE2(0.125) 0 4 5 8\n",
            "    TICK\n",
            "    X_ERROR(0.375) 2 4 8\n",
            "    MR 2 4 8\n",
            "    X_ERROR(0.25) 2 4 8\n",
            "    SHIFT_COORDS(0, 0, 1)\n",
            "    DETECTOR(2, 0, 0) rec[-3] rec[-6] rec[-9]\n",
            "    DETECTOR(0.5, 1, 0) rec[-2] rec[-5] rec[-8]\n",
            "    DETECTOR(2, 2, 0) rec[-1] rec[-4] rec[-7]\n",
            "}\n",
            "Z_ERROR(0.375) 0 1 3 5 6 7 9\n",
            "MX 0 1 3 5 6 7 9\n",
            "DETECTOR(2, 0, 1) rec[-3] rec[-4] rec[-5] rec[-6] rec[-13]\n",
            "DETECTOR(0.5, 1, 1) rec[-2] rec[-4] rec[-6] rec[-7] rec[-12]\n",
            "DETECTOR(2, 2, 1) rec[-1] rec[-2] rec[-3] rec[-4] rec[-11]\n",
            "OBSERVABLE_INCLUDE(0) rec[-5] rec[-6] rec[-7]\n",
        )
    );
}

#[test]
fn color_code_generator_rejects_invalid_rounds_and_distance() {
    let too_few_rounds = ColorCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params");
    let even_distance = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(4).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params");

    assert!(generate_color_code_circuit(&too_few_rounds).is_err());
    assert!(generate_color_code_circuit(&even_distance).is_err());
}
