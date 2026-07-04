use crate::run_from;

fn run_cli(args: &[&str], input: &[u8]) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);
    (
        status,
        String::from_utf8(stdout).expect("stdout is UTF-8"),
        String::from_utf8(stderr).expect("stderr is UTF-8"),
    )
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
