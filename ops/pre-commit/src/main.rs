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

mod install;
mod instruction_docs;
mod large_files;
mod paths;
mod rust_checks;
mod staged_tree;
mod support;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Args, Parser, Subcommand};
use install::{InstallError, Replaced, install};
use instruction_docs::{
    render_report as render_instruction_doc_report, report_passed as instruction_doc_report_passed,
    scan_instruction_docs,
};
use large_files::{
    BlobScanConfig, DEFAULT_LINE_THRESHOLD, DEFAULT_WATCH_THRESHOLD, SourceBlob,
    is_source_file_path, render_report as render_large_file_report,
    report_passed as large_file_report_passed, scan_large_file_blobs,
};
use paths::{is_docs_policy_path, is_rust_affecting_path};
use rust_checks::{DRY_RUN_ENV, RUST_TOOLS, RustTool, WorktreeDrift, worktree_drift};
use staged_tree::{StagedChange, StagedTree, StagedTreeError};
use support::{CommandOutput, DEFAULT_OUTPUT_LIMIT_BYTES, interrupted, run_command};
use thiserror::Error;
use tokio::process::Command;

const PREFIX: &str = "stab-pre-commit";
const TOOL_OUTPUT_LIMIT_BYTES: usize = DEFAULT_OUTPUT_LIMIT_BYTES * 128;

#[derive(Debug, Parser)]
#[command(
    about = "Runs staged-aware Stab repository pre-commit checks.",
    long_about = "Reads the index being committed (GIT_INDEX_FILE when Git sets it, otherwise the repository index) with gix, classifies touched paths, and runs Rust, instruction-doc, and large-file checks only when the staged commit requires them.",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,

    #[command(flatten)]
    check: CheckArgs,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Install this binary as the repository's Git pre-commit hook.
    Install {
        /// Any path inside the target repository or worktree.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Replace an existing hook that is not a copy of stab-pre-commit.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Debug, Args)]
struct CheckArgs {
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
    if let Some(CliCommand::Install { root, force }) = cli.command {
        return run_install(&root, force);
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let run_cancel = Arc::clone(&cancel);
    tokio::select! {
        result = run(cli.check, run_cancel) => match result {
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

fn run_install(root: &std::path::Path, force: bool) -> ExitCode {
    match install(root, force) {
        Ok(report) => {
            let note = match report.replaced {
                Replaced::Nothing => "",
                Replaced::PreviousCopy => " (replaced a previous stab-pre-commit copy)",
                Replaced::ForeignHook => " (replaced a foreign hook because of --force)",
            };
            println!(
                "[{PREFIX}] installed pre-commit hook at {}{note}",
                report.hook_path.display()
            );
            ExitCode::SUCCESS
        }
        Err(error @ InstallError::ForeignHook { .. }) => {
            eprintln!("[{PREFIX}] ERROR: {error}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("[{PREFIX}] ERROR: {error}");
            ExitCode::from(2)
        }
    }
}

async fn run(args: CheckArgs, cancel: Arc<AtomicBool>) -> Result<PreCommitSummary, PreCommitError> {
    let staged = tokio::task::spawn_blocking(move || check_staged_tree(&args, &cancel))
        .await
        .map_err(|error| PreCommitError::Task(error.to_string()))??;
    let Some(staged) = staged else {
        return Ok(PreCommitSummary::empty());
    };
    let rust = match staged.rust {
        RustDecision::Run(root) => run_rust_checks(root).await,
        RustDecision::Done(outcome) => vec![outcome],
    };
    let mut outcomes = staged.outcomes;
    outcomes.extend(rust);
    Ok(PreCommitSummary {
        staged: Some(staged.plan.summary()),
        outcomes,
    })
}

/// Results of every check that reads the staged tree, plus what the Rust checks must do next.
struct StagedResults {
    plan: CheckPlan,
    outcomes: Vec<CheckOutcome>,
    rust: RustDecision,
}

enum RustDecision {
    Run(PathBuf),
    Done(CheckOutcome),
}

fn check_staged_tree(
    args: &CheckArgs,
    cancel: &AtomicBool,
) -> Result<Option<StagedResults>, PreCommitError> {
    let tree = StagedTree::open(&args.root)?;
    let changes = tree.changes();
    if changes.is_empty() {
        return Ok(None);
    }
    let plan = CheckPlan::from_changes(&changes);
    let instruction_docs = instruction_docs_outcome(&tree, plan.has_docs, cancel);
    let large_files = large_files_outcome(
        &tree,
        &changes,
        args.large_file_threshold,
        args.large_file_watch_threshold,
        plan.large_files,
    );
    let rust = rust_decision(&tree, plan.has_rust);
    Ok(Some(StagedResults {
        plan,
        outcomes: vec![instruction_docs, large_files],
        rust,
    }))
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
            if is_large_file_candidate(change) {
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

fn is_large_file_candidate(change: &StagedChange) -> bool {
    !change.is_submodule && change.has_staged_blob() && is_source_file_path(&change.path)
}

fn instruction_docs_outcome(tree: &StagedTree, enabled: bool, cancel: &AtomicBool) -> CheckOutcome {
    if !enabled {
        return CheckOutcome::skip("instruction docs", "no staged instruction docs");
    }
    match scan_instruction_docs(tree, Some(cancel)) {
        Ok(report) if instruction_doc_report_passed(&report) => {
            CheckOutcome::pass("instruction docs", render_instruction_doc_report(&report))
        }
        Ok(report) => {
            CheckOutcome::fail("instruction docs", render_instruction_doc_report(&report))
        }
        Err(error) => CheckOutcome::error("instruction docs", error.to_string()),
    }
}

fn large_files_outcome(
    tree: &StagedTree,
    changes: &[StagedChange],
    threshold: usize,
    watch_threshold: usize,
    expected_files: usize,
) -> CheckOutcome {
    if expected_files == 0 {
        return CheckOutcome::skip("large files", "no staged source blobs");
    }
    let mut blobs = Vec::new();
    for change in changes
        .iter()
        .filter(|change| is_large_file_candidate(change))
    {
        match tree.blob(&change.path) {
            Ok(content) => blobs.push(SourceBlob::new(change.path.clone(), content)),
            Err(error) => return CheckOutcome::error("large files", error.to_string()),
        }
    }
    let watch_threshold = (watch_threshold != 0).then_some(watch_threshold);
    let config = match BlobScanConfig::new(blobs, threshold, watch_threshold) {
        Ok(config) => config,
        Err(error) => return CheckOutcome::error("large files", error.to_string()),
    };
    match scan_large_file_blobs(&config) {
        Ok(report) if large_file_report_passed(&report) => {
            CheckOutcome::pass("large files", render_large_file_report(&report))
        }
        Ok(report) => CheckOutcome::fail("large files", render_large_file_report(&report)),
        Err(error) => CheckOutcome::error("large files", error.to_string()),
    }
}

/// Decides whether the working-tree Rust tools may run: only when Cargo would read exactly the staged Rust-affecting content.
fn rust_decision(tree: &StagedTree, enabled: bool) -> RustDecision {
    if !enabled {
        return RustDecision::Done(CheckOutcome::skip(
            "rust checks",
            "no staged Rust-affecting files",
        ));
    }
    match worktree_drift(tree) {
        Ok(drift) if drift.is_empty() => RustDecision::Run(tree.root().to_path_buf()),
        Ok(drift) => RustDecision::Done(CheckOutcome::fail("rust checks", drift_lines(&drift))),
        Err(error) => RustDecision::Done(CheckOutcome::error("rust checks", error.to_string())),
    }
}

fn drift_lines(drift: &[WorktreeDrift]) -> Vec<String> {
    let mut lines = vec![format!(
        "[{PREFIX}] FAIL cargo fmt and clippy read the working tree, which differs from the index being committed in {} Rust-affecting path(s); not running them",
        drift.len()
    )];
    lines.extend(
        drift
            .iter()
            .map(|entry| format!("[{PREFIX}] FAIL {}", entry.message())),
    );
    lines.push(format!(
        "[{PREFIX}] HINT stage the changes, ignore or remove the untracked files, or set them aside with `git stash push --keep-index --include-untracked` before committing"
    ));
    lines
}

async fn run_rust_checks(repo_root: PathBuf) -> Vec<CheckOutcome> {
    let dry_run = std::env::var_os(DRY_RUN_ENV).is_some_and(|value| !value.is_empty());
    let mut outcomes = Vec::with_capacity(RUST_TOOLS.len());
    let mut runs = Vec::with_capacity(RUST_TOOLS.len());
    for tool in RUST_TOOLS {
        if dry_run {
            outcomes.push(CheckOutcome::skip(
                tool.name,
                format!("dry run: would run `{}`", tool.command_line()),
            ));
        } else {
            runs.push(run_tool_check(*tool, repo_root.clone()));
        }
    }
    let mut tasks = Vec::with_capacity(runs.len());
    for run in runs {
        tasks.push(tokio::spawn(run));
    }
    for task in tasks {
        outcomes.push(match task.await {
            Ok(outcome) => outcome,
            Err(error) => CheckOutcome::error("rust checks", format!("check task failed: {error}")),
        });
    }
    outcomes
}

async fn run_tool_check(tool: RustTool, cwd: PathBuf) -> CheckOutcome {
    let RustTool {
        name,
        program,
        args,
    } = tool;
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
    #[error(transparent)]
    Tree(#[from] StagedTreeError),
    #[error("staged-tree check task failed: {0}")]
    Task(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use staged_tree::{ChangeKind, EntryKind, StagedEntry};

    fn change(path: &str, kind: EntryKind, is_submodule: bool) -> StagedChange {
        StagedChange {
            path: PathBuf::from(path),
            kind: ChangeKind::Modified,
            staged: Some(StagedEntry {
                kind,
                id: gix::ObjectId::empty_blob(gix::hash::Kind::Sha1),
            }),
            is_submodule,
        }
    }

    #[test]
    fn staged_file_classification_detects_instruction_docs() {
        for path in [
            "README.md",
            "docs/AGENTS.md",
            "docs/CLAUDE.md",
            ".gitmodules",
        ] {
            let plan = CheckPlan::from_changes(&[change(path, EntryKind::File, false)]);
            assert!(plan.has_docs, "{path}");
        }
    }

    #[test]
    fn staged_file_classification_treats_submodule_as_pointer() {
        let plan = CheckPlan::from_changes(&[change("vendor/stim", EntryKind::Gitlink, true)]);

        assert_eq!(plan.submodules, 1);
        assert!(!plan.has_rust);
        assert!(!plan.has_docs);
        assert_eq!(plan.large_files, 0);
    }

    #[test]
    fn staged_file_classification_counts_only_staged_source_blobs() {
        let mut deleted = change("ops/pre-commit/src/old.rs", EntryKind::File, false);
        deleted.kind = ChangeKind::Deleted;
        deleted.staged = None;
        let plan = CheckPlan::from_changes(&[
            change("ops/pre-commit/src/main.rs", EntryKind::File, false),
            change("Cargo.lock", EntryKind::File, false),
            deleted,
        ]);

        assert_eq!(plan.large_files, 1);
        assert!(plan.has_rust);
    }
}
