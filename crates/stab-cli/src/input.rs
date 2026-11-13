use std::io::Read;
use std::path::Path;

#[cfg(test)]
use std::fs::File;
#[cfg(test)]
use std::path::PathBuf;

use stab_records::{FormatError, FormatErrorCode, RecordStreamReadError};

use crate::CliError;
use crate::io_plan::InputFile;

/// Maps a stream-framed [`stab_records::RecordStreamReader`] failure onto the CLI's
/// transport-aware error surface: transport failures keep their path context and format
/// violations keep their typed diagnostics.
///
/// Record-framed failures carry command-specific vocabulary, so callers that enable record
/// framing render them before falling back here.
pub(crate) fn record_stream_error(error: RecordStreamReadError, path: Option<&Path>) -> CliError {
    match error {
        RecordStreamReadError::Io(source) => read_stream_error(path, source),
        RecordStreamReadError::Format(error) => CliError::from(error),
        error => CliError::Record(FormatError::new(
            FormatErrorCode::InvalidData,
            error.to_string(),
            None,
        )),
    }
}

#[cfg(test)]
pub(crate) fn read_limited_input<R>(
    path: Option<&PathBuf>,
    stdin: &mut R,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    if let Some(path) = path {
        let mut file = open_limited_input_path(path, limit, kind)?;
        return read_limited_open_path(path, &mut file, limit, kind);
    }
    read_limited_stdin(stdin, limit, kind)
}

pub(crate) fn read_limited_stdin<R>(
    stdin: &mut R,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    let mut input = Vec::new();
    stdin
        .take(limit.saturating_add(1))
        .read_to_end(&mut input)
        .map_err(CliError::ReadInput)?;
    reject_oversized_input(input.len(), kind, limit)?;
    Ok(input)
}

pub(crate) fn read_limited_input_file(
    file: &mut InputFile,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError> {
    let path = file.path().to_path_buf();
    if file.len()? > limit {
        return Err(CliError::InputTooLarge { kind, limit });
    }
    read_limited_open_path(&path, file, limit, kind)
}

#[cfg(test)]
pub(crate) fn open_limited_input_path(
    path: &Path,
    limit: u64,
    kind: &'static str,
) -> Result<File, CliError> {
    let metadata = std::fs::metadata(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    if metadata.len() > limit {
        return Err(CliError::InputTooLarge { kind, limit });
    }
    File::open(path).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn read_limited_open_path<R>(
    path: &Path,
    reader: &mut R,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    let mut input = Vec::new();
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut input)
        .map_err(|source| CliError::ReadPath {
            path: path.to_path_buf(),
            source,
        })?;
    reject_oversized_input(input.len(), kind, limit)?;
    Ok(input)
}

fn reject_oversized_input(len: usize, kind: &'static str, limit: u64) -> Result<(), CliError> {
    if u64::try_from(len).unwrap_or(u64::MAX) > limit {
        return Err(CliError::InputTooLarge { kind, limit });
    }
    Ok(())
}

fn read_stream_error(path: Option<&Path>, source: std::io::Error) -> CliError {
    match path {
        Some(path) => CliError::ReadPath {
            path: path.to_path_buf(),
            source,
        },
        None => CliError::ReadInput(source),
    }
}
