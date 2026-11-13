use super::support::run;

#[test]
fn help_lists_implemented_commands_for_help_spellings() {
    for args in [&["stab", "help"][..], &["stab", "help", "commands"][..]] {
        let result = run(args, b"");
        assert_eq!(result.status, 0, "{args:?}");
        assert_eq!(result.stderr_text(), "", "{args:?}");
        let stdout = result.stdout_text();
        for expected in [
            "Available stab commands",
            "stab capabilities",
            "stab inspect",
            "stab plan sample",
            "stab convert",
            "stab sample_dem",
        ] {
            assert!(stdout.contains(expected), "{stdout}");
        }
        assert!(!stdout.contains("stab diagram"), "{stdout}");
    }
}

#[test]
fn help_topics_cover_commands_formats_and_gates() {
    for (args, expected) in [
        (&["stab", "help", "convert"][..], "--obs_out_format"),
        (
            &["stab", "help", "capabilities"][..],
            "source-owned descriptors",
        ),
        (&["stab", "help", "inspect"][..], "--type=stim"),
        (&["stab", "help", "plan"][..], "without executing shots"),
        (
            &["stab", "help", "analyze_errors"][..],
            "--block_decompose_from_introducing_remnant_edges",
        ),
        (
            &["stab", "help", "sample"][..],
            "Samples measurements from a circuit",
        ),
        (&["stab", "help", "01"][..], "0 and 1"),
        (&["stab", "help", "ptb64"][..], "groups of exactly 64"),
        (&["stab", "help", "H"][..], "Hadamard-like"),
    ] {
        let result = run(args, b"");
        assert_eq!(result.status, 0, "{args:?}");
        assert_eq!(result.stderr_text(), "", "{args:?}");
        assert!(
            result.stdout_text().contains(expected),
            "{args:?}: {}",
            result.stdout_text()
        );
    }
}

#[test]
fn agent_command_help_names_the_current_success_schema() {
    for (topic, schema) in [
        ("capabilities", "schema-version-5"),
        ("inspect", "schema-version-2"),
        ("plan", "schema-version-4"),
    ] {
        let result = run(&["stab", "help", topic], b"");
        assert_eq!(result.status, 0, "{topic}");
        assert_eq!(result.stderr_text(), "", "{topic}");
        let stdout = result.stdout_text();
        assert!(stdout.contains(schema), "{topic}: {stdout}");
        assert!(!stdout.contains("schema-version-1"), "{topic}: {stdout}");
    }
}

#[test]
fn unknown_help_topic_exits_nonzero_with_stderr() {
    run(&["stab", "help", "not-a-topic"], b"").assert_err(&["unrecognized help topic"]);
}
