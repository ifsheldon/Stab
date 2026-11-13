//! `stab gen` behavior not covered by the pinned-Stim exact-output manifest rows.

use super::support::{Scratch, run};

const REPETITION: &[&str] = &[
    "stab",
    "gen",
    "--code",
    "repetition_code",
    "--task",
    "memory",
    "--distance",
    "3",
    "--rounds",
    "2",
];

#[test]
fn gen_writes_output_path_and_maps_noise_flags() {
    let scratch = Scratch::new();
    let output = scratch.path("generated.stim");
    let mut args: Vec<&str> = REPETITION.to_vec();
    args.extend([
        "--after_clifford_depolarization",
        "0.01",
        "--after_reset_flip_probability",
        "0.02",
        "--before_measure_flip_probability",
        "0.03",
        "--before_round_data_depolarization",
        "0.04",
        "--out",
        output.as_str(),
    ]);
    run(&args, b"").assert_ok(b"");

    let generated = String::from_utf8(scratch.read("generated.stim")).expect("UTF-8 circuit");
    for expected in [
        "# before_round_data_depolarization: 0.04",
        "# before_measure_flip_probability: 0.03",
        "# after_reset_flip_probability: 0.02",
        "# after_clifford_depolarization: 0.01",
        "R 0 1 2 3 4\nX_ERROR(0.02) 0 1 2 3 4\n",
        "TICK\nDEPOLARIZE1(0.04) 0 2 4\nCX 0 1 2 3\n",
        "CX 0 1 2 3\nDEPOLARIZE2(0.01) 0 1 2 3\n",
        "TICK\nX_ERROR(0.03) 1 3\nMR 1 3\nX_ERROR(0.02) 1 3\n",
        "X_ERROR(0.03) 0 2 4\nM 0 2 4\n",
    ] {
        assert!(generated.contains(expected), "missing {expected}");
    }
}

/// Stim's `arg_parse.test.cc` cases: unknown, missing, malformed, out-of-range, and over-i64
/// arguments fail with status 1, no stdout, and a diagnostic naming the offending input.
#[test]
fn gen_rejects_invalid_arguments_like_arg_parse_test() {
    let replace = |flag: &str, value: &'static str| {
        let mut args = REPETITION.to_vec();
        let index = args.iter().position(|arg| *arg == flag).expect("flag") + 1;
        *args.get_mut(index).expect("flag value") = value;
        args
    };
    let mut unknown = REPETITION.to_vec();
    unknown.extend(["--unknown", "5"]);
    let mut probability = REPETITION.to_vec();
    probability.extend(["--before_measure_flip_probability", "1.1"]);
    let missing_code = REPETITION
        .iter()
        .copied()
        .filter(|arg| !matches!(*arg, "--code" | "repetition_code"))
        .collect::<Vec<_>>();

    for (args, expected) in [
        (unknown, "--unknown"),
        (missing_code, "--code"),
        (replace("--distance", "not-an-int"), "invalid value"),
        (replace("--code", "unknown_code"), "unknown_code"),
        (probability, "probability"),
        (
            replace("--rounds", "9223372036854775808"),
            "greater than Stim's i64 maximum",
        ),
    ] {
        run(&args, b"").assert_err(&[expected]);
    }
}

#[test]
fn gen_rejects_quadratic_outputs_before_writing() {
    for (task, distance, projected_qubits) in [
        ("rotated_memory_z", "257", "132097"),
        ("unrotated_memory_z", "182", "131769"),
    ] {
        run(
            &[
                "stab",
                "gen",
                "--code",
                "surface_code",
                "--task",
                task,
                "--distance",
                distance,
                "--rounds",
                "1",
            ],
            b"",
        )
        .assert_err(&[projected_qubits, "current limit is 131072"]);
    }

    let scratch = Scratch::new();
    let output = scratch.path("oversized.stim");
    run(
        &[
            "stab",
            "gen",
            "--code",
            "color_code",
            "--task",
            "memory_xyz",
            "--distance",
            "343",
            "--rounds",
            "2",
            "--out",
            &output,
        ],
        b"",
    )
    .assert_err(&["132355", "current limit is 131072"]);
    assert!(
        !scratch.exists("oversized.stim"),
        "rejection must not create a partial output"
    );
}
