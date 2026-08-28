//! End-to-end performance evidence for pinned Stim and Stab workflows.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

mod config;
mod e2e;
mod error;
pub(crate) mod process;
mod root;
mod stim;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use config::PREFIX;
use error::BenchError;
use root::RepoRoot;

#[derive(Debug, Parser)]
#[command(about = "Runs Stab's source-owned end-to-end performance suite.")]
struct Cli {
    /// Repository root containing Cargo.toml and vendor/stim.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Internal reusable Rust workflow worker used by the E2E controller.
    #[command(name = "e2e-worker", hide = true)]
    Worker(e2e::WorkerArgs),

    /// Validate the source-owned suite and generated documentation.
    E2eCheck(e2e::CheckArgs),

    /// Run selected workflows and publish one replayable bundle.
    E2eRun(e2e::RunArgs),

    /// Replay a bundle and deterministically reconstruct its report.
    E2eReplay(e2e::ReplayArgs),

    /// Generate reviewed self-regression baselines from full and soak bundles.
    E2eBaselineCandidate(e2e::BaselineCandidateArgs),

    /// Validate the checked controlled-host evidence required for release.
    E2eReleaseCheck,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[{PREFIX}] ERROR: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), BenchError> {
    let Cli { root, command } = cli;
    let command = match command {
        Command::Worker(args) => return e2e::run_worker(args),
        command => command,
    };
    let root = RepoRoot::resolve(&root)?;
    match command {
        Command::Worker(_) => Err(BenchError::E2e(
            "E2E worker dispatch reached the repository controller".to_string(),
        )),
        Command::E2eCheck(args) => e2e::check(&root, args),
        Command::E2eRun(args) => e2e::run(&root, args),
        Command::E2eReplay(args) => e2e::replay(&root, args),
        Command::E2eBaselineCandidate(args) => e2e::baseline_candidate(&root, args),
        Command::E2eReleaseCheck => e2e::release_check(&root),
    }
}
