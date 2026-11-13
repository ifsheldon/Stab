use std::io::{self, Write};

use crate::{CliError, io_plan::OutputFile};

pub(crate) struct FileOutputSink {
    file: OutputFile,
}

impl FileOutputSink {
    pub(crate) fn from_output(file: OutputFile) -> Self {
        Self { file }
    }

    pub(crate) fn write_with(
        &mut self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), CliError> {
        write(&mut self.file).map_err(|source| CliError::WritePath {
            path: self.file.path().to_path_buf(),
            source,
        })
    }

    /// Flushes buffered bytes, reporting a failure exactly like a failed write.
    pub(crate) fn flush(&mut self) -> Result<(), CliError> {
        self.write_with(|writer| writer.flush())
    }
}

pub(crate) enum OutputSink<'a, W>
where
    W: Write,
{
    Stdout(&'a mut W),
    File(FileOutputSink),
}

impl<'a, W> OutputSink<'a, W>
where
    W: Write,
{
    pub(crate) fn from_output(output: Option<OutputFile>, stdout: &'a mut W) -> Self {
        match output {
            Some(output) => Self::File(FileOutputSink::from_output(output)),
            None => Self::Stdout(stdout),
        }
    }

    pub(crate) fn write_with(
        &mut self,
        write: impl FnOnce(&mut dyn Write) -> io::Result<()>,
    ) -> Result<(), CliError> {
        match self {
            Self::Stdout(stdout) => write(*stdout).map_err(CliError::WriteOutput),
            Self::File(sink) => sink.write_with(write),
        }
    }

    /// Flushes buffered bytes, reporting a failure exactly like a failed write.
    pub(crate) fn flush(&mut self) -> Result<(), CliError> {
        self.write_with(|writer| writer.flush())
    }
}

/// Reports a pending output failure ahead of a later input or layout failure.
///
/// Unbuffered output surfaced a full device or closed pipe at the write that hit it, before a
/// later malformed record could stop the command. Buffered output can still hold those bytes when
/// such an error arrives, so this flushes them first and reports a flush failure in its place.
/// Output failures are returned unchanged, because retrying the failed stream only repeats them.
pub(crate) fn flush_before_failure(
    result: Result<(), CliError>,
    flush: impl FnOnce() -> Result<(), CliError>,
) -> Result<(), CliError> {
    match result {
        Ok(()) => Ok(()),
        Err(error @ (CliError::WriteOutput(_) | CliError::WritePath { .. })) => Err(error),
        Err(error) => {
            flush()?;
            Err(error)
        }
    }
}
