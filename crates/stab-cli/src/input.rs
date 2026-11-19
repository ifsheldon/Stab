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
    let input = read_bounded(stdin, limit, 0).map_err(CliError::ReadInput)?;
    reject_oversized_input(input.len(), kind, limit)?;
    Ok(input)
}

pub(crate) fn read_limited_input_file(
    file: &mut InputFile,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError> {
    let path = file.path().to_path_buf();
    let len = file.len()?;
    if len > limit {
        return Err(CliError::InputTooLarge { kind, limit });
    }
    read_limited_open_path_with_hint(&path, file.file_mut(), limit, kind, len)
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

#[cfg(test)]
pub(crate) fn read_limited_open_path<R>(
    path: &Path,
    reader: &mut R,
    limit: u64,
    kind: &'static str,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    read_limited_open_path_with_hint(path, reader, limit, kind, 0)
}

fn read_limited_open_path_with_hint<R>(
    path: &Path,
    reader: &mut R,
    limit: u64,
    kind: &'static str,
    size_hint: u64,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    let input = read_bounded(reader, limit, size_hint).map_err(|source| CliError::ReadPath {
        path: path.to_path_buf(),
        source,
    })?;
    reject_oversized_input(input.len(), kind, limit)?;
    Ok(input)
}

/// Reads at most `limit + 1` bytes with fallible reservation, so an input admitted by the
/// memory budget never aborts on allocation failure. `size_hint` (a file's length, already
/// admitted) is reserved exactly up front; otherwise storage grows geometrically and is capped at
/// `limit + 1` bytes.
///
/// Reads land directly in reserved capacity without zero-filling it first, and a full buffer is
/// probed for end of input through a small stack buffer, so an input that exactly matches its
/// hint is never copied into a larger allocation.
fn read_bounded<R>(reader: &mut R, limit: u64, size_hint: u64) -> std::io::Result<Vec<u8>>
where
    R: Read,
{
    const CHUNK_BYTES: usize = 64 * 1024;
    const PROBE_BYTES: usize = 64;
    let cap = usize::try_from(limit.saturating_add(1)).unwrap_or(usize::MAX);
    let out_of_memory = |error: std::collections::TryReserveError| {
        std::io::Error::new(std::io::ErrorKind::OutOfMemory, error)
    };
    let mut input = Vec::new();
    let hint = usize::try_from(size_hint).unwrap_or(usize::MAX).min(cap);
    input.try_reserve_exact(hint).map_err(out_of_memory)?;
    loop {
        if input.len() >= cap {
            return Ok(input);
        }
        if input.len() == input.capacity() {
            let mut probe = [0_u8; PROBE_BYTES];
            let probe_len = PROBE_BYTES.min(cap - input.len());
            let window = probe.get_mut(..probe_len).unwrap_or_default();
            let read = loop {
                match reader.read(window) {
                    Ok(read) => break read,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error),
                }
            };
            if read == 0 {
                return Ok(input);
            }
            let probed = window.get(..read).ok_or_else(|| {
                std::io::Error::other("input reader reported more bytes than its buffer holds")
            })?;
            // `grown` already doubles the buffer, and an amortized reservation would double it
            // again past `cap`, so the reservation is exact.
            let grown = input.capacity().max(CHUNK_BYTES).min(cap - input.len());
            input.try_reserve_exact(grown).map_err(out_of_memory)?;
            input.extend_from_slice(probed);
            continue;
        }
        // `Take` bounds the read to the spare capacity, so `read_to_end` never grows `input`.
        let window = (input.capacity() - input.len()).min(cap - input.len());
        let read = reader
            .by_ref()
            .take(u64::try_from(window).unwrap_or(u64::MAX))
            .read_to_end(&mut input)?;
        if read < window {
            return Ok(input);
        }
    }
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

#[cfg(test)]
mod tests {
    use super::read_bounded;

    /// An input one byte past the cap, read without a size hint so storage grows geometrically,
    /// must never reserve more than the `limit + 1` bytes the budget admitted, including on the
    /// last growth step that the cap truncates.
    #[test]
    fn bounded_read_never_reserves_past_its_cap() {
        for limit in [0_u64, 63, 64 * 1024, 100_000, 3 * 64 * 1024 + 17] {
            let cap = usize::try_from(limit + 1).expect("small cap");
            let source = vec![0xA5_u8; cap + 1];
            let input = read_bounded(&mut source.as_slice(), limit, 0).expect("bounded read");
            assert_eq!(input.len(), cap, "limit {limit}");
            assert!(
                input.capacity() <= cap,
                "limit {limit}: reserved {} bytes past a {cap}-byte cap",
                input.capacity()
            );
        }
    }
}
