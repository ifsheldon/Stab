#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        reason = "tests use direct assertions for compact compatibility diagnostics"
    )
)]

use stab_core::{
    CodeDistance, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    generate_repetition_code_circuit,
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
