use super::run_from;

struct CliRun {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_analyze_errors(args: &[&str], input: &[u8]) -> CliRun {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);
    CliRun {
        status,
        stdout: String::from_utf8(stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(stderr).expect("stderr is UTF-8"),
    }
}

fn assert_success(args: &[&str], input: &[u8], expected_stdout: &str) {
    let run = run_analyze_errors(args, input);
    assert_eq!(run.status, 0, "{args:?} stderr: {}", run.stderr);
    assert_eq!(run.stdout, expected_stdout, "{args:?}");
    assert_eq!(run.stderr, "", "{args:?}");
}

fn assert_failure_contains(args: &[&str], input: &[u8], expected_parts: &[&str]) {
    let run = run_analyze_errors(args, input);
    assert_eq!(run.status, 1, "{args:?}");
    assert_eq!(run.stdout, "", "{args:?}");
    assert_ne!(run.stderr, "", "{args:?}");
    for expected in expected_parts {
        assert!(
            run.stderr.contains(expected),
            "{args:?} missing {expected:?} in {}",
            run.stderr
        );
    }
}

#[test]
fn pf7_analyze_errors_cli_accepts_selected_flag_shapes() {
    assert_success(
        &["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n",
        "error(0.25) D0\n",
    );
    assert_success(
        &["stab", "analyze_errors", "--fold_loops"],
        b"REPEAT 3 {\n    R 0\n    X_ERROR(0.25) 0\n    M 0\n    DETECTOR rec[-1]\n}\n",
        "repeat 3 {\n    error(0.25) D0\n    shift_detectors 1\n}\n",
    );
    assert_success(
        &["stab", "analyze_errors", "--allow_gauge_detectors"],
        b"R 0\nH 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "error(0.5) D0 D1\n",
    );
    assert_success(
        &["stab", "analyze_errors", "--approximate_disjoint_errors"],
        b"R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
        "error(0.375) D0\n",
    );
    assert_success(
        &[
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "0.5",
        ],
        b"R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
        "error(0.375) D0\n",
    );
    assert_success(
        &[
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--block_decompose_from_introducing_remnant_edges",
            "--ignore_decomposition_failures",
        ],
        b"X_ERROR(0.125) 0\nX_ERROR(0.375) 2\nM 0 1 2\nDETECTOR rec[-3] rec[-1]\nDETECTOR rec[-2] rec[-1]\nDETECTOR rec[-3] rec[-1]\n",
        "error(0.375) D0 D1 D2\nerror(0.125) D0 D2\n",
    );
}

#[test]
fn pf7_analyze_errors_cli_rejects_selected_analysis_failures() {
    assert_failure_contains(
        &["stab", "analyze_errors"],
        b"R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
        &["PAULI_CHANNEL_1", "requires approximate_disjoint_errors"],
    );
    assert_failure_contains(
        &[
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "0.3",
        ],
        b"R 0\nPAULI_CHANNEL_1(0, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
        &["PAULI_CHANNEL_1", "0.375", "0.3"],
    );
    assert_failure_contains(
        &["stab", "analyze_errors"],
        b"R 0\nH 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        &["non-deterministic detectors", "D0", "D1"],
    );
    assert_failure_contains(
        &["stab", "analyze_errors", "--allow_gauge_detectors"],
        b"R 0\nH 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &["non-deterministic observables", "L0"],
    );
    assert_failure_contains(
        &[
            "stab",
            "analyze_errors",
            "--decompose_errors",
            "--block_decompose_from_introducing_remnant_edges",
        ],
        b"X_ERROR(0.125) 0\nX_ERROR(0.375) 2\nM 0 1 2\nDETECTOR rec[-3] rec[-1]\nDETECTOR rec[-2] rec[-1]\nDETECTOR rec[-3] rec[-1]\n",
        &[
            "Failed to decompose errors into graphlike components",
            "block_decomposition_from_introducing_remnant_edges",
        ],
    );
}

#[test]
fn pf7_analyze_errors_cli_rejects_invalid_threshold_arguments() {
    assert_failure_contains(
        &[
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "not-a-probability",
        ],
        b"",
        &["invalid probability threshold not-a-probability"],
    );
    assert_failure_contains(
        &[
            "stab",
            "analyze_errors",
            "--approximate_disjoint_errors",
            "1.2",
        ],
        b"",
        &["probability threshold 1.2 is not in [0, 1]"],
    );
}

#[test]
fn pf7_analyze_errors_cli_rejects_malformed_stdin_without_output() {
    assert_failure_contains(
        &["stab", "analyze_errors"],
        b"NOT_A_GATE\n",
        &["NOT_A_GATE"],
    );
    assert_failure_contains(
        &["stab", "analyze_errors"],
        b"DETECTOR rec[-1]\n",
        &["rec[-1]"],
    );
}
