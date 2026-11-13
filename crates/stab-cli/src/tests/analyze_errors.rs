//! `stab analyze_errors` CLI behavior beyond the pinned-Stim exact-output manifest rows.

use super::support::{Scratch, run};

const REMNANT_EDGE_CIRCUIT: &[u8] = b"X_ERROR(0.125) 0\nX_ERROR(0.375) 2\nM 0 1 2\n\
DETECTOR rec[-3] rec[-1]\nDETECTOR rec[-2] rec[-1]\nDETECTOR rec[-3] rec[-1]\n";
const PAULI_CHANNEL_1: &[u8] =
    b"R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n";
const GAUGE_PAIR: &[u8] = b"R 0\nH 0\nCNOT 0 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n";

#[test]
fn analyze_errors_cli_accepts_selected_flag_shapes() {
    for (args, input, expected) in [
        (
            &["stab", "analyze_errors"][..],
            &b"X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n"[..],
            "error(0.25) D0\n",
        ),
        (
            &["stab", "analyze_errors", "--fold_loops"],
            b"REPEAT 3 {\n    R 0\n    X_ERROR(0.25) 0\n    M 0\n    DETECTOR rec[-1]\n}\n",
            "repeat 3 {\n    error(0.25) D0\n    shift_detectors 1\n}\n",
        ),
        (
            &["stab", "analyze_errors", "--allow_gauge_detectors"],
            GAUGE_PAIR,
            "error(0.5) D0 D1\n",
        ),
        (
            &["stab", "analyze_errors", "--approximate_disjoint_errors"],
            PAULI_CHANNEL_1,
            "error(0.375) D0\n",
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--approximate_disjoint_errors",
                "0.5",
            ],
            PAULI_CHANNEL_1,
            "error(0.375) D0\n",
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--decompose_errors",
                "--block_decompose_from_introducing_remnant_edges",
                "--ignore_decomposition_failures",
            ],
            REMNANT_EDGE_CIRCUIT,
            "error(0.375) D0 D1 D2\nerror(0.125) D0 D2\n",
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--decompose_errors",
                "--fold_loops",
                "--allow_gauge_detectors",
                "--approximate_disjoint_errors",
            ],
            b"M 0\nDETECTOR rec[-1]\n",
            "detector D0\n",
        ),
    ] {
        run(args, input).assert_ok(expected);
    }
}

#[test]
fn analyze_errors_cli_rejects_selected_analysis_failures() {
    for (args, input, expected) in [
        (
            &["stab", "analyze_errors"][..],
            PAULI_CHANNEL_1,
            &["PAULI_CHANNEL_1", "requires approximate_disjoint_errors"][..],
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--approximate_disjoint_errors",
                "0.3",
            ],
            b"R 0\nPAULI_CHANNEL_1(0, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
            &["PAULI_CHANNEL_1", "0.375", "0.3"],
        ),
        (
            &["stab", "analyze_errors"],
            GAUGE_PAIR,
            &["non-deterministic detectors", "D0", "D1"],
        ),
        (
            &["stab", "analyze_errors", "--allow_gauge_detectors"],
            b"R 0\nH 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            &["non-deterministic observables", "L0"],
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--decompose_errors",
                "--block_decompose_from_introducing_remnant_edges",
            ],
            REMNANT_EDGE_CIRCUIT,
            &[
                "Failed to decompose errors into graphlike components",
                "block_decomposition_from_introducing_remnant_edges",
            ],
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--approximate_disjoint_errors",
                "not-a-probability",
            ],
            b"",
            &["invalid probability threshold not-a-probability"],
        ),
        (
            &[
                "stab",
                "analyze_errors",
                "--approximate_disjoint_errors",
                "1.2",
            ],
            b"",
            &["probability threshold 1.2 is not in [0, 1]"],
        ),
        (
            &["stab", "analyze_errors"],
            b"NOT_A_GATE\n",
            &["NOT_A_GATE"],
        ),
        (
            &["stab", "analyze_errors"],
            b"DETECTOR rec[-1]\n",
            &["rec[-1]"],
        ),
    ] {
        run(args, input).assert_err(expected);
    }
}

#[test]
fn analyze_errors_opaque_error_tags_remain_distinct() {
    run(
        &["stab", "analyze_errors"],
        b"X_ERROR[\xff](0.25) 0\n\
          X_ERROR[\xfe](0.25) 0\n\
          M 0\n\
          DETECTOR rec[-1]\n",
    )
    .assert_ok(
        b"error[\xfe](0.25) D0\n\
          error[\xff](0.25) D0\n",
    );
}

#[test]
fn analyze_errors_opaque_tag_bytes_survive_fold_loops() {
    let expected_stdout = [
        b"error[\xff](0.25) D0 L0\n".as_slice(),
        b"repeat[\xfd] 99 {\n",
        b"    error[\xfe](0.125) D0 L0\n",
        b"    detector[\xfc] D0\n",
        b"    logical_observable[\xfb] L0\n",
        b"    shift_detectors 1\n",
        b"}\n",
        b"error[\xfe](0.125) D0 L0\n",
        b"detector[\xfc] D0\n",
        b"logical_observable[\xfb] L0\n",
    ]
    .concat();

    run(
        &["stab", "analyze_errors", "--fold_loops"],
        b"R 0\n\
          X_ERROR[\xff](0.25) 0\n\
          REPEAT[\xfd] 100 {\n\
              X_ERROR[\xfe](0.125) 0\n\
              MR 0\n\
              DETECTOR[\xfc] rec[-1]\n\
              OBSERVABLE_INCLUDE[\xfb](0) rec[-1]\n\
          }\n",
    )
    .assert_ok(expected_stdout);
}

#[test]
fn analyze_errors_sweep_controls_match_pf3_oracle() {
    run(
        &["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\n\
          CX sweep[0] 0\n\
          CY sweep[1] 0\n\
          CZ sweep[2] 0\n\
          CZ 0 sweep[3]\n\
          CZ sweep[4] sweep[5]\n\
          XCZ 0 sweep[6]\n\
          YCZ 0 sweep[7]\n\
          M 1\n\
          CZ rec[-1] sweep[8]\n\
          CZ sweep[9] rec[-1]\n\
          M 2\n\
          CZ rec[-1] rec[-2]\n\
          M 0\n\
          DETECTOR rec[-1]\n",
    )
    .assert_ok(b"error(0.25) D0\n");
}

#[test]
fn analyze_errors_sweep_controls_reject_invalid_target_positions() {
    run(
        &["stab", "analyze_errors"],
        b"X_ERROR(0.25) 0\nCX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
    )
    .assert_err(&["CX target sweep[0] is not a qubit"]);

    for (gate, target) in [("CY", "rec[-2]"), ("XCZ", "rec[-1]"), ("YCZ", "rec[-1]")] {
        let input =
            format!("X_ERROR(0.25) 0\nM 0 1\n{gate} rec[-1] rec[-2]\nM 0\nDETECTOR rec[-1]\n");
        run(&["stab", "analyze_errors"], input.as_bytes())
            .assert_err(&[&format!("{gate} target {target} is not a qubit")]);
    }
}

#[test]
fn analyze_errors_rejects_excessive_repeat_nesting() {
    let input = format!("{}TICK\n{}", "REPEAT 1 {\n".repeat(257), "}\n".repeat(257));
    run(&["stab", "analyze_errors"], input.as_bytes())
        .assert_err(&["repeat nesting exceeds current limit 256"]);
}

#[test]
fn analyze_errors_reads_input_path_and_writes_output_path() {
    let scratch = Scratch::new();
    let input = scratch.file("input.stim", "X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n");
    let output = scratch.path("output.dem");
    run(
        &["stab", "analyze_errors", "--in", &input, "--out", &output],
        b"ignored stdin",
    )
    .assert_ok(b"");
    assert_eq!(scratch.read("output.dem"), b"error(0.25) D0\n");
}

#[test]
fn analyze_errors_empty_model_matches_stim_terminal_newline() {
    run(&["stab", "analyze_errors"], b"M[\xff] 0\n").assert_ok(b"\n");
}
