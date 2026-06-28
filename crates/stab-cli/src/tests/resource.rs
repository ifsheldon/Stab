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
        Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "intentional write stop",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
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
            .contains("failed to write output: intentional write stop")
    );
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
