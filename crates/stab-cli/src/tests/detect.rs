//! `stab detect` CLI behavior beyond the pinned-Stim exact-output manifest rows.

use super::support::{Scratch, ptb64_words, run};

const FLIPPED_DETECTOR_AND_OBSERVABLE: &[u8] =
    b"X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";

#[test]
fn detect_and_sample_enforce_storage_without_restricting_sparse_qubit_syntax() {
    // Detection's two frame planes and both packed batches reach 256 MiB at this ID. Sampling's
    // wider bit-plane frame reaches its limit at a lower ID, and no single-measurement fast path
    // exempts the full Stim qubit-ID range from that frame any more.
    for (command, qubit, expected_error) in [
        ("detect", 16_777_150, None),
        (
            "detect",
            16_777_215,
            Some("detection session needs an estimated 268436496 bytes"),
        ),
        ("sample", 4_000_000, None),
        (
            "sample",
            16_777_215,
            Some("sampling session needs an estimated 1073741864 bytes"),
        ),
    ] {
        let input = format!("M {qubit}\nDETECTOR rec[-1]\n");
        // Sampling skips the reference sample, whose tableau would be quadratic in the ID.
        let args: &[&str] = if command == "sample" {
            &["stab", command, "--shots", "1", "--skip_reference_sample"]
        } else {
            &["stab", command, "--shots", "1"]
        };
        let result = run(args, input.as_bytes());
        match expected_error {
            None => result.assert_ok(b"0\n"),
            Some(message) => result.assert_err(&[message, "268435456-byte safety limit"]),
        }
    }
}

#[test]
fn detect_accepts_default_false_sweep_conditioned_sampling() {
    run(
        &["stab", "detect", "--shots", "3"],
        b"CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .assert_ok(b"0\n0\n0\n");
    run(
        &["stab", "detect", "--shots", "3", "--append_observables"],
        b"RX 0\nCX sweep[0] 0\nCY sweep[1] 0\nCZ 0 sweep[2]\nXCZ 0 sweep[3]\nYCZ 0 sweep[4]\n\
OBSERVABLE_INCLUDE(0) X0\n",
    )
    .assert_ok(b"0\n0\n0\n");
}

#[test]
fn detect_accepts_frame_path_xcz_ycz_measurement_feedback() {
    for measured_state in ["M 0", "X_ERROR(1) 0\nM 0"] {
        let feedback = |first: &str, second: &str| {
            format!(
                "R 0 1 2\n{measured_state}\n{first}\n{second}\n\
                 OBSERVABLE_INCLUDE(0) Z1\nOBSERVABLE_INCLUDE(1) Z2\n"
            )
        };
        let args = [
            "stab",
            "detect",
            "--shots",
            "3",
            "--append_observables",
            "--seed=7",
        ];
        let xcz = run(&args, feedback("XCZ 1 rec[-1]", "YCZ 2 rec[-1]").as_bytes());
        let cxy = run(&args, feedback("CX rec[-1] 1", "CY rec[-1] 2").as_bytes());

        assert_eq!(xcz.status, 0);
        assert_eq!(xcz.stderr_text(), "");
        assert!(!xcz.stdout.is_empty());
        assert_eq!(xcz, cxy);
    }
}

#[test]
fn detect_dets_output_includes_observables_by_default_like_stim() {
    let input = FLIPPED_DETECTOR_AND_OBSERVABLE;
    run(&["stab", "detect", "--out_format=dets"], input).assert_ok(b"shot L0 D0\n");
    run(
        &[
            "stab",
            "detect",
            "--out_format=dets",
            "--append_observables",
        ],
        input,
    )
    .assert_ok(b"shot D0 L0\n");
}

#[test]
fn detect_zero_shots_returns_without_parsing_input() {
    run(&["stab", "detect", "--shots", "0"], b"not a stim circuit").assert_ok(b"");
}

#[test]
fn detect_rejects_conflicting_observable_routes() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.01");
    run(
        &["stab", "detect", "--out_format=dets", "--obs_out", &obs],
        b"M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .assert_err(&["cannot combine --obs_out with detector output that includes observables"]);
    assert!(!scratch.exists("obs.01"));
}

#[test]
fn detect_appends_observables_and_writes_bit_packed_output() {
    let input = FLIPPED_DETECTOR_AND_OBSERVABLE;
    run(&["stab", "detect", "--append_observables"], input).assert_ok(b"11\n");
    run(
        &[
            "stab",
            "detect",
            "--append_observables",
            "--out_format",
            "b8",
        ],
        input,
    )
    .assert_ok([0b0000_0011]);
}

#[test]
fn detect_writes_ptb64_detector_and_observable_outputs() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.ptb64");
    run(
        &[
            "stab",
            "detect",
            "--shots=64",
            "--out_format=ptb64",
            "--obs_out_format=ptb64",
            "--obs_out",
            &obs,
        ],
        FLIPPED_DETECTOR_AND_OBSERVABLE,
    )
    .assert_ok(ptb64_words(&[u64::MAX]));
    assert_eq!(scratch.read("obs.ptb64"), ptb64_words(&[u64::MAX]));
}

#[test]
fn detect_can_write_observables_to_a_separate_file() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.b8");
    run(
        &[
            "stab",
            "detect",
            "--out_format=01",
            "--obs_out_format=b8",
            "--obs_out",
            &obs,
        ],
        FLIPPED_DETECTOR_AND_OBSERVABLE,
    )
    .assert_ok(b"1\n");
    assert_eq!(scratch.read("obs.b8"), [0b0000_0001]);
}
