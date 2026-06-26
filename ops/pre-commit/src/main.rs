//! Staged-aware repository pre-commit hook.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for concise failure diagnostics"
    )
)]

mod instruction_docs;
mod large_files;
mod support;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;
use gix::bstr::ByteSlice;
use gix::index::entry::{Mode, Stage};
use instruction_docs::{
    DocCheckConfig, render_report as render_instruction_doc_report,
    report_passed as instruction_doc_report_passed, scan_instruction_docs_with_cancel,
};
use large_files::{
    BlobScanConfig, DEFAULT_LINE_THRESHOLD, DEFAULT_WATCH_THRESHOLD, SourceBlob,
    is_source_file_path, render_report as render_large_file_report,
    report_passed as large_file_report_passed, scan_large_file_blobs,
};
use support::{CommandOutput, DEFAULT_OUTPUT_LIMIT_BYTES, interrupted, run_command};
use thiserror::Error;
use tokio::process::Command;

const PREFIX: &str = "stab-pre-commit";
const TOOL_OUTPUT_LIMIT_BYTES: usize = DEFAULT_OUTPUT_LIMIT_BYTES * 128;

#[derive(Debug, Parser)]
#[command(
    about = "Runs staged-aware Stab repository pre-commit checks.",
    long_about = "Reads the staged index with gix, classifies touched paths, and runs Rust, instruction-doc, and large-file checks only when the staged commit requires them."
)]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[arg(long, default_value_t = DEFAULT_LINE_THRESHOLD)]
    large_file_threshold: usize,

    #[arg(long, default_value_t = DEFAULT_WATCH_THRESHOLD)]
    large_file_watch_threshold: usize,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let cancel = Arc::new(AtomicBool::new(false));
    let run_cancel = Arc::clone(&cancel);
    tokio::select! {
        result = run(cli, run_cancel) => match result {
            Ok(summary) => {
                summary.print();
                summary.exit_code()
            }
            Err(error) => {
                eprintln!("[{PREFIX}] ERROR: {error}");
                ExitCode::from(2)
            }
        },
        signal = tokio::signal::ctrl_c() => {
            cancel.store(true, Ordering::Relaxed);
            match signal {
                Ok(()) => eprintln!("[{PREFIX}] interrupted by Ctrl-C"),
                Err(error) => eprintln!("[{PREFIX}] failed to listen for Ctrl-C: {error}"),
            }
            interrupted()
        }
    }
}

async fn run(cli: Cli, cancel: Arc<AtomicBool>) -> Result<PreCommitSummary, PreCommitError> {
    let (repo_root, plan, large_file_blobs) = {
        let repo = StagedRepository::open(&cli.root)?;
        let staged = repo.staged_changes()?;
        if staged.is_empty() {
            return Ok(PreCommitSummary::empty());
        }
        let plan = CheckPlan::from_changes(&staged);
        let large_file_blobs = repo.source_blobs(&staged)?;
        (repo.root.clone(), plan, large_file_blobs)
    };

    let instruction_docs =
        run_instruction_docs_check(repo_root.clone(), plan.has_docs, Arc::clone(&cancel));
    let large_files = run_large_files_check(
        large_file_blobs,
        cli.large_file_threshold,
        cli.large_file_watch_threshold,
        plan.large_files,
    );
    let rust = run_rust_checks(repo_root, plan.has_rust);

    let (instruction_docs, large_files, rust) = tokio::join!(instruction_docs, large_files, rust);

    let mut outcomes = vec![instruction_docs, large_files];
    outcomes.extend(rust);
    Ok(PreCommitSummary {
        staged: Some(plan.summary()),
        outcomes,
    })
}

struct StagedRepository {
    repo: gix::Repository,
    root: PathBuf,
}

impl StagedRepository {
    fn open(root: &Path) -> Result<Self, PreCommitError> {
        let repo = gix::discover(root).map_err(|source| PreCommitError::Discover {
            path: root.to_path_buf(),
            source: Box::new(source),
        })?;
        let workdir = repo.workdir().ok_or(PreCommitError::BareRepository)?;
        let root =
            std::fs::canonicalize(workdir).map_err(|source| PreCommitError::ResolveRoot {
                path: workdir.to_path_buf(),
                source,
            })?;
        Ok(Self { repo, root })
    }

    fn staged_changes(&self) -> Result<Vec<StagedChange>, PreCommitError> {
        let head_tree_id =
            self.repo
                .head_tree_id_or_empty()
                .map_err(|source| PreCommitError::HeadTree {
                    source: Box::new(source),
                })?;
        let head_index = self
            .repo
            .index_from_tree(head_tree_id.as_ref())
            .map_err(|source| PreCommitError::HeadIndex {
                source: Box::new(source),
            })?;
        let worktree_index =
            self.repo
                .index_or_empty()
                .map_err(|source| PreCommitError::Index {
                    source: Box::new(source),
                })?;

        let head_entries = collect_index_entries(&head_index)?;
        let staged_entries = collect_index_entries(&worktree_index)?;
        let all_paths = head_entries
            .keys()
            .chain(staged_entries.keys())
            .cloned()
            .collect::<BTreeSet<_>>();

        let mut changes = Vec::new();
        for path in all_paths {
            match (head_entries.get(&path), staged_entries.get(&path)) {
                (None, Some(new)) => changes.push(StagedChange::added(new)),
                (Some(old), None) => changes.push(StagedChange::deleted(old)),
                (Some(old), Some(new)) if old.id == new.id && old.mode == new.mode => {}
                (Some(old), Some(new)) if old.mode != new.mode => {
                    changes.push(StagedChange::type_changed(old, new));
                }
                (Some(_old), Some(new)) => changes.push(StagedChange::modified(new)),
                (None, None) => {}
            }
        }
        Ok(changes)
    }

    fn source_blobs(&self, changes: &[StagedChange]) -> Result<Vec<SourceBlob>, PreCommitError> {
        let mut blobs = Vec::new();
        for change in changes {
            if !change.has_staged_blob()
                || change.is_submodule
                || !change.mode_has_blob()
                || !is_source_file_path(&change.path)
            {
                continue;
            }
            let id = change
                .object_id
                .ok_or_else(|| PreCommitError::MissingBlobId {
                    path: change.path.clone(),
                })?;
            let mut blob = self
                .repo
                .find_blob(id)
                .map_err(|source| PreCommitError::Blob {
                    path: change.path.clone(),
                    source: Box::new(source),
                })?;
            blobs.push(SourceBlob::new(change.path.clone(), blob.take_data()));
        }
        Ok(blobs)
    }
}

fn collect_index_entries(
    index: &gix::index::State,
) -> Result<BTreeMap<Vec<u8>, IndexEntry>, PreCommitError> {
    let mut entries = BTreeMap::new();
    for entry in index.entries() {
        let path = entry.path(index);
        if entry.stage() != Stage::Unconflicted {
            return Err(PreCommitError::UnresolvedConflict {
                path: path_to_path_buf(path.as_bytes())?,
            });
        }
        let path_bytes = path.as_bytes().to_vec();
        let path_buf = path_to_path_buf(&path_bytes)?;
        let info = IndexEntry {
            path: path_buf.clone(),
            id: entry.id,
            mode: entry.mode,
        };
        if entries.insert(path_bytes, info).is_some() {
            return Err(PreCommitError::DuplicateIndexPath { path: path_buf });
        }
    }
    Ok(entries)
}

fn path_to_path_buf(path: &[u8]) -> Result<PathBuf, PreCommitError> {
    let path = std::str::from_utf8(path).map_err(|source| PreCommitError::NonUtf8Path {
        source,
        path: path.to_vec(),
    })?;
    Ok(PathBuf::from(path))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexEntry {
    path: PathBuf,
    id: gix::ObjectId,
    mode: Mode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StagedChange {
    path: PathBuf,
    kind: ChangeKind,
    object_id: Option<gix::ObjectId>,
    mode: Option<Mode>,
    is_submodule: bool,
}

impl StagedChange {
    fn added(new: &IndexEntry) -> Self {
        Self::from_entries(new.path.clone(), ChangeKind::Added, None, Some(new))
    }

    fn modified(new: &IndexEntry) -> Self {
        Self::from_entries(new.path.clone(), ChangeKind::Modified, None, Some(new))
    }

    fn deleted(old: &IndexEntry) -> Self {
        Self::from_entries(old.path.clone(), ChangeKind::Deleted, Some(old), None)
    }

    fn type_changed(old: &IndexEntry, new: &IndexEntry) -> Self {
        Self::from_entries(
            new.path.clone(),
            ChangeKind::TypeChanged,
            Some(old),
            Some(new),
        )
    }

    fn from_entries(
        path: PathBuf,
        kind: ChangeKind,
        old: Option<&IndexEntry>,
        new: Option<&IndexEntry>,
    ) -> Self {
        let effective = new.or(old);
        let mode = new.map(|entry| entry.mode);
        let is_submodule = effective.is_some_and(|entry| entry.mode == Mode::COMMIT);
        Self {
            path,
            kind,
            object_id: new.map(|entry| entry.id),
            mode,
            is_submodule,
        }
    }

    fn has_staged_blob(&self) -> bool {
        matches!(
            self.kind,
            ChangeKind::Added | ChangeKind::Modified | ChangeKind::TypeChanged
        ) && self.object_id.is_some()
    }

    fn mode_has_blob(&self) -> bool {
        self.mode
            .is_some_and(|mode| matches!(mode, Mode::FILE | Mode::FILE_EXECUTABLE | Mode::SYMLINK))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChangeKind {
    Added,
    Modified,
    Deleted,
    TypeChanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckPlan {
    has_docs: bool,
    has_rust: bool,
    large_files: usize,
    submodules: usize,
}

impl CheckPlan {
    fn from_changes(changes: &[StagedChange]) -> Self {
        let mut has_docs = false;
        let mut has_rust = false;
        let mut large_files = 0usize;
        let mut submodules = 0usize;

        for change in changes {
            if change.is_submodule {
                submodules = submodules.saturating_add(1);
                continue;
            }
            if is_docs_policy_path(&change.path) {
                has_docs = true;
            }
            if is_rust_affecting_path(&change.path) {
                has_rust = true;
            }
            if change.has_staged_blob()
                && change.mode_has_blob()
                && is_source_file_path(&change.path)
            {
                large_files = large_files.saturating_add(1);
            }
        }

        Self {
            has_docs,
            has_rust,
            large_files,
            submodules,
        }
    }

    fn summary(&self) -> String {
        format!(
            "staged: rust={} docs={} large_files={} submodules={}",
            usize::from(self.has_rust),
            usize::from(self.has_docs),
            self.large_files,
            self.submodules,
        )
    }
}

fn is_docs_policy_path(path: &Path) -> bool {
    if path == Path::new(".gitmodules") {
        return true;
    }
    path.file_name().is_some_and(|file_name| {
        matches!(
            file_name.to_str(),
            Some("README.md" | "AGENTS.md" | "CLAUDE.md")
        )
    })
}

fn is_rust_affecting_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "rs")
        || path.file_name().is_some_and(|file_name| {
            matches!(file_name.to_str(), Some("Cargo.toml" | "Cargo.lock"))
        })
}

async fn run_instruction_docs_check(
    repo_root: PathBuf,
    enabled: bool,
    cancel: Arc<AtomicBool>,
) -> CheckOutcome {
    if !enabled {
        return CheckOutcome::skip("instruction docs", "no staged instruction docs");
    }
    match tokio::task::spawn_blocking(move || {
        let config = DocCheckConfig::new(repo_root);
        scan_instruction_docs_with_cancel(&config, Some(&cancel))
    })
    .await
    {
        Ok(Ok(report)) => {
            if instruction_doc_report_passed(&report) {
                CheckOutcome::pass("instruction docs", render_instruction_doc_report(&report))
            } else {
                CheckOutcome::fail("instruction docs", render_instruction_doc_report(&report))
            }
        }
        Ok(Err(error)) => CheckOutcome::error("instruction docs", error.to_string()),
        Err(error) if error.is_cancelled() => {
            CheckOutcome::error("instruction docs", "check task was cancelled".to_string())
        }
        Err(error) => {
            CheckOutcome::error("instruction docs", format!("check task failed: {error}"))
        }
    }
}

async fn run_large_files_check(
    blobs: Vec<SourceBlob>,
    threshold: usize,
    watch_threshold: usize,
    expected_files: usize,
) -> CheckOutcome {
    if expected_files == 0 {
        return CheckOutcome::skip("large files", "no staged source blobs");
    }
    let watch_threshold = if watch_threshold == 0 {
        None
    } else {
        Some(watch_threshold)
    };
    let config = match BlobScanConfig::new(blobs, threshold, watch_threshold) {
        Ok(config) => config,
        Err(error) => return CheckOutcome::error("large files", error.to_string()),
    };
    match tokio::task::spawn_blocking(move || scan_large_file_blobs(&config)).await {
        Ok(Ok(report)) => {
            if large_file_report_passed(&report) {
                CheckOutcome::pass("large files", render_large_file_report(&report))
            } else {
                CheckOutcome::fail("large files", render_large_file_report(&report))
            }
        }
        Ok(Err(error)) => CheckOutcome::error("large files", error.to_string()),
        Err(error) if error.is_cancelled() => {
            CheckOutcome::error("large files", "check task was cancelled".to_string())
        }
        Err(error) => CheckOutcome::error("large files", format!("check task failed: {error}")),
    }
}

async fn run_rust_checks(repo_root: PathBuf, enabled: bool) -> Vec<CheckOutcome> {
    if !enabled {
        return vec![CheckOutcome::skip(
            "rust checks",
            "no staged Rust-affecting files",
        )];
    }

    let rustfmt = run_tool_check(
        "rustfmt",
        "cargo",
        ["fmt", "--all", "--check"]
            .into_iter()
            .map(str::to_string)
            .collect(),
        repo_root.clone(),
    );
    let clippy = run_tool_check(
        "rust clippy",
        "cargo",
        [
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ]
        .into_iter()
        .map(str::to_string)
        .collect(),
        repo_root,
    );

    let (rustfmt, clippy) = tokio::join!(rustfmt, clippy);
    vec![rustfmt, clippy]
}

async fn run_tool_check(
    name: &'static str,
    program: &'static str,
    args: Vec<String>,
    cwd: PathBuf,
) -> CheckOutcome {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    match run_command(command, program, TOOL_OUTPUT_LIMIT_BYTES).await {
        Ok(output) if output.success() => {
            CheckOutcome::pass(name, render_command_output(name, &output))
        }
        Ok(output) => {
            let mut lines = vec![format!(
                "[{PREFIX}] FAIL {name} exited with status {}",
                output
                    .status
                    .map_or_else(|| "signal".to_string(), |status| status.to_string())
            )];
            lines.extend(render_command_output(name, &output));
            CheckOutcome::fail(name, lines)
        }
        Err(error) => CheckOutcome::error(name, error.to_string()),
    }
}

fn render_command_output(name: &str, output: &CommandOutput) -> Vec<String> {
    let mut lines = Vec::new();
    push_stream_lines(&mut lines, name, "stdout", &output.stdout);
    push_stream_lines(&mut lines, name, "stderr", &output.stderr);
    if output.truncated {
        lines.push(format!("[{PREFIX}] WARN {name} output was truncated"));
    }
    lines
}

fn push_stream_lines(lines: &mut Vec<String>, name: &str, stream: &str, text: &str) {
    if text.trim().is_empty() {
        return;
    }
    for line in text.lines() {
        lines.push(format!("[{PREFIX}] {name} {stream}: {line}"));
    }
}

#[derive(Debug, Clone)]
struct PreCommitSummary {
    staged: Option<String>,
    outcomes: Vec<CheckOutcome>,
}

impl PreCommitSummary {
    fn empty() -> Self {
        Self {
            staged: None,
            outcomes: Vec::new(),
        }
    }

    fn print(&self) {
        if let Some(staged) = &self.staged {
            println!("[{PREFIX}] {staged}");
        } else {
            println!("[{PREFIX}] no staged changes");
        }
        for outcome in &self.outcomes {
            outcome.print();
        }
        println!("[{PREFIX}] Done.");
    }

    fn exit_code(&self) -> ExitCode {
        if self
            .outcomes
            .iter()
            .any(|outcome| matches!(outcome.status, CheckStatus::Error(_)))
        {
            return ExitCode::from(2);
        }
        if self
            .outcomes
            .iter()
            .any(|outcome| matches!(outcome.status, CheckStatus::Fail))
        {
            return ExitCode::from(1);
        }
        ExitCode::SUCCESS
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckOutcome {
    name: &'static str,
    status: CheckStatus,
    lines: Vec<String>,
}

impl CheckOutcome {
    fn pass(name: &'static str, lines: Vec<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Pass,
            lines,
        }
    }

    fn skip(name: &'static str, message: impl Into<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Skip(message.into()),
            lines: Vec::new(),
        }
    }

    fn fail(name: &'static str, lines: Vec<String>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            lines,
        }
    }

    fn error(name: &'static str, message: String) -> Self {
        Self {
            name,
            status: CheckStatus::Error(message),
            lines: Vec::new(),
        }
    }

    fn print(&self) {
        match &self.status {
            CheckStatus::Pass => println!("[{PREFIX}] PASS {}", self.name),
            CheckStatus::Skip(message) => println!("[{PREFIX}] SKIP {} - {message}", self.name),
            CheckStatus::Fail => println!("[{PREFIX}] FAIL {}", self.name),
            CheckStatus::Error(message) => {
                println!("[{PREFIX}] ERROR {} - {message}", self.name);
            }
        }
        for line in &self.lines {
            println!("{line}");
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CheckStatus {
    Pass,
    Skip(String),
    Fail,
    Error(String),
}

#[derive(Debug, Error)]
enum PreCommitError {
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
    #[error("failed to read staged index: {source}")]
    Index {
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
    #[error("staged source file {path:?} has no blob id")]
    MissingBlobId { path: PathBuf },
    #[error("failed to read staged blob for {path:?}: {source}")]
    Blob {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(path: &str, kind: ChangeKind, is_submodule: bool) -> StagedChange {
        StagedChange {
            path: PathBuf::from(path),
            kind,
            object_id: None,
            mode: Some(Mode::FILE),
            is_submodule,
        }
    }

    fn blob_change(path: &str) -> StagedChange {
        StagedChange {
            path: PathBuf::from(path),
            kind: ChangeKind::Added,
            object_id: Some(gix::ObjectId::empty_blob(gix::hash::Kind::Sha1)),
            mode: Some(Mode::FILE),
            is_submodule: false,
        }
    }

    #[test]
    fn staged_file_classification_detects_rust_work() {
        let staged = vec![
            change("crates/stab-core/src/lib.rs", ChangeKind::Modified, false),
            change(
                "docs/plans/rust-stim-drop-in-rewrite.md",
                ChangeKind::Modified,
                false,
            ),
        ];

        let plan = CheckPlan::from_changes(&staged);

        assert!(plan.has_rust);
        assert!(!plan.has_docs);
    }

    #[test]
    fn staged_file_classification_detects_cargo_files() {
        let manifest = CheckPlan::from_changes(&[change(
            "crates/stab-core/Cargo.toml",
            ChangeKind::Modified,
            false,
        )]);
        let lock = CheckPlan::from_changes(&[change("Cargo.lock", ChangeKind::Modified, false)]);

        assert!(manifest.has_rust);
        assert!(lock.has_rust);
    }

    #[test]
    fn staged_file_classification_detects_instruction_docs() {
        for path in [
            "README.md",
            "docs/AGENTS.md",
            "docs/CLAUDE.md",
            ".gitmodules",
        ] {
            let plan = CheckPlan::from_changes(&[change(path, ChangeKind::Modified, false)]);
            assert!(plan.has_docs, "{path}");
        }
    }

    #[test]
    fn staged_file_classification_treats_submodule_as_pointer() {
        let plan = CheckPlan::from_changes(&[change("vendor/stim", ChangeKind::Modified, true)]);

        assert_eq!(plan.submodules, 1);
        assert!(!plan.has_rust);
        assert!(!plan.has_docs);
        assert_eq!(plan.large_files, 0);
    }

    #[test]
    fn staged_file_classification_counts_large_file_candidates() {
        let plan = CheckPlan::from_changes(&[
            blob_change("ops/pre-commit/src/main.rs"),
            change("Cargo.lock", ChangeKind::Modified, false),
        ]);

        assert_eq!(plan.large_files, 1);
    }

    #[test]
    fn passing_summary_exits_successfully() {
        let summary = PreCommitSummary {
            staged: Some("staged: rust=0 docs=0 large_files=0 submodules=0".to_string()),
            outcomes: vec![CheckOutcome::skip("rust checks", "no Rust files")],
        };

        assert_eq!(summary.exit_code(), ExitCode::SUCCESS);
    }
}
