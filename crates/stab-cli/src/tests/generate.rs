//! `stab gen` behavior not covered by the pinned-Stim exact-output manifest rows.

use super::support::{CountingWriter, FailAfterOneWrite, Scratch, run};
use crate::run_from;

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
fn gen_admits_generated_size_against_the_memory_budget_before_writing() {
    // At 2048 conservative bytes per physical qubit, a 256 MiB budget admits exactly the
    // 131,072 qubits the retired fixed cap admitted.
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
                "--memory-budget=256MiB",
            ],
            b"",
        )
        .assert_err(&[
            &format!("{projected_qubits} physical qubits"),
            "268435456-byte memory budget",
            "--memory-budget",
        ]);
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
            "--memory-budget=256MiB",
        ],
        b"",
    )
    .assert_err(&["132355 physical qubits", "268435456-byte memory budget"]);
    assert!(
        !scratch.exists("oversized.stim"),
        "rejection must not create a partial output"
    );

    // The default budget admits the distance the retired cap refused.
    let admitted = run(
        &[
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "257",
            "--rounds",
            "1",
        ],
        b"",
    );
    assert_eq!(admitted.status, 0, "{}", admitted.stderr_text());
}

/// Pinned Stim narrows generator probabilities to `float` and echoes `-0` in
/// the header; Stab keeps the canonical `f64` probability, so only header
/// comments for negative zero and gate arguments below the `float` range differ.
/// Stim also writes the header before rejecting an unsupported generator shape;
/// Stab validates first and writes nothing.
#[test]
fn gen_keeps_double_precision_parameters_and_validates_before_writing() {
    let mut args: Vec<&str> = REPETITION.to_vec();
    args.extend(["--after_clifford_depolarization", "1e-40"]);
    let tiny = run(&args, b"");
    assert_eq!(tiny.status, 0, "{}", tiny.stderr_text());
    assert!(
        tiny.stdout_text().contains("DEPOLARIZE2(1e-40) "),
        "{}",
        tiny.stdout_text()
    );

    let mut args: Vec<&str> = REPETITION.to_vec();
    args.extend(["--after_clifford_depolarization=-0"]);
    let negative_zero = run(&args, b"");
    assert_eq!(negative_zero.status, 0, "{}", negative_zero.stderr_text());
    assert!(
        negative_zero
            .stdout_text()
            .contains("# after_clifford_depolarization: 0\n"),
        "{}",
        negative_zero.stdout_text()
    );

    let unsupported = run(
        &[
            "stab",
            "gen",
            "--code",
            "color_code",
            "--task",
            "memory_xyz",
            "--distance",
            "3",
            "--rounds",
            "1",
        ],
        b"",
    );
    assert_eq!(unsupported.status, 1);
    assert!(
        unsupported.stdout.is_empty(),
        "{}",
        unsupported.stdout_text()
    );
}

/// Generated text streams through the output buffer instead of being rendered whole first, so a
/// large circuit reaches standard output in several buffer spills and a failure on a later
/// spill still exits non-zero with the write diagnostic. A named output that cannot be created
/// reports its path, exactly as the former one-call write did.
#[test]
fn gen_streams_its_text_and_reports_write_failures() {
    let args = [
        "stab",
        "gen",
        "--code",
        "surface_code",
        "--task",
        "rotated_memory_x",
        "--distance",
        "25",
        "--rounds",
        "3",
    ];
    let mut stdout = crate::io_plan::buffered_output(CountingWriter::default());
    let mut stderr = Vec::new();
    let status = run_from(args, b"".as_slice(), &mut stdout, &mut stderr);
    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    let streamed = stdout.get_ref();
    assert!(
        streamed.bytes > 2 * crate::io_plan::OUTPUT_BUFFER_BYTES,
        "{}",
        streamed.bytes
    );
    assert!(streamed.nonempty_writes > 1, "text was written in one call");

    let mut inner = FailAfterOneWrite::default();
    let mut stderr = Vec::new();
    let status = run_from(
        args,
        b"".as_slice(),
        crate::io_plan::buffered_output(&mut inner),
        &mut stderr,
    );
    assert_eq!(status, 1);
    assert!(inner.writes >= 2);
    let stderr = String::from_utf8(stderr).expect("UTF-8 stderr");
    assert!(stderr.contains("failed to write output"), "{stderr}");

    let scratch = Scratch::new();
    let output = scratch.path("missing-directory/generated.stim");
    run(&[REPETITION, &["--out", &output]].concat(), b"")
        .assert_err(&["failed to write", "generated.stim"]);
    assert!(!scratch.exists("missing-directory"));
}
