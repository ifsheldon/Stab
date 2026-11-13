use std::ffi::OsString;

use serde_json::Value;
use tempfile::tempdir;

use super::support::{FailAfterOneWrite, FailingWriter, FlushFailWriter, run_parts};
use crate::{diagnostics::probe_error_format, run_from};

fn json_lines(stderr: &[u8]) -> Vec<Value> {
    std::str::from_utf8(stderr)
        .expect("diagnostics are UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("diagnostic is JSON"))
        .collect()
}

fn only_json_line(stderr: &[u8]) -> Value {
    let [line] = <[Value; 1]>::try_from(json_lines(stderr))
        .expect("stderr contains exactly one JSON diagnostic");
    line
}

fn field<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).expect("diagnostic field exists")
}

fn assert_invalid_utf8_diagnostic(args: &[&str], dialect: &str) {
    let (status, stdout, stderr) = run_parts(args, b"\xc3");
    assert_eq!(status, 1, "{args:?}");
    assert_eq!(stdout, b"", "{args:?}");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(
        field(&diagnostic, "/code"),
        "invalid-utf8-input",
        "{args:?}"
    );
    assert_eq!(field(&diagnostic, "/span/byte_start"), 0, "{args:?}");
    assert_eq!(field(&diagnostic, "/span/byte_length"), 1, "{args:?}");
    assert_eq!(field(&diagnostic, "/context/dialect"), dialect, "{args:?}");
    assert_eq!(field(&diagnostic, "/context/valid_up_to"), 0, "{args:?}");
    assert!(
        field(&diagnostic, "/context/error_length").is_null(),
        "{args:?}"
    );
}

#[test]
fn json_result_format_diagnostic_has_schema_span_and_typed_context() {
    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
            "--error-format=json",
        ],
        b"0\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        json_lines(&stderr),
        vec![serde_json::json!({
            "schema_version": 1,
            "code": "invalid-record-width",
            "severity": "error",
            "message": "01 record ended after 1 bits; expected 2 bits",
            "span": {
                "byte_start": 1,
                "byte_length": 1,
            },
            "labels": [],
            "help": null,
            "context": {
                "actual_bits": 1,
                "expected_bits": 2,
            },
        })]
    );
}

#[test]
fn json_packed_format_diagnostics_include_exact_span_and_context() {
    let cases = [
        (
            &[
                "stab",
                "convert",
                "--in_format=b8",
                "--out_format=01",
                "--bits_per_shot=9",
                "--error-format=json",
            ][..],
            &b"\x01"[..],
            "invalid-packed-length",
            serde_json::json!({
                "actual_bytes": 1,
                "byte_multiple": 2,
            }),
        ),
        (
            &[
                "stab",
                "convert",
                "--in_format=r8",
                "--out_format=01",
                "--bits_per_shot=3",
                "--error-format=json",
            ][..],
            &b"\x04"[..],
            "run-length-overshoot",
            serde_json::json!({
                "decoded_bits": 4,
                "expected_bits": 3,
            }),
        ),
    ];

    for (args, input, expected_code, expected_context) in cases {
        let (status, stdout, stderr) = run_parts(args, input);
        assert_eq!(status, 1, "{args:?}");
        assert_eq!(stdout, b"", "{args:?}");
        let diagnostic = only_json_line(&stderr);
        assert_eq!(field(&diagnostic, "/code"), expected_code, "{args:?}");
        assert_eq!(field(&diagnostic, "/span/byte_start"), 0, "{args:?}");
        assert_eq!(field(&diagnostic, "/span/byte_length"), 1, "{args:?}");
        assert_eq!(
            field(&diagnostic, "/context"),
            &expected_context,
            "{args:?}"
        );
    }
}

#[test]
fn json_resource_limit_diagnostic_preserves_typed_parse_context() {
    // 300 bytes of source whose parsed targets exceed a 512-byte budget.
    let mut input = String::from("H");
    while input.len() < 300 {
        input.push_str(" 0");
    }
    input.push('\n');
    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "convert",
            "--in_format=stim",
            "--out_format=stim",
            "--memory-budget=512",
            "--error-format=json",
        ],
        input.as_bytes(),
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "resource-limit-exceeded");
    assert_eq!(field(&diagnostic, "/context/operation"), "circuit-parse");
    assert_eq!(field(&diagnostic, "/context/resource"), "retained-bytes");
    assert_eq!(field(&diagnostic, "/context/limit"), 512);
    assert!(field(&diagnostic, "/context/actual").as_u64() > Some(512));
    assert!(
        field(&diagnostic, "/message")
            .as_str()
            .is_some_and(|message| message.contains("512-byte memory budget")),
        "{diagnostic}"
    );
    assert!(
        field(&diagnostic, "/help")
            .as_str()
            .is_some_and(|help| help.contains("--memory-budget")),
        "{diagnostic}"
    );
}

#[test]
fn json_sampling_budget_refusal_names_the_budget_and_how_to_raise_it() {
    // The parsed circuit fits the budget, but the sampling session's frame does not.
    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "sample",
            "--shots=1",
            "--memory-budget=4KiB",
            "--error-format=json",
        ],
        b"M 1000\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert!(
        field(&diagnostic, "/message")
            .as_str()
            .is_some_and(|message| message.contains("4096-byte memory budget")),
        "{diagnostic}"
    );
    assert!(
        field(&diagnostic, "/help")
            .as_str()
            .is_some_and(|help| help.contains("--memory-budget")),
        "{diagnostic}"
    );
    let (status, stdout, _) = run_parts(
        ["stab", "sample", "--shots=1", "--memory-budget=8MiB"],
        b"M 1000\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, b"0\n");
}

#[test]
fn compact_sampling_work_far_above_u64_is_refused_by_its_record_storage() {
    // 10^24 expanded instructions and 10^12 measurements per shot: the compact program fits, but
    // the session's record storage exceeds the budget, so the run is refused before any work.
    let circuit =
        b"REPEAT 1000000000000 {\n    REPEAT 1000000000000 {\n        H 0\n    }\n    M 0\n}\n";
    let (status, stdout, stderr) = run_parts(["stab", "sample", "--shots=2"], circuit);
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let stderr = String::from_utf8_lossy(&stderr);
    assert!(stderr.contains("1073741824-byte memory budget"), "{stderr}");
    assert!(stderr.contains("--memory-budget"), "{stderr}");
}

#[test]
fn json_invalid_utf8_diagnostics_are_consistent_across_model_inputs() {
    for (args, dialect) in [
        (
            &[
                "stab",
                "convert",
                "--in_format=stim",
                "--out_format=stim",
                "--error-format=json",
            ][..],
            "stim-circuit",
        ),
        (
            &["stab", "sample", "--shots=1", "--error-format=json"][..],
            "stim-circuit",
        ),
        (
            &["stab", "detect", "--shots=1", "--error-format=json"][..],
            "stim-circuit",
        ),
        (
            &["stab", "analyze_errors", "--error-format=json"][..],
            "stim-circuit",
        ),
        (
            &["stab", "inspect", "--type=stim", "--error-format=json"][..],
            "stim-circuit",
        ),
        (
            &["stab", "plan", "sample", "--shots=0", "--error-format=json"][..],
            "stim-circuit",
        ),
        (
            &["stab", "sample_dem", "--shots=1", "--error-format=json"][..],
            "detector-error-model",
        ),
        (
            &["stab", "inspect", "--type=dem", "--error-format=json"][..],
            "detector-error-model",
        ),
    ] {
        assert_invalid_utf8_diagnostic(args, dialect);
    }
}

#[test]
fn json_invalid_utf8_diagnostics_cover_circuit_and_dem_side_files() {
    let temp = tempdir().expect("temporary directory");
    let invalid_path = temp.path().join("invalid-model");
    std::fs::write(&invalid_path, b"\xc3").expect("write invalid UTF-8 model");
    let invalid_path = invalid_path.to_str().expect("UTF-8 test path");

    for (args, dialect) in [
        (
            vec![
                "stab",
                "m2d",
                "--circuit",
                invalid_path,
                "--in_format=01",
                "--out_format=01",
                "--error-format=json",
            ],
            "stim-circuit",
        ),
        (
            vec![
                "stab",
                "convert",
                "--circuit",
                invalid_path,
                "--types=M",
                "--in_format=01",
                "--out_format=01",
                "--error-format=json",
            ],
            "stim-circuit",
        ),
        (
            vec![
                "stab",
                "convert",
                "--dem",
                invalid_path,
                "--in_format=01",
                "--out_format=01",
                "--error-format=json",
            ],
            "detector-error-model",
        ),
    ] {
        assert_invalid_utf8_diagnostic(&args, dialect);
    }
}

#[test]
fn model_cli_paths_accept_opaque_metadata_bytes_and_preserve_stim_conversion() {
    let (status, stdout, stderr) = run_parts(
        ["stab", "convert", "--in_format=stim", "--out_format=stim"],
        b"H[\xff] 0 # \xfe\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, b"H[\xff] 0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_parts(["stab", "sample", "--shots=1"], b"M 0 # \xff\n");
    assert_eq!(status, 0);
    assert_eq!(stdout, b"0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_parts(
        ["stab", "inspect", "--type=dem", "--format=json"],
        b"error[\xff](0) D0\n",
    );
    assert_eq!(status, 0);
    assert!(serde_json::from_slice::<Value>(&stdout).is_ok());
    assert_eq!(stderr, b"");
}

#[test]
fn json_circuit_parse_diagnostic_preserves_code_span_context_and_human_mode() {
    let input = b"H 0\r\nUNKNOWN 1\r\n";
    let json_args = [
        "stab",
        "convert",
        "--in_format=stim",
        "--out_format=stim",
        "--error-format=json",
    ];
    let (status, stdout, stderr) = run_parts(json_args, input);
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        only_json_line(&stderr),
        serde_json::json!({
            "schema_version": 1,
            "code": "unknown-instruction",
            "severity": "error",
            "message": "unknown gate UNKNOWN",
            "span": {
                "byte_start": 5,
                "byte_length": 7,
            },
            "labels": [],
            "help": null,
            "context": {
                "dialect": "stim-circuit",
                "instruction": "UNKNOWN",
            },
        })
    );

    let human_args = ["stab", "convert", "--in_format=stim", "--out_format=stim"];
    let (status, stdout, stderr) = run_parts(human_args, input);
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        stderr,
        b"error: failed to parse line 2: unknown gate UNKNOWN\n"
    );
}

#[test]
fn parser_diagnostics_bound_attacker_controlled_text_in_human_and_json_output() {
    let gate = "A".repeat(16_384);
    let input = format!("{gate} 0\n");
    let json_args = [
        "stab",
        "convert",
        "--in_format=stim",
        "--out_format=stim",
        "--error-format=json",
    ];
    let (status, stdout, stderr) = run_parts(json_args, input.as_bytes());
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(stderr.len() <= 1_024);

    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "unknown-instruction");
    assert_eq!(field(&diagnostic, "/span/byte_start"), 0);
    assert_eq!(field(&diagnostic, "/span/byte_length"), gate.len());
    let message = field(&diagnostic, "/message")
        .as_str()
        .expect("diagnostic message is text");
    assert!(message.len() <= 256);
    assert!(message.contains("original length:"));
    let instruction = field(&diagnostic, "/context/instruction")
        .as_str()
        .expect("instruction excerpt is text");
    assert!(instruction.len() <= 256);
    assert!(instruction.ends_with(" [truncated; original length: 16384 bytes]"));

    let human_args = ["stab", "convert", "--in_format=stim", "--out_format=stim"];
    let (status, stdout, stderr) = run_parts(human_args, input.as_bytes());
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert!(stderr.len() <= 264);
    assert!(
        std::str::from_utf8(&stderr)
            .expect("human diagnostic is UTF-8")
            .contains("original length:")
    );
}

#[test]
fn json_dem_parse_diagnostic_preserves_numeric_location_and_typed_context() {
    let (status, stdout, stderr) = run_parts(
        ["stab", "inspect", "--type=dem", "--error-format=json"],
        b"error(0.1) D1152921504606846976\n",
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "integer-out-of-range");
    assert_eq!(field(&diagnostic, "/span/byte_start"), 11);
    assert_eq!(field(&diagnostic, "/span/byte_length"), 20);
    assert_eq!(
        field(&diagnostic, "/context/dialect"),
        "detector-error-model"
    );
    assert_eq!(field(&diagnostic, "/context/instruction"), "error");
}

#[test]
fn clap_errors_use_json_only_for_one_unambiguous_request() {
    let (status, stdout, stderr) = run_parts(["stab", "--error-format=json", "nope"], b"");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "cli-invalid-subcommand");
    assert_eq!(
        field(&diagnostic, "/context/clap_error_kind"),
        "invalid-subcommand"
    );

    let (_, _, duplicate_stderr) = run_parts(
        ["stab", "--error-format=json", "--error-format=json", "nope"],
        b"",
    );
    assert!(
        std::str::from_utf8(&duplicate_stderr)
            .expect("Clap stderr is UTF-8")
            .starts_with("error:")
    );

    assert_eq!(
        probe_error_format(
            &["stab", "--", "--error-format=json"]
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        ),
        crate::diagnostics::ErrorFormatArg::Human
    );
}

#[test]
fn help_and_version_remain_successful_human_stdout() {
    for args in [
        &["stab", "--error-format=json", "--help"][..],
        &["stab", "--error-format=json", "--version"][..],
    ] {
        let (status, stdout, stderr) = run_parts(args, b"");
        assert_eq!(status, 0, "{args:?}");
        assert!(!stdout.is_empty(), "{args:?}");
        assert_eq!(stderr, b"", "{args:?}");
        assert!(
            serde_json::from_slice::<Value>(&stdout).is_err(),
            "{args:?} unexpectedly emitted JSON"
        );
    }
}

#[test]
fn global_error_format_can_precede_named_commands_and_help_topics() {
    let (status, stdout, stderr) = run_parts(
        ["stab", "--error-format=json", "sample", "--shots=1"],
        b"M 0\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, b"0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "--error-format=json",
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
        ],
        b"10\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, [1]);
    assert_eq!(stderr, b"");

    let args = &["stab", "--error-format=json", "help", "sample"];
    let (status, stdout, stderr) = run_parts(args, b"");
    assert_eq!(status, 0);
    assert!(stdout.starts_with(b"stab sample\n"));
    assert_eq!(stderr, b"");
}

#[test]
fn m2d_streaming_diagnostic_shifts_second_record_span_to_input_offset() {
    let temp = tempdir().expect("temporary directory");
    let circuit_path = temp.path().join("layout.stim");
    std::fs::write(&circuit_path, "M 0 1\nDETECTOR rec[-1]\n").expect("write circuit");
    let circuit_path = circuit_path.to_str().expect("UTF-8 path");

    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "m2d",
            "--circuit",
            circuit_path,
            "--in_format=01",
            "--error-format=json",
        ],
        b"00\n0\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"0\n");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "invalid-record-width");
    assert_eq!(field(&diagnostic, "/span/byte_start"), 4);
    assert_eq!(field(&diagnostic, "/span/byte_length"), 1);
}

#[test]
fn sample_dem_replay_diagnostic_shifts_second_record_span_to_input_offset() {
    let temp = tempdir().expect("temporary directory");
    let replay_path = temp.path().join("errors.01");
    std::fs::write(&replay_path, b"0\nx\n").expect("write replay input");
    let args = vec![
        OsString::from("stab"),
        OsString::from("sample_dem"),
        OsString::from("--replay_err_in"),
        replay_path.into_os_string(),
        OsString::from("--replay_err_in_format=01"),
        OsString::from("--shots=2"),
        OsString::from("--error-format=json"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run_from(
        args,
        b"error(0.25) D0\n".as_slice(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "invalid-byte");
    assert_eq!(field(&diagnostic, "/span/byte_start"), 2);
    assert_eq!(field(&diagnostic, "/span/byte_length"), 1);
}

#[test]
fn json_mode_preserves_alias_rejection_before_truncation() {
    let temp = tempdir().expect("temporary directory");
    let path = temp.path().join("records.01");
    let sentinel = b"01\n";
    std::fs::write(&path, sentinel).expect("write input");
    let path = path.to_str().expect("UTF-8 path");

    let (status, stdout, stderr) = run_parts(
        [
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
            "--in",
            path,
            "--out",
            path,
            "--error-format=json",
        ],
        b"",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(std::fs::read(path).expect("read preserved input"), sentinel);
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "conflicting-file-roles");
    assert_eq!(field(&diagnostic, "/context/first_role"), "--in");
    assert_eq!(field(&diagnostic, "/context/second_role"), "--out");
}

#[test]
fn json_mode_reports_the_original_output_writer_failure() {
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
            "--error-format=json",
        ],
        b"01\n".as_slice(),
        FailingWriter,
        &mut stderr,
    );

    assert_eq!(status, 1);
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "stdout-write-failed");
    assert!(
        field(&diagnostic, "/message")
            .as_str()
            .is_some_and(|message| message.contains("intentional write stop"))
    );
}

#[test]
fn json_sample_writer_failure_preserves_phase_and_progress() {
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=128", "--error-format=json"],
        b"M 0\n".as_slice(),
        FailAfterOneWrite::default(),
        &mut stderr,
    );

    assert_eq!(status, 1);
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "stdout-write-failed");
    assert_eq!(field(&diagnostic, "/context/failure_kind"), "sink");
    assert_eq!(field(&diagnostic, "/context/sink_phase"), "write-batch");
    assert_eq!(field(&diagnostic, "/context/committed_shots"), 64);
    assert_eq!(field(&diagnostic, "/context/attempted_batch_shots"), 64);
    assert!(
        field(&diagnostic, "/message")
            .as_str()
            .is_some_and(|message| message.contains("intentional second-batch stop"))
    );
}

#[test]
fn json_sample_flush_failure_preserves_finish_phase_and_progress() {
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=65", "--error-format=json"],
        b"M 0\n".as_slice(),
        FlushFailWriter::default(),
        &mut stderr,
    );

    assert_eq!(status, 1);
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "stdout-write-failed");
    assert_eq!(field(&diagnostic, "/context/failure_kind"), "sink");
    assert_eq!(field(&diagnostic, "/context/sink_phase"), "finish");
    assert_eq!(field(&diagnostic, "/context/committed_shots"), 65);
    assert_eq!(field(&diagnostic, "/context/attempted_batch_shots"), 0);
    assert!(
        field(&diagnostic, "/message")
            .as_str()
            .is_some_and(|message| message.contains("intentional finish failure"))
    );
}
