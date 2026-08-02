use std::ffi::{OsStr, OsString};
use std::path::Path;
#[cfg(test)]
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{RELEASE_TAG, ReleaseError, process};

const MAX_COMMAND_OUTPUT: usize = 8 << 20;
const GIT_TIMEOUT: Duration = Duration::from_secs(30);
const TOOLCHAIN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToolchainIdentity {
    pub(crate) cargo_program: String,
    pub(crate) cargo_version: String,
    pub(crate) rustc_program: String,
    pub(crate) rustc_version: String,
    pub(crate) active_toolchain: String,
}

pub(crate) fn require_clean(root: &Path) -> Result<String, ReleaseError> {
    let status = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("status"),
            OsStr::new("--porcelain=v1"),
            OsStr::new("--untracked-files=normal"),
            OsStr::new("--ignore-submodules=none"),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    if !status.trim().is_empty() {
        return Err(ReleaseError::DirtyRepository(status.trim().to_string()));
    }
    let commit = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new("HEAD"),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    let commit = commit.trim().to_string();
    if commit.len() != 40 || !commit.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ReleaseError::PackageContract(format!(
            "git returned invalid HEAD identity {commit:?}"
        )));
    }
    Ok(commit)
}

pub(crate) fn require_unchanged(root: &Path, before: &str) -> Result<(), ReleaseError> {
    let after = require_clean(root)?;
    if after != before {
        return Err(ReleaseError::RepositoryChanged {
            before: before.to_string(),
            after,
        });
    }
    Ok(())
}

pub(crate) fn require_clean_tag(root: &Path, tag: &str) -> Result<String, ReleaseError> {
    if tag != RELEASE_TAG {
        return Err(ReleaseError::TagName {
            expected: RELEASE_TAG.to_string(),
            actual: tag.to_string(),
        });
    }
    let head = require_clean(root)?;
    let tag_ref = format!("refs/tags/{tag}");
    let kind = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("cat-file"),
            OsStr::new("-t"),
            OsStr::new(&tag_ref),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?;
    if kind.trim() != "tag" {
        return Err(ReleaseError::TagKind {
            tag: tag.to_string(),
        });
    }
    let peeled = format!("{tag_ref}^{{commit}}");
    let tag_commit = run_capture(
        root,
        OsStr::new("git"),
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&peeled),
        ],
        GIT_TIMEOUT,
        MAX_COMMAND_OUTPUT,
    )?
    .trim()
    .to_string();
    if tag_commit != head {
        return Err(ReleaseError::TagCommit {
            tag: tag.to_string(),
            tag_commit,
            head,
        });
    }
    Ok(head)
}

pub(crate) fn capture_toolchain(root: &Path) -> Result<ToolchainIdentity, ReleaseError> {
    let cargo = cargo_program();
    let rustc = rustc_program();
    Ok(ToolchainIdentity {
        cargo_program: cargo.to_string_lossy().into_owned(),
        cargo_version: run_capture(
            root,
            &cargo,
            [OsStr::new("--version"), OsStr::new("--verbose")],
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
        rustc_program: rustc.to_string_lossy().into_owned(),
        rustc_version: run_capture(
            root,
            &rustc,
            [OsStr::new("--version"), OsStr::new("--verbose")],
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
        active_toolchain: run_capture(
            root,
            OsStr::new("rustup"),
            [OsStr::new("show"), OsStr::new("active-toolchain")],
            TOOLCHAIN_TIMEOUT,
            MAX_COMMAND_OUTPUT,
        )?,
    })
}

pub(crate) fn require_toolchain(
    root: &Path,
    expected: &ToolchainIdentity,
) -> Result<(), ReleaseError> {
    let actual = capture_toolchain(root)?;
    if actual != *expected {
        return Err(ReleaseError::ToolchainIdentity(format!(
            "expected {expected:?}, found {actual:?}"
        )));
    }
    Ok(())
}

pub(crate) fn cargo_program() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

pub(crate) fn run_capture<I, S>(
    root: &Path,
    program: &OsStr,
    args: I,
    timeout: Duration,
    limit: usize,
) -> Result<String, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = process::run(root, program, args, &[], timeout, limit)?;
    String::from_utf8(output.stdout).map_err(ReleaseError::from)
}

pub(crate) fn run_with_environment<I, S>(
    root: &Path,
    program: &OsStr,
    args: I,
    environment: &[(OsString, OsString)],
    timeout: Duration,
    limit: usize,
) -> Result<process::ProcessOutput, ReleaseError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    process::run(root, program, args, environment, timeout, limit)
}

fn rustc_program() -> OsString {
    std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?}");
    }

    fn repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("repository");
        git(root.path(), &["init", "-b", "main"]);
        git(root.path(), &["config", "user.name", "Stab Release Test"]);
        git(
            root.path(),
            &["config", "user.email", "release-test@example.invalid"],
        );
        fs::write(root.path().join("tracked"), b"source").expect("tracked file");
        git(root.path(), &["add", "tracked"]);
        git(root.path(), &["commit", "-m", "source"]);
        root
    }

    #[test]
    fn annotated_release_tag_must_match_clean_head() {
        let root = repository();
        git(root.path(), &["tag", "-a", RELEASE_TAG, "-m", "release"]);
        let head = require_clean(root.path()).expect("clean head");
        assert_eq!(
            require_clean_tag(root.path(), RELEASE_TAG).expect("release tag"),
            head
        );

        fs::write(root.path().join("tracked"), b"changed").expect("dirty file");
        assert!(matches!(
            require_clean_tag(root.path(), RELEASE_TAG),
            Err(ReleaseError::DirtyRepository(_))
        ));
    }

    #[test]
    fn lightweight_and_wrong_release_tags_are_rejected() {
        let root = repository();
        git(root.path(), &["tag", RELEASE_TAG]);
        assert!(matches!(
            require_clean_tag(root.path(), RELEASE_TAG),
            Err(ReleaseError::TagKind { .. })
        ));
        assert!(matches!(
            require_clean_tag(root.path(), "v0.2.1"),
            Err(ReleaseError::TagName { .. })
        ));
    }
}
