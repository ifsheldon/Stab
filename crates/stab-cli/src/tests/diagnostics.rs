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
fn json_warnings_and_errors_are_ordered_json_lines() {
    let (status, stdout, stderr) = run_cli(
        &["stab", "sample", "--frame0", "--error-format=json"],
        b"\xff",
    );

    assert_eq!(status, 1);
    assert_eq!(stdout, b"");
    let [warning, error] =
        <[Value; 2]>::try_from(json_lines(&stderr)).expect("warning followed by error");
    assert_eq!(field(&warning, "/code"), "deprecated-frame0");
    assert_eq!(field(&warning, "/severity"), "warning");
    assert_eq!(
        field(&warning, "/context/replacement"),
        "--skip_reference_sample"
    );
    assert_eq!(field(&error, "/code"), "invalid-utf8-input");
    assert_eq!(field(&error, "/severity"), "error");
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

struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("injected writer failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
