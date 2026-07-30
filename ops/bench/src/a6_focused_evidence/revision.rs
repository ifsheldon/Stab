use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::focused_error;
use crate::error::BenchError;
use crate::process::run_process;
use crate::report::stab_metadata;
use crate::root::RepoRoot;
use crate::source_file::read_repo_regular_file_bounded;

const ALLOWED_CLOSURE_PATHS: [&str; 3] = [
    "benchmarks/a6-focused-evidence.json",
    "docs/plans/GOAL.md",
    "docs/plans/agent-native-modular-qec-progress-report.md",
];

pub(super) fn validate_source_revision(
    root: &RepoRoot,
    source_revision: &str,
) -> Result<(), BenchError> {
    let current = stab_metadata(root)?;
    if current.local_modifications {
        return Err(focused_error(
            "A6 evidence validation requires a clean repository",
        ));
    }
    let ancestor = run_process(
        Path::new("git"),
        &[
            OsString::from("merge-base"),
            OsString::from("--is-ancestor"),
            OsString::from(source_revision),
            OsString::from("HEAD"),
        ],
        b"",
        &root.path,
        true,
    )?;
    if ancestor.status != Some(0) {
        return Err(focused_error(format!(
            "source_revision {source_revision} is not an ancestor of HEAD"
        )));
    }
    if current.commit == source_revision {
        return Ok(());
    }
    let changed = run_process(
        Path::new("git"),
        &[
            OsString::from("diff"),
            OsString::from("--name-only"),
            OsString::from(format!("{source_revision}..HEAD")),
            OsString::from("--"),
        ],
        b"",
        &root.path,
        true,
    )?;
    if changed.status != Some(0) {
        return Err(focused_error(
            "failed to enumerate files changed after the A6 evidence revision",
        ));
    }
    let text = std::str::from_utf8(&changed.stdout)
        .map_err(|error| focused_error(format!("Git changed-path output is not UTF-8: {error}")))?;
    let paths = text.lines().map(PathBuf::from).collect::<Vec<_>>();
    validate_closure_paths(&paths)
}

pub(super) fn validate_preserved_commit(root: &RepoRoot, revision: &str) -> Result<(), BenchError> {
    let object = run_process(
        Path::new("git"),
        &[
            OsString::from("cat-file"),
            OsString::from("-e"),
            OsString::from(format!("{revision}^{{commit}}")),
        ],
        b"",
        &root.path,
        true,
    )?;
    if object.status == Some(0) {
        Ok(())
    } else {
        Err(focused_error(format!(
            "predecessor revision {revision} is not a preserved Git commit"
        )))
    }
}

pub(super) fn read_tracked_source_file(
    root: &RepoRoot,
    revision: &str,
    relative_path: &Path,
    max_bytes: usize,
) -> Result<Option<Vec<u8>>, BenchError> {
    if relative_path.is_absolute()
        || relative_path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        return Err(focused_error(format!(
            "tracked source path {} is not a normal repository-relative path",
            relative_path.display()
        )));
    }
    let object = format!("{revision}:{}", relative_path.display());
    let working_dir = root.process_working_dir();
    let tracked = run_process(
        Path::new("git"),
        &[
            OsString::from("cat-file"),
            OsString::from("-e"),
            OsString::from(&object),
        ],
        b"",
        &working_dir,
        true,
    )?;
    if tracked.status != Some(0) {
        return Ok(None);
    }
    let blob = run_process(
        Path::new("git"),
        &[OsString::from("show"), OsString::from(&object)],
        b"",
        &working_dir,
        true,
    )?;
    if blob.status != Some(0) {
        return Err(focused_error(format!(
            "failed to read tracked source blob {object}"
        )));
    }
    if blob.stdout.len() > max_bytes {
        return Err(focused_error(format!(
            "tracked source blob {object} exceeds {max_bytes} bytes"
        )));
    }
    let working_path = root.resolve_relative(relative_path);
    let working = read_repo_regular_file_bounded(root, &working_path, max_bytes)?;
    if working != blob.stdout {
        return Err(focused_error(format!(
            "working source {} differs from tracked blob {object}",
            relative_path.display()
        )));
    }
    Ok(Some(working))
}

fn validate_closure_paths(paths: &[PathBuf]) -> Result<(), BenchError> {
    let unexpected = paths
        .iter()
        .filter(|path| {
            !ALLOWED_CLOSURE_PATHS
                .iter()
                .any(|allowed| path.as_path() == Path::new(allowed))
        })
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if unexpected.is_empty() {
        Ok(())
    } else {
        Err(focused_error(format!(
            "post-evidence revision changes non-closure paths: {}",
            unexpected.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_git(repository: &Path, arguments: &[&str]) -> String {
        let output = std::process::Command::new("git")
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
            "Git {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("Git output UTF-8")
            .trim()
            .to_string()
    }

    fn initialized_repository() -> (tempfile::TempDir, RepoRoot, String) {
        let repository = tempfile::tempdir().expect("temporary repository");
        test_git(repository.path(), &["init", "--quiet"]);
        test_git(repository.path(), &["config", "user.name", "Stab Test"]);
        test_git(
            repository.path(),
            &["config", "user.email", "stab@example.invalid"],
        );
        std::fs::create_dir(repository.path().join("benchmarks"))
            .expect("create benchmarks directory");
        std::fs::write(
            repository.path().join("benchmarks/policy.json"),
            b"{\"schema_version\":1}\n",
        )
        .expect("write tracked policy");
        test_git(repository.path(), &["add", "--all"]);
        test_git(repository.path(), &["commit", "--quiet", "-m", "initial"]);
        let revision = test_git(repository.path(), &["rev-parse", "HEAD"]);
        let root = RepoRoot::resolve(repository.path()).expect("resolve repository");
        (repository, root, revision)
    }

    #[test]
    fn closure_path_policy_allows_only_the_ledger_and_status_docs() {
        validate_closure_paths(
            &ALLOWED_CLOSURE_PATHS
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>(),
        )
        .expect("allowed closure paths");

        let error = validate_closure_paths(&[PathBuf::from("ops/bench/src/compare.rs")])
            .expect_err("compiled source change must invalidate evidence");
        assert!(error.to_string().contains("non-closure paths"));
    }

    #[test]
    fn predecessor_revision_must_resolve_to_a_commit() {
        let root = RepoRoot::resolve(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("repository root"),
        )
        .expect("resolve repository");
        let current = stab_metadata(&root).expect("current revision");
        validate_preserved_commit(&root, &current.commit).expect("HEAD commit is preserved");

        let error = validate_preserved_commit(&root, &"0".repeat(40))
            .expect_err("missing predecessor commit");
        assert!(error.to_string().contains("not a preserved Git commit"));
    }

    #[test]
    fn tracked_policy_reader_requires_the_committed_working_blob() {
        let (repository, root, revision) = initialized_repository();
        let relative = Path::new("benchmarks/policy.json");
        let bytes = read_tracked_source_file(&root, &revision, relative, 1 << 20)
            .expect("read tracked policy")
            .expect("tracked policy");
        assert_eq!(bytes, b"{\"schema_version\":1}\n");

        std::fs::write(
            repository.path().join(relative),
            b"{\"schema_version\":2}\n",
        )
        .expect("mutate working policy");
        let error = read_tracked_source_file(&root, &revision, relative, 1 << 20)
            .expect_err("working policy drift must fail");
        assert!(error.to_string().contains("differs from tracked blob"));
    }

    #[test]
    fn tracked_policy_reader_does_not_promote_untracked_files() {
        let (repository, root, revision) = initialized_repository();
        let relative = Path::new("benchmarks/untracked-policy.json");
        std::fs::write(repository.path().join(relative), b"{}\n").expect("write untracked policy");

        assert_eq!(
            read_tracked_source_file(&root, &revision, relative, 1 << 20)
                .expect("inspect untracked policy"),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn tracked_policy_reader_rejects_symlink_substitution() {
        use std::os::unix::fs::symlink;

        let (repository, root, revision) = initialized_repository();
        let relative = Path::new("benchmarks/policy.json");
        std::fs::remove_file(repository.path().join(relative)).expect("remove tracked policy");
        std::fs::write(repository.path().join("replacement.json"), b"{}\n")
            .expect("write replacement");
        symlink("../replacement.json", repository.path().join(relative))
            .expect("substitute symlink");

        let error = read_tracked_source_file(&root, &revision, relative, 1 << 20)
            .expect_err("symlink substitution must fail");
        assert!(
            error.to_string().contains("symbolic link")
                || error.to_string().contains("regular nonsymlink file"),
            "{error}"
        );
    }
}
