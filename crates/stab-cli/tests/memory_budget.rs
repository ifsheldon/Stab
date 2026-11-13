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
