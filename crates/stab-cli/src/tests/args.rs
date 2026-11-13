use crate::run_from;

fn run_cli(args: &[&str], stdin: &str) -> (i32, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        std::iter::once("stab").chain(args.iter().copied()),
        stdin.as_bytes(),
        &mut stdout,
        &mut stderr,
    );
    (
        status,
        String::from_utf8(stdout).expect("stdout is utf-8"),
        String::from_utf8(stderr).expect("stderr is utf-8"),
    )
}

const CIRCUIT: &str = "M 0\nDETECTOR rec[-1]\n";

/// Minimal valid invocations with stdin input, one per computational command,
/// plus one value flag and, where the command has one, one boolean flag.
const COMMANDS: &[(&[&str], &str, &str, &str)] = &[
    (
        &[
            "gen",
            "--code",
            "repetition_code",
            "--task",
            "memory",
            "--distance",
            "3",
        ],
        "",
        "--rounds=2",
        "",
    ),
    (&["sample"], CIRCUIT, "--shots=2", "--skip_reference_sample"),
    (&["detect"], CIRCUIT, "--shots=2", "--append_observables"),
    (
        &["m2d", "--circuit=/dev/null", "--in_format=01"],
        "",
        "--out_format=01",
        "--skip_reference_sample",
    ),
    (
        &["analyze_errors"],
        CIRCUIT,
        "--approximate_disjoint_errors=0.1",
        "--fold_loops",
    ),
    (&["sample_dem"], "error(0.1) D0\n", "--shots=2", ""),
    (
        &["convert", "--in_format=01", "--bits_per_shot=1"],
        "0\n",
        "--out_format=01",
        "",
    ),
];

/// Stim's argument parser takes the first of a repeated flag and ignores every
/// argument after `--`. Stab rejects both so a later or trailing flag is never
/// silently dropped, while a bare trailing `--` stays accepted like Stim.
#[test]
fn repeated_flags_and_arguments_after_terminator_are_rejected() {
    for (base, stdin, value_flag, bool_flag) in COMMANDS {
        let mut once = base.to_vec();
        once.push(value_flag);
        let (status, _, stderr) = run_cli(&once, stdin);
        assert_eq!(status, 0, "{once:?} should succeed: {stderr}");

        let mut repeated = once.clone();
        repeated.push(value_flag);
        let (status, stdout, stderr) = run_cli(&repeated, stdin);
        assert_eq!(status, 1, "{repeated:?}");
        assert!(stdout.is_empty(), "{repeated:?}");
        assert!(
            stderr.contains("cannot be used multiple times"),
            "{repeated:?}: {stderr}"
        );

        if !bool_flag.is_empty() {
            let mut repeated_bool = once.clone();
            repeated_bool.extend([*bool_flag, *bool_flag]);
            let (status, _, stderr) = run_cli(&repeated_bool, stdin);
            assert_eq!(status, 1, "{repeated_bool:?}");
            assert!(
                stderr.contains("cannot be used multiple times"),
                "{base:?}: {stderr}"
            );
        }

        let mut after_terminator = base.to_vec();
        after_terminator.extend(["--", value_flag]);
        let (status, stdout, stderr) = run_cli(&after_terminator, stdin);
        assert_eq!(status, 1, "{after_terminator:?}");
        assert!(stdout.is_empty(), "{after_terminator:?}");
        assert!(stderr.contains("unexpected argument"), "{base:?}: {stderr}");

        let mut trailing_terminator = once.clone();
        trailing_terminator.push("--");
        let (status, _, stderr) = run_cli(&trailing_terminator, stdin);
        assert_eq!(status, 0, "{trailing_terminator:?}: {stderr}");
    }
}
