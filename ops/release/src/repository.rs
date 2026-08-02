use std::path::Path;
use std::process::{Command, Stdio};

use crate::{RELEASE_TAG, ReleaseError};

const MAX_COMMAND_OUTPUT: usize = 1 << 20;

pub(crate) fn require_clean(root: &Path) -> Result<String, ReleaseError> {
    let status = run_capture(
        root,
        "git",
        &[
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
            "--ignore-submodules=none",
        ],
        MAX_COMMAND_OUTPUT,
    )?;
    if !status.trim().is_empty() {
        return Err(ReleaseError::DirtyRepository(status.trim().to_string()));
    }
    let commit = run_capture(
        root,
        "git",
        &["rev-parse", "--verify", "HEAD"],
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
        "git",
        &["cat-file", "-t", &tag_ref],
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
        "git",
        &["rev-parse", "--verify", &peeled],
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

pub(crate) fn run_capture(
    root: &Path,
    program: &str,
    args: &[&str],
    limit: usize,
) -> Result<String, ReleaseError> {
    let output = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|source| ReleaseError::CommandIo {
            program: program.to_string(),
            source,
        })?;
    if output.stdout.len().saturating_add(output.stderr.len()) > limit {
        return Err(ReleaseError::CommandOutputLimit {
            program: program.to_string(),
            limit,
        });
    }
    if !output.status.success() {
        return Err(ReleaseError::CommandFailed {
            program: program.to_string(),
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    String::from_utf8(output.stdout).map_err(ReleaseError::from)
}

pub(crate) fn run_inherit(root: &Path, program: &str, args: &[&str]) -> Result<(), ReleaseError> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .status()
        .map_err(|source| ReleaseError::CommandIo {
            program: program.to_string(),
            source,
        })?;
    if !status.success() {
        return Err(ReleaseError::CommandFailed {
            program: program.to_string(),
            status: status.to_string(),
            stderr: "see inherited Cargo diagnostics".to_string(),
        });
    }
    Ok(())
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
