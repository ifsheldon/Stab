//! Shared harness for in-process CLI tests: one runner, one result type, scratch files, and the
//! injected writers that pin streaming-failure behavior.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use stab_records::{
    DetsResultType, PackedShotBatch, RecordBatchPart, RecordEncoder, RecordFormat,
    RecordStreamReader,
};
use tempfile::TempDir;

use crate::run_from;

/// The observable result of one in-process CLI invocation.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CliRun {
    pub(super) status: i32,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

impl CliRun {
    /// Splits the run into `(status, stdout, stderr)` for tests that inspect raw bytes.
    pub(super) fn into_parts(self) -> (i32, Vec<u8>, Vec<u8>) {
        (self.status, self.stdout, self.stderr)
    }

    pub(super) fn stdout_text(&self) -> &str {
        std::str::from_utf8(&self.stdout).expect("stdout is UTF-8")
    }

    pub(super) fn stderr_text(&self) -> &str {
        std::str::from_utf8(&self.stderr).expect("stderr is UTF-8")
    }

    /// Asserts a successful run with empty stderr and exactly `expected` on stdout.
    #[track_caller]
    pub(super) fn assert_ok(&self, expected: impl AsRef<[u8]>) {
        assert_eq!(
            self.status,
            0,
            "stderr: {}",
            String::from_utf8_lossy(&self.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&self.stdout),
            String::from_utf8_lossy(expected.as_ref())
        );
        assert_eq!(self.stdout, expected.as_ref());
        assert_eq!(self.stderr_text(), "");
    }

    /// Asserts a status-1 failure with no stdout and every `parts` substring on stderr.
    #[track_caller]
    pub(super) fn assert_err(&self, parts: &[&str]) {
        assert_eq!(self.status, 1, "stderr: {}", self.stderr_text());
        assert_eq!(String::from_utf8_lossy(&self.stdout), "");
        self.assert_stderr_contains(parts);
    }

    #[track_caller]
    pub(super) fn assert_stderr_contains(&self, parts: &[&str]) {
        let stderr = self.stderr_text();
        assert!(!stderr.is_empty(), "failure must report a diagnostic");
        for part in parts {
            assert!(stderr.contains(part), "missing {part:?} in {stderr}");
        }
    }
}

/// Runs the CLI with arbitrary `OsString`-convertible arguments.
pub(super) fn run_cli<I, S>(args: I, stdin: &[u8]) -> CliRun
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_from(args, stdin, &mut stdout, &mut stderr);
    CliRun {
        status,
        stdout,
        stderr,
    }
}

/// Runs the CLI and returns `(status, stdout, stderr)` for tests that inspect raw bytes.
pub(super) fn run_parts<I, S>(args: I, stdin: &[u8]) -> (i32, Vec<u8>, Vec<u8>)
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    run_cli(args, stdin).into_parts()
}

/// Runs the CLI with string arguments; `&String` values coerce at the call site.
pub(super) fn run(args: &[&str], stdin: &[u8]) -> CliRun {
    run_cli(args.iter().copied(), stdin)
}

/// A temporary directory whose files are addressed by UTF-8 path strings.
#[derive(Debug)]
pub(super) struct Scratch {
    directory: TempDir,
}

impl Scratch {
    pub(super) fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("temporary directory"),
        }
    }

    pub(super) fn path_buf(&self, name: &str) -> PathBuf {
        self.directory.path().join(name)
    }

    /// Returns the path for `name` without creating it.
    pub(super) fn path(&self, name: &str) -> String {
        self.path_buf(name)
            .into_os_string()
            .into_string()
            .expect("UTF-8 temporary path")
    }

    /// Writes `contents` to `name` and returns its path.
    pub(super) fn file(&self, name: &str, contents: impl AsRef<[u8]>) -> String {
        std::fs::write(self.path_buf(name), contents).expect("write scratch file");
        self.path(name)
    }

    pub(super) fn read(&self, name: &str) -> Vec<u8> {
        std::fs::read(self.path_buf(name)).expect("read scratch file")
    }

    pub(super) fn exists(&self, name: &str) -> bool {
        self.path_buf(name).exists()
    }

    /// Creates a sparse file of `len` bytes, so size limits can be probed without writing data.
    pub(super) fn sparse_file(&self, name: &str, len: u64) -> String {
        std::fs::File::create(self.path_buf(name))
            .expect("create sparse file")
            .set_len(len)
            .expect("size sparse file");
        self.path(name)
    }
}

pub(super) fn ptb64_words(words: &[u64]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// Encodes complete measurement records of one width, as a fixture or an expected output.
pub(super) fn encode_records(records: &[Vec<bool>], format: RecordFormat) -> Vec<u8> {
    let width = records.first().map_or(0, Vec::len);
    let batch = PackedShotBatch::from_records(records, width).expect("fixture records");
    let mut encoder = RecordEncoder::try_new(format, width).expect("fixture encoder");
    let mut out = Vec::new();
    encoder
        .write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                batch.view(),
            )],
            &mut out,
        )
        .expect("encode fixture records");
    assert_eq!(encoder.pending_group_records(), 0, "complete PTB64 groups");
    out
}

/// Decodes every measurement record of a CLI output.
pub(super) fn decode_records(input: &[u8], format: RecordFormat, width: usize) -> Vec<Vec<bool>> {
    let mut reader = RecordStreamReader::measurements(input, format, width, usize::MAX);
    let mut records = Vec::new();
    while let Some(view) = reader.read_batch(64).expect("decode CLI records") {
        for shot in 0..view.shot_count() {
            let row = view.shot(shot).expect("decoded shot");
            records.push(
                (0..row.len())
                    .map(|bit| row.get(bit) == Some(true))
                    .collect(),
            );
        }
    }
    records
}

/// Fails every write. Deliberately not `BrokenPipe`: pipe-rooted failures take the silent
/// status-141 path (decision D2), and this writer pins the diagnostic path for every other
/// streaming write failure.
#[derive(Debug)]
pub(super) struct FailingWriter;

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("intentional write stop"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Accepts the first write and fails every later one.
#[derive(Debug, Default)]
pub(super) struct FailAfterOneWrite {
    pub(super) writes: usize,
}

impl Write for FailAfterOneWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.writes > 1 {
            return Err(io::Error::other("intentional second-batch stop"));
        }
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Accepts every write and fails the final flush.
#[derive(Debug, Default)]
pub(super) struct FlushFailWriter {
    pub(super) bytes: usize,
}

impl Write for FlushFailWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("intentional finish failure"))
    }
}

/// Counts bytes and non-empty writes, to observe batch-by-batch streaming.
#[derive(Debug, Default)]
pub(super) struct CountingWriter {
    pub(super) bytes: usize,
    pub(super) nonempty_writes: usize,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes += buffer.len();
        self.nonempty_writes += usize::from(!buffer.is_empty());
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Fails every write and flush with one error kind.
#[derive(Debug)]
pub(super) struct KindFailWriter(pub(super) io::ErrorKind);

impl Write for KindFailWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(self.0))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::from(self.0))
    }
}
