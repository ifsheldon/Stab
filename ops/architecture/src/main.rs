//! Command-line entry point for Stab architecture checks.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

const PREFIX: &str = "stab-architecture";

#[derive(Debug, Parser)]
#[command(about = "Checks Stab's Cargo dependency and SIMD source boundaries.")]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate the current workspace against the target product architecture.
    Check,
    /// Compile external Stable, Nightly, and mixed feature consumers.
    ConsumerCheck,
    /// Validate repository-owned Markdown links and heading anchors.
    DocsCheck,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Check => match stab_architecture::check_workspace(&cli.root) {
            Ok(report) => {
                report.print();
                if report.passed() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                }
            }
            Err(error) => {
                eprintln!("[{PREFIX}] ERROR: {error}");
                ExitCode::from(2)
            }
        },
        Command::ConsumerCheck => match stab_architecture::check_external_consumers(&cli.root) {
            Ok(summary) => {
                summary.print();
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("[{PREFIX}] ERROR: {error}");
                ExitCode::from(2)
            }
        },
        Command::DocsCheck => match stab_architecture::check_markdown_docs(&cli.root) {
            Ok(report) => {
                report.print();
                if report.passed() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::from(1)
                }
            }
            Err(error) => {
                eprintln!("[{PREFIX}] ERROR: {error}");
                ExitCode::from(2)
            }
        },
    }
}
