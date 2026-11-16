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
        ("capabilities", "schema-version-6"),
        ("inspect", "schema-version-2"),
        ("plan", "schema-version-5"),
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

/// Collects `(help topic, --long)` for every visible long flag of `command` and its nested
/// subcommands, so `stab help plan` must also document `plan sample`'s flags.
fn visible_long_flags(topic: &str, command: &clap::Command, flags: &mut Vec<(String, String)>) {
    for argument in command.get_arguments() {
        let Some(long) = argument.get_long() else {
            continue;
        };
        if argument.is_hide_set() || argument.is_global_set() || matches!(long, "help" | "version")
        {
            continue;
        }
        flags.push((topic.to_string(), format!("--{long}")));
    }
    for subcommand in command.get_subcommands() {
        visible_long_flags(topic, subcommand, flags);
    }
}

/// `stab help <command>` is hand-written, so a flag added to a command's parser can silently go
/// undocumented. Every visible long flag must be named as a whole flag, not as a prefix of a
/// longer one such as `--out` inside `--out_format`.
#[test]
fn command_help_names_every_visible_long_flag() {
    let descriptor = crate::command_descriptor();
    let mut flags = Vec::new();
    for command in descriptor.get_subcommands() {
        visible_long_flags(command.get_name(), command, &mut flags);
    }
    assert!(
        flags
            .iter()
            .any(|(topic, flag)| topic == "plan" && flag == "--shots"),
        "nested subcommand flags must be collected: {flags:?}"
    );

    let mut missing = Vec::new();
    for (topic, flag) in &flags {
        let result = run(&["stab", "help", topic], b"");
        assert_eq!(result.status, 0, "help {topic}: {}", result.stderr_text());
        let help = result.stdout_text();
        let named = help.match_indices(flag.as_str()).any(|(start, _)| {
            help.get(start + flag.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|next| !(next.is_ascii_alphanumeric() || next == '_' || next == '-'))
        });
        if !named {
            missing.push(format!("{topic} {flag}"));
        }
    }
    assert!(missing.is_empty(), "help omits flags: {missing:?}");
}
