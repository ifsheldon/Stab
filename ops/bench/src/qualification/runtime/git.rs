use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;

use super::process::{ProcessLimits, ProcessRequest, ProcessResult, run_bounded_process};
use crate::config::{STIM_COMMIT, STIM_TAG};
use crate::root::RepoRoot;

const GIT: &str = "/usr/bin/git";
const MAX_GIT_FILE_BYTES: u64 = 512 << 20;
const MAX_GIT_OUTPUT_BYTES: usize = 4 << 20;
const MAX_GIT_STDERR_BYTES: usize = 64 << 10;
const MAX_SHARED_INDEX_FILES: usize = 256;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RepositoryState {
    pub(super) commit: String,
    pub(super) local_modifications: bool,
}

pub(super) fn repository_state(root: &RepoRoot) -> Result<RepositoryState, GitError> {
    GitView::open(&root.path)?.state()
}

pub(super) fn validate_pinned_stim(root: &RepoRoot) -> Result<(), GitError> {
    let view = GitView::open(&root.default_stim_source())?;
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
    materialize_worktree_commit(&root.path, expected_commit, destination)
}

pub(super) fn materialize_worktree_commit(
    worktree: &Path,
    expected_commit: &str,
    destination: &Path,
) -> Result<(), GitError> {
    let view = GitView::open(worktree)?;
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
    worktree: PathBuf,
    git_dir: PathBuf,
    head_index: PathBuf,
    staged_index: PathBuf,
    scratch: PathBuf,
    commit: String,
}

impl GitView {
    fn open(worktree: &Path) -> Result<Self, GitError> {
        ensure_git()?;
        if !worktree.is_absolute() {
            return Err(GitError::UnsafeGitDirectory(worktree.to_path_buf()));
        }
        let worktree = worktree.to_path_buf();
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
        copy_git_file(&source_git_dir.join("HEAD"), &git_dir.join("HEAD"), true)?;
        copy_git_file(
            &common_dir.join("packed-refs"),
            &git_dir.join("packed-refs"),
            false,
        )?;
        copy_git_file(&common_dir.join("shallow"), &git_dir.join("shallow"), false)?;
        link_git_directory(&common_dir.join("objects"), &git_dir.join("objects"))?;
        link_git_directory(&common_dir.join("refs"), &git_dir.join("refs"))?;
        let head_index = git_dir.join("head-index");
        let staged_index = git_dir.join("staged-index");
        copy_git_file(&source_git_dir.join("index"), &staged_index, true)?;
        copy_shared_indexes(&source_git_dir, &git_dir)?;

        let mut view = Self {
            _temporary: temporary,
            worktree,
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
            self.worktree.as_os_str().to_owned(),
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
            working_directory: self.worktree.clone(),
            environment,
            affinity_cpu: None,
            limits: ProcessLimits {
                stdin_bytes: 0,
                stdout_bytes: MAX_GIT_OUTPUT_BYTES,
                stderr_bytes: MAX_GIT_STDERR_BYTES,
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

fn resolve_git_dir(worktree: &Path) -> Result<PathBuf, GitError> {
    let marker = worktree.join(".git");
    let metadata = std::fs::symlink_metadata(&marker).map_err(|source| GitError::Io {
        path: marker.clone(),
        source,
    })?;
    if metadata.file_type().is_symlink() {
        return Err(GitError::UnsafeGitMarker(marker));
    }
    if metadata.is_dir() {
        return Ok(marker);
    }
    if !metadata.is_file() {
        return Err(GitError::UnsafeGitMarker(marker));
    }
    let text = read_git_text_file(&marker, 4096)?;
    let relative = text
        .trim()
        .strip_prefix("gitdir: ")
        .ok_or_else(|| GitError::MalformedGitMarker(marker.clone()))?;
    let path = PathBuf::from(relative);
    let path = if path.is_absolute() {
        path
    } else {
        worktree.join(path)
    };
    std::fs::canonicalize(&path).map_err(|source| GitError::Io { path, source })
}

fn resolve_common_dir(git_dir: &Path) -> Result<PathBuf, GitError> {
    let marker = git_dir.join("commondir");
    let metadata = match std::fs::symlink_metadata(&marker) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return Ok(git_dir.to_path_buf());
        }
        Err(source) => {
            return Err(GitError::Io {
                path: marker,
                source,
            });
        }
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(GitError::UnsafeGitMarker(marker));
    }
    let text = read_git_text_file(&marker, 4096)?;
    let path = PathBuf::from(text.trim());
    let path = if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    };
    std::fs::canonicalize(&path).map_err(|source| GitError::Io { path, source })
}

fn read_git_text_file(path: &Path, maximum: u64) -> Result<String, GitError> {
    let bytes = read_git_file(path, maximum)?;
    String::from_utf8(bytes).map_err(|source| GitError::NonUtf8File {
        path: path.to_path_buf(),
        source,
    })
}

fn read_git_file(path: &Path, maximum: u64) -> Result<Vec<u8>, GitError> {
    use std::io::Read as _;

    let mut file = crate::source_file::open_regular_file_bounded_descriptor(path, maximum)
        .map_err(|source| GitError::UnsafeFile {
            path: path.to_path_buf(),
            reason: source.to_string(),
        })?;
    let length = file
        .metadata()
        .map_err(|source| GitError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .len();
    let capacity = usize::try_from(length).map_err(|_| GitError::UnsafeFile {
        path: path.to_path_buf(),
        reason: "file size does not fit usize".to_string(),
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|source| GitError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > maximum {
        return Err(GitError::UnsafeFile {
            path: path.to_path_buf(),
            reason: format!("file grew beyond {maximum} bytes"),
        });
    }
    Ok(bytes)
}

fn copy_git_file(source: &Path, destination: &Path, required: bool) -> Result<(), GitError> {
    match std::fs::symlink_metadata(source) {
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(GitError::UnsafeFile {
                path: source.to_path_buf(),
                reason: "expected a nonsymlink regular file".to_string(),
            });
        }
        Ok(_) => {}
        Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source_error) => {
            return Err(GitError::Io {
                path: source.to_path_buf(),
                source: source_error,
            });
        }
    }
    let bytes = read_git_file(source, MAX_GIT_FILE_BYTES)?;
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

fn copy_shared_indexes(source_git_dir: &Path, destination: &Path) -> Result<(), GitError> {
    let mut count = 0_usize;
    for entry in std::fs::read_dir(source_git_dir).map_err(|source| GitError::Io {
        path: source_git_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| GitError::Io {
            path: source_git_dir.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        if !is_shared_index_name(&name) {
            continue;
        }
        count = count.checked_add(1).ok_or(GitError::TooManySharedIndexes)?;
        if count > MAX_SHARED_INDEX_FILES {
            return Err(GitError::TooManySharedIndexes);
        }
        copy_git_file(&entry.path(), &destination.join(name), true)?;
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

fn link_git_directory(source: &Path, destination: &Path) -> Result<(), GitError> {
    let metadata = std::fs::symlink_metadata(source).map_err(|error| GitError::Io {
        path: source.to_path_buf(),
        source: error,
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(GitError::UnsafeGitDirectory(source.to_path_buf()));
    }
    std::os::unix::fs::symlink(source, destination).map_err(|source| GitError::Io {
        path: destination.to_path_buf(),
        source,
    })
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
