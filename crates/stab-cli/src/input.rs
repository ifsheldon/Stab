use std::io::Read;
use std::path::PathBuf;

use crate::CliError;

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
        return read_limited_path(path, limit, kind);
    }
    let mut input = Vec::new();
    stdin
        .take(limit.saturating_add(1))
        .read_to_end(&mut input)
        .map_err(CliError::ReadInput)?;
    reject_oversized_input(input.len(), kind, limit)?;
    Ok(input)
}

fn read_limited_path(path: &PathBuf, limit: u64, kind: &'static str) -> Result<Vec<u8>, CliError> {
    let metadata = std::fs::metadata(path).map_err(|source| CliError::ReadPath {
        path: path.clone(),
        source,
    })?;
    if metadata.len() > limit {
        return Err(CliError::InputTooLarge { kind, limit });
    }
    let mut input = Vec::new();
    std::fs::File::open(path)
        .map_err(|source| CliError::ReadPath {
            path: path.clone(),
            source,
        })?
        .take(limit.saturating_add(1))
        .read_to_end(&mut input)
        .map_err(|source| CliError::ReadPath {
            path: path.clone(),
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
