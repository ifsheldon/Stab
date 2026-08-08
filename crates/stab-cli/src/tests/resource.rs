use std::ffi::OsString;

use tempfile::tempdir;

use crate::{MAX_CIRCUIT_INPUT_BYTES, run_from};

/// One streamed conversion record: 2048 bits, so 256 b8-packed bytes.
const STREAMING_RECORD_BYTES: usize = 256;

/// Synthesizes a deterministic b8 record stream larger than the retired 64 MiB whole-input cap
/// without materializing it.
struct PatternInput {
    remaining: usize,
    offset: usize,
}

impl PatternInput {
    fn new(len: usize) -> Self {
        Self {
            remaining: len,
            offset: 0,
        }
    }

    fn byte_at(offset: usize) -> u8 {
        u8::try_from(offset % 251).unwrap_or(0)
    }
}

impl std::io::Read for PatternInput {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let step = self.remaining.min(buffer.len()).min(64 * 1024);
        for slot in buffer.iter_mut().take(step) {
            *slot = Self::byte_at(self.offset);
            self.offset += 1;
        }
        self.remaining -= step;
        Ok(step)
    }
}

/// Verifies streamed output against the synthesized pattern without buffering it.
#[derive(Debug, Default)]
struct PatternVerifier {
    verified: usize,
    mismatch: Option<usize>,
}

impl std::io::Write for PatternVerifier {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        for byte in buffer {
            if self.mismatch.is_none() && *byte != PatternInput::byte_at(self.verified) {
                self.mismatch = Some(self.verified);
            }
            self.verified += 1;
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

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

/// Decision D5 (docs/plans/post-review-remediation-plan.md): the shared streaming record reader
/// replaced convert's whole-input 64 MiB cap, so an input past the retired cap must now convert
/// successfully. The fixture is synthesized by the test instead of committed.
#[test]
fn convert_streams_inputs_past_the_retired_whole_input_cap() {
    let input_len =
        usize::try_from(MAX_CIRCUIT_INPUT_BYTES).expect("cap fits usize") + STREAMING_RECORD_BYTES;
    assert_eq!(input_len % STREAMING_RECORD_BYTES, 0);
    let mut stdout = PatternVerifier::default();
    let mut stderr = Vec::new();
    let status = run_from(
        [
            "stab",
            "convert",
            "--in_format=b8",
            "--out_format=b8",
            "--bits_per_shot=2048",
        ],
        PatternInput::new(input_len),
        &mut stdout,
        &mut stderr,
    );

    assert_eq!(status, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(String::from_utf8(stderr).unwrap(), "");
    assert_eq!(stdout.verified, input_len);
    assert_eq!(stdout.mismatch, None);
}

/// Streamed conversion must hold memory bounded by the record size, not the input length: peak
/// allocation while converting an over-64-MiB input stays orders of magnitude below the input.
#[test]
fn convert_peak_memory_is_bounded_while_streaming_an_over_cap_input() {
    const PEAK_ALLOCATION_BOUND_BYTES: u64 = 8 * 1024 * 1024;

    let input_len =
        usize::try_from(MAX_CIRCUIT_INPUT_BYTES).expect("cap fits usize") + STREAMING_RECORD_BYTES;
    let mut stdout = PatternVerifier::default();
    let mut stderr = Vec::new();
    let mut status = -1;
    let allocations = allocation_counter::measure(|| {
        status = run_from(
            [
                "stab",
                "convert",
                "--in_format=b8",
                "--out_format=b8",
                "--bits_per_shot=2048",
            ],
            PatternInput::new(input_len),
            &mut stdout,
            &mut stderr,
        );
    });

    assert_eq!(status, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout.verified, input_len);
    assert_eq!(stdout.mismatch, None);
    assert!(
        allocations.bytes_max < PEAK_ALLOCATION_BOUND_BYTES,
        "streaming convert held {} bytes at peak for a {input_len}-byte input: {allocations:?}",
        allocations.bytes_max
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
