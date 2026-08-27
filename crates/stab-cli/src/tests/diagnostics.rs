use std::ffi::OsString;
use std::io::{self, Write};

use serde_json::Value;
use tempfile::tempdir;

use crate::{diagnostics::probe_error_format, run_from};

fn run_cli(args: &[&str], input: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args.iter().copied(), input, &mut stdout, &mut stderr);
    (status, stdout, stderr)
}

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
    let (status, stdout, stderr) = run_cli(args, b"\xc3");
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
fn human_result_format_diagnostic_remains_byte_exact() {
    let (status, stdout, stderr) = run_cli(
        &[
            "stab",
            "convert",
            "--in_format=01",
            "--out_format=b8",
            "--bits_per_shot=2",
        ],
        b"0\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        stderr,
        b"error: invalid result format data: 01 record ended after 1 bits; expected 2 bits\n"
    );
}

#[test]
fn json_result_format_diagnostic_has_schema_span_and_typed_context() {
    let (status, stdout, stderr) = run_cli(
        &[
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
        let (status, stdout, stderr) = run_cli(args, input);
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
    let input = vec![b'\n'; 1_000_001];
    let (status, stdout, stderr) = run_cli(
        &[
            "stab",
            "convert",
            "--in_format=stim",
            "--out_format=stim",
            "--error-format=json",
        ],
        &input,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        only_json_line(&stderr),
        serde_json::json!({
            "schema_version": 1,
            "code": "resource-limit-exceeded",
            "severity": "error",
            "message": "failed to parse line 1000001: circuit input has more than 1000000 lines",
            "span": {
                "byte_start": 1_000_000,
                "byte_length": 0,
            },
            "labels": [],
            "help": null,
            "context": {
                "operation": "circuit-parse",
                "resource": "source-lines",
                "actual": 1_000_001,
                "limit": 1_000_000,
            },
        })
    );
}

#[test]
fn json_sampling_work_limit_preserves_typed_resource_context() {
    let (status, stdout, stderr) = run_cli(
        &["stab", "sample", "--shots=1", "--error-format=json"],
        b"REPEAT 1000000 {\n    H 0\n}\nM 0\n",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        only_json_line(&stderr),
        serde_json::json!({
            "schema_version": 1,
            "code": "resource-limit-exceeded",
            "severity": "error",
            "message": "cannot compile circuit sampler: expanded operation work 1000001 exceeds per-shot limit 1000000",
            "span": null,
            "labels": [],
            "help": null,
            "context": {
                "resource": "expanded-operations-per-shot",
                "actual": "1000001",
                "limit": "1000000",
            },
        })
    );
}

#[test]
fn json_sampling_work_above_u64_is_an_explicit_lower_bound() {
    let circuit =
        b"REPEAT 1000000000000 {\n    REPEAT 1000000000000 {\n        H 0\n    }\n    M 0\n}\n";
    let (status, stdout, stderr) = run_cli(
        &["stab", "sample", "--shots=1", "--error-format=json"],
        circuit,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    assert_eq!(
        only_json_line(&stderr),
        serde_json::json!({
            "schema_version": 1,
            "code": "resource-limit-exceeded",
            "severity": "error",
            "message": "cannot compile circuit sampler: expanded operation work at least 18446744073709551615 exceeds per-shot limit 1000000",
            "span": null,
            "labels": [],
            "help": null,
            "context": {
                "resource": "expanded-operations-per-shot",
                "actual": "18446744073709551615",
                "actual_is_lower_bound": true,
                "limit": "1000000",
            },
        })
    );

    let (status, stdout, stderr) = run_cli(
        &["stab", "detect", "--shots=1", "--error-format=json"],
        circuit,
    );
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "resource-limit-exceeded");
    assert_eq!(
        field(&diagnostic, "/context/operation"),
        "detection-conversion"
    );
    assert_eq!(field(&diagnostic, "/context/actual"), 1_000_000_000_000u64);
    assert_eq!(diagnostic.pointer("/context/actual_is_lower_bound"), None);
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
    let (status, stdout, stderr) = run_cli(
        &["stab", "convert", "--in_format=stim", "--out_format=stim"],
        b"H[\xff] 0 # \xfe\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, b"H[\xff] 0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_cli(&["stab", "sample", "--shots=1"], b"M 0 # \xff\n");
    assert_eq!(status, 0);
    assert_eq!(stdout, b"0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_cli(
        &["stab", "inspect", "--type=dem", "--format=json"],
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
    let (status, stdout, stderr) = run_cli(&json_args, input);
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
    let (status, stdout, stderr) = run_cli(&human_args, input);
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
    let (status, stdout, stderr) = run_cli(&json_args, input.as_bytes());
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
    let (status, stdout, stderr) = run_cli(&human_args, input.as_bytes());
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
    let (status, stdout, stderr) = run_cli(
        &["stab", "inspect", "--type=dem", "--error-format=json"],
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
    let (status, stdout, stderr) = run_cli(&["stab", "--error-format=json", "nope"], b"");
    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let diagnostic = only_json_line(&stderr);
    assert_eq!(field(&diagnostic, "/code"), "cli-invalid-subcommand");
    assert_eq!(
        field(&diagnostic, "/context/clap_error_kind"),
        "invalid-subcommand"
    );

    let (_, _, duplicate_stderr) = run_cli(
        &["stab", "--error-format=json", "--error-format=json", "nope"],
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
        let (status, stdout, stderr) = run_cli(args, b"");
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
    let (status, stdout, stderr) = run_cli(
        &["stab", "--error-format=json", "sample", "--shots=1"],
        b"M 0\n",
    );
    assert_eq!(status, 0);
    assert_eq!(stdout, b"0\n");
    assert_eq!(stderr, b"");

    let (status, stdout, stderr) = run_cli(
        &[
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
    let (status, stdout, stderr) = run_cli(args, b"");
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

    let (status, stdout, stderr) = run_cli(
        &[
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

    let (status, stdout, stderr) = run_cli(
        &[
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
            .is_some_and(|message| message.contains("injected writer failure"))
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
            .is_some_and(|message| message.contains("injected second-batch failure"))
    );
}

#[test]
fn json_sample_flush_failure_preserves_finish_phase_and_progress() {
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=65", "--error-format=json"],
        b"M 0\n".as_slice(),
        FlushFailWriter,
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
            .is_some_and(|message| message.contains("injected finish failure"))
    );
}

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("injected writer failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct FailAfterOneWrite {
    writes: usize,
}

struct FlushFailWriter;

impl Write for FlushFailWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("injected finish failure"))
    }
}

impl Write for FailAfterOneWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes > 1 {
            return Err(io::Error::other("injected second-batch failure"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
