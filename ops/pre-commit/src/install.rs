//! Installation of this binary as the repository's Git pre-commit hook.
//!
//! The hooks directory is resolved the way Git resolves it: `core.hooksPath` when configured, relative to the working-tree root, and otherwise `hooks/` in the common Git directory, which linked worktrees share with the main checkout.
//! An existing hook is replaced only when it is a copy of this tool, recognised by [`HOOK_IDENTITY_MARKER`], or when the caller passes `--force`.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;

/// Identity string compiled into every build of this tool; a hook file that contains it is a previous copy of this tool.
///
/// Builds from before the marker existed do not contain it, so replacing one of those needs `--force` once.
pub(crate) static HOOK_IDENTITY_MARKER: &[u8] =
    b"stab-pre-commit installed-hook identity v1 7c1e0b9a-4f53-4d2e-a8b6-3e95d06f21c4";

const HOOK_NAME: &str = "pre-commit";
const READ_CHUNK_BYTES: usize = 64 * 1024;

/// What an install replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Replaced {
    Nothing,
    PreviousCopy,
    ForeignHook,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallReport {
    pub(crate) hook_path: PathBuf,
    pub(crate) replaced: Replaced,
}

#[derive(Debug, Error)]
pub(crate) enum InstallError {
    #[error("failed to discover Git repository from {path}: {source}")]
    Discover {
        path: PathBuf,
        source: Box<gix::discover::Error>,
    },
    #[error("hook installation requires a non-bare repository with a worktree")]
    BareRepository,
    #[error("failed to interpolate core.hooksPath: {0}")]
    HooksPath(String),
    #[error(
        "{path} exists and is not a stab-pre-commit hook; inspect it and rerun with --force to replace it"
    )]
    ForeignHook { path: PathBuf },
    #[error("{path} is a directory, not a hook file")]
    HookIsDirectory { path: PathBuf },
    #[error("failed to locate the running stab-pre-commit binary: {0}")]
    CurrentExe(std::io::Error),
    #[error("failed to {action} {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Installs the running binary as the pre-commit hook of the repository containing `root`.
pub(crate) fn install(root: &Path, force: bool) -> Result<InstallReport, InstallError> {
    let source = std::env::current_exe().map_err(InstallError::CurrentExe)?;
    let hooks_dir = hooks_directory(root)?;
    install_into(&source, &hooks_dir, force)
}

fn hooks_directory(root: &Path) -> Result<PathBuf, InstallError> {
    let repo = gix::discover(root).map_err(|source| InstallError::Discover {
        path: root.to_path_buf(),
        source: Box::new(source),
    })?;
    let workdir = repo.workdir().ok_or(InstallError::BareRepository)?;
    let configured = repo
        .config_snapshot()
        .trusted_path("core.hooksPath")
        .transpose()
        .map_err(|error| InstallError::HooksPath(error.to_string()))?
        .map(|path| path.into_owned())
        .filter(|path| !path.as_os_str().is_empty());
    Ok(match configured {
        Some(path) if path.is_absolute() => path,
        Some(path) => workdir.join(path),
        None => repo.common_dir().join("hooks"),
    })
}

fn install_into(
    source: &Path,
    hooks_dir: &Path,
    force: bool,
) -> Result<InstallReport, InstallError> {
    let hook_path = hooks_dir.join(HOOK_NAME);
    let replaced = match std::fs::symlink_metadata(&hook_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Replaced::Nothing,
        Err(source) => {
            return Err(io_error("inspect", &hook_path, source));
        }
        Ok(metadata) if metadata.is_dir() => {
            return Err(InstallError::HookIsDirectory { path: hook_path });
        }
        Ok(metadata) if metadata.is_file() && contains_marker(&hook_path)? => {
            Replaced::PreviousCopy
        }
        Ok(_) if force => Replaced::ForeignHook,
        Ok(_) => return Err(InstallError::ForeignHook { path: hook_path }),
    };

    std::fs::create_dir_all(hooks_dir).map_err(|source| io_error("create", hooks_dir, source))?;
    let content = std::fs::read(source).map_err(|error| io_error("read", source, error))?;
    let mut staged = tempfile::Builder::new()
        .prefix(".pre-commit.")
        .suffix(".tmp")
        .tempfile_in(hooks_dir)
        .map_err(|source| io_error("create a temporary file in", hooks_dir, source))?;
    staged
        .write_all(&content)
        .and_then(|()| staged.as_file().sync_all())
        .map_err(|source| io_error("write", staged.path(), source))?;
    set_executable(staged.as_file())
        .map_err(|source| io_error("mark executable", staged.path(), source))?;
    staged
        .persist(&hook_path)
        .map_err(|error| io_error("replace", &hook_path, error.error))?;
    Ok(InstallReport {
        hook_path,
        replaced,
    })
}

#[cfg(unix)]
fn set_executable(file: &std::fs::File) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_executable(_file: &std::fs::File) -> std::io::Result<()> {
    Ok(())
}

/// Streams `path` looking for [`HOOK_IDENTITY_MARKER`] with memory bounded by the chunk size.
fn contains_marker(path: &Path) -> Result<bool, InstallError> {
    let mut file = std::fs::File::open(path).map_err(|source| io_error("open", path, source))?;
    let overlap = HOOK_IDENTITY_MARKER.len().saturating_sub(1);
    let mut window = Vec::with_capacity(READ_CHUNK_BYTES.saturating_add(overlap));
    let mut chunk = vec![0u8; READ_CHUNK_BYTES];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|source| io_error("read", path, source))?;
        let Some(fresh) = chunk.get(..read) else {
            return Ok(false);
        };
        if fresh.is_empty() {
            return Ok(false);
        }
        window.extend_from_slice(fresh);
        if window
            .windows(HOOK_IDENTITY_MARKER.len())
            .any(|candidate| candidate == HOOK_IDENTITY_MARKER)
        {
            return Ok(true);
        }
        let keep_from = window.len().saturating_sub(overlap);
        window.drain(..keep_from);
    }
}

fn io_error(action: &'static str, path: &Path, source: std::io::Error) -> InstallError {
    InstallError::Io {
        action,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_is_found_across_read_chunk_boundaries() {
        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("hook");
        let mut content = vec![b'x'; READ_CHUNK_BYTES - 7];
        content.extend_from_slice(HOOK_IDENTITY_MARKER);
        content.extend_from_slice(b"tail");
        std::fs::write(&path, &content).expect("write hook");
        assert!(contains_marker(&path).expect("scan hook"));

        let truncated = content
            .get(..READ_CHUNK_BYTES + 3)
            .expect("truncated prefix")
            .to_vec();
        std::fs::write(&path, truncated).expect("write truncated hook");
        assert!(!contains_marker(&path).expect("scan truncated hook"));
    }
}
