use crate::run_from;

fn run_cli_bytes(args: &[&str], input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);
    (status, stdout, stderr)
}

fn run_cli(args: &[&str], input: &[u8]) -> (i32, String, String) {
    let (status, stdout, stderr) = run_cli_bytes(args, input);
    (
        status,
        String::from_utf8(stdout).expect("stdout is UTF-8"),
        String::from_utf8(stderr).expect("stderr is UTF-8"),
    )
}

#[test]
fn legacy_dispatch_accepts_selected_aliases() {
    for (legacy_args, subcommand_args, input) in [
        (
            &[
                "stab",
                "--gen=repetition_code",
                "--task",
                "memory",
                "--distance",
                "3",
                "--rounds",
                "2",
            ][..],
            &[
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
            ][..],
            b"".as_slice(),
        ),
        (
            &["stab", "--convert", "--bits_per_shot", "2"][..],
            &["stab", "convert", "--bits_per_shot", "2"][..],
            b"10\n01\n".as_slice(),
        ),
        (
            &["stab", "--sample=2"][..],
            &["stab", "sample", "--shots", "2"][..],
            b"M 0\n".as_slice(),
        ),
        (
            &["stab", "--detect=3"][..],
            &["stab", "detect", "--shots", "3"][..],
            b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        ),
        (
            &["stab", "--detect", "3"][..],
            &["stab", "detect", "--shots", "3"][..],
            b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        ),
        (
            &[
                "stab",
                "--m2d",
                "--in_format=01",
                "--out_format=dets",
                concat!(
                    "--circuit=",
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../oracle/fixtures/inputs/m2d_basic.stim"
                ),
            ][..],
            &[
                "stab",
                "m2d",
                "--in_format=01",
                "--out_format=dets",
                concat!(
                    "--circuit=",
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../oracle/fixtures/inputs/m2d_basic.stim"
                ),
            ][..],
            include_bytes!("../../../../oracle/fixtures/inputs/m2d_basic_measurements.01")
                .as_slice(),
        ),
        (
            &["stab", "--analyze_errors"][..],
            &["stab", "analyze_errors"][..],
            b"M 0\nDETECTOR rec[-1]\n".as_slice(),
        ),
    ] {
        let legacy = run_cli_bytes(legacy_args, input);
        let subcommand = run_cli_bytes(subcommand_args, input);

        assert_eq!(legacy.0, 0, "{legacy_args:?}");
        assert_eq!(subcommand.0, 0, "{subcommand_args:?}");
        assert_eq!(legacy, subcommand, "{legacy_args:?}");
    }
}

#[test]
fn legacy_dispatch_rejects_multiple_modes() {
    for (args, input, conflicting_mode) in [
        (
            &["stab", "--convert", "--sample"][..],
            b"10\n".as_slice(),
            "--sample",
        ),
        (
            &["stab", "--sample", "--detect"][..],
            b"M 0\n".as_slice(),
            "--detect",
        ),
        (
            &["stab", "--detect", "--m2d"][..],
            b"M 0\n".as_slice(),
            "--m2d",
        ),
        (
            &["stab", "--m2d", "--analyze_errors"][..],
            b"0\n".as_slice(),
            "--analyze_errors",
        ),
        (
            &["stab", "--analyze_errors", "--sample"][..],
            b"M 0\n".as_slice(),
            "--sample",
        ),
        (
            &[
                "stab",
                "--gen=repetition_code",
                "--sample",
                "--task",
                "memory",
            ][..],
            b"".as_slice(),
            "--sample",
        ),
    ] {
        let (status, stdout, stderr) = run_cli(args, input);

        assert_eq!(status, 1, "{args:?}");
        assert_eq!(stdout, "", "{args:?}");
        assert!(
            stderr.contains(conflicting_mode),
            "{args:?} stderr should mention {conflicting_mode}: {stderr}"
        );
    }
}

#[test]
fn legacy_dispatch_rejects_detector_hypergraph() {
    let (status, stdout, stderr) = run_cli(&["stab", "--detector_hypergraph"], b"");

    assert_eq!(status, 1);
    assert_eq!(stdout, "");
    assert!(stderr.contains("--detector_hypergraph"), "{stderr}");

    let (help_status, help_stdout, help_stderr) =
        run_cli(&["stab", "help", "detector_hypergraph"], b"");
    assert_eq!(help_status, 1);
    assert_eq!(help_stdout, "");
    assert!(
        help_stderr.contains("unrecognized help topic"),
        "{help_stderr}"
    );
}

#[test]
fn legacy_dispatch_rejects_unselected_legacy_modes() {
    for flag in ["--diagram", "--explain_errors", "--repl", "--sample_dem"] {
        let (status, stdout, stderr) = run_cli(&["stab", flag], b"");

        assert_eq!(status, 1, "{flag}");
        assert_eq!(stdout, "", "{flag}");
        assert!(
            stderr.contains(flag) || stderr.contains("unexpected argument"),
            "{flag} stderr should mention the rejected flag or unexpected argument: {stderr}"
        );
    }
}
