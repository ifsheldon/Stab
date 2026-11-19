#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "real-process I/O contracts use fixed fixtures and direct assertions"
)]

//! Real-process contracts for the binary's standard streams: write-failure
//! exit status and standard-stream aliasing. These behaviors depend on the
//! process's actual file descriptors, so they cannot be exercised in-process.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::{TempDir, tempdir};

const CIRCUIT: &str = "R 0 1\nX_ERROR(0.25) 0 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
const DEM: &str = "error(0.25) D0 L0\nerror(0.25) D1\n";
const MEASUREMENTS: &str = "01\n10\n11\n";
const SWEEPS: &str = "\n\n\n";
const DETECTIONS: &str = "shot D0\nshot D1 L0\n";
const ERRORS: &str = "00\n10\n";

/// Builds a temporary directory holding one valid input for every file role.
fn fixture() -> TempDir {
    let dir = tempdir().expect("tempdir");
    for (name, contents) in [
        ("c.stim", CIRCUIT),
        ("model.dem", DEM),
        ("m.01", MEASUREMENTS),
        ("sweep.01", SWEEPS),
        ("d.dets", DETECTIONS),
        ("err.01", ERRORS),
    ] {
        fs::write(dir.path().join(name), contents).expect("write fixture");
    }
    dir
}

fn stab(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stab"));
    command.current_dir(dir).args(args);
    command
}

fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(dir)
        .expect("read fixture dir")
        .map(|entry| {
            let path = entry.expect("fixture entry").path();
            let bytes = fs::read(&path).expect("read fixture file");
            (path, bytes)
        })
        .collect()
}

/// Every command that writes a primary output, with arguments that succeed on
/// [`fixture`] and produce a nonempty output.
const PRODUCING_COMMANDS: &[&[&str]] = &[
    &[
        "gen",
        "--code",
        "repetition_code",
        "--task",
        "memory",
        "--distance",
        "3",
        "--rounds",
        "2",
    ],
    &["sample", "--shots", "64", "--in", "c.stim"],
    &["detect", "--shots", "64", "--in", "c.stim"],
    &[
        "m2d",
        "--circuit",
        "c.stim",
        "--in",
        "m.01",
        "--in_format",
        "01",
    ],
    &["analyze_errors", "--in", "c.stim"],
    &["sample_dem", "--shots", "64", "--in", "model.dem"],
    &[
        "convert",
        "--in",
        "m.01",
        "--in_format",
        "01",
        "--out_format",
        "b8",
        "--bits_per_shot",
        "2",
    ],
];

fn assert_reported_failure(output: &Output, context: &str) {
    assert_eq!(output.status.code(), Some(1), "{context}: {output:?}");
    assert!(!output.stderr.is_empty(), "{context}: missing diagnostic");
}

/// Stim ignores write errors on a full device and exits 0; Stab reports them,
/// including bytes still buffered when the command finishes.
#[test]
fn full_device_write_failures_exit_with_a_diagnostic() {
    let dir = fixture();
    for args in PRODUCING_COMMANDS {
        let stdout_full = stab(dir.path(), args)
            .stdout(File::create("/dev/full").expect("open /dev/full"))
            .output()
            .expect("run stab");
        assert_reported_failure(&stdout_full, &format!("{args:?} > /dev/full"));

        let mut with_out = args.to_vec();
        with_out.extend(["--out", "/dev/full"]);
        let out_full = stab(dir.path(), &with_out).output().expect("run stab");
        assert_reported_failure(&out_full, &format!("{with_out:?}"));
    }

    for args in [
        &[
            "detect",
            "--shots",
            "64",
            "--in",
            "c.stim",
            "--obs_out",
            "/dev/full",
        ][..],
        &[
            "m2d",
            "--circuit",
            "c.stim",
            "--in",
            "m.01",
            "--in_format",
            "01",
            "--obs_out",
            "/dev/full",
        ],
        &[
            "sample_dem",
            "--shots",
            "64",
            "--in",
            "model.dem",
            "--obs_out",
            "/dev/full",
        ],
        &[
            "sample_dem",
            "--shots",
            "64",
            "--in",
            "model.dem",
            "--err_out",
            "/dev/full",
        ],
    ] {
        let output = stab(dir.path(), args)
            .stdout(Stdio::null())
            .output()
            .expect("run stab");
        assert_reported_failure(&output, &format!("{args:?}"));
    }
}

/// A closed output pipe ends the command like Stim's SIGPIPE death: status 141
/// with no diagnostic.
#[test]
fn closed_stdout_pipe_exits_141_without_a_diagnostic() {
    let dir = fixture();
    for args in PRODUCING_COMMANDS {
        let (reader, writer) = std::io::pipe().expect("create pipe");
        drop(reader);
        let output = stab(dir.path(), args)
            .stdout(writer)
            .output()
            .expect("run stab");
        assert_eq!(output.status.code(), Some(141), "{args:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
    }
}

enum Stream {
    Inherit,
    File(&'static str),
}

struct AliasCase {
    args: &'static [&'static str],
    stdin: Stream,
    stdout: Stream,
    roles: [&'static str; 2],
}

fn run_with_streams(dir: &Path, case: &AliasCase) -> Output {
    let mut command = stab(dir, case.args);
    if let Stream::File(name) = case.stdin {
        command.stdin(File::open(dir.join(name)).expect("open stdin file"));
    } else {
        command.stdin(Stdio::null());
    }
    if let Stream::File(name) = case.stdout {
        // Append without truncation, so a rejected run leaves the file intact.
        let file = OpenOptions::new()
            .append(true)
            .open(dir.join(name))
            .expect("open stdout file");
        command.stdout(file);
    }
    command.output().expect("run stab")
}

/// A redirected standard stream that is the same regular file as a named input
/// or output would let the command truncate data it has not read yet.
#[test]
fn standard_stream_aliases_fail_without_touching_files() {
    use Stream::{File as F, Inherit};
    let cases = [
        AliasCase {
            args: &["sample", "--shots", "8", "--out", "c.stim"],
            stdin: F("c.stim"),
            stdout: Inherit,
            roles: ["stdin", "--out"],
        },
        AliasCase {
            args: &[
                "detect",
                "--shots",
                "8",
                "--out",
                "d.dets",
                "--obs_out",
                "c.stim",
            ],
            stdin: F("c.stim"),
            stdout: Inherit,
            roles: ["stdin", "--obs_out"],
        },
        AliasCase {
            args: &[
                "m2d",
                "--circuit",
                "c.stim",
                "--in_format",
                "01",
                "--out",
                "m.01",
            ],
            stdin: F("m.01"),
            stdout: Inherit,
            roles: ["stdin", "--out"],
        },
        AliasCase {
            args: &[
                "m2d",
                "--circuit",
                "c.stim",
                "--in",
                "m.01",
                "--in_format",
                "01",
            ],
            stdin: Inherit,
            stdout: F("c.stim"),
            roles: ["--circuit", "stdout"],
        },
        AliasCase {
            args: &[
                "m2d",
                "--circuit",
                "c.stim",
                "--in",
                "m.01",
                "--in_format",
                "01",
                "--sweep",
                "sweep.01",
                "--sweep_format",
                "01",
            ],
            stdin: Inherit,
            stdout: F("sweep.01"),
            roles: ["--sweep", "stdout"],
        },
        AliasCase {
            args: &["analyze_errors", "--in", "c.stim"],
            stdin: Inherit,
            stdout: F("c.stim"),
            roles: ["--in", "stdout"],
        },
        AliasCase {
            args: &["analyze_errors"],
            stdin: F("c.stim"),
            stdout: F("c.stim"),
            roles: ["stdin", "stdout"],
        },
        AliasCase {
            args: &["sample_dem", "--shots", "2", "--err_out", "model.dem"],
            stdin: F("model.dem"),
            stdout: Inherit,
            roles: ["stdin", "--err_out"],
        },
        AliasCase {
            args: &[
                "sample_dem",
                "--shots",
                "2",
                "--in",
                "model.dem",
                "--replay_err_in",
                "err.01",
                "--replay_err_in_format",
                "01",
            ],
            stdin: Inherit,
            stdout: F("err.01"),
            roles: ["--replay_err_in", "stdout"],
        },
        AliasCase {
            args: &[
                "sample_dem",
                "--shots",
                "2",
                "--in",
                "model.dem",
                "--obs_out",
                "d.dets",
            ],
            stdin: Inherit,
            stdout: F("d.dets"),
            roles: ["--obs_out", "stdout"],
        },
        AliasCase {
            args: &[
                "convert",
                "--in",
                "d.dets",
                "--in_format",
                "dets",
                "--out_format",
                "01",
                "--dem",
                "model.dem",
            ],
            stdin: Inherit,
            stdout: F("model.dem"),
            roles: ["--dem", "stdout"],
        },
        AliasCase {
            args: &[
                "convert",
                "--in_format",
                "dets",
                "--out_format",
                "01",
                "--circuit",
                "c.stim",
                "--out",
                "d.dets",
            ],
            stdin: F("d.dets"),
            stdout: Inherit,
            roles: ["stdin", "--out"],
        },
        AliasCase {
            args: &["inspect", "c.stim"],
            stdin: Inherit,
            stdout: F("c.stim"),
            roles: ["--in", "stdout"],
        },
    ];
    for case in &cases {
        let dir = fixture();
        let before = snapshot(dir.path());
        let output = run_with_streams(dir.path(), case);
        let context = format!("{:?}", case.args);
        assert_reported_failure(&output, &context);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&format!(
                "file roles {} and {} refer to the same file",
                case.roles[0], case.roles[1]
            )),
            "{context}: {stderr}"
        );
        assert_eq!(snapshot(dir.path()), before, "{context} changed a file");
    }
}

/// Non-file streams and distinct files never alias, and an unused stream is
/// never compared.
#[test]
fn unaliased_and_unused_standard_streams_are_accepted() {
    let dir = fixture();
    let path = dir.path();

    let devices = stab(path, &["m2d", "--circuit", "c.stim", "--in_format", "01"])
        .stdin(File::open("/dev/null").expect("open /dev/null"))
        .stdout(Stdio::null())
        .status()
        .expect("run stab");
    assert!(devices.success(), "/dev/null streams: {devices:?}");

    let distinct = stab(
        path,
        &[
            "m2d",
            "--circuit",
            "c.stim",
            "--in_format",
            "01",
            "--out_format",
            "01",
        ],
    )
    .stdin(File::open(path.join("m.01")).expect("open measurements"))
    .stdout(File::create(path.join("out.01")).expect("create output"))
    .status()
    .expect("run stab");
    assert!(distinct.success(), "distinct files: {distinct:?}");
    assert_eq!(
        fs::read_to_string(path.join("out.01")).expect("read output"),
        "10\n01\n11\n"
    );

    let mut piped = stab(path, &["analyze_errors"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn stab");
    std::io::Write::write_all(
        &mut piped.stdin.take().expect("stdin pipe"),
        CIRCUIT.as_bytes(),
    )
    .expect("write circuit");
    let piped = piped.wait_with_output().expect("wait stab");
    assert!(piped.status.success(), "piped stdin: {piped:?}");

    // Zero detection shots read no input, so the redirected stdin is unused.
    let zero_shots = stab(path, &["detect", "--shots", "0", "--out", "d.dets"])
        .stdin(File::open(path.join("d.dets")).expect("open detections"))
        .status()
        .expect("run stab");
    assert!(zero_shots.success(), "zero shots: {zero_shots:?}");
}

/// Runs `sample_dem` with its replay piped through standard input as `--replay_err_in
/// /dev/stdin`, or read from `replay_file` when given.
fn sample_dem_replay(
    dir: &Path,
    args: &[&str],
    replay: &[u8],
    replay_file: Option<&str>,
) -> Output {
    let mut args = args.to_vec();
    args.extend(["--replay_err_in", replay_file.unwrap_or("/dev/stdin")]);
    let mut child = stab(dir, &args)
        .stdin(if replay_file.is_some() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stab");
    if let Some(mut stdin) = child.stdin.take() {
        std::io::Write::write_all(&mut stdin, replay).expect("write replay");
    }
    child.wait_with_output().expect("wait stab")
}

/// A replay the memory budget retains whole is delivered from the store without touching the
/// input again, so a pipe replays exactly like a file in every replay format.
#[test]
fn piped_replay_input_matches_a_file_when_the_store_keeps_every_record() {
    let dir = fixture();
    let path = dir.path();
    let replays: [(&str, &[u8]); 5] = [
        ("01", b"00\n10\n11\n01\n"),
        ("b8", &[0x00, 0x01, 0x03, 0x02]),
        ("r8", &[2, 0, 1, 0, 0, 0, 1, 0]),
        ("hits", b"\n0\n0,1\n1\n"),
        ("dets", b"shot\nshot M0\nshot M0 M1\nshot M1\n"),
    ];
    let mut expected = None;
    for (format, replay) in replays {
        let file = format!("replay.{format}");
        fs::write(path.join(&file), replay).expect("write replay file");
        let format_arg = format!("--replay_err_in_format={format}");
        let base = [
            "sample_dem",
            "--in",
            "model.dem",
            "--shots",
            "4",
            "--out_format",
            "dets",
            &format_arg,
        ];
        let from_file = sample_dem_replay(path, &base, replay, Some(&file));
        let piped = sample_dem_replay(path, &base, replay, None);
        for (source, output) in [("file", &from_file), ("pipe", &piped)] {
            assert!(
                output.status.success(),
                "{format} {source}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert_eq!(piped.stdout, from_file.stdout, "{format}");
        // Every format encodes the same four records, so all outputs agree.
        let expected = expected.get_or_insert_with(|| from_file.stdout.clone());
        assert_eq!(&from_file.stdout, expected, "{format}");
    }
    assert_eq!(
        expected.expect("replayed outputs"),
        b"shot\nshot D0\nshot D0 D1\nshot D1\n"
    );
}

/// A replay whose records the budget does not retain must be decoded again after validation,
/// which a pipe cannot rewind: the run fails before any output file exists and names the cause,
/// while the same records from a file replay under the same budget.
#[test]
fn piped_replay_input_the_budget_cannot_retain_fails_before_creating_outputs() {
    use stab_engine::{DemSamplerLimits, DemSamplingCompiler, ShotCount};
    use stab_model::{DetectorErrorModel, MemoryBudget, ParseLimits};

    const DEM: &str = "repeat 64 {\n    error(0.5) D0 L0\n    shift_detectors 1\n}\n";
    const SHOTS: u64 = 130;
    let (model, _) =
        DetectorErrorModel::from_dem_bytes_admitted(DEM.as_bytes(), ParseLimits::default())
            .expect("parse");
    let plan = DemSamplingCompiler::new().compile(&model).expect("compile");
    // The smallest whole-command budget whose store retains anything; the CLI's own holds leave
    // the store less than this, so one byte below it the store keeps nothing.
    let retains = |bytes: u64| {
        plan.replay_record_store(
            &DemSamplerLimits::new(MemoryBudget::from_bytes(bytes)),
            ShotCount::new(SHOTS),
        )
        .is_retaining()
    };
    let (mut refused, mut retaining) = (0_u64, MemoryBudget::DEFAULT.bytes());
    while refused + 1 < retaining {
        let middle = refused + (retaining - refused) / 2;
        if retains(middle) {
            retaining = middle;
        } else {
            refused = middle;
        }
    }

    let dir = fixture();
    let path = dir.path();
    fs::write(path.join("wide.dem"), DEM).expect("write model");
    let replay = format!("{}\n", "01".repeat(32)).repeat(130);
    fs::write(path.join("replay.01"), &replay).expect("write replay file");
    let budget = format!("--memory-budget={refused}");
    let shots = SHOTS.to_string();
    let base = [
        "sample_dem",
        "--in",
        "wide.dem",
        "--shots",
        &shots,
        "--out",
        "out.dets",
        &budget,
    ];
    let from_file = sample_dem_replay(path, &base, replay.as_bytes(), Some("replay.01"));
    assert!(
        from_file.status.success(),
        "file replay under {budget}: {}",
        String::from_utf8_lossy(&from_file.stderr)
    );
    fs::remove_file(path.join("out.dets")).expect("remove file-replay output");

    let piped = sample_dem_replay(path, &base, replay.as_bytes(), None);
    assert_reported_failure(&piped, "piped replay under a small budget");
    let stderr = String::from_utf8_lossy(&piped.stderr);
    assert!(
        stderr.contains("cannot rewind --replay_err_in /dev/stdin")
            && stderr.contains("pass a regular file or raise --memory-budget"),
        "{stderr}"
    );
    assert!(
        !path.join("out.dets").exists(),
        "a refused piped replay created its output"
    );
}
