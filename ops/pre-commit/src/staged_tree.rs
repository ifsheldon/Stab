//! One read-only view of the tree a commit will record.
//!
//! Git hands a pre-commit hook the index it is about to commit through `GIT_INDEX_FILE`: a temporary index for `git commit -a`, `-i`, `-o`, and pathspec commits, and the repository index otherwise.
//! Every staged check reads paths, blobs, and symlink targets through [`StagedTree`] so no check falls back to the working tree by accident.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use gix::bstr::ByteSlice;
use gix::index::entry::{Mode, Stage};
use thiserror::Error;

/// Environment variable Git uses to name the index a hook must read.
pub(crate) const INDEX_FILE_ENV: &str = "GIT_INDEX_FILE";

/// Longest symlink target accepted from a staged blob, matching Linux `PATH_MAX`.
const MAX_SYMLINK_TARGET_BYTES: usize = 4096;

/// Most symlink hops followed while resolving one path, matching Linux `MAXSYMLINKS`.
const MAX_SYMLINK_HOPS: usize = 40;

/// The kind of object a staged index entry records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    /// A regular or executable file blob.
    File,
    /// A symlink whose blob holds the link target.
    Symlink,
    /// A submodule pointer; its contents are not part of this repository's tree.
    Gitlink,
}

impl EntryKind {
    fn from_mode(mode: Mode) -> Option<Self> {
        if mode == Mode::FILE || mode == Mode::FILE_EXECUTABLE {
            Some(Self::File)
        } else if mode == Mode::SYMLINK {
            Some(Self::Symlink)
        } else if mode == Mode::COMMIT {
            Some(Self::Gitlink)
        } else {
            None
        }
    }
}

/// One unconflicted index entry with a repository-relative path made only of normal components.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StagedEntry {
    pub(crate) kind: EntryKind,
    pub(crate) id: gix::ObjectId,
}

/// How a staged path differs from `HEAD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeKind {
    Added,
    Modified,
    Deleted,
    TypeChanged,
}

/// One path whose staged entry differs from `HEAD`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedChange {
    pub(crate) path: PathBuf,
    pub(crate) kind: ChangeKind,
    /// The staged entry, absent for deletions.
    pub(crate) staged: Option<StagedEntry>,
    /// True when either side of the change is a submodule pointer.
    pub(crate) is_submodule: bool,
}

impl StagedChange {
    /// True when the commit will record a file or symlink blob at this path.
    pub(crate) fn has_staged_blob(&self) -> bool {
        self.staged
            .is_some_and(|entry| matches!(entry.kind, EntryKind::File | EntryKind::Symlink))
    }
}

/// Why a staged symlink does not resolve to a staged file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrokenLink {
    Absolute { link: PathBuf },
    NonUtf8 { link: PathBuf },
    TooLong { link: PathBuf },
    EscapesRepository { link: PathBuf },
    TooManyHops,
    ThroughFile { file: PathBuf },
    IntoSubmodule { submodule: PathBuf },
    Missing { target: PathBuf },
}

impl std::fmt::Display for BrokenLink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Absolute { link } => write!(
                formatter,
                "{} has an absolute target, which does not resolve inside the staged tree",
                link.display()
            ),
            Self::NonUtf8 { link } => {
                write!(formatter, "{} has a non-UTF-8 target", link.display())
            }
            Self::TooLong { link } => write!(
                formatter,
                "{} has a target longer than {MAX_SYMLINK_TARGET_BYTES} bytes",
                link.display()
            ),
            Self::EscapesRepository { link } => write!(
                formatter,
                "{} points outside the repository",
                link.display()
            ),
            Self::TooManyHops => write!(
                formatter,
                "more than {MAX_SYMLINK_HOPS} symlink hops while resolving the target"
            ),
            Self::ThroughFile { file } => write!(
                formatter,
                "the target path continues below staged file {}",
                file.display()
            ),
            Self::IntoSubmodule { submodule } => write!(
                formatter,
                "the target lies in submodule {}, outside this repository's staged tree",
                submodule.display()
            ),
            Self::Missing { target } => {
                write!(
                    formatter,
                    "target {} is not a staged file",
                    target.display()
                )
            }
        }
    }
}

/// Outcome of resolving a staged path through staged symlinks.
#[derive(Debug)]
pub(crate) enum ResolveError {
    /// The link chain does not end at a staged file.
    Broken(BrokenLink),
    /// The staged tree could not be read.
    Tree(StagedTreeError),
}

impl From<StagedTreeError> for ResolveError {
    fn from(error: StagedTreeError) -> Self {
        Self::Tree(error)
    }
}

enum BlobStore {
    Repository(Box<gix::Repository>),
    #[cfg(test)]
    Memory(BTreeMap<gix::ObjectId, Vec<u8>>),
}

/// The index a commit will record, compared against `HEAD`, with blob access.
pub(crate) struct StagedTree {
    root: PathBuf,
    object_hash: gix::hash::Kind,
    head: BTreeMap<PathBuf, StagedEntry>,
    entries: BTreeMap<PathBuf, StagedEntry>,
    store: BlobStore,
}

impl StagedTree {
    /// Opens the repository containing `root` and reads the index named by `GIT_INDEX_FILE`, resolved against the current directory when relative, or the repository index when it is unset.
    pub(crate) fn open(root: &Path) -> Result<Self, StagedTreeError> {
        let cwd = std::env::current_dir().map_err(StagedTreeError::CurrentDir)?;
        Self::open_with_index_override(root, std::env::var_os(INDEX_FILE_ENV), &cwd)
    }

    fn open_with_index_override(
        root: &Path,
        index_override: Option<OsString>,
        cwd: &Path,
    ) -> Result<Self, StagedTreeError> {
        let repo = gix::discover(root).map_err(|source| StagedTreeError::Discover {
            path: root.to_path_buf(),
            source: Box::new(source),
        })?;
        let workdir = repo.workdir().ok_or(StagedTreeError::BareRepository)?;
        let canonical_root =
            std::fs::canonicalize(workdir).map_err(|source| StagedTreeError::ResolveRoot {
                path: workdir.to_path_buf(),
                source,
            })?;

        let head_tree_id =
            repo.head_tree_id_or_empty()
                .map_err(|source| StagedTreeError::HeadTree {
                    source: Box::new(source),
                })?;
        let head_index = repo
            .index_from_tree(head_tree_id.as_ref())
            .map_err(|source| StagedTreeError::HeadIndex {
                source: Box::new(source),
            })?;

        let (index_path, explicit) = match index_override.filter(|value| !value.is_empty()) {
            Some(value) => {
                let path = PathBuf::from(value);
                (
                    if path.is_absolute() {
                        path
                    } else {
                        cwd.join(path)
                    },
                    true,
                )
            }
            None => (repo.index_path(), false),
        };
        let index = match gix::index::File::at(
            &index_path,
            repo.object_hash(),
            false,
            gix::index::decode::Options::default(),
        ) {
            Ok(index) => Some(index),
            Err(gix::index::file::init::Error::Io(error))
                if !explicit && error.kind() == std::io::ErrorKind::NotFound =>
            {
                None
            }
            Err(source) => {
                return Err(StagedTreeError::Index {
                    path: index_path,
                    source: Box::new(source),
                });
            }
        };

        let head = collect_entries(&head_index)?;
        let entries = match &index {
            Some(index) => collect_entries(index)?,
            None => BTreeMap::new(),
        };
        Ok(Self {
            root: canonical_root,
            object_hash: repo.object_hash(),
            head,
            entries,
            store: BlobStore::Repository(Box::new(repo)),
        })
    }

    /// Builds a tree from in-memory entries; every blob is stored by its content hash.
    #[cfg(test)]
    pub(crate) fn from_memory(
        root: PathBuf,
        head: &[(&str, EntryKind, &[u8])],
        staged: &[(&str, EntryKind, &[u8])],
    ) -> Self {
        let mut blobs = BTreeMap::new();
        let mut build = |items: &[(&str, EntryKind, &[u8])]| {
            items
                .iter()
                .map(|(path, kind, content)| {
                    let id = gix::objs::compute_hash(
                        gix::hash::Kind::Sha1,
                        gix::objs::Kind::Blob,
                        content,
                    )
                    .expect("hash test blob");
                    blobs.insert(id, content.to_vec());
                    (PathBuf::from(path), StagedEntry { kind: *kind, id })
                })
                .collect::<BTreeMap<_, _>>()
        };
        let head = build(head);
        let entries = build(staged);
        Self {
            root,
            object_hash: gix::hash::Kind::Sha1,
            head,
            entries,
            store: BlobStore::Memory(blobs),
        }
    }

    /// The canonical working-tree root of the repository.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The hash kind object ids in this tree use.
    pub(crate) fn object_hash(&self) -> gix::hash::Kind {
        self.object_hash
    }

    /// Every staged entry in path order.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (&Path, StagedEntry)> {
        self.entries
            .iter()
            .map(|(path, entry)| (path.as_path(), *entry))
    }

    /// Paths whose staged entry differs from `HEAD`, in path order.
    pub(crate) fn changes(&self) -> Vec<StagedChange> {
        let paths = self
            .head
            .keys()
            .chain(self.entries.keys())
            .collect::<BTreeSet<_>>();
        let mut changes = Vec::new();
        for path in paths {
            let old = self.head.get(path);
            let new = self.entries.get(path);
            let kind = match (old, new) {
                (None, Some(_)) => ChangeKind::Added,
                (Some(_), None) => ChangeKind::Deleted,
                (Some(old), Some(new)) if old == new => continue,
                (Some(old), Some(new)) if old.kind != new.kind => ChangeKind::TypeChanged,
                (Some(_), Some(_)) => ChangeKind::Modified,
                (None, None) => continue,
            };
            let is_submodule = [old, new]
                .into_iter()
                .flatten()
                .any(|entry| entry.kind == EntryKind::Gitlink);
            changes.push(StagedChange {
                path: path.clone(),
                kind,
                staged: new.copied(),
                is_submodule,
            });
        }
        changes
    }

    /// Reads the staged blob at `path`, which must be a file or symlink entry.
    pub(crate) fn blob(&self, path: &Path) -> Result<Vec<u8>, StagedTreeError> {
        let entry = self
            .entries
            .get(path)
            .filter(|entry| entry.kind != EntryKind::Gitlink)
            .ok_or_else(|| StagedTreeError::NotABlob {
                path: path.to_path_buf(),
            })?;
        match &self.store {
            BlobStore::Repository(repo) => {
                let mut blob =
                    repo.find_blob(entry.id)
                        .map_err(|source| StagedTreeError::Blob {
                            path: path.to_path_buf(),
                            source: Box::new(source),
                        })?;
                Ok(blob.take_data())
            }
            #[cfg(test)]
            BlobStore::Memory(blobs) => {
                blobs
                    .get(&entry.id)
                    .cloned()
                    .ok_or_else(|| StagedTreeError::NotABlob {
                        path: path.to_path_buf(),
                    })
            }
        }
    }

    /// Follows staged symlinks, including symlinked directories, from `path` to the staged file it names.
    ///
    /// Resolution is lexical over the staged tree, never the working tree: each link's blob is its target, relative targets resolve against the link's directory, and the result must be a staged regular file.
    pub(crate) fn resolve(&self, path: &Path) -> Result<PathBuf, ResolveError> {
        let mut pending = path
            .components()
            .map(|component| component.as_os_str().to_os_string())
            .collect::<VecDeque<_>>();
        let mut resolved = PathBuf::new();
        let mut hops = 0usize;
        while let Some(component) = pending.pop_front() {
            if component == ".." {
                if !resolved.pop() {
                    return Err(ResolveError::Broken(BrokenLink::EscapesRepository {
                        link: path.to_path_buf(),
                    }));
                }
                continue;
            }
            if component == "." || component.is_empty() {
                continue;
            }
            resolved.push(&component);
            let Some(entry) = self.entries.get(&resolved) else {
                continue;
            };
            match entry.kind {
                EntryKind::File if pending.is_empty() => {}
                EntryKind::File => {
                    return Err(ResolveError::Broken(BrokenLink::ThroughFile {
                        file: resolved,
                    }));
                }
                EntryKind::Gitlink => {
                    return Err(ResolveError::Broken(BrokenLink::IntoSubmodule {
                        submodule: resolved,
                    }));
                }
                EntryKind::Symlink => {
                    hops = hops.saturating_add(1);
                    if hops > MAX_SYMLINK_HOPS {
                        return Err(ResolveError::Broken(BrokenLink::TooManyHops));
                    }
                    let target = self.symlink_target(&resolved)?;
                    resolved.pop();
                    for component in target.components().rev() {
                        match component {
                            Component::Normal(name) => pending.push_front(name.to_os_string()),
                            Component::ParentDir => pending.push_front(OsString::from("..")),
                            Component::CurDir => {}
                            Component::RootDir | Component::Prefix(_) => {
                                return Err(ResolveError::Broken(BrokenLink::Absolute {
                                    link: path.to_path_buf(),
                                }));
                            }
                        }
                    }
                }
            }
        }
        match self.entries.get(&resolved) {
            Some(entry) if entry.kind == EntryKind::File => Ok(resolved),
            _ => Err(ResolveError::Broken(BrokenLink::Missing {
                target: resolved,
            })),
        }
    }

    fn symlink_target(&self, link: &Path) -> Result<PathBuf, ResolveError> {
        let bytes = self.blob(link)?;
        if bytes.len() > MAX_SYMLINK_TARGET_BYTES {
            return Err(ResolveError::Broken(BrokenLink::TooLong {
                link: link.to_path_buf(),
            }));
        }
        let target = String::from_utf8(bytes).map_err(|_| {
            ResolveError::Broken(BrokenLink::NonUtf8 {
                link: link.to_path_buf(),
            })
        })?;
        let target = PathBuf::from(target);
        if target.has_root() {
            return Err(ResolveError::Broken(BrokenLink::Absolute {
                link: link.to_path_buf(),
            }));
        }
        Ok(target)
    }
}

fn collect_entries(
    index: &gix::index::State,
) -> Result<BTreeMap<PathBuf, StagedEntry>, StagedTreeError> {
    let mut entries = BTreeMap::new();
    for entry in index.entries() {
        let raw = entry.path(index).as_bytes();
        let path = index_path_to_path_buf(raw)?;
        if entry.stage() != Stage::Unconflicted {
            return Err(StagedTreeError::UnresolvedConflict { path });
        }
        let kind =
            EntryKind::from_mode(entry.mode).ok_or_else(|| StagedTreeError::UnsupportedMode {
                path: path.clone(),
                mode: format!("{:?}", entry.mode),
            })?;
        let staged = StagedEntry { kind, id: entry.id };
        if entries.insert(path.clone(), staged).is_some() {
            return Err(StagedTreeError::DuplicateIndexPath { path });
        }
    }
    Ok(entries)
}

/// Converts an index path to a repository-relative path, rejecting anything other than normal components.
fn index_path_to_path_buf(raw: &[u8]) -> Result<PathBuf, StagedTreeError> {
    let text = std::str::from_utf8(raw).map_err(|source| StagedTreeError::NonUtf8Path {
        source,
        path: raw.to_vec(),
    })?;
    let path = PathBuf::from(text);
    let normal = text
        .split('/')
        .all(|segment| !matches!(segment, "" | "." | ".."));
    if normal {
        Ok(path)
    } else {
        Err(StagedTreeError::UnsafeIndexPath { path })
    }
}

#[derive(Debug, Error)]
pub(crate) enum StagedTreeError {
    #[error("failed to read the current directory: {0}")]
    CurrentDir(std::io::Error),
    #[error("failed to discover Git repository from {path}: {source}")]
    Discover {
        path: PathBuf,
        source: Box<gix::discover::Error>,
    },
    #[error("pre-commit requires a non-bare repository with a worktree")]
    BareRepository,
    #[error("failed to resolve repository root {path}: {source}")]
    ResolveRoot {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to resolve HEAD tree: {source}")]
    HeadTree {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("failed to build HEAD index: {source}")]
    HeadIndex {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("failed to read staged index {path}: {source}")]
    Index {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error(
        "index contains unresolved conflict entry for {path:?}; resolve conflicts before committing"
    )]
    UnresolvedConflict { path: PathBuf },
    #[error("index contains duplicate path {path:?}")]
    DuplicateIndexPath { path: PathBuf },
    #[error("index path is not UTF-8: {path:?}: {source}")]
    NonUtf8Path {
        path: Vec<u8>,
        source: std::str::Utf8Error,
    },
    #[error("index path {path:?} is empty or has a non-normal component")]
    UnsafeIndexPath { path: PathBuf },
    #[error("index entry {path:?} has unsupported mode {mode}")]
    UnsupportedMode { path: PathBuf, mode: String },
    #[error("{path:?} is not a staged file or symlink")]
    NotABlob { path: PathBuf },
    #[error("failed to read staged blob for {path:?}: {source}")]
    Blob {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(staged: &[(&str, EntryKind, &[u8])]) -> StagedTree {
        StagedTree::from_memory(PathBuf::from("/repo"), &[], staged)
    }

    fn broken(tree: &StagedTree, path: &str) -> BrokenLink {
        match tree.resolve(Path::new(path)) {
            Err(ResolveError::Broken(reason)) => reason,
            other => panic!("expected a broken link for {path}, got {other:?}"),
        }
    }

    #[test]
    fn resolve_follows_chains_and_symlinked_directories() {
        let tree = tree(&[
            ("docs/README.md", EntryKind::File, b"readme"),
            ("docs/AGENTS.md", EntryKind::Symlink, b"README.md"),
            ("alias", EntryKind::Symlink, b"docs"),
            ("CLAUDE.md", EntryKind::Symlink, b"alias/./AGENTS.md"),
        ]);

        let resolved = tree.resolve(Path::new("CLAUDE.md")).expect("resolve chain");

        assert_eq!(resolved, PathBuf::from("docs/README.md"));
    }

    #[test]
    fn resolve_rejects_targets_outside_the_staged_tree() {
        let tree = tree(&[
            ("README.md", EntryKind::File, b"readme"),
            ("up", EntryKind::Symlink, b"../README.md"),
            ("abs", EntryKind::Symlink, b"/etc/passwd"),
            ("loop", EntryKind::Symlink, b"loop"),
            ("through", EntryKind::Symlink, b"README.md/x"),
            ("vendor/stim", EntryKind::Gitlink, b"commit"),
            ("sub", EntryKind::Symlink, b"vendor/stim/README.md"),
            ("gone", EntryKind::Symlink, b"AGENTS.md"),
        ]);

        assert!(matches!(
            broken(&tree, "up"),
            BrokenLink::EscapesRepository { .. }
        ));
        assert!(matches!(broken(&tree, "abs"), BrokenLink::Absolute { .. }));
        assert_eq!(broken(&tree, "loop"), BrokenLink::TooManyHops);
        assert!(matches!(
            broken(&tree, "through"),
            BrokenLink::ThroughFile { .. }
        ));
        assert!(matches!(
            broken(&tree, "sub"),
            BrokenLink::IntoSubmodule { .. }
        ));
        assert_eq!(
            broken(&tree, "gone"),
            BrokenLink::Missing {
                target: PathBuf::from("AGENTS.md")
            }
        );
    }

    #[test]
    fn index_paths_with_traversal_components_are_rejected() {
        for raw in [&b"../escape.rs"[..], b"/abs.rs", b"a/./b.rs", b""] {
            assert!(
                matches!(
                    index_path_to_path_buf(raw),
                    Err(StagedTreeError::UnsafeIndexPath { .. })
                ),
                "{raw:?}"
            );
        }
    }
}
