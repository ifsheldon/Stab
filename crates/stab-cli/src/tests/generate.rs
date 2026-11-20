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
    // At 1 MiB plus 768 bytes per physical qubit, a 256 MiB budget admits at most 348,160 qubits:
    // rotated distance 417 (347,777 qubits) and unrotated distance 295 (346,921) fit, and the next
    // distances are refused.
    for (task, distance, projected_qubits) in [
        ("rotated_memory_z", "418", "349447"),
        ("unrotated_memory_z", "296", "349281"),
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
            "557",
            "--rounds",
            "2",
            "--out",
            &output,
            "--memory-budget=256MiB",
        ],
        b"",
    )
    .assert_err(&["349030 physical qubits", "-byte memory budget"]);
    assert!(
        !scratch.exists("oversized.stim"),
        "rejection must not create a partial output"
    );

    // 256 MiB admits rotated distance 257 (132,097 qubits), which the former 2048-byte-per-qubit
    // projection refused, and the default 1 GiB budget refuses distance 836 (1,397,791 qubits).
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
            "--memory-budget=256MiB",
        ],
        b"",
    );
    assert_eq!(admitted.status, 0, "{}", admitted.stderr_text());
    run(
        &[
            "stab",
            "gen",
            "--code",
            "surface_code",
            "--task",
            "rotated_memory_z",
            "--distance",
            "836",
            "--rounds",
            "1",
        ],
        b"",
    )
    .assert_err(&["1397791 physical qubits", "1073741824-byte memory budget"]);
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

/// A named `--out` file's write buffer is held beside the generated circuit, so the generation
/// that just fits a budget writing to stdout needs exactly one output buffer more to write the
/// same circuit to a file, and the refusal leaves an existing output untouched.
#[test]
fn gen_charges_a_named_output_buffer_against_the_budget() {
    use stab_model::MemoryBudget;

    let scratch = Scratch::new();
    let out = scratch.path("generated.stim");
    let generate = |budget: u64, named: bool| {
        let budget = format!("--memory-budget={budget}");
        let mut args = REPETITION.to_vec();
        args.push(budget.as_str());
        if named {
            args.extend(["--out", out.as_str()]);
        }
        run(&args, b"")
    };
    let (mut refused, mut smallest) = (0_u64, MemoryBudget::DEFAULT.bytes());
    while refused + 1 < smallest {
        let middle = refused + (smallest - refused) / 2;
        if generate(middle, false).status == 0 {
            smallest = middle;
        } else {
            refused = middle;
        }
    }
    let to_stdout = generate(smallest, false);
    assert_eq!(to_stdout.status, 0, "{}", to_stdout.stderr_text());

    let buffer = crate::io_plan::OUTPUT_BUFFER_BYTES as u64;
    scratch.file("generated.stim", "sentinel\n");
    let short = generate(smallest + buffer - 1, true);
    assert_eq!(short.status, 1, "a named output must charge its buffer");
    short.assert_stderr_contains(&["-byte memory budget"]);
    assert_eq!(
        scratch.read("generated.stim"),
        b"sentinel\n",
        "refusal truncated the output"
    );
    generate(smallest + buffer, true).assert_ok(b"");
    assert_eq!(scratch.read("generated.stim"), to_stdout.stdout);
}

/// A task is looked up among its `--code` family's tasks, whose names `stab-analysis` owns, and
/// the refusal keeps one diagnostic code per family. Arguments are checked in pinned Stim's
/// order, so an invalid round count is reported before an unknown task, and an unknown task
/// before the color code's shape constraints.
#[test]
fn gen_rejects_tasks_outside_the_code_family_in_stim_order() {
    let gen_args = |code: &'static str, task: &'static str, distance: &'static str, rounds| {
        vec![
            "stab",
            "gen",
            "--error-format=json",
            "--code",
            code,
            "--task",
            task,
            "--distance",
            distance,
            "--rounds",
            rounds,
        ]
    };
    for (code, task, diagnostic, expected) in [
        (
            "repetition_code",
            "rotated_memory_x",
            "unsupported-repetition-task",
            "unsupported repetition_code task \\\"rotated_memory_x\\\"; expected memory",
        ),
        (
            "surface_code",
            "memory",
            "unsupported-surface-task",
            "expected rotated_memory_x, rotated_memory_z, unrotated_memory_x, or unrotated_memory_z",
        ),
        (
            "color_code",
            "bogus",
            "unsupported-color-task",
            "unsupported color_code task \\\"bogus\\\"; expected memory_xyz",
        ),
    ] {
        run(&gen_args(code, task, "2", "1"), b"").assert_err(&[
            &format!("\"code\":\"{diagnostic}\""),
            expected,
            &format!("\"task\":\"{task}\""),
        ]);
    }
    run(&gen_args("surface_code", "bogus", "1", "0"), b"").assert_err(&["round count"]);
    run(&gen_args("color_code", "memory_xyz", "2", "1"), b"")
        .assert_err(&["color code round count"]);
    run(&gen_args("color_code", "memory_xyz", "4", "2"), b"")
        .assert_err(&["color code distance value 4"]);
}

/// `gen --out` opens its file through the output preflight every other command uses, after
/// generation succeeds: a path it cannot open fails exactly as `analyze_errors --out` does, a
/// symlink or device is written through as in Stim, an existing file is replaced only by a run
/// that succeeds, and a write failure mid-stream names the path.
#[test]
fn gen_named_output_matches_the_shared_output_preflight() {
    let scratch = Scratch::new();
    let circuit = scratch.file("circuit.stim", "R 0\nM 0\nDETECTOR rec[-1]\n");
    std::fs::create_dir(scratch.path_buf("directory")).expect("create directory");
    let expected = run(REPETITION, b"");
    assert_eq!(expected.status, 0, "{}", expected.stderr_text());
    let gen_to = |path: &str| run(&[REPETITION, &["--out", path]].concat(), b"");

    for name in ["missing-directory/generated.stim", "directory"] {
        let path = scratch.path(name);
        let generated = gen_to(&path);
        generated.assert_err(&["failed to write", &path]);
        let analyzed = run(
            &["stab", "analyze_errors", "--in", &circuit, "--out", &path],
            b"",
        );
        assert_eq!(generated, analyzed, "{name}");
    }
    assert!(!scratch.exists("missing-directory"));

    let longer = "x".repeat(expected.stdout.len() * 2);
    scratch.file("target.stim", &longer);
    std::os::unix::fs::symlink(
        scratch.path_buf("target.stim"),
        scratch.path_buf("link.stim"),
    )
    .expect("create symlink");
    gen_to(&scratch.path("link.stim")).assert_ok(b"");
    assert_eq!(scratch.read("target.stim"), expected.stdout);

    std::os::unix::fs::symlink(
        scratch.path_buf("created.stim"),
        scratch.path_buf("dangling"),
    )
    .expect("create dangling symlink");
    gen_to(&scratch.path("dangling")).assert_ok(b"");
    assert_eq!(scratch.read("created.stim"), expected.stdout);

    gen_to("/dev/null").assert_ok(b"");

    scratch.file("existing.stim", &longer);
    let existing = scratch.path("existing.stim");
    let mut invalid = REPETITION.to_vec();
    invalid.extend([
        "--before_measure_flip_probability",
        "1.1",
        "--out",
        &existing,
    ]);
    run(&invalid, b"").assert_err(&["probability"]);
    let mut refused = REPETITION.to_vec();
    let fresh = scratch.path("fresh.stim");
    refused.extend(["--memory-budget=1KiB", "--out", &fresh]);
    run(&refused, b"").assert_err(&["memory budget"]);
    assert_eq!(scratch.read("existing.stim"), longer.as_bytes());
    assert!(!scratch.exists("fresh.stim"), "refusal created the output");
    gen_to(&existing).assert_ok(b"");
    assert_eq!(scratch.read("existing.stim"), expected.stdout);

    // Rotated distance 25 spills the output buffer several times before the final flush.
    run(
        &[
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
            "--out",
            "/dev/full",
        ],
        b"",
    )
    .assert_err(&["failed to write /dev/full", "No space left on device"]);
}
