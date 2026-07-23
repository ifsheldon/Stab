use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::mem::MaybeUninit;
use std::os::unix::ffi::OsStringExt as _;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

use super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::error::BenchError;
use crate::root::RepoRoot;

const GIT: &str = "/usr/bin/git";
const MAX_GIT_FILE_BYTES: u64 = 512 << 20;
const MAX_GIT_OUTPUT_BYTES: usize = 4 << 20;
const MAX_GIT_STDERR_BYTES: usize = 64 << 10;
const MAX_GIT_DIRECTORY_ENTRIES: usize = 4096;
const MAX_SHARED_INDEX_FILES: usize = 256;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryState {
    pub(super) commit: String,
    pub(super) local_modifications: bool,
}

pub(super) fn repository_state(root: &RepoRoot) -> Result<RepositoryState, GitError> {
    GitView::open(root)?.state()
}

pub(super) fn validate_pinned_stim(root: &RepoRoot) -> Result<(), GitError> {
    let stim_root = open_nested_worktree(root, &root.default_stim_source())?;
    let view = GitView::open(&stim_root)?;
    let state = view.state()?;
    if state.commit != STIM_COMMIT {
        return Err(GitError::WrongStimCommit {
            actual: state.commit,
            expected: STIM_COMMIT,
        });
    }
    let tag = view.command_text(&["describe", "--tags", "--exact-match", STIM_COMMIT], None)?;
    if tag != STIM_TAG {
        return Err(GitError::WrongStimTag {
            actual: tag,
            expected: STIM_TAG,
        });
    }
    if state.local_modifications {
        return Err(GitError::DirtyStim);
    }
    Ok(())
}

pub(super) fn materialize_repository_commit(
    root: &RepoRoot,
    expected_commit: &str,
    destination: &Path,
) -> Result<(), GitError> {
    materialize_worktree_commit(root, &root.path, expected_commit, destination)
}

pub(super) fn materialize_worktree_commit(
    root: &RepoRoot,
    worktree: &Path,
    expected_commit: &str,
    destination: &Path,
) -> Result<(), GitError> {
    let worktree = open_nested_worktree(root, worktree)?;
    let view = GitView::open(&worktree)?;
    if view.commit != expected_commit {
        return Err(GitError::RepositoryCommitChanged {
            actual: view.commit,
            expected: expected_commit.to_string(),
        });
    }
    let destination = std::fs::canonicalize(destination).map_err(|source| GitError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    if std::fs::read_dir(&destination)
        .map_err(|source| GitError::Io {
            path: destination.clone(),
            source,
        })?
        .next()
        .is_some()
    {
        return Err(GitError::NonemptyMaterialization(destination));
    }
    let destination = destination
        .to_str()
        .ok_or_else(|| GitError::NonUtf8Materialization(destination.clone()))?;
    let prefix = format!("--prefix={destination}/");
    view.command(
        &["checkout-index", "--all", "--force", &prefix],
        Some(&view.head_index),
        false,
    )?;
    Ok(())
}

struct GitView {
    _temporary: tempfile::TempDir,
    worktree: RepoRoot,
    _source_directories: Vec<RepoRoot>,
    git_dir: PathBuf,
    head_index: PathBuf,
    staged_index: PathBuf,
    scratch: PathBuf,
    commit: String,
}

impl GitView {
    fn open(worktree: &RepoRoot) -> Result<Self, GitError> {
        ensure_git()?;
        if !worktree.path.is_absolute() {
            return Err(GitError::UnsafeGitDirectory(worktree.path.clone()));
        }
        let worktree = worktree.clone();
        let source_git_dir = resolve_git_dir(&worktree)?;
        let common_dir = resolve_common_dir(&source_git_dir)?;
        let temporary = tempfile::Builder::new()
            .prefix("stab-pq1-git-view-")
            .tempdir()
            .map_err(GitError::Scratch)?;
        let git_dir = temporary.path().join("git");
        let scratch = temporary.path().join("home");
        create_directory(&git_dir)?;
        create_directory(&scratch)?;
        copy_git_file(&source_git_dir, "HEAD", &git_dir.join("HEAD"), true)?;
        copy_git_file(
            &common_dir,
            "packed-refs",
            &git_dir.join("packed-refs"),
            false,
        )?;
        copy_git_file(&common_dir, "shallow", &git_dir.join("shallow"), false)?;
        let objects = link_git_directory(&common_dir, "objects", &git_dir.join("objects"))?;
        let refs = link_git_directory(&common_dir, "refs", &git_dir.join("refs"))?;
        let head_index = git_dir.join("head-index");
        let staged_index = git_dir.join("staged-index");
        copy_git_file(&source_git_dir, "index", &staged_index, true)?;
        copy_shared_indexes(&source_git_dir, &git_dir)?;

        let mut view = Self {
            _temporary: temporary,
            worktree,
            _source_directories: vec![source_git_dir, common_dir, objects, refs],
            git_dir,
            head_index,
            staged_index,
            scratch,
            commit: String::new(),
        };
        let commit = view.command_text(&["rev-parse", "--verify", "HEAD^{commit}"], None)?;
        if !valid_commit(&commit) {
            return Err(GitError::InvalidCommit(commit));
        }
        view.commit = commit.to_ascii_lowercase();
        view.command(&["read-tree", &view.commit], Some(&view.head_index), false)?;
        Ok(view)
    }

    fn state(&self) -> Result<RepositoryState, GitError> {
        let refresh = self.command(
            &["update-index", "--really-refresh", "--ignore-submodules"],
            Some(&self.head_index),
            true,
        )?;
        let tracked = self.command(
            &[
                "diff-index",
                "--name-only",
                "-z",
                "--no-renames",
                "--ignore-submodules=none",
                &self.commit,
                "--",
            ],
            Some(&self.head_index),
            false,
        )?;
        let staged = self.command(
            &[
                "diff-index",
                "--cached",
                "--name-only",
                "-z",
                "--no-renames",
                "--ignore-submodules=none",
                &self.commit,
                "--",
            ],
            Some(&self.staged_index),
            false,
        )?;
        let untracked = self.command(
            &[
                "ls-files",
                "--others",
                "--exclude-per-directory=.gitignore",
                "-z",
                "--",
            ],
            Some(&self.head_index),
            false,
        )?;
        Ok(RepositoryState {
            commit: self.commit.clone(),
            local_modifications: refresh.status == Some(1)
                || !tracked.stdout.is_empty()
                || !staged.stdout.is_empty()
                || !untracked.stdout.is_empty(),
        })
    }

    fn command_text(&self, arguments: &[&str], index: Option<&Path>) -> Result<String, GitError> {
        let output = self.command(arguments, index, false)?;
        let text = std::str::from_utf8(&output.stdout).map_err(GitError::Utf8)?;
        Ok(text.trim_end_matches(['\r', '\n']).to_string())
    }

    fn command(
        &self,
        arguments: &[&str],
        index: Option<&Path>,
        allow_status_one: bool,
    ) -> Result<ProcessResult, GitError> {
        let mut args = vec![
            OsString::from("--no-optional-locks"),
            OsString::from("--git-dir"),
            self.git_dir.as_os_str().to_owned(),
            OsString::from("--work-tree"),
            self.worktree.path.as_os_str().to_owned(),
            OsString::from("-c"),
            OsString::from("core.fsmonitor=false"),
            OsString::from("-c"),
            OsString::from("core.untrackedCache=false"),
            OsString::from("-c"),
            OsString::from("core.excludesFile=/dev/null"),
            OsString::from("-c"),
            OsString::from("diff.external="),
            OsString::from("-c"),
            OsString::from("submodule.recurse=false"),
        ];
        args.extend(arguments.iter().map(OsString::from));
        let mut environment = controlled_environment(&self.scratch);
        if let Some(index) = index {
            environment.push((
                OsString::from("GIT_INDEX_FILE"),
                index.as_os_str().to_owned(),
            ));
        }
        let output = run_bounded_process(&ProcessRequest {
            program: PathBuf::from(GIT),
            args,
            stdin: Vec::new(),
            working_directory: self.worktree.path.clone(),
            environment: environment.into(),
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 0,
                stdout: (MAX_GIT_OUTPUT_BYTES).into(),
                stderr: (MAX_GIT_STDERR_BYTES).into(),
                regular_file_bytes: None,
                timeout: GIT_TIMEOUT,
            },
        })?;
        let accepted = output.status == Some(0) || allow_status_one && output.status == Some(1);
        if !accepted || output.status == Some(0) && !output.stderr.is_empty() {
            return Err(GitError::Command {
                arguments: arguments.join(" "),
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }
        Ok(output)
    }
}

fn ensure_git() -> Result<(), GitError> {
    let path = PathBuf::from(GIT);
    let metadata = std::fs::metadata(&path).map_err(|source| GitError::Io {
        path: path.clone(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(GitError::MissingGit(path));
    }
    Ok(())
}

fn controlled_environment(scratch: &Path) -> Vec<(OsString, OsString)> {
    vec![
        (OsString::from("HOME"), scratch.as_os_str().to_owned()),
        (
            OsString::from("XDG_CONFIG_HOME"),
            scratch.as_os_str().to_owned(),
        ),
        (OsString::from("LANG"), OsString::from("C")),
        (OsString::from("LC_ALL"), OsString::from("C")),
        (OsString::from("PATH"), OsString::from("/usr/bin:/bin")),
        (OsString::from("GIT_CONFIG_NOSYSTEM"), OsString::from("1")),
        (
            OsString::from("GIT_CONFIG_GLOBAL"),
            OsString::from("/dev/null"),
        ),
        (
            OsString::from("GIT_NO_REPLACE_OBJECTS"),
            OsString::from("1"),
        ),
        (OsString::from("GIT_OPTIONAL_LOCKS"), OsString::from("0")),
        (OsString::from("GIT_TERMINAL_PROMPT"), OsString::from("0")),
        (OsString::from("GIT_LITERAL_PATHSPECS"), OsString::from("1")),
    ]
}

fn create_directory(path: &Path) -> Result<(), GitError> {
    std::fs::create_dir(path).map_err(|source| GitError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn open_nested_worktree(root: &RepoRoot, path: &Path) -> Result<RepoRoot, GitError> {
    if path == root.path {
        return Ok(root.clone());
    }
    let descriptor = crate::source_file::open_repo_directory_descriptor(root, path)
        .map_err(|error| map_source_error(path, error))?;
    Ok(RepoRoot::from_retained_descriptor(descriptor))
}

fn resolve_git_dir(worktree: &RepoRoot) -> Result<RepoRoot, GitError> {
    let marker = worktree.path.join(".git");
    match crate::source_file::open_repo_directory_descriptor(worktree, &marker) {
        Ok(descriptor) => return Ok(RepoRoot::from_retained_descriptor(descriptor)),
        Err(BenchError::SourceInputIo { source, .. })
            if source.kind() == std::io::ErrorKind::NotADirectory => {}
        Err(error) => return Err(map_git_marker_error(&marker, error)),
    }
    let text = read_git_marker_text(worktree, OsStr::new(".git"), 4096)?;
    let relative = text
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or_else(|| GitError::MalformedGitMarker(marker.clone()))?;
    let path = PathBuf::from(relative);
    let display = if path.is_absolute() {
        path.clone()
    } else {
        worktree.path.join(&path)
    };
    open_git_directory_reference(worktree, &path, &display)
}

fn resolve_common_dir(git_dir: &RepoRoot) -> Result<RepoRoot, GitError> {
    let marker = git_dir.path.join("commondir");
    let bytes = match read_git_marker_file(git_dir, OsStr::new("commondir"), 4096) {
        Ok(bytes) => bytes,
        Err(GitError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(git_dir.clone());
        }
        Err(error) => return Err(error),
    };
    let text = String::from_utf8(bytes).map_err(|source| GitError::NonUtf8File {
        path: marker.clone(),
        source,
    })?;
    let path = PathBuf::from(text.trim());
    let display = if path.is_absolute() {
        path.clone()
    } else {
        git_dir.path.join(&path)
    };
    open_git_directory_reference(git_dir, &path, &display)
}

fn read_git_marker_text(root: &RepoRoot, name: &OsStr, maximum: u64) -> Result<String, GitError> {
    let path = root.path.join(name);
    let bytes = read_git_marker_file(root, name, maximum)?;
    String::from_utf8(bytes).map_err(|source| GitError::NonUtf8File { path, source })
}

fn read_git_marker_file(root: &RepoRoot, name: &OsStr, maximum: u64) -> Result<Vec<u8>, GitError> {
    let path = root.path.join(name);
    let maximum = usize::try_from(maximum).map_err(|_| GitError::UnsafeGitMarker(path.clone()))?;
    crate::source_file::read_repo_regular_file_bounded(root, &path, maximum)
        .map_err(|error| map_git_marker_error(&path, error))
}

fn read_git_file(root: &RepoRoot, name: &OsStr, maximum: u64) -> Result<Vec<u8>, GitError> {
    let path = root.path.join(name);
    let maximum = usize::try_from(maximum).map_err(|_| GitError::UnsafeFile {
        path: path.clone(),
        reason: "file size limit does not fit usize".to_string(),
    })?;
    crate::source_file::read_repo_regular_file_bounded(root, &path, maximum)
        .map_err(|error| map_source_error(&path, error))
}

fn copy_git_file(
    source_root: &RepoRoot,
    name: &str,
    destination: &Path,
    required: bool,
) -> Result<(), GitError> {
    let bytes = match read_git_file(source_root, OsStr::new(name), MAX_GIT_FILE_BYTES) {
        Ok(bytes) => bytes,
        Err(GitError::Io { source, .. })
            if !required && source.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(());
        }
        Err(error) => return Err(error),
    };
    let mut output = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(destination)
        .map_err(|source| GitError::Io {
            path: destination.to_path_buf(),
            source,
        })?;
    output.write_all(&bytes).map_err(|source| GitError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    output.sync_all().map_err(|source| GitError::Io {
        path: destination.to_path_buf(),
        source,
    })
}

fn copy_shared_indexes(source_git_dir: &RepoRoot, destination: &Path) -> Result<(), GitError> {
    copy_shared_indexes_bounded(
        source_git_dir,
        destination,
        MAX_GIT_DIRECTORY_ENTRIES,
        MAX_SHARED_INDEX_FILES,
    )
}

fn copy_shared_indexes_bounded(
    source_git_dir: &RepoRoot,
    destination: &Path,
    maximum_entries: usize,
    maximum_shared_indexes: usize,
) -> Result<(), GitError> {
    let descriptor =
        crate::source_file::duplicate_repo_root_descriptor(source_git_dir, &source_git_dir.path)
            .map_err(|error| map_source_error(&source_git_dir.path, error))?;
    let mut buffer = [MaybeUninit::uninit(); 8192];
    let mut entries = rustix::fs::RawDir::new(descriptor, &mut buffer);
    let mut entry_count = 0_usize;
    let mut count = 0_usize;
    while let Some(entry) = entries.next() {
        let entry = entry.map_err(|source| GitError::Io {
            path: source_git_dir.path.clone(),
            source: source.into(),
        })?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entry_count = entry_count
            .checked_add(1)
            .ok_or(GitError::TooManyGitDirectoryEntries)?;
        if entry_count > maximum_entries {
            return Err(GitError::TooManyGitDirectoryEntries);
        }
        let name = OsString::from_vec(name.to_vec());
        if !is_shared_index_name(&name) {
            continue;
        }
        count = count.checked_add(1).ok_or(GitError::TooManySharedIndexes)?;
        if count > maximum_shared_indexes {
            return Err(GitError::TooManySharedIndexes);
        }
        let name_text = name.to_str().ok_or_else(|| GitError::UnsafeFile {
            path: source_git_dir.path.join(&name),
            reason: "shared index name is not UTF-8".to_string(),
        })?;
        copy_git_file(source_git_dir, name_text, &destination.join(&name), true)?;
    }
    Ok(())
}

fn is_shared_index_name(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    let Some(digest) = name.strip_prefix("sharedindex.") else {
        return false;
    };
    valid_commit(digest)
}

fn link_git_directory(
    source_root: &RepoRoot,
    name: &str,
    destination: &Path,
) -> Result<RepoRoot, GitError> {
    let source = source_root.path.join(name);
    let descriptor = crate::source_file::open_repo_directory_descriptor(source_root, &source)
        .map_err(|error| map_source_error(&source, error))?;
    let retained = RepoRoot::from_retained_descriptor(descriptor);
    std::os::unix::fs::symlink(&retained.path, destination).map_err(|source| GitError::Io {
        path: destination.to_path_buf(),
        source,
    })?;
    Ok(retained)
}

fn open_git_directory_reference(
    base: &RepoRoot,
    reference: &Path,
    display: &Path,
) -> Result<RepoRoot, GitError> {
    let mut directory = if reference.is_absolute() {
        rustix::fs::open("/", directory_flags(), rustix::fs::Mode::empty()).map_err(|source| {
            GitError::Io {
                path: display.to_path_buf(),
                source: source.into(),
            }
        })?
    } else {
        crate::source_file::duplicate_repo_root_descriptor(base, display)
            .map_err(|error| map_source_error(display, error))?
    };
    for component in reference.components() {
        let name = match component {
            Component::Prefix(_) => {
                return Err(GitError::UnsafeGitDirectory(display.to_path_buf()));
            }
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => OsStr::new(".."),
            Component::Normal(name) => name,
        };
        directory = rustix::fs::openat(
            &directory,
            name,
            directory_flags(),
            rustix::fs::Mode::empty(),
        )
        .map_err(|source| GitError::Io {
            path: display.to_path_buf(),
            source: source.into(),
        })?;
    }
    Ok(RepoRoot::from_retained_descriptor(directory))
}

fn map_git_marker_error(path: &Path, error: BenchError) -> GitError {
    match error {
        BenchError::SourceInputIo { source, .. }
            if source.raw_os_error() == Some(rustix::io::Errno::LOOP.raw_os_error()) =>
        {
            GitError::UnsafeGitMarker(path.to_path_buf())
        }
        BenchError::SourceInput(_) => GitError::UnsafeGitMarker(path.to_path_buf()),
        other => map_source_error(path, other),
    }
}

fn map_source_error(path: &Path, error: BenchError) -> GitError {
    match error {
        BenchError::SourceInputIo { source, .. } => GitError::Io {
            path: path.to_path_buf(),
            source,
        },
        other => GitError::UnsafeFile {
            path: path.to_path_buf(),
            reason: other.to_string(),
        },
    }
}

fn directory_flags() -> rustix::fs::OFlags {
    rustix::fs::OFlags::RDONLY
        | rustix::fs::OFlags::CLOEXEC
        | rustix::fs::OFlags::DIRECTORY
        | rustix::fs::OFlags::NOFOLLOW
}

fn valid_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Debug, Error)]
pub(super) enum GitError {
    #[error("required Git executable is missing: {0}")]
    MissingGit(PathBuf),
    #[error("failed to create a private Git view: {0}")]
    Scratch(std::io::Error),
    #[error("Git metadata path {path} could not be accessed: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Git metadata path is unsafe: {0}")]
    UnsafeGitMarker(PathBuf),
    #[error("Git directory marker is malformed: {0}")]
    MalformedGitMarker(PathBuf),
    #[error("Git metadata file {path} is unsafe: {reason}")]
    UnsafeFile { path: PathBuf, reason: String },
    #[error("Git metadata file {path} is not UTF-8: {source}")]
    NonUtf8File {
        path: PathBuf,
        source: std::string::FromUtf8Error,
    },
    #[error("Git directory is missing or unsafe: {0}")]
    UnsafeGitDirectory(PathBuf),
    #[error("Git metadata contains too many shared indexes")]
    TooManySharedIndexes,
    #[error("Git metadata directory contains too many entries")]
    TooManyGitDirectoryEntries,
    #[error("Git command {arguments:?} failed with status {status:?}: {stderr}")]
    Command {
        arguments: String,
        status: Option<i32>,
        stderr: String,
    },
    #[error("Git command output is not UTF-8: {0}")]
    Utf8(std::str::Utf8Error),
    #[error("Git returned invalid commit {0:?}")]
    InvalidCommit(String),
    #[error("pinned Stim commit is {actual}, expected {expected}")]
    WrongStimCommit {
        actual: String,
        expected: &'static str,
    },
    #[error("pinned Stim tag is {actual}, expected {expected}")]
    WrongStimTag {
        actual: String,
        expected: &'static str,
    },
    #[error("pinned Stim checkout has local modifications")]
    DirtyStim,
    #[error("repository commit changed before materialization: {actual}, expected {expected}")]
    RepositoryCommitChanged { actual: String, expected: String },
    #[error("repository materialization directory is not empty: {0}")]
    NonemptyMaterialization(PathBuf),
    #[error("repository materialization path is not UTF-8: {0}")]
    NonUtf8Materialization(PathBuf),
    #[error(transparent)]
    Process(#[from] super::process::ProcessError),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_git(repository: &Path, arguments: &[&str]) -> String {
        let output = std::process::Command::new(GIT)
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .output()
            .expect("run test Git command");
        assert!(
            output.status.success(),
            "Git {:?} failed: {}",
            arguments,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git output UTF-8")
            .trim()
            .to_string()
    }

    fn initialized_repository() -> (tempfile::TempDir, RepoRoot) {
        let repository = tempfile::tempdir().expect("temporary repository");
        test_git(repository.path(), &["init", "--quiet"]);
        test_git(repository.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            repository.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        std::fs::write(repository.path().join(".gitignore"), "ignored/\n")
            .expect("write ignore policy");
        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("write tracked file");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "initial"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        (repository, root)
    }

    #[test]
    fn descriptor_backed_git_view_supports_linked_worktrees() {
        let (repository, _) = initialized_repository();
        let parent = tempfile::tempdir().expect("temporary worktree parent");
        let worktree = parent.path().join("linked-worktree");
        let worktree_text = worktree.to_str().expect("UTF-8 worktree path");
        test_git(
            repository.path(),
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                worktree_text,
                "HEAD",
            ],
        );
        let expected = test_git(repository.path(), &["rev-parse", "HEAD"]);
        let root = RepoRoot::resolve(&worktree).expect("resolve linked worktree");
        let repository_binding =
            super::super::artifact::RepositoryBinding::open(&root).expect("bind worktree");
        let retained = repository_binding
            .descriptor_root(&root)
            .expect("retain worktree root");

        let state = repository_state(&retained).expect("audit linked worktree");
        assert_eq!(state.commit, expected);
        assert!(!state.local_modifications);

        drop(retained);
        drop(repository_binding);
        test_git(
            repository.path(),
            &["worktree", "remove", "--force", worktree_text],
        );
    }

    #[cfg(unix)]
    #[test]
    fn repository_state_rejects_symlink_git_marker() {
        let (repository, root) = initialized_repository();
        let marker = repository.path().join(".git");
        let retained_git = repository.path().join("retained-git");
        std::fs::rename(&marker, &retained_git).expect("move Git directory");
        std::os::unix::fs::symlink(&retained_git, &marker).expect("create Git marker symlink");

        let error = repository_state(&root).expect_err("symlink Git marker must fail");

        assert!(matches!(error, GitError::UnsafeGitMarker(path) if path == marker));
    }

    #[test]
    fn shared_index_scan_enforces_directory_entry_limit() {
        let source = tempfile::tempdir().expect("temporary Git directory");
        let destination = tempfile::tempdir().expect("temporary private Git directory");
        std::fs::write(source.path().join("first"), b"").expect("write first entry");
        std::fs::write(source.path().join("second"), b"").expect("write second entry");
        let source_root = RepoRoot::resolve(source.path()).expect("resolve Git directory");

        let error = copy_shared_indexes_bounded(&source_root, destination.path(), 1, usize::MAX)
            .expect_err("directory entry limit must fail closed");

        assert!(matches!(error, GitError::TooManyGitDirectoryEntries));
    }

    #[test]
    fn clean_revision_audit_defeats_index_flags_and_local_excludes() {
        let (repository, root) = initialized_repository();
        assert!(
            !repository_state(&root)
                .expect("clean state")
                .local_modifications
        );

        std::fs::create_dir(repository.path().join("ignored")).expect("create ignored directory");
        std::fs::write(repository.path().join("ignored/generated"), "generated\n")
            .expect("write ignored output");
        assert!(
            !repository_state(&root)
                .expect("ignored output state")
                .local_modifications
        );

        test_git(
            repository.path(),
            &["update-index", "--skip-worktree", "tracked.txt"],
        );
        std::fs::write(repository.path().join("tracked.txt"), "hidden change\n")
            .expect("modify skipped file");
        assert!(
            repository_state(&root)
                .expect("skip-worktree state")
                .local_modifications
        );

        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("restore tracked file");
        test_git(
            repository.path(),
            &["update-index", "--no-skip-worktree", "tracked.txt"],
        );
        std::fs::write(repository.path().join("tracked.txt"), "staged change\n")
            .expect("write staged change");
        test_git(repository.path(), &["add", "tracked.txt"]);
        std::fs::write(repository.path().join("tracked.txt"), "base\n")
            .expect("restore worktree only");
        assert!(
            repository_state(&root)
                .expect("staged-only state")
                .local_modifications
        );

        test_git(
            repository.path(),
            &["reset", "--quiet", "HEAD", "--", "tracked.txt"],
        );
        std::fs::write(repository.path().join(".git/info/exclude"), "hidden.txt\n")
            .expect("write local exclude");
        std::fs::write(repository.path().join("hidden.txt"), "untracked\n")
            .expect("write locally excluded file");
        assert!(
            repository_state(&root)
                .expect("locally excluded state")
                .local_modifications
        );
    }

    #[test]
    fn clean_revision_audit_ignores_local_clean_filter_configuration() {
        let (repository, root) = initialized_repository();
        test_git(
            repository.path(),
            &["config", "filter.hide.clean", "printf 'canonical\\n'"],
        );
        test_git(repository.path(), &["config", "filter.hide.smudge", "cat"]);
        test_git(
            repository.path(),
            &["config", "filter.hide.required", "true"],
        );
        std::fs::write(
            repository.path().join(".gitattributes"),
            "filtered.txt filter=hide\n",
        )
        .expect("write attributes");
        std::fs::write(repository.path().join("filtered.txt"), "original\n")
            .expect("write filtered file");
        test_git(
            repository.path(),
            &["add", ".gitattributes", "filtered.txt"],
        );
        test_git(repository.path(), &["commit", "--quiet", "-m", "filtered"]);
        std::fs::write(repository.path().join("filtered.txt"), "modified\n")
            .expect("modify filtered file");
        assert!(
            test_git(repository.path(), &["status", "--porcelain"]).is_empty(),
            "local filter should hide the modification from ordinary Git"
        );
        assert!(
            repository_state(&root)
                .expect("config-free state")
                .local_modifications
        );
    }

    #[test]
    fn clean_revision_audit_disables_replacement_refs() {
        let (repository, root) = initialized_repository();
        let original = test_git(repository.path(), &["rev-parse", "HEAD"]);
        std::fs::write(repository.path().join("tracked.txt"), "replacement bytes\n")
            .expect("write replacement tree");
        test_git(repository.path(), &["add", "tracked.txt"]);
        test_git(
            repository.path(),
            &["commit", "--quiet", "-m", "replacement"],
        );
        let replacement = test_git(repository.path(), &["rev-parse", "HEAD"]);
        test_git(
            repository.path(),
            &["reset", "--quiet", "--hard", &original],
        );
        std::fs::write(repository.path().join("tracked.txt"), "replacement bytes\n")
            .expect("restore replacement worktree");
        test_git(repository.path(), &["add", "tracked.txt"]);
        test_git(repository.path(), &["replace", &original, &replacement]);
        assert!(
            test_git(repository.path(), &["status", "--porcelain"]).is_empty(),
            "replacement ref should hide the worktree change from ordinary Git"
        );
        let state = repository_state(&root).expect("replacement-free state");
        assert_eq!(state.commit, original);
        assert!(state.local_modifications);
    }
}
