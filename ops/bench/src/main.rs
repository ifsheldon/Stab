//! Benchmark orchestration for pinned Stim and Stab benchmark contracts.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

mod allocations;
mod baseline;
mod compare;
mod config;
mod error;
mod manifest;
mod process;
mod report;
mod root;
mod stim;

use std::path::PathBuf;
use std::process::ExitCode;

use baseline::{BaselineOptions, run_baseline};
use clap::{Parser, Subcommand};
use compare::{CompareOptions, run_compare};
use config::{DEFAULT_BASELINE_DIR, DEFAULT_BASELINE_REPORT, DEFAULT_STIM_PATH, PREFIX};
use error::BenchError;
use manifest::BenchmarkManifest;
use root::RepoRoot;

#[derive(Debug, Parser)]
#[command(
    about = "Runs Stab benchmark contract and baseline workflows.",
    long_about = "Validates benchmark contracts, builds pinned C++ Stim benchmark targets, records baseline results, and reports planned Stab-vs-Stim comparisons."
)]
struct Cli {
    /// Repository root containing Cargo.toml and vendor/stim.
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Print planned benchmark contracts grouped by milestone and threshold.
    List {
        /// Restrict output to one milestone such as M8.
        #[arg(long)]
        milestone: Option<String>,
    },

    /// Validate benchmark contracts without running long benchmark workloads.
    Smoke,

    /// Record pinned C++ Stim baseline benchmark results.
    Baseline {
        /// Path to the pinned Stim source checkout.
        #[arg(long, default_value = DEFAULT_STIM_PATH)]
        stim: PathBuf,

        /// Directory receiving baseline.json and report.md.
        #[arg(long, default_value = DEFAULT_BASELINE_DIR)]
        out: PathBuf,

        /// Target seconds passed to Stim's stim_perf harness for each filter.
        #[arg(long, default_value_t = 0.01)]
        target_seconds: f64,

        /// Number of process launches for Stim CLI benchmark rows.
        #[arg(long, default_value_t = 3)]
        cli_iterations: u32,

        /// Run only rows whose id or milestone matches this value.
        #[arg(long = "only")]
        only: Vec<String>,

        /// Force reconfiguration and rebuild of the pinned Stim benchmark binaries.
        #[arg(long)]
        rebuild_stim: bool,
    },

    /// Report planned Stab-vs-Stim benchmark comparisons.
    Compare {
        /// Restrict comparison planning to one milestone such as M8.
        #[arg(long)]
        milestone: Option<String>,

        /// Run the benchmark comparison under the named Cargo profile.
        #[arg(long, default_value = "release")]
        profile: String,

        /// Compare the frozen M12 primary matrix instead of every manifest row.
        #[arg(long)]
        primary: bool,

        /// Baseline JSON report produced by `bench::baseline`.
        #[arg(long, default_value = DEFAULT_BASELINE_REPORT)]
        baseline: PathBuf,

        /// Directory receiving compare.json and report.md.
        #[arg(long)]
        report: Option<PathBuf>,

        /// Fail when rows slower than the hot-path threshold do not have valid profiler notes beside the report.
        #[arg(long)]
        require_profiler_notes: bool,

        /// Fail when selected rows do not prove the 2.0x beta performance gate.
        #[arg(long)]
        require_beta_gate: bool,

        /// Measure Stab-side allocation counts using the count-allocations feature.
        #[arg(long)]
        track_allocations: bool,

        /// Exit with an error while Stab benchmark comparison runners are pending.
        #[arg(long)]
        strict: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("[{PREFIX}] ERROR: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), BenchError> {
    let root = RepoRoot::resolve(&cli.root)?;
    let manifest = BenchmarkManifest::read(&root)?;
    manifest.check(&root)?;
    match cli.command {
        Command::List { milestone } => {
            manifest.list(milestone.as_deref());
        }
        Command::Smoke => {
            println!(
                "[{PREFIX}] benchmark manifest OK: {} planned rows",
                manifest.rows.len()
            );
        }
        Command::Baseline {
            stim,
            out,
            target_seconds,
            cli_iterations,
            only,
            rebuild_stim,
        } => {
            run_baseline(
                &root,
                &manifest,
                &BaselineOptions {
                    stim,
                    out,
                    target_seconds,
                    cli_iterations,
                    only,
                    rebuild_stim,
                },
            )?;
        }
        Command::Compare {
            milestone,
            profile,
            primary,
            baseline,
            report,
            require_profiler_notes,
            require_beta_gate,
            track_allocations,
            strict,
        } => {
            run_compare(
                &root,
                &manifest,
                &CompareOptions {
                    baseline,
                    milestone,
                    profile,
                    primary,
                    report,
                    require_profiler_notes,
                    require_beta_gate,
                    track_allocations,
                    strict,
                },
            )?;
        }
    }
    Ok(())
}
