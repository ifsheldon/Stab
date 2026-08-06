use std::ffi::OsString;

use tempfile::tempdir;

use crate::{MAX_CIRCUIT_INPUT_BYTES, MAX_CONVERT_INPUT_BYTES, run_from};

fn sparse_file_with_len(directory: &tempfile::TempDir, len: u64) -> std::path::PathBuf {
    let path = directory.path().join("oversized-input");
    std::fs::File::create(&path)
        .expect("create sparse file")
        .set_len(len)
        .expect("size sparse file");
    path
}

#[derive(Debug)]
struct FailingWriter;

impl std::io::Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("intentional write stop"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FailAfterOneWrite {
    writes: usize,
}

impl std::io::Write for FailAfterOneWrite {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.writes += 1;
        if self.writes > 1 {
            return Err(std::io::Error::other("intentional second-batch stop"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Default)]
struct FlushFailWriter {
    bytes: usize,
}

impl std::io::Write for FlushFailWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("intentional finish failure"))
    }
}

#[test]
fn sample_streams_output_without_materializing_all_shots() {
    let mut stdout = FailingWriter;
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=1000000000"],
        "M 0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("sampling sink write-batch failed after 0 committed shots while attempting 64 shots: intentional write stop")
    );
}

#[test]
fn sample_reports_second_batch_writer_failure_with_exact_progress() {
    let mut stdout = FailAfterOneWrite::default();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=128"],
        "M 0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout.writes, 2);
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    assert!(stderr.contains("sampling sink write-batch failed"));
    assert!(stderr.contains("after 64 committed shots"));
    assert!(stderr.contains("while attempting 64 shots"));
    assert!(stderr.contains("intentional second-batch stop"));
}

#[test]
fn sample_reports_flush_failure_as_sink_finalization() {
    let mut stdout = FlushFailWriter::default();
    let mut stderr = Vec::new();
    let status = run_from(
        ["stab", "sample", "--shots=65"],
        "M 0\n".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(stdout.bytes, 130);
    let stderr = String::from_utf8(stderr).expect("stderr is utf-8");
    assert!(stderr.contains("sampling sink finish failed"));
    assert!(stderr.contains("after 65 committed shots"));
    assert!(stderr.contains("while attempting 0 shots"));
    assert!(stderr.contains("intentional finish failure"));
}

#[test]
fn sample_rejects_oversized_circuit_input() {
    let directory = tempdir().expect("create temp dir");
    let input_path = sparse_file_with_len(&directory, MAX_CIRCUIT_INPUT_BYTES + 1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("sample"),
            OsString::from("--in"),
            input_path.into_os_string(),
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("sample circuit input is too large; limit is 67108864 bytes")
    );
}

#[test]
fn convert_rejects_oversized_input() {
    let directory = tempdir().expect("create temp dir");
    let input_path = sparse_file_with_len(&directory, MAX_CONVERT_INPUT_BYTES + 1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("convert"),
            OsString::from("--in_format=01"),
            OsString::from("--bits_per_shot=1"),
            OsString::from("--in"),
            input_path.into_os_string(),
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("convert input is too large; limit is 67108864 bytes")
    );
}

#[test]
fn detect_rejects_oversized_circuit_input() {
    let directory = tempdir().expect("create temp dir");
    let input_path = sparse_file_with_len(&directory, MAX_CIRCUIT_INPUT_BYTES + 1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("detect"),
            OsString::from("--in"),
            input_path.into_os_string(),
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("detect circuit input is too large; limit is 67108864 bytes")
    );
}

#[test]
fn m2d_rejects_oversized_circuit_input() {
    let directory = tempdir().expect("create temp dir");
    let circuit_path = sparse_file_with_len(&directory, MAX_CIRCUIT_INPUT_BYTES + 1);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(
        vec![
            OsString::from("stab"),
            OsString::from("m2d"),
            OsString::from("--circuit"),
            circuit_path.into_os_string(),
            OsString::from("--in_format=01"),
        ],
        "".as_bytes(),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 1);
    assert_eq!(String::from_utf8(stdout).unwrap(), "");
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("m2d circuit input is too large; limit is 67108864 bytes")
    );
}
