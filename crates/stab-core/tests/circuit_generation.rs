#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        reason = "tests use direct assertions for compact compatibility diagnostics"
    )
)]

use stab_core::{
    CircuitError, CodeDistance, ColorCodeParams, ColorCodeTask, ErrorAnalyzerOptions,
    GeneratedCircuit, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    SurfaceCodeParams, SurfaceCodeTask, circuit_to_detector_error_model,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
    sample_detection_events,
};

#[path = "circuit_generation/resource.rs"]
mod resource;

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("valid probability")
}

fn assert_invalid_domain_value<T>(
    result: Result<T, CircuitError>,
    expected_kind: &'static str,
    expected_value: &str,
) {
    assert_eq!(
        result.err(),
        Some(CircuitError::InvalidDomainValue {
            kind: expected_kind,
            value: expected_value.to_string(),
        })
    );
}

#[test]
fn cq2_generation_parameter_contract_covers_typed_values_and_noise_builders() {
    let minimum_distance = CodeDistance::try_new(2).expect("minimum distance");
    let maximum_distance = CodeDistance::try_new(2047).expect("maximum distance");
    assert_eq!(minimum_distance.get(), 2);
    assert_eq!(maximum_distance.get(), 2047);
    assert_eq!(minimum_distance, minimum_distance.clone());
    assert_eq!(format!("{minimum_distance:?}"), "CodeDistance(2)");
    assert_invalid_domain_value(CodeDistance::try_new(0), "code distance", "0");
    assert_invalid_domain_value(CodeDistance::try_new(1), "code distance", "1");
    assert_invalid_domain_value(CodeDistance::try_new(2048), "code distance", "2048");

    let minimum_rounds = RoundCount::try_new(1).expect("minimum rounds");
    let maximum_rounds = RoundCount::try_new(u64::MAX).expect("maximum rounds");
    assert_eq!(minimum_rounds.get(), 1);
    assert_eq!(maximum_rounds.get(), u64::MAX);
    assert_eq!(minimum_rounds, minimum_rounds.clone());
    assert_eq!(format!("{minimum_rounds:?}"), "RoundCount(1)");
    assert_invalid_domain_value(RoundCount::try_new(0), "round count", "0");

    let repetition_task = RepetitionCodeTask::Memory;
    assert_eq!(repetition_task, repetition_task.clone());
    assert_eq!(format!("{repetition_task:?}"), "Memory");

    let surface_tasks = [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
        SurfaceCodeTask::UnrotatedMemoryX,
        SurfaceCodeTask::UnrotatedMemoryZ,
    ];
    for (task, expected_debug) in surface_tasks.into_iter().zip([
        "RotatedMemoryX",
        "RotatedMemoryZ",
        "UnrotatedMemoryX",
        "UnrotatedMemoryZ",
    ]) {
        assert_eq!(task, task.clone());
        assert_eq!(format!("{task:?}"), expected_debug);
    }
    for (index, task) in surface_tasks.iter().enumerate() {
        for distinct in surface_tasks.iter().skip(index + 1) {
            assert_ne!(task, distinct);
        }
    }

    let color_task = ColorCodeTask::MemoryXyz;
    assert_eq!(color_task, color_task.clone());
    assert_eq!(format!("{color_task:?}"), "MemoryXyz");

    let before_round = probability(0.0625);
    let before_measure = probability(0.125);
    let after_reset = probability(0.25);
    let after_clifford = probability(0.5);
    let zero = probability(0.0);

    let repetition_defaults = RepetitionCodeParams::new(
        RoundCount::try_new(3).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        repetition_task,
    )
    .expect("repetition parameters");
    assert_default_generation_probabilities(
        repetition_defaults.before_round_data_depolarization(),
        repetition_defaults.before_measure_flip_probability(),
        repetition_defaults.after_reset_flip_probability(),
        repetition_defaults.after_clifford_depolarization(),
        zero,
    );
    let repetition = repetition_defaults
        .clone()
        .with_before_round_data_depolarization(before_round)
        .with_before_measure_flip_probability(before_measure)
        .with_after_reset_flip_probability(after_reset)
        .with_after_clifford_depolarization(after_clifford);
    assert_eq!(repetition.rounds().get(), 3);
    assert_eq!(repetition.distance().get(), 5);
    assert_eq!(repetition.task(), RepetitionCodeTask::Memory);
    assert_eq!(repetition.before_round_data_depolarization(), before_round);
    assert_eq!(repetition.before_measure_flip_probability(), before_measure);
    assert_eq!(repetition.after_reset_flip_probability(), after_reset);
    assert_eq!(repetition.after_clifford_depolarization(), after_clifford);
    assert_eq!(repetition, repetition.clone());
    assert!(format!("{repetition:?}").contains("RepetitionCodeParams"));
    assert_eq!(repetition_defaults.before_round_data_depolarization(), zero);

    let surface_defaults = SurfaceCodeParams::new(
        RoundCount::try_new(7).expect("rounds"),
        CodeDistance::try_new(9).expect("distance"),
        SurfaceCodeTask::UnrotatedMemoryX,
    )
    .expect("surface parameters");
    assert_default_generation_probabilities(
        surface_defaults.before_round_data_depolarization(),
        surface_defaults.before_measure_flip_probability(),
        surface_defaults.after_reset_flip_probability(),
        surface_defaults.after_clifford_depolarization(),
        zero,
    );
    let surface = surface_defaults
        .clone()
        .with_before_round_data_depolarization(before_round)
        .with_before_measure_flip_probability(before_measure)
        .with_after_reset_flip_probability(after_reset)
        .with_after_clifford_depolarization(after_clifford);
    assert_eq!(surface.rounds().get(), 7);
    assert_eq!(surface.distance().get(), 9);
    assert_eq!(surface.task(), SurfaceCodeTask::UnrotatedMemoryX);
    assert_eq!(surface.before_round_data_depolarization(), before_round);
    assert_eq!(surface.before_measure_flip_probability(), before_measure);
    assert_eq!(surface.after_reset_flip_probability(), after_reset);
    assert_eq!(surface.after_clifford_depolarization(), after_clifford);
    assert_eq!(surface, surface.clone());
    assert!(format!("{surface:?}").contains("SurfaceCodeParams"));
    assert_eq!(surface_defaults.before_round_data_depolarization(), zero);

    let color_defaults = ColorCodeParams::new(
        RoundCount::try_new(11).expect("rounds"),
        CodeDistance::try_new(13).expect("distance"),
        color_task,
    )
    .expect("color parameters");
    assert_default_generation_probabilities(
        color_defaults.before_round_data_depolarization(),
        color_defaults.before_measure_flip_probability(),
        color_defaults.after_reset_flip_probability(),
        color_defaults.after_clifford_depolarization(),
        zero,
    );
    let color = color_defaults
        .clone()
        .with_before_round_data_depolarization(before_round)
        .with_before_measure_flip_probability(before_measure)
        .with_after_reset_flip_probability(after_reset)
        .with_after_clifford_depolarization(after_clifford);
    assert_eq!(color.rounds().get(), 11);
    assert_eq!(color.distance().get(), 13);
    assert_eq!(color.task(), ColorCodeTask::MemoryXyz);
    assert_eq!(color.before_round_data_depolarization(), before_round);
    assert_eq!(color.before_measure_flip_probability(), before_measure);
    assert_eq!(color.after_reset_flip_probability(), after_reset);
    assert_eq!(color.after_clifford_depolarization(), after_clifford);
    assert_eq!(color, color.clone());
    assert!(format!("{color:?}").contains("ColorCodeParams"));
    assert_eq!(color_defaults.before_round_data_depolarization(), zero);

    let allocations = allocation_counter::measure(|| {
        for _ in 0..128 {
            let repetition = RepetitionCodeParams::new(
                minimum_rounds,
                minimum_distance,
                RepetitionCodeTask::Memory,
            )
            .expect("fixed repetition parameters")
            .with_before_round_data_depolarization(before_round)
            .with_before_measure_flip_probability(before_measure)
            .with_after_reset_flip_probability(after_reset)
            .with_after_clifford_depolarization(after_clifford);
            let surface = SurfaceCodeParams::new(
                minimum_rounds,
                minimum_distance,
                SurfaceCodeTask::RotatedMemoryX,
            )
            .expect("fixed surface parameters")
            .with_before_round_data_depolarization(before_round)
            .with_before_measure_flip_probability(before_measure)
            .with_after_reset_flip_probability(after_reset)
            .with_after_clifford_depolarization(after_clifford);
            let color = ColorCodeParams::new(
                minimum_rounds,
                CodeDistance::try_new(3).expect("fixed color distance"),
                ColorCodeTask::MemoryXyz,
            )
            .expect("fixed color parameters")
            .with_before_round_data_depolarization(before_round)
            .with_before_measure_flip_probability(before_measure)
            .with_after_reset_flip_probability(after_reset)
            .with_after_clifford_depolarization(after_clifford);
            std::hint::black_box((repetition, surface, color));
        }
    });
    assert_eq!(
        allocations.count_total, 0,
        "fixed-size parameter operations allocated: {allocations:?}"
    );
    assert_eq!(
        allocations.bytes_total, 0,
        "fixed-size parameter operations allocated: {allocations:?}"
    );
}

fn assert_default_generation_probabilities(
    before_round: Probability,
    before_measure: Probability,
    after_reset: Probability,
    after_clifford: Probability,
    zero: Probability,
) {
    assert_eq!(before_round, zero);
    assert_eq!(before_measure, zero);
    assert_eq!(after_reset, zero);
    assert_eq!(after_clifford, zero);
}

#[test]
fn cq2_generation_repetition_matches_complete_pinned_stim_case() {
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
        generated.hint_text(),
        "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     Z# = measurement qubit\n"
    );
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
    assert_eq!(generated, generated.clone());
    assert!(format!("{generated:?}").contains("GeneratedCircuit"));
}

#[test]
fn cq2_generation_surface_matches_complete_pinned_stim_cases() {
    assert_unrotated_surface_code_matches_stim();
    assert_rotated_surface_code_matches_stim();
}

fn assert_unrotated_surface_code_matches_stim() {
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
        generated.hint_text(),
        "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     X# = measurement qubit (X stabilizer)\n#     Z# = measurement qubit (Z stabilizer)\n"
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

    let memory_x = SurfaceCodeParams::new(
        RoundCount::try_new(1).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::UnrotatedMemoryX,
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated = generate_surface_code_circuit(&memory_x).expect("generate unrotated memory X");
    assert_eq!(
        generated.layout_text(),
        concat!("# L6 X7 d8\n", "# Z3 d4 Z5\n", "# L0 X1 d2\n")
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
            "RX 0 2 4 6 8\n",
            "Z_ERROR(0.25) 0 2 4 6 8\n",
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
            "DETECTOR(1, 0, 0) rec[-4]\n",
            "DETECTOR(1, 2, 0) rec[-1]\n",
            "Z_ERROR(0.375) 0 2 4 6 8\n",
            "MX 0 2 4 6 8\n",
            "DETECTOR(1, 0, 1) rec[-3] rec[-4] rec[-5] rec[-9]\n",
            "DETECTOR(1, 2, 1) rec[-1] rec[-2] rec[-3] rec[-6]\n",
            "OBSERVABLE_INCLUDE(0) rec[-2] rec[-5]\n",
        )
    );
}

fn assert_rotated_surface_code_matches_stim() {
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

    let distance_four = SurfaceCodeParams::new(
        RoundCount::try_new(5).expect("rounds"),
        CodeDistance::try_new(4).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated =
        generate_surface_code_circuit(&distance_four).expect("generate distance-four surface code");
    assert_eq!(
        generated.layout_text(),
        concat!(
            "#         X38             X42\n",
            "#     d28     d30     d32     d34\n",
            "#         Z29     X31     Z33\n",
            "#     d19     d21     d23     d25\n",
            "# Z18     X20     Z22     X24     Z26\n",
            "#     d10     d12     d14     d16\n",
            "#         Z11     X13     Z15\n",
            "#     L1      L3      L5      L7 \n",
            "#         X2              X6 \n",
        )
    );

    let distance_five = SurfaceCodeParams::new(
        RoundCount::try_new(5).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated =
        generate_surface_code_circuit(&distance_five).expect("generate distance-five surface code");
    assert_eq!(
        generated.layout_text(),
        concat!(
            "#                 X59             X63\n",
            "#     d45     d47     d49     d51     d53\n",
            "# Z44     X46     Z48     X50     Z52\n",
            "#     d34     d36     d38     d40     d42\n",
            "#         Z35     X37     Z39     X41     Z43\n",
            "#     d23     d25     d27     d29     d31\n",
            "# Z22     X24     Z26     X28     Z30\n",
            "#     d12     d14     d16     d18     d20\n",
            "#         Z13     X15     Z17     X19     Z21\n",
            "#     L1      L3      L5      L7      L9 \n",
            "#         X2              X6 \n",
        )
    );
}

#[test]
fn cq2_generation_color_matches_complete_pinned_stim_case() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_color_code.test.cc.
    let distance_five = ColorCodeParams::new(
        RoundCount::try_new(100).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated =
        generate_color_code_circuit(&distance_five).expect("generate distance-five color code");
    assert_eq!(
        generated.layout_text(),
        concat!(
            "#                         d27\n",
            "#                     d25     R26\n",
            "#                 B22     d23     d24\n",
            "#             d18     d19     G20     d21\n",
            "#         d13     R14     d15     d16     R17\n",
            "#     B7      d8      d9      B10     d11     d12\n",
            "# L0      L1      G2      L3      L4      G5      L6 \n",
        )
    );

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
    assert_eq!(
        generated.hint_text(),
        "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     R# = measurement qubit (red hex)\n#     G# = measurement qubit (green hex)\n#     B# = measurement qubit (blue hex)\n"
    );
}

#[test]
fn cq2_generation_color_rejects_invalid_family_parameters() {
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
    let distance_two = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params");
    let minimum = ColorCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        ColorCodeTask::MemoryXyz,
    )
    .expect("params");

    assert_invalid_domain_value(
        generate_color_code_circuit(&too_few_rounds),
        "color code round count",
        "1",
    );
    for params in [&distance_two, &even_distance] {
        assert_invalid_domain_value(
            generate_color_code_circuit(params),
            "color code distance",
            &params.distance().get().to_string(),
        );
    }
    generate_color_code_circuit(&minimum).expect("minimum color-code shape must generate");
}

#[test]
fn cq2_generation_no_noise_matrix_has_no_detection_or_observable_events() {
    let distances = [2, 3, 4, 5, 6, 7, 15];
    let rounds = [1, 2, 3, 4, 5, 6, 20];
    let surface_tasks = [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
        SurfaceCodeTask::UnrotatedMemoryX,
        SurfaceCodeTask::UnrotatedMemoryZ,
    ];
    let cases = distances
        .into_iter()
        .flat_map(|distance| rounds.into_iter().map(move |rounds| (distance, rounds)))
        .collect::<Vec<_>>();
    let next_case = std::sync::atomic::AtomicUsize::new(0);
    let worker_count = std::thread::available_parallelism()
        .map_or(1, std::num::NonZeroUsize::get)
        .min(8)
        .min(cases.len());
    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            scope.spawn(|| {
                loop {
                    let index = next_case.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(distance, rounds)) = cases.get(index) else {
                        break;
                    };
                    assert_no_noise_matrix_cell(distance, rounds, surface_tasks);
                }
            });
        }
    });

    assert_representative_no_noise_samples(surface_tasks);

    let batch_reference = SurfaceCodeParams::new(
        RoundCount::try_new(3).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface parameters");
    let generated =
        generate_surface_code_circuit(&batch_reference).expect("generate batch reference case");
    assert_generated_samples_are_zero(&generated, 256, "portable 256-shot reference");

    let python_reference = SurfaceCodeParams::new(
        RoundCount::try_new(10).expect("rounds"),
        CodeDistance::try_new(5).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("surface parameters");
    let generated =
        generate_surface_code_circuit(&python_reference).expect("generate Python reference case");
    let output = sample_detection_events(generated.circuit(), 5, Some(0xC0DE))
        .expect("sample Python reference case");
    assert_eq!(output.detector_count, 24 * 10);
    assert_eq!(output.observable_count, 1);
    assert_detection_output_is_zero(&output, 5, "Python d=5 r=10 reference");
}

fn assert_no_noise_matrix_cell(
    distance_value: u32,
    round_value: u64,
    surface_tasks: [SurfaceCodeTask; 4],
) {
    let distance = CodeDistance::try_new(distance_value).expect("matrix distance");
    let round_count = RoundCount::try_new(round_value).expect("matrix rounds");
    let repetition = RepetitionCodeParams::new(round_count, distance, RepetitionCodeTask::Memory)
        .expect("repetition parameters");
    let generated =
        generate_repetition_code_circuit(&repetition).expect("generate repetition matrix");
    assert_generated_structure(
        &generated,
        u64::from(distance_value - 1) * (round_value + 1),
        &format!("repetition d={distance_value} r={round_value}"),
    );

    for task in surface_tasks {
        let params =
            SurfaceCodeParams::new(round_count, distance, task).expect("surface parameters");
        let generated = generate_surface_code_circuit(&params).expect("generate surface matrix");
        let rotated = matches!(
            task,
            SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::RotatedMemoryZ
        );
        let (x_measurements, z_measurements) = surface_measurement_counts(distance_value, rotated);
        let chosen_measurements = match task {
            SurfaceCodeTask::RotatedMemoryX | SurfaceCodeTask::UnrotatedMemoryX => x_measurements,
            SurfaceCodeTask::RotatedMemoryZ | SurfaceCodeTask::UnrotatedMemoryZ => z_measurements,
        };
        assert_generated_structure(
            &generated,
            (round_value - 1) * (x_measurements + z_measurements) + 2 * chosen_measurements,
            &format!("surface {task:?} d={distance_value} r={round_value}"),
        );
    }

    if round_value >= 2 && distance_value >= 3 && distance_value % 2 == 1 {
        let params = ColorCodeParams::new(round_count, distance, ColorCodeTask::MemoryXyz)
            .expect("color parameters");
        let generated = generate_color_code_circuit(&params).expect("generate color matrix");
        assert_generated_structure(
            &generated,
            color_measurement_count(distance_value) * round_value,
            &format!("color d={distance_value} r={round_value}"),
        );
    }
}

fn assert_representative_no_noise_samples(surface_tasks: [SurfaceCodeTask; 4]) {
    let distance = CodeDistance::try_new(7).expect("sample distance");
    let rounds = RoundCount::try_new(6).expect("sample rounds");
    let repetition = RepetitionCodeParams::new(rounds, distance, RepetitionCodeTask::Memory)
        .expect("repetition parameters");
    let generated =
        generate_repetition_code_circuit(&repetition).expect("generate repetition sample");
    assert_generated_samples_are_zero(&generated, 1, "repetition d=7 r=6");

    for task in surface_tasks {
        let params = SurfaceCodeParams::new(rounds, distance, task).expect("surface parameters");
        let generated = generate_surface_code_circuit(&params).expect("generate surface sample");
        assert_generated_samples_are_zero(&generated, 1, &format!("surface {task:?} d=7 r=6"));
    }

    let color =
        ColorCodeParams::new(rounds, distance, ColorCodeTask::MemoryXyz).expect("color parameters");
    let generated = generate_color_code_circuit(&color).expect("generate color sample");
    assert_generated_samples_are_zero(&generated, 1, "color d=7 r=6");
}

fn assert_generated_structure(
    generated: &GeneratedCircuit,
    expected_detectors: u64,
    context: &str,
) {
    let circuit = generated.circuit();
    assert_eq!(
        circuit.count_detectors().expect("detector count"),
        expected_detectors,
        "{context}: detector count"
    );
    assert_eq!(
        circuit.count_observables().expect("observable count"),
        1,
        "{context}: observable count"
    );
    let text = circuit.to_stim_string();
    for noise_gate in ["X_ERROR", "Z_ERROR", "DEPOLARIZE1", "DEPOLARIZE2"] {
        assert!(
            !text.contains(noise_gate),
            "{context}: zero-noise circuit contains {noise_gate}"
        );
    }
    let dem = circuit_to_detector_error_model(
        circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    );
    assert!(
        dem.is_ok(),
        "{context}: deterministic analysis failed: {:?}",
        dem.as_ref().err()
    );
    let dem = dem.expect("deterministic analysis result was checked above");
    assert_eq!(
        dem.count_errors().expect("analyzed error count"),
        0,
        "{context}: noiseless generated circuit produced an error mechanism"
    );
}

fn color_measurement_count(distance: u32) -> u64 {
    let width = distance + (distance - 1) / 2;
    let mut count = 0_u64;
    for y in 0..width {
        for x in 0..(width - y) {
            if (x + 2 * y) % 3 == 2 {
                count += 1;
            }
        }
    }
    count
}

fn surface_measurement_counts(distance: u32, rotated: bool) -> (u64, u64) {
    let mut x_measurements = 0_u64;
    let mut z_measurements = 0_u64;
    if rotated {
        for x in 0..=distance {
            for y in 0..=distance {
                let on_x_boundary = x == 0 || x == distance;
                let on_y_boundary = y == 0 || y == distance;
                let parity = x % 2 != y % 2;
                if (on_x_boundary && parity) || (on_y_boundary && !parity) {
                    continue;
                }
                if parity {
                    x_measurements += 1;
                } else {
                    z_measurements += 1;
                }
            }
        }
    } else {
        for x in 0..(2 * distance - 1) {
            for y in 0..(2 * distance - 1) {
                if x % 2 == y % 2 {
                    continue;
                }
                if x % 2 == 0 {
                    z_measurements += 1;
                } else {
                    x_measurements += 1;
                }
            }
        }
    }
    (x_measurements, z_measurements)
}

fn assert_generated_samples_are_zero(generated: &GeneratedCircuit, shots: usize, context: &str) {
    let output = sample_detection_events(generated.circuit(), shots, Some(0x5EED));
    assert!(
        output.is_ok(),
        "{context}: failed to sample: {:?}",
        output.as_ref().err()
    );
    let output = output.expect("sampling result was checked above");
    assert_detection_output_is_zero(&output, shots, context);
}

fn assert_detection_output_is_zero(
    output: &stab_core::DetectionConversionOutput,
    shots: usize,
    context: &str,
) {
    assert_eq!(output.records.len(), shots, "{context}: shot count");
    for record in &output.records {
        assert!(
            record.detectors.iter().all(|bit| !bit),
            "{context}: nonzero detector"
        );
        assert!(
            record.observables.iter().all(|bit| !bit),
            "{context}: nonzero observable"
        );
    }
}
