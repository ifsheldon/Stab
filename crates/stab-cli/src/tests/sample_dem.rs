//! `stab sample_dem` output-role and replay behavior beyond the pinned-Stim manifest rows.

use stab_records::write_ptb64_records_checked;

use super::support::{CliRun, Scratch, ptb64_words, run};

const TWO_ERRORS: &[u8] = b"error(0.25) D0 L0\nerror(0.25) D1\n";

fn sample_dem(args: &[&str], dem: &[u8]) -> CliRun {
    let mut argv = vec!["stab", "sample_dem"];
    argv.extend_from_slice(args);
    run(&argv, dem)
}

#[test]
fn sample_dem_writes_observables_to_obs_out_like_upstream() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--obs_out",
            &obs,
            "--out_format",
            "01",
            "--obs_out_format",
            "01",
            "--shots",
            "5",
            "--seed",
            "0",
        ],
        b"error(0) D0\nerror(1) D1 L2\n",
    )
    .assert_ok(b"01\n01\n01\n01\n01\n");
    assert_eq!(scratch.read("obs.01"), b"001\n001\n001\n001\n001\n");
}

#[test]
fn sample_dem_dets_output_keeps_observables_separate_like_upstream() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.dets");
    sample_dem(
        &[
            "--out_format",
            "dets",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "dets",
            "--shots",
            "2",
        ],
        b"error(1) D0 L0\n",
    )
    .assert_ok(b"shot D0\nshot D0\n");
    assert_eq!(scratch.read("obs.dets"), b"shot L0\nshot L0\n");
}

#[test]
fn sample_dem_writes_error_records_to_err_out_like_upstream() {
    let scratch = Scratch::new();
    let err = scratch.path("errors.01");
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--err_out",
            &err,
            "--err_out_format",
            "01",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "01",
            "--shots",
            "2",
        ],
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n",
    )
    .assert_ok(b"11\n11\n");
    assert_eq!(scratch.read("errors.01"), b"101\n101\n");
    assert_eq!(scratch.read("obs.01"), b"1\n1\n");
}

#[test]
fn sample_dem_writes_ptb64_detector_observable_and_error_streams() {
    let scratch = Scratch::new();
    let obs = scratch.path("obs.ptb64");
    let err = scratch.path("errors.ptb64");
    sample_dem(
        &[
            "--out_format",
            "ptb64",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "ptb64",
            "--err_out",
            &err,
            "--err_out_format",
            "ptb64",
            "--shots",
            "64",
        ],
        b"error(1) D0 L0\nerror(0) D1\nerror(1) D1\n",
    )
    .assert_ok(ptb64_words(&[u64::MAX, u64::MAX]));
    assert_eq!(scratch.read("obs.ptb64"), ptb64_words(&[u64::MAX]));
    assert_eq!(
        scratch.read("errors.ptb64"),
        ptb64_words(&[u64::MAX, 0, u64::MAX])
    );
}

#[test]
fn sample_dem_replays_error_records_into_detector_and_observable_streams() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.dets", "shot M0\nshot M1\nshot M0 M1\nshot M0\n");
    let err_copy = scratch.path("errors.hits");
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "dets",
            "--err_out",
            &err_copy,
            "--err_out_format",
            "hits",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "01",
            "--shots",
            "3",
            "--seed",
            "0",
        ],
        TWO_ERRORS,
    )
    .assert_ok(b"10\n01\n11\n");
    assert_eq!(scratch.read("obs.01"), b"1\n0\n1\n");
    assert_eq!(scratch.read("errors.hits"), b"0\n1\n0,1\n");
}

#[test]
fn sample_dem_hits_replay_sets_duplicate_error_indices() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.hits", "0,0\n1,1\n");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format=hits",
            "--shots=2",
            "--seed=0",
        ],
        b"error(0.25) D0\nerror(0.25) D1\n",
    )
    .assert_ok(b"10\n01\n");
}

#[test]
fn sample_dem_replays_ptb64_error_records() {
    let scratch = Scratch::new();
    let mut replay_input =
        write_ptb64_records_checked(&vec![vec![true, false]; 64]).expect("ptb64 replay fixture");
    replay_input.push(0xA5);
    let replay = scratch.file("errors.ptb64", replay_input);
    let obs = scratch.path("obs.01");
    sample_dem(
        &[
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "ptb64",
            "--obs_out",
            &obs,
            "--shots",
            "64",
            "--seed",
            "0",
        ],
        TWO_ERRORS,
    )
    .assert_ok("10\n".repeat(64));
    assert_eq!(scratch.read("obs.01"), "1\n".repeat(64).as_bytes());
}

#[test]
fn sample_dem_round_trips_r8_detector_observable_error_and_replay_streams() {
    let scratch = Scratch::new();
    let replay = scratch.file("errors.r8", [0, 1, 1, 0]);
    let obs = scratch.path("obs.r8");
    let err = scratch.path("errors-copy.r8");
    sample_dem(
        &[
            "--out_format",
            "r8",
            "--replay_err_in",
            &replay,
            "--replay_err_in_format",
            "r8",
            "--obs_out",
            &obs,
            "--obs_out_format",
            "r8",
            "--err_out",
            &err,
            "--err_out_format",
            "r8",
            "--shots",
            "2",
        ],
        b"error(0.25) D0 L0\nerror(0.25) D1 L1\n",
    )
    .assert_ok([0, 1, 1, 0]);
    assert_eq!(scratch.read("obs.r8"), [0, 1, 1, 0]);
    assert_eq!(scratch.read("errors-copy.r8"), [0, 1, 1, 0]);
}

#[test]
fn sample_dem_replays_stim_compatible_crlf_text_records() {
    for (format, replay_input, dem_input, shots, expected_stdout) in [
        ("01", &b"1\r\n"[..], &b"error(0.25) D0\n"[..], "1", "1\n"),
        ("hits", b"0\r\n", TWO_ERRORS, "1", "10\n"),
        (
            "dets",
            b"shot M1\r\n\r\n\n   shot M0\r\n\n",
            TWO_ERRORS,
            "2",
            "01\n10\n",
        ),
    ] {
        let scratch = Scratch::new();
        let replay = scratch.file(&format!("errors.{format}"), replay_input);
        sample_dem(
            &[
                "--replay_err_in",
                &replay,
                "--replay_err_in_format",
                format,
                "--shots",
                shots,
            ],
            dem_input,
        )
        .assert_ok(expected_stdout);
    }
}

#[test]
fn sample_dem_rejects_malformed_or_short_replay_input() {
    let scratch = Scratch::new();
    let mut blank_prefix = vec![b'\n'; 1_048_577];
    blank_prefix.extend_from_slice(b"shot M0\n");
    for (format, replay_bytes, shots, dem, expected) in [
        (
            "dets",
            blank_prefix,
            "1",
            &b"error(1) D0\n"[..],
            "sample_dem replay text record has more than 1048576 bytes, exceeding the memory budget",
        ),
        (
            "ptb64",
            ptb64_words(&[u64::MAX]),
            "64",
            TWO_ERRORS,
            "ptb64 input expected at least 16 bytes",
        ),
        (
            "01",
            b"1\n".to_vec(),
            "2",
            b"error(1) D0\n",
            "replay error input has 1 records but --shots requested 2",
        ),
    ] {
        let replay = scratch.file(&format!("errors.{format}"), replay_bytes);
        sample_dem(
            &[
                "--replay_err_in",
                &replay,
                "--replay_err_in_format",
                format,
                "--shots",
                shots,
                "--memory-budget=2MiB",
            ],
            dem,
        )
        .assert_err(&[expected]);
    }
}

#[test]
fn sample_dem_replay_ignores_extra_records_after_requested_shots() {
    let scratch = Scratch::new();
    let text = scratch.file("errors.01", "1\nnot-a-record\n");
    sample_dem(
        &["--replay_err_in", &text, "--shots", "1"],
        b"error(1) D0\n",
    )
    .assert_ok(b"1\n");

    let b8 = scratch.file("errors.b8", [0b0000_0001, 0, 0b0000_0001]);
    let nine_errors = (0..9)
        .map(|index| format!("error(1) D{index}\n"))
        .collect::<String>();
    sample_dem(
        &[
            "--replay_err_in",
            &b8,
            "--replay_err_in_format",
            "b8",
            "--shots",
            "1",
        ],
        nine_errors.as_bytes(),
    )
    .assert_ok(b"100000000\n");
}

#[test]
fn sample_dem_record_format_options_reject_the_stim_circuit_format() {
    for option in [
        "--obs_out_format",
        "--err_out_format",
        "--replay_err_in_format",
    ] {
        let result = sample_dem(&[option, "stim"], b"");
        assert_ne!(result.status, 0, "{option}");
        assert_eq!(result.stdout, b"", "{option}");
        result.assert_stderr_contains(&["invalid value 'stim'"]);
    }
}
