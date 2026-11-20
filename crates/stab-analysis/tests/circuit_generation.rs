#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        reason = "tests use direct assertions for compact compatibility diagnostics"
    )
)]

use stab_analysis::{
    AnalysisError, CodeCircuitParams, CodeDistance, CodeFamily, CodeTask, ColorCodeTask,
    GeneratorParameter, RepetitionCodeTask, RoundCount, SurfaceCodeTask,
};
use stab_model::Probability;

#[path = "circuit_generation/resource.rs"]
mod resource;

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("valid probability")
}

fn assert_invalid_generator_parameter<T>(
    result: Result<T, AnalysisError>,
    parameter: GeneratorParameter,
    value: u64,
    message: &str,
) {
    let error = result.err();
    assert_eq!(
        error,
        Some(AnalysisError::InvalidGeneratorParameter { parameter, value })
    );
    assert_eq!(
        error.map(|error| error.to_string()).as_deref(),
        Some(message)
    );
}

#[test]
fn cq2_generation_parameter_contract_covers_typed_values_and_noise_builders() {
    let minimum_distance = CodeDistance::try_new(2).expect("minimum distance");
    let maximum_distance = CodeDistance::try_new(2047).expect("maximum distance");
    assert_eq!(minimum_distance.get(), 2);
    assert_eq!(maximum_distance.get(), 2047);
    assert_invalid_generator_parameter(
        CodeDistance::try_new(0),
        GeneratorParameter::CodeDistance,
        0,
        "invalid code distance value 0",
    );
    assert_invalid_generator_parameter(
        CodeDistance::try_new(1),
        GeneratorParameter::CodeDistance,
        1,
        "invalid code distance value 1",
    );
    assert_invalid_generator_parameter(
        CodeDistance::try_new(2048),
        GeneratorParameter::CodeDistance,
        2048,
        "invalid code distance value 2048",
    );

    let minimum_rounds = RoundCount::try_new(1).expect("minimum rounds");
    let maximum_rounds = RoundCount::try_new(u64::MAX).expect("maximum rounds");
    assert_eq!(minimum_rounds.get(), 1);
    assert_eq!(maximum_rounds.get(), u64::MAX);
    assert_invalid_generator_parameter(
        RoundCount::try_new(0),
        GeneratorParameter::RoundCount,
        0,
        "invalid round count value 0",
    );

    let before_round = probability(0.0625);
    let before_measure = probability(0.125);
    let after_reset = probability(0.25);
    let after_clifford = probability(0.5);
    let zero = probability(0.0);

    for (task, distance, rounds) in [
        (CodeTask::Repetition(RepetitionCodeTask::Memory), 5, 3),
        (CodeTask::Surface(SurfaceCodeTask::UnrotatedMemoryX), 9, 7),
        (CodeTask::Color(ColorCodeTask::MemoryXyz), 13, 11),
    ] {
        let defaults = CodeCircuitParams::try_new(
            task,
            CodeDistance::try_new(distance).expect("distance"),
            RoundCount::try_new(rounds).expect("rounds"),
        )
        .expect("parameters");
        for default in [
            defaults.before_round_data_depolarization(),
            defaults.before_measure_flip_probability(),
            defaults.after_reset_flip_probability(),
            defaults.after_clifford_depolarization(),
        ] {
            assert_eq!(default, zero, "{task:?}");
        }
        let noisy = defaults
            .clone()
            .with_before_round_data_depolarization(before_round)
            .with_before_measure_flip_probability(before_measure)
            .with_after_reset_flip_probability(after_reset)
            .with_after_clifford_depolarization(after_clifford);
        assert_eq!(noisy.task(), task);
        assert_eq!(noisy.distance().get(), distance);
        assert_eq!(noisy.rounds().get(), rounds);
        assert_eq!(noisy.before_round_data_depolarization(), before_round);
        assert_eq!(noisy.before_measure_flip_probability(), before_measure);
        assert_eq!(noisy.after_reset_flip_probability(), after_reset);
        assert_eq!(noisy.after_clifford_depolarization(), after_clifford);
        assert_eq!(defaults.before_round_data_depolarization(), zero);
    }
}

/// Code and task names are Stim's `--code` and `--task` spellings, owned by the task types, and a
/// task name is looked up only among its own family's tasks.
#[test]
fn cq2_generation_code_and_task_names_round_trip_stim_spellings() {
    let families = CodeFamily::ALL.map(CodeFamily::as_str);
    assert_eq!(families, ["repetition_code", "surface_code", "color_code"]);
    for family in CodeFamily::ALL {
        assert_eq!(family.as_str().parse::<CodeFamily>(), Ok(family));
        for name in family.task_names() {
            let task = family.parse_task(name).expect("own task");
            assert_eq!(task.family(), family);
            assert_eq!(task.as_str(), *name);
            assert_eq!(task.to_string(), *name);
        }
    }
    assert_eq!(
        SurfaceCodeTask::ALL
            .iter()
            .map(|task| task.as_str())
            .collect::<Vec<_>>(),
        [
            "rotated_memory_x",
            "rotated_memory_z",
            "unrotated_memory_x",
            "unrotated_memory_z"
        ]
    );
    assert_eq!(
        "memory_xyz".parse::<ColorCodeTask>(),
        Ok(ColorCodeTask::MemoryXyz)
    );
    assert_eq!(
        "memory".parse::<RepetitionCodeTask>(),
        Ok(RepetitionCodeTask::Memory)
    );

    let unknown_family = "toric_code"
        .parse::<CodeFamily>()
        .expect_err("unknown code");
    assert_eq!(unknown_family.name(), "toric_code");

    // Each family's message lists exactly its own tasks, in Stim's order.
    for (family, name, message) in [
        (
            CodeFamily::Repetition,
            "rotated_memory_x",
            "unsupported repetition_code task \"rotated_memory_x\"; expected memory",
        ),
        (
            CodeFamily::Surface,
            "memory",
            "unsupported surface_code task \"memory\"; expected rotated_memory_x, rotated_memory_z, unrotated_memory_x, or unrotated_memory_z",
        ),
        (
            CodeFamily::Color,
            "Memory_XYZ",
            "unsupported color_code task \"Memory_XYZ\"; expected memory_xyz",
        ),
    ] {
        let error = family.parse_task(name).expect_err("foreign task");
        assert_eq!(error.family(), family);
        assert_eq!(error.task(), name);
        assert_eq!(error.to_string(), message);
    }
}

#[test]
fn cq2_generation_repetition_matches_complete_pinned_stim_case() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_rep_code.test.cc.
    let params = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeDistance::try_new(4).expect("distance"),
        RoundCount::try_new(5000).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = params.generate().expect("generate repetition code");

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
}

#[test]
fn cq2_generation_surface_matches_complete_pinned_stim_cases() {
    assert_unrotated_surface_code_matches_stim();
    assert_rotated_surface_code_matches_stim();
}

fn assert_unrotated_surface_code_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/gen/gen_surface_code.test.cc.
    let params = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::UnrotatedMemoryZ),
        CodeDistance::try_new(2).expect("distance"),
        RoundCount::try_new(5).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = params.generate().expect("generate surface code");

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

    let memory_x = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::UnrotatedMemoryX),
        CodeDistance::try_new(2).expect("distance"),
        RoundCount::try_new(1).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated = memory_x.generate().expect("generate unrotated memory X");
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
    let params = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ),
        CodeDistance::try_new(2).expect("distance"),
        RoundCount::try_new(5).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = params.generate().expect("generate surface code");

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

    let distance_four = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ),
        CodeDistance::try_new(4).expect("distance"),
        RoundCount::try_new(5).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated = distance_four
        .generate()
        .expect("generate distance-four surface code");
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

    let distance_five = CodeCircuitParams::try_new(
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryZ),
        CodeDistance::try_new(5).expect("distance"),
        RoundCount::try_new(5).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated = distance_five
        .generate()
        .expect("generate distance-five surface code");
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
    let distance_five = CodeCircuitParams::try_new(
        CodeTask::Color(ColorCodeTask::MemoryXyz),
        CodeDistance::try_new(5).expect("distance"),
        RoundCount::try_new(100).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(probability(0.125))
    .with_after_reset_flip_probability(probability(0.25))
    .with_before_measure_flip_probability(probability(0.375))
    .with_before_round_data_depolarization(probability(0.0625));
    let generated = distance_five
        .generate()
        .expect("generate distance-five color code");
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

    let params = CodeCircuitParams::try_new(
        CodeTask::Color(ColorCodeTask::MemoryXyz),
        CodeDistance::try_new(3).expect("distance"),
        RoundCount::try_new(100).expect("rounds"),
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.125).expect("probability"))
    .with_after_reset_flip_probability(Probability::try_new(0.25).expect("probability"))
    .with_before_measure_flip_probability(Probability::try_new(0.375).expect("probability"))
    .with_before_round_data_depolarization(Probability::try_new(0.0625).expect("probability"));

    let generated = params.generate().expect("generate color code");

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

/// The color code's shape constraints are checked when the parameters are built, so no
/// `CodeCircuitParams` holds an invalid combination.
#[test]
fn cq2_generation_color_rejects_invalid_family_parameters() {
    let color = |distance: u32, rounds: u64| {
        CodeCircuitParams::try_new(
            CodeTask::Color(ColorCodeTask::MemoryXyz),
            CodeDistance::try_new(distance).expect("distance"),
            RoundCount::try_new(rounds).expect("rounds"),
        )
    };
    assert_invalid_generator_parameter(
        color(3, 1),
        GeneratorParameter::ColorCodeRoundCount,
        1,
        "invalid color code round count value 1",
    );
    // The round count is checked first, as Stim does.
    assert_invalid_generator_parameter(
        color(2, 1),
        GeneratorParameter::ColorCodeRoundCount,
        1,
        "invalid color code round count value 1",
    );
    for distance in [2, 4, 2046] {
        assert_invalid_generator_parameter(
            color(distance, 2),
            GeneratorParameter::ColorCodeDistance,
            u64::from(distance),
            &format!("invalid color code distance value {distance}"),
        );
    }
    color(3, 2)
        .expect("minimum color-code shape")
        .generate()
        .expect("minimum color-code shape must generate");
    // The other families accept every typed distance and round count.
    for task in [
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        CodeTask::Surface(SurfaceCodeTask::RotatedMemoryX),
    ] {
        CodeCircuitParams::try_new(
            task,
            CodeDistance::try_new(2).expect("distance"),
            RoundCount::try_new(1).expect("rounds"),
        )
        .expect("non-color shape")
        .generate()
        .expect("non-color minimum shape must generate");
    }
}
