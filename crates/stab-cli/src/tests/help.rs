use crate::run_from;

fn run_cli(args: &[&str]) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        args.iter().copied(),
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );
    (
        status,
        String::from_utf8(stdout).expect("stdout is utf-8"),
        String::from_utf8(stderr).expect("stderr is utf-8"),
    )
}

#[test]
fn help_lists_implemented_commands_for_help_spellings() {
    for args in [
        &["stab", "help"][..],
        &["stab", "--help"][..],
        &["stab", "help", "commands"][..],
    ] {
        let (status, stdout, stderr) = run_cli(args);
        assert_eq!(status, 0, "{args:?}");
        assert_eq!(stderr, "", "{args:?}");
        assert!(stdout.contains("Available stab commands"), "{stdout}");
        assert!(stdout.contains("stab convert"), "{stdout}");
        assert!(stdout.contains("stab sample_dem"), "{stdout}");
        assert!(!stdout.contains("stab diagram"), "{stdout}");
    }
}

#[test]
fn help_topics_cover_commands_formats_and_gates() {
    for (args, expected) in [
        (&["stab", "help", "convert"][..], "--obs_out_format"),
        (
            &["stab", "help", "analyze_errors"][..],
            "--block_decompose_from_introducing_remnant_edges",
        ),
        (
            &["stab", "--help", "sample"][..],
            "Samples measurements from a circuit",
        ),
        (&["stab", "help", "01"][..], "0 and 1"),
        (&["stab", "help", "ptb64"][..], "groups of exactly 64"),
        (&["stab", "help", "H"][..], "Hadamard-like"),
    ] {
        let (status, stdout, stderr) = run_cli(args);
        assert_eq!(status, 0, "{args:?}");
        assert_eq!(stderr, "", "{args:?}");
        assert!(stdout.contains(expected), "{args:?}: {stdout}");
    }
}

#[test]
fn subcommand_clap_help_still_works() {
    let (status, stdout, stderr) = run_cli(&["stab", "sample", "--help"]);

    assert_eq!(status, 0);
    assert_eq!(stderr, "");
    assert!(stdout.contains("Samples measurements from a circuit"));
    assert!(stdout.contains("--shots"));
}

#[test]
fn unknown_help_topic_exits_nonzero_with_stderr() {
    let (status, stdout, stderr) = run_cli(&["stab", "help", "not-a-topic"]);

    assert_eq!(status, 1);
    assert_eq!(stdout, "");
    assert!(stderr.contains("unrecognized help topic"));
}
