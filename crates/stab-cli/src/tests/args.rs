use super::support::{Scratch, run_cli as run_owned};
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

/// How pinned Stim v1.16.0 and Stab treat one argument spelling.
enum Acceptance {
    /// Exit status 1, empty stdout, and a diagnostic.
    Rejected,
    /// Exit status 0 with stdout identical to the run using these canonical arguments.
    SameAs(&'static [&'static str]),
}

use Acceptance::{Rejected, SameAs};

/// The computational commands, in Stim's implementation order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Computational {
    Gen,
    Convert,
    Sample,
    Detect,
    M2d,
    AnalyzeErrors,
    SampleDem,
}

use Computational::{AnalyzeErrors, Convert, Detect, Gen, M2d, Sample, SampleDem};

impl Computational {
    const ALL: [Self; 7] = [Gen, Convert, Sample, Detect, M2d, AnalyzeErrors, SampleDem];

    const fn name(self) -> &'static str {
        match self {
            Gen => "gen",
            Convert => "convert",
            Sample => "sample",
            Detect => "detect",
            M2d => "m2d",
            AnalyzeErrors => "analyze_errors",
            SampleDem => "sample_dem",
        }
    }
}

/// One spelling appended to a command's baseline. A baseline argument for the same flag is
/// dropped first, so a row can respell, or omit the value of, a flag the baseline supplies.
struct Spelling {
    command: Computational,
    args: &'static [&'static str],
    acceptance: Acceptance,
}

const fn row(
    command: Computational,
    args: &'static [&'static str],
    acceptance: Acceptance,
) -> Spelling {
    Spelling {
        command,
        args,
        acceptance,
    }
}

/// Every row was run through the pinned Stim v1.16.0 binary and the Stab release binary with
/// these exact baselines: both agree on the exit status and on whether stdout is empty, and
/// accepted spellings print the same bytes as their canonical spelling. Spellings where the two
/// binaries currently disagree, such as an empty inline boolean value (`--fold_loops=`) or a
/// value-less `--shots`/`--seed`, are deliberately absent until that parity decision is made.
const SPELLINGS: &[Spelling] = &[
    row(Gen, &["--not_a_flag"], Rejected),
    row(Gen, &["--not_a_flag=1"], Rejected),
    row(Gen, &["--code=Repetition_code"], Rejected),
    row(Gen, &["--code"], Rejected),
    row(Gen, &["--rounds"], Rejected),
    row(Gen, &["--rounds="], Rejected),
    row(Gen, &["--out"], Rejected),
    row(Gen, &["--distance=+5"], SameAs(&["--distance=5"])),
    row(Gen, &["--distance=05"], SameAs(&["--distance=5"])),
    row(Gen, &["--distance=010"], SameAs(&["--distance=10"])),
    row(Gen, &["--distance=-5"], Rejected),
    row(Gen, &["--distance", "-5"], Rejected),
    row(Gen, &["--distance=0x10"], Rejected),
    row(Gen, &["--distance= 5"], Rejected),
    row(Gen, &["--distance=5 "], Rejected),
    row(Gen, &["--distance=5.0"], Rejected),
    row(Gen, &["--distance=1e1"], Rejected),
    row(Gen, &["--distance="], Rejected),
    row(Gen, &["--rounds=+5"], SameAs(&["--rounds=5"])),
    row(Gen, &["--rounds=010"], SameAs(&["--rounds=10"])),
    row(Gen, &["--rounds=-5"], Rejected),
    row(Convert, &["--not_a_flag"], Rejected),
    row(Convert, &["--not_a_flag=1"], Rejected),
    row(Convert, &["--in_format=B8"], Rejected),
    row(Convert, &["--out_format=B8"], Rejected),
    row(Convert, &["--out_format=HITS"], Rejected),
    row(Convert, &["--out_format"], Rejected),
    row(Convert, &["--bits_per_shot"], Rejected),
    row(Convert, &["--in"], Rejected),
    row(
        Convert,
        &["--bits_per_shot=+5"],
        SameAs(&["--bits_per_shot=5"]),
    ),
    row(
        Convert,
        &["--bits_per_shot=05"],
        SameAs(&["--bits_per_shot=5"]),
    ),
    row(Convert, &["--bits_per_shot=0x5"], Rejected),
    row(Convert, &["--bits_per_shot=-5"], Rejected),
    row(Convert, &["--bits_per_shot= 5"], Rejected),
    row(Convert, &["--bits_per_shot=5.0"], Rejected),
    row(Sample, &["--not_a_flag"], Rejected),
    row(Sample, &["--not_a_flag=1"], Rejected),
    row(Sample, &["--skip_reference_sample=true"], Rejected),
    row(Sample, &["--skip_reference_sample=false"], Rejected),
    row(Sample, &["--skip_reference_sample=1"], Rejected),
    row(Sample, &["--skip_reference_sample=0"], Rejected),
    row(Sample, &["--skip_reference_sample", "1"], Rejected),
    row(Sample, &["--skip_loop_folding=true"], Rejected),
    row(Sample, &["--skip_loop_folding", "1"], Rejected),
    row(Sample, &["--out_format=B8"], Rejected),
    row(Sample, &["--out_format"], Rejected),
    row(Sample, &["--out_format="], Rejected),
    row(Sample, &["--shots=+5"], SameAs(&["--shots=5"])),
    row(Sample, &["--shots=05"], SameAs(&["--shots=5"])),
    row(Sample, &["--shots=010"], SameAs(&["--shots=10"])),
    row(Sample, &["--shots=-5"], Rejected),
    row(Sample, &["--shots", "-5"], Rejected),
    row(Sample, &["--shots=0x10"], Rejected),
    row(Sample, &["--shots= 5"], Rejected),
    row(Sample, &["--shots=5 "], Rejected),
    row(Sample, &["--shots=5.0"], Rejected),
    row(Sample, &["--shots=1e1"], Rejected),
    row(Detect, &["--not_a_flag"], Rejected),
    row(Detect, &["--not_a_flag=1"], Rejected),
    row(Detect, &["--append_observables=true"], Rejected),
    row(Detect, &["--append_observables=0"], Rejected),
    row(Detect, &["--append_observables", "1"], Rejected),
    row(Detect, &["--out_format=B8"], Rejected),
    row(Detect, &["--out_format"], Rejected),
    row(Detect, &["--shots=+5"], SameAs(&["--shots=5"])),
    row(Detect, &["--shots=010"], SameAs(&["--shots=10"])),
    row(Detect, &["--shots=-5"], Rejected),
    row(Detect, &["--shots=0x10"], Rejected),
    row(Detect, &["--shots=5.0"], Rejected),
    row(M2d, &["--not_a_flag"], Rejected),
    row(M2d, &["--not_a_flag=1"], Rejected),
    row(M2d, &["--append_observables=true"], Rejected),
    row(M2d, &["--skip_reference_sample=1"], Rejected),
    row(M2d, &["--ran_without_feedback=false"], Rejected),
    row(M2d, &["--ran_without_feedback", "1"], Rejected),
    row(M2d, &["--in_format=B8"], Rejected),
    row(M2d, &["--out_format=B8"], Rejected),
    row(M2d, &["--out_format"], Rejected),
    row(M2d, &["--obs_out_format"], Rejected),
    row(AnalyzeErrors, &["--not_a_flag"], Rejected),
    row(AnalyzeErrors, &["--not_a_flag=1"], Rejected),
    row(AnalyzeErrors, &["--fold_loops=true"], Rejected),
    row(AnalyzeErrors, &["--decompose_errors=1"], Rejected),
    row(AnalyzeErrors, &["--allow_gauge_detectors=0"], Rejected),
    row(AnalyzeErrors, &["--fold_loops", "1"], Rejected),
    row(AnalyzeErrors, &["--out"], Rejected),
    row(
        AnalyzeErrors,
        &["--approximate_disjoint_errors"],
        SameAs(&["--approximate_disjoint_errors=1"]),
    ),
    row(SampleDem, &["--not_a_flag"], Rejected),
    row(SampleDem, &["--not_a_flag=1"], Rejected),
    row(SampleDem, &["--out_format=B8"], Rejected),
    row(SampleDem, &["--out_format"], Rejected),
    row(SampleDem, &["--shots=+5"], SameAs(&["--shots=5"])),
    row(SampleDem, &["--shots=010"], SameAs(&["--shots=10"])),
    row(SampleDem, &["--shots=-5"], Rejected),
    row(SampleDem, &["--shots=0x10"], Rejected),
    row(SampleDem, &["--shots= 5"], Rejected),
];

/// The flag name of `--name` or `--name=value`, or `None` for a bare value.
fn flag_name(arg: &str) -> Option<&str> {
    arg.starts_with("--")
        .then(|| arg.split_once('=').map_or(arg, |(name, _)| name))
}

#[test]
fn flag_spellings_match_pinned_stim_acceptance() {
    let scratch = Scratch::new();
    let circuit = format!(
        "--circuit={}",
        scratch.file(
            "m2d.stim",
            "M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n"
        )
    );
    let baseline = |command: Computational| -> (Vec<String>, &'static [u8]) {
        let (args, stdin): (&[&str], &'static [u8]) = match command {
            Gen => (
                &[
                    "--code=repetition_code",
                    "--task=memory",
                    "--distance=3",
                    "--rounds=2",
                ],
                b"",
            ),
            Convert => (
                &["--in_format=01", "--out_format=01", "--bits_per_shot=5"],
                b"10000\n01001\n",
            ),
            Sample => (&["--shots=2"], b"X 0\nM 0\n"),
            Detect => (
                &["--shots=2"],
                b"X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            ),
            M2d => (&[circuit.as_str(), "--in_format=01"], b"0\n1\n"),
            // HERALDED_ERASE needs the approximation, so its threshold value is observable.
            AnalyzeErrors => (
                &["--approximate_disjoint_errors=1"],
                b"HERALDED_ERASE(0.25) 0\nM 0\nDETECTOR rec[-1]\n",
            ),
            SampleDem => (&["--shots=2"], b"error(1) D0\nerror(0) D1\n"),
        };
        let mut argv = vec!["stab".to_string(), command.name().to_string()];
        argv.extend(args.iter().map(ToString::to_string));
        (argv, stdin)
    };
    let invoke = |command: Computational, extra: &[&str]| {
        let (mut argv, stdin) = baseline(command);
        let overridden: Vec<&str> = extra.iter().filter_map(|arg| flag_name(arg)).collect();
        argv.retain(|arg| flag_name(arg).is_none_or(|name| !overridden.contains(&name)));
        argv.extend(extra.iter().map(ToString::to_string));
        (argv.clone(), run_owned(argv, stdin))
    };

    for command in Computational::ALL {
        assert!(
            SPELLINGS.iter().any(|spelling| spelling.command == command),
            "the table must cover {}",
            command.name()
        );
    }
    for spelling in SPELLINGS {
        let (argv, result) = invoke(spelling.command, spelling.args);
        match spelling.acceptance {
            Rejected => {
                assert_eq!(result.status, 1, "{argv:?} must be rejected");
                assert!(result.stdout.is_empty(), "{argv:?} wrote stdout");
                assert!(!result.stderr.is_empty(), "{argv:?} gave no diagnostic");
            }
            SameAs(canonical) => {
                let (canonical_argv, expected) = invoke(spelling.command, canonical);
                assert_eq!(
                    result.status,
                    0,
                    "{argv:?} must be accepted: {}",
                    result.stderr_text()
                );
                assert_eq!(expected.status, 0, "{canonical_argv:?} must be accepted");
                assert!(
                    !expected.stdout.is_empty(),
                    "{canonical_argv:?} printed nothing"
                );
                assert_eq!(
                    result.stdout, expected.stdout,
                    "{argv:?} must behave like {canonical_argv:?}"
                );
            }
        }
    }
}

/// Pinned Stim reads an empty boolean value as true, a missing or empty
/// `--shots` as one shot, a missing or empty `--seed` as seed 0, and an empty
/// `--approximate_disjoint_errors` as 1. Each silently replaces what the user
/// typed with a default, so Stab rejects them as usage errors.
#[test]
fn empty_and_missing_flag_values_are_rejected() {
    for (args, stdin) in [
        (
            &["sample", "--shots=2", "--skip_reference_sample="][..],
            CIRCUIT,
        ),
        (
            &["sample", "--shots=2", "--skip_loop_folding="][..],
            CIRCUIT,
        ),
        (
            &["detect", "--shots=2", "--append_observables="][..],
            CIRCUIT,
        ),
        (&["analyze_errors", "--fold_loops="][..], CIRCUIT),
        (&["analyze_errors", "--decompose_errors="][..], CIRCUIT),
        (&["analyze_errors", "--allow_gauge_detectors="][..], CIRCUIT),
        (
            &["analyze_errors", "--approximate_disjoint_errors="][..],
            CIRCUIT,
        ),
        (&["sample", "--shots"][..], CIRCUIT),
        (&["sample", "--shots="][..], CIRCUIT),
        (&["detect", "--shots", "--seed=1"][..], CIRCUIT),
        (&["sample_dem", "--shots="][..], "error(0.1) D0\n"),
        (&["sample", "--shots=2", "--seed"][..], CIRCUIT),
        // Pinned Stim's overflow check misses exactly 2^64 and reads it as 0.
        (&["sample", "--shots=18446744073709551616"][..], CIRCUIT),
        (
            &["sample", "--shots=2", "--seed=18446744073709551616"][..],
            CIRCUIT,
        ),
        (
            &["sample_dem", "--shots=18446744073709551616"][..],
            "error(0.1) D0\n",
        ),
        (
            &["sample_dem", "--shots=2", "--seed="][..],
            "error(0.1) D0\n",
        ),
        (
            &[
                "m2d",
                "--circuit=/dev/null",
                "--in_format=01",
                "--skip_reference_sample=",
            ][..],
            "",
        ),
    ] {
        let (status, stdout, stderr) = run_cli(args, stdin);
        assert_eq!(status, 1, "{args:?}: {stderr}");
        assert!(stdout.is_empty(), "{args:?}");
        assert!(!stderr.is_empty(), "{args:?}");
    }
}
