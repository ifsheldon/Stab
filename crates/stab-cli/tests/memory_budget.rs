#![allow(
    clippy::expect_used,
    reason = "real-process budget fixtures use direct setup and assertions for compact diagnostics"
)]

//! Real-process coverage of `--memory-budget` for each workload class that a retired fixed count,
//! work, or input cap used to refuse, scaled to just past the retired cap.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::tempdir;

fn stab(args: &[&str], input: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stab"));
    command.args(args);
    if let Some(input) = input {
        command.arg("--in").arg(input);
    }
    command.output().expect("run stab")
}

fn assert_success(output: &Output, label: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_budget_refusal(output: &Output, budget_bytes: u64, label: &str) {
    assert_eq!(output.status.code(), Some(1), "{label}");
    assert!(output.stdout.is_empty(), "{label}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("{budget_bytes}")) && stderr.contains("memory budget"),
        "{label}: {stderr}"
    );
    assert!(stderr.contains("--memory-budget"), "{label}: {stderr}");
}

#[test]
fn memory_budget_admits_formerly_refused_workload_classes_and_refuses_beyond_the_budget() {
    let temp = tempdir().expect("temp dir");
    let write = |name: &str, text: &str| {
        let path = temp.path().join(name);
        fs::write(&path, text).expect("write fixture");
        path
    };

    // More physical lines than the retired one-million-line parse cap.
    let mut many_lines = "TICK\n".repeat(1_000_001);
    many_lines.push_str("M 0\nDETECTOR rec[-1]\n");
    let many_lines = write("lines.stim", &many_lines);
    let output = stab(&["detect", "--shots", "1"], Some(&many_lines));
    assert_success(&output, "detect above the source-line cap");
    assert_eq!(output.stdout, b"0\n");

    let mut many_dem_lines = "error(0) D0\n".repeat(1_000_001);
    many_dem_lines.push_str("detector D0\n");
    let many_dem_lines = write("lines.dem", &many_dem_lines);
    let output = stab(
        &["sample_dem", "--shots", "1", "--seed", "1"],
        Some(&many_dem_lines),
    );
    assert_success(&output, "sample_dem above the source-line cap");
    assert_eq!(output.stdout, b"0\n");

    // A record wider than the retired one-million-bit detection record width.
    let wide = write(
        "wide.stim",
        "REPEAT 1000001 {\n    M 0\n    DETECTOR rec[-1]\n}\n",
    );
    let output = stab(
        &["detect", "--shots", "1", "--out_format", "dets"],
        Some(&wide),
    );
    assert_success(&output, "detect above the record-width cap");
    assert_eq!(output.stdout, b"shot\n");

    // More expanded instructions per shot than the retired one-million work caps.
    let long = write(
        "long.stim",
        "REPEAT 1000001 {\n    H 0\n    H 0\n}\nM 0\nDETECTOR rec[-1]\n",
    );
    let output = stab(
        &["detect", "--shots", "64", "--out_format", "dets"],
        Some(&long),
    );
    assert_success(&output, "detect above the expanded-instruction cap");
    assert_eq!(output.stdout, "shot\n".repeat(64).as_bytes());
    let output = stab(&["sample", "--shots", "1"], Some(&long));
    assert_success(&output, "sample above the per-shot operation cap");
    assert_eq!(output.stdout, b"0\n");

    // The fixed input-file cap is replaced by the budget: an input over the budget is refused
    // before it is read, and the same run succeeds once the budget is raised.
    let mut flat = String::new();
    for qubit in 0..40_000 {
        writeln!(flat, "I {qubit}").expect("write gate");
    }
    flat.push_str("M 0\nDETECTOR rec[-1]\n");
    let flat = write("flat.stim", &flat);
    let output = stab(
        &["detect", "--shots", "1", "--memory-budget", "256KiB"],
        Some(&flat),
    );
    assert_budget_refusal(&output, 262_144, "detect input above the budget");
    let output = stab(
        &["detect", "--shots", "1", "--memory-budget", "64MiB"],
        Some(&flat),
    );
    assert_success(&output, "detect after raising the budget");
    assert_eq!(output.stdout, b"0\n");

    // A compact hostile record far beyond the budget is refused quickly, before allocation.
    let hostile = write(
        "hostile.stim",
        "REPEAT 1000000000000 {\n    M 0\n}\nDETECTOR rec[-1]\n",
    );
    for command in ["detect", "sample"] {
        let output = stab(&[command, "--shots", "1"], Some(&hostile));
        assert_budget_refusal(&output, 1 << 30, command);
    }
}

#[test]
fn memory_budget_option_parses_bytes_and_binary_suffixes_only() {
    let temp = tempdir().expect("temp dir");
    let circuit = temp.path().join("c.stim");
    fs::write(&circuit, "M 0\n").expect("write circuit");
    for accepted in ["1048576", "1024KiB", "1MiB", "1GiB"] {
        let output = stab(&["sample", "--memory-budget", accepted], Some(&circuit));
        assert_success(&output, accepted);
        assert_eq!(output.stdout, b"0\n", "{accepted}");
    }
    for rejected in [
        "1.5GiB",
        "1GB",
        "1 MiB",
        "-1",
        "GiB",
        "1TiB",
        "18446744073709551616",
    ] {
        let flag = format!("--memory-budget={rejected}");
        let output = stab(&["sample", &flag], Some(&circuit));
        assert_eq!(output.status.code(), Some(1), "{rejected}");
        assert!(output.stdout.is_empty(), "{rejected}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("--memory-budget"),
            "{rejected}"
        );
    }
}

const MEMORY_BUDGET_HELP: &str =
    "Raise the budget with `--memory-budget <SIZE>`, for example `--memory-budget 4GiB`.";

/// Runs `stab` with `args` and `stdin`, in human or JSON diagnostic mode.
fn stab_with_stdin(args: &[&str], stdin: &[u8], json: bool) -> Output {
    use std::io::Write as _;
    use std::process::Stdio;

    let mut command = Command::new(env!("CARGO_BIN_EXE_stab"));
    if json {
        command.args(["--error-format", "json"]);
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stab");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin)
        .expect("write stdin");
    child.wait_with_output().expect("run stab")
}

/// One command driven to a refusal by a tiny `--memory-budget`: the arguments, the standard
/// input, the stderr text that names the refusing layer, and the expected JSON code.
struct BudgetRefusalCase {
    label: &'static str,
    args: Vec<String>,
    stdin: String,
    message: &'static str,
    code: &'static str,
}

fn case(
    label: &'static str,
    args: &[&str],
    stdin: impl Into<String>,
    message: &'static str,
) -> BudgetRefusalCase {
    BudgetRefusalCase {
        label,
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
        stdin: stdin.into(),
        message,
        code: "resource-limit-exceeded",
    }
}

/// Every command family reaches a memory-budget refusal from a different layer, and each one is
/// recognised from its typed refusal: the human diagnostic carries the `--memory-budget` help
/// line and the JSON diagnostic carries the help and the single resource code. A refusal that
/// reached the CLI without being classified as a budget refusal would lose both.
#[test]
fn every_command_family_reports_budget_refusals_with_the_memory_budget_help() {
    let temp = tempdir().expect("temp dir");
    let write = |name: &str, text: &str| {
        let path = temp.path().join(name);
        fs::write(&path, text).expect("write fixture");
        path.to_str().expect("UTF-8 temp path").to_owned()
    };
    let random_detector = "H 0\nM 0\nDETECTOR rec[-1]\n";
    let deep_repeat = format!(
        "{}X_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n{}",
        "REPEAT 3 {\n".repeat(40),
        "}\n".repeat(40)
    );
    let huge_repeat = "REPEAT 1000000000000000000 {\nREPEAT 1000000000000000000 {\nX_ERROR(0.1) 0\nM 0\nDETECTOR rec[-1]\n}\n}\n";
    let hyperedge =
        "X_ERROR(0.1) 0\nM 0 0 0\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nDETECTOR rec[-3]\n";
    let repeat_dem = "repeat 500 {\nerror(0.1) D0 D1\nerror(0.1) D1 L0\nshift_detectors 2\n}\n";
    let detector_circuit = write("detector.stim", random_detector);
    let huge_circuit = write("huge.stim", huge_repeat);
    let measurements = "0\n".repeat(3);

    let mut cases = vec![
        case(
            "circuit parse",
            &["sample", "--shots", "5", "--memory-budget", "100"],
            random_detector,
            "circuit input and its parsed model need at least",
        ),
        case(
            "DEM parse",
            &["sample_dem", "--shots", "5", "--memory-budget", "100"],
            "error(0.1) D0 D1\n",
            "DEM input and its parsed model need at least",
        ),
        case(
            "sample compiled program",
            &["sample", "--shots", "5", "--memory-budget", "4KiB"],
            deep_repeat.clone(),
            "cannot compile circuit sampler: the compiled program needs at least",
        ),
        case(
            "sample session",
            &["sample", "--shots", "5", "--memory-budget", "1000"],
            random_detector,
            "sampling session needs an estimated",
        ),
        case(
            "sample output encoders",
            &[
                "sample",
                "--shots",
                "300",
                "--out_format",
                "b8",
                "--memory-budget",
                "1000",
            ],
            random_detector,
            "sample needs at least",
        ),
        case(
            "detect compiled plan",
            &["detect", "--shots", "5", "--memory-budget", "1000"],
            huge_repeat,
            "the compiled plan and its records need at least",
        ),
        case(
            "detect session",
            &["detect", "--shots", "5", "--memory-budget", "1000"],
            random_detector,
            "detection session needs an estimated",
        ),
        case(
            "m2d session",
            &[
                "m2d",
                "--in_format",
                "01",
                "--out_format",
                "01",
                "--circuit",
                &detector_circuit,
                "--memory-budget",
                "1000",
            ],
            measurements.clone(),
            "detection session needs an estimated",
        ),
        case(
            "m2d output encoders",
            &[
                "m2d",
                "--in_format",
                "01",
                "--out_format",
                "01",
                "--circuit",
                &detector_circuit,
                "--memory-budget",
                "256KiB",
            ],
            measurements,
            "m2d needs at least",
        ),
        case(
            "analyze_errors analysis",
            &["analyze_errors", "--memory-budget", "1000"],
            hyperedge,
            "circuit-to-detector-error-model materialized-bytes needs at least",
        ),
        case(
            "sample_dem compiled program",
            &["sample_dem", "--shots", "5", "--memory-budget", "1000"],
            repeat_dem,
            "compiled DEM program requires",
        ),
        case(
            "sample_dem session",
            &["sample_dem", "--shots", "5", "--memory-budget", "4KiB"],
            repeat_dem,
            "the sampling session would require at least",
        ),
        case(
            "convert record storage",
            &[
                "convert",
                "--in_format",
                "01",
                "--out_format",
                "b8",
                "--bits_per_shot",
                "4",
                "--memory-budget",
                "0",
            ],
            "0101\n",
            "convert needs at least",
        ),
        case(
            "convert detection plan",
            &[
                "convert",
                "--in_format",
                "01",
                "--out_format",
                "dets",
                "--circuit",
                &huge_circuit,
                "--types",
                "MDL",
                "--memory-budget",
                "1000",
            ],
            "0\n",
            "the compiled plan and its records need at least",
        ),
        case(
            "gen",
            &[
                "gen",
                "--code",
                "repetition_code",
                "--task",
                "memory",
                "--distance",
                "3",
                "--rounds",
                "3",
                "--memory-budget",
                "0",
            ],
            "",
            "physical qubits for repetition code need at least",
        ),
    ];
    cases.push(BudgetRefusalCase {
        code: "input-too-large",
        ..case(
            "input size",
            &["sample", "--shots", "1", "--memory-budget", "0"],
            random_detector,
            "sample circuit input has more than 0 bytes",
        )
    });

    for case in &cases {
        let args = case.args.iter().map(String::as_str).collect::<Vec<_>>();
        let human = stab_with_stdin(&args, case.stdin.as_bytes(), false);
        assert_eq!(human.status.code(), Some(1), "{}", case.label);
        let stderr = String::from_utf8_lossy(&human.stderr);
        let lines = stderr.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2, "{}: {stderr}", case.label);
        assert!(
            lines
                .first()
                .is_some_and(|line| line.starts_with("error: ") && line.contains(case.message)),
            "{}: {stderr}",
            case.label
        );
        let help = format!("help: {MEMORY_BUDGET_HELP}");
        assert_eq!(lines.get(1).copied(), Some(help.as_str()), "{}", case.label);

        let json = stab_with_stdin(&args, case.stdin.as_bytes(), true);
        assert_eq!(json.status.code(), Some(1), "{}", case.label);
        let diagnostic: serde_json::Value =
            serde_json::from_slice(&json.stderr).expect("one JSON diagnostic");
        assert_eq!(
            json_field(&diagnostic, "code"),
            Some(case.code),
            "{}: {diagnostic}",
            case.label
        );
        assert_eq!(
            json_field(&diagnostic, "help"),
            Some(MEMORY_BUDGET_HELP),
            "{}: {diagnostic}",
            case.label
        );
        assert!(
            json_field(&diagnostic, "message")
                .is_some_and(|message| message.contains(case.message)),
            "{}: {diagnostic}",
            case.label
        );
    }
}

/// A fixed safety limit is a resource limit but not a budget refusal: it keeps the resource
/// code and never suggests raising `--memory-budget`, which would not admit it.
#[test]
fn fixed_limits_keep_the_resource_code_without_the_memory_budget_help() {
    let nested = format!("{}M 0\n{}", "REPEAT 2 {\n".repeat(300), "}\n".repeat(300));
    let human = stab_with_stdin(&["sample", "--shots", "1"], nested.as_bytes(), false);
    assert_eq!(human.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&human.stderr);
    assert!(
        stderr.contains("repeat nesting exceeds current limit"),
        "{stderr}"
    );
    assert!(!stderr.contains("--memory-budget"), "{stderr}");

    let json = stab_with_stdin(&["sample", "--shots", "1"], nested.as_bytes(), true);
    let diagnostic: serde_json::Value =
        serde_json::from_slice(&json.stderr).expect("one JSON diagnostic");
    assert_eq!(
        json_field(&diagnostic, "code"),
        Some("resource-limit-exceeded"),
        "{diagnostic}"
    );
    assert_eq!(
        diagnostic.get("help"),
        Some(&serde_json::Value::Null),
        "{diagnostic}"
    );
}

/// The string value of a top-level JSON diagnostic field.
fn json_field<'a>(diagnostic: &'a serde_json::Value, field: &str) -> Option<&'a str> {
    diagnostic.get(field).and_then(serde_json::Value::as_str)
}
