use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::focused_error;
use crate::error::BenchError;
use crate::process::run_process;
use crate::report::stab_metadata;
use crate::root::RepoRoot;

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
}
