//! Which files the Markdown check treats as repository-owned.
//!
//! In a Git checkout the inventory is the index: only tracked Markdown files
//! are checked, and local link targets must be tracked too, so the result is
//! the same on a developer machine with untracked notes or build output as on
//! a clean CI checkout. Submodule gitlinks count as tracked trees whose
//! contents the submodule owns. A directory with no `.git` entry is checked as
//! a plain tree with the filesystem walker in the parent module; a `.git`
//! entry that cannot be opened as a repository is an error, never a silent
//! fallback.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::index::entry::Mode;

use super::{DocsCheckError, is_markdown_path};

#[derive(Debug)]
pub(super) enum Inventory {
    Git(TrackedTree),
    Filesystem,
}

#[derive(Debug, Default)]
pub(super) struct TrackedTree {
    blobs: BTreeSet<PathBuf>,
    gitlinks: BTreeSet<PathBuf>,
}

impl Inventory {
    pub(super) fn load(root: &Path) -> Result<Self, DocsCheckError> {
        if root.join(".git").symlink_metadata().is_err() {
            return Ok(Self::Filesystem);
        }
        let repository = gix::open(root).map_err(|source| DocsCheckError::OpenRepository {
            path: root.to_path_buf(),
            source: Box::new(source),
        })?;
        let index = repository
            .index_or_empty()
            .map_err(|source| DocsCheckError::ReadIndex {
                path: root.to_path_buf(),
                source: Box::new(source),
            })?;
        let mut tree = TrackedTree::default();
        for entry in index.entries() {
            let raw = entry.path(&index);
            let path = PathBuf::from(raw.to_str().map_err(|_| DocsCheckError::NonUtf8Path {
                path: raw.to_str_lossy().into_owned(),
            })?);
            if entry.mode == Mode::COMMIT {
                tree.gitlinks.insert(path);
            } else {
                tree.blobs.insert(path);
            }
        }
        Ok(Self::Git(tree))
    }
}

impl TrackedTree {
    pub(super) fn markdown_paths(&self) -> Vec<PathBuf> {
        self.blobs
            .iter()
            .filter(|path| is_markdown_path(path))
            .cloned()
            .collect()
    }

    /// A target is tracked when it is a tracked file, a directory containing
    /// one, or lies inside a submodule.
    pub(super) fn contains(&self, target: &Path) -> bool {
        if self.blobs.contains(target) || self.gitlinks.contains(target) {
            return true;
        }
        let has_tracked_descendant = |set: &BTreeSet<PathBuf>| {
            set.range(target.to_path_buf()..)
                .next()
                .is_some_and(|path| path.starts_with(target))
        };
        has_tracked_descendant(&self.blobs)
            || has_tracked_descendant(&self.gitlinks)
            || target
                .ancestors()
                .skip(1)
                .any(|ancestor| self.gitlinks.contains(ancestor))
    }
}

pub(super) fn untracked_target_error(target: &Path) -> (&'static str, String) {
    (
        "markdown-untracked-target",
        format!(
            "local target {} exists but is not tracked by Git, so the link breaks on a clean checkout",
            target.display()
        ),
    )
}
