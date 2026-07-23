use std::io::{BufRead, Read};
use std::path::Path;

#[cfg(test)]
use std::fs::File;
#[cfg(test)]
use std::path::PathBuf;

use crate::CliError;
use crate::io_plan::InputFile;

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

pub(crate) fn read_limited_line<R>(
    reader: &mut R,
    path: Option<&Path>,
    limit: usize,
    kind: &'static str,
) -> Result<Option<Vec<u8>>, CliError>
where
    R: BufRead + ?Sized,
{
    let mut line = Vec::new();
    loop {
        let (consumed, found_newline) = {
            let available = reader
                .fill_buf()
                .map_err(|source| read_stream_error(path, source))?;
            if available.is_empty() {
                return if line.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(line))
                };
            }
            let consumed = available
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(available.len(), |index| index + 1);
            if line
                .len()
                .checked_add(consumed)
                .is_none_or(|length| length > limit)
            {
                return Err(CliError::InputTooLarge {
                    kind,
                    limit: u64::try_from(limit).unwrap_or(u64::MAX),
                });
            }
            let chunk = available.get(..consumed).ok_or_else(|| {
                CliError::from(stab_core::CircuitError::InvalidResultFormat {
                    message: format!("{kind} byte range was out of bounds"),
                })
            })?;
            line.extend_from_slice(chunk);
            (consumed, chunk.last() == Some(&b'\n'))
        };
        reader.consume(consumed);
        if found_newline {
            return Ok(Some(line));
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
    use std::io::Cursor;

    use super::*;

    #[test]
    fn limited_line_rejects_before_consuming_an_over_limit_chunk() {
        let mut reader = Cursor::new(vec![b'0'; 17]);
        let error =
            read_limited_line(&mut reader, None, 16, "bounded line").expect_err("over-limit line");

        assert!(matches!(
            error,
            CliError::InputTooLarge {
                kind: "bounded line",
                limit: 16
            }
        ));
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.fill_buf().expect("remaining buffer").len(), 17);
    }

    #[test]
    fn limited_line_accumulates_bounded_chunks_and_preserves_unterminated_eof() {
        let input = Cursor::new(b"012345\nabc".to_vec());
        let mut reader = std::io::BufReader::with_capacity(3, input);

        assert_eq!(
            read_limited_line(&mut reader, None, 7, "bounded line").expect("first line"),
            Some(b"012345\n".to_vec())
        );
        assert_eq!(
            read_limited_line(&mut reader, None, 7, "bounded line").expect("EOF line"),
            Some(b"abc".to_vec())
        );
        assert_eq!(
            read_limited_line(&mut reader, None, 7, "bounded line").expect("EOF"),
            None
        );
    }
}
