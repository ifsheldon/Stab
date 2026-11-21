//! Rust formatting and lint checks, guarded so they only pass for the content being committed.
//!
//! `cargo fmt` and `cargo clippy` read the working tree, and an export of the index cannot be linted without a second Cargo target directory.
//! The hook therefore keeps the working-tree run and refuses it when any Rust-affecting path in the working tree differs from the index being committed, so a pass always describes the committed content.

use std::collections::BTreeSet;
use std::path::PathBuf;

use ignore::WalkBuilder;
use thiserror::Error;

use crate::paths::is_rust_affecting_path;
use crate::staged_tree::{EntryKind, StagedTree};

/// Environment variable that makes the hook report the Rust tool commands it would run instead of running them.
pub(crate) const DRY_RUN_ENV: &str = "STAB_PRE_COMMIT_DRY_RUN_TOOLS";

/// One external Rust tool invocation the hook runs from the repository root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RustTool {
    pub(crate) name: &'static str,
    pub(crate) program: &'static str,
    pub(crate) args: &'static [&'static str],
}

impl RustTool {
    pub(crate) fn command_line(&self) -> String {
        std::iter::once(self.program)
            .chain(self.args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// The Rust tools run for a staged Rust-affecting change; `--locked` keeps Cargo from rewriting `Cargo.lock` behind a staged manifest change.
pub(crate) const RUST_TOOLS: &[RustTool] = &[
    RustTool {
        name: "rustfmt",
        program: "cargo",
        args: &["fmt", "--all", "--check"],
    },
    RustTool {
        name: "rust clippy",
        program: "cargo",
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--config",
            "build.warnings='deny'",
        ],
    },
];

/// How a Rust-affecting working-tree path differs from the index being committed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum WorktreeDrift {
    /// The working-tree content or file kind differs from the staged entry.
    Modified(PathBuf),
    /// The staged path is absent from the working tree.
    Missing(PathBuf),
    /// The working tree holds a path that is neither staged nor ignored.
    Untracked(PathBuf),
}

impl WorktreeDrift {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Modified(path) => format!("{} has unstaged changes", path.display()),
            Self::Missing(path) => format!("{} is staged but missing on disk", path.display()),
            Self::Untracked(path) => format!("{} is untracked and not ignored", path.display()),
        }
    }
}

#[derive(Debug, Error)]
pub(crate) enum DriftError {
    #[error("failed to inspect {path}: {source}")]
    Inspect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to hash {path}: {message}")]
    Hash { path: PathBuf, message: String },
    #[error("failed to walk the working tree: {0}")]
    Walk(#[from] ignore::Error),
}

/// Lists every Rust-affecting path where the working tree Cargo would read differs from the staged tree.
///
/// Content is compared by raw blob hash without Git clean filters; a filtered path therefore reports drift, which fails safe.
pub(crate) fn worktree_drift(tree: &StagedTree) -> Result<Vec<WorktreeDrift>, DriftError> {
    let root = tree.root();
    let mut drift = Vec::new();
    let mut staged = BTreeSet::new();
    let mut submodules = BTreeSet::new();
    for (path, entry) in tree.entries() {
        match entry.kind {
            EntryKind::Gitlink => {
                submodules.insert(path.to_path_buf());
                continue;
            }
            EntryKind::File | EntryKind::Symlink => {
                staged.insert(path.to_path_buf());
            }
        }
        if !is_rust_affecting_path(path) {
            continue;
        }
        let on_disk = root.join(path);
        let metadata = match std::fs::symlink_metadata(&on_disk) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                drift.push(WorktreeDrift::Missing(path.to_path_buf()));
                continue;
            }
            Err(source) => {
                return Err(DriftError::Inspect {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let content = match entry.kind {
            EntryKind::File if metadata.file_type().is_file() => std::fs::read(&on_disk),
            EntryKind::Symlink if metadata.file_type().is_symlink() => {
                std::fs::read_link(&on_disk).map(path_bytes)
            }
            _ => {
                drift.push(WorktreeDrift::Modified(path.to_path_buf()));
                continue;
            }
        }
        .map_err(|source| DriftError::Inspect {
            path: path.to_path_buf(),
            source,
        })?;
        let id = gix::objs::compute_hash(tree.object_hash(), gix::objs::Kind::Blob, &content)
            .map_err(|error| DriftError::Hash {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        if id != entry.id {
            drift.push(WorktreeDrift::Modified(path.to_path_buf()));
        }
    }

    let walk_root = root.to_path_buf();
    let skip = submodules;
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .follow_links(false)
        .filter_entry(move |entry| {
            let relative = entry
                .path()
                .strip_prefix(&walk_root)
                .unwrap_or(entry.path());
            entry.file_name() != ".git" && !skip.contains(relative)
        })
        .build()
    {
        let entry = entry?;
        if entry.file_type().is_none_or(|kind| kind.is_dir()) {
            continue;
        }
        let Ok(relative) = entry.path().strip_prefix(root) else {
            continue;
        };
        if is_rust_affecting_path(relative) && !staged.contains(relative) {
            drift.push(WorktreeDrift::Untracked(relative.to_path_buf()));
        }
    }
    drift.sort();
    Ok(drift)
}

#[cfg(unix)]
fn path_bytes(path: PathBuf) -> Vec<u8> {
    use std::os::unix::ffi::OsStringExt;
    path.into_os_string().into_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: PathBuf) -> Vec<u8> {
    path.to_string_lossy().replace('\\', "/").into_bytes()
}
