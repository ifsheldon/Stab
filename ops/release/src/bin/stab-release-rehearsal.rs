use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "stab-release-rehearsal")]
#[command(about = "Non-production GitHub draft rehearsal for Stab")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(name = "__isolated-cargo", hide = true)]
    IsolatedCargo {
        #[arg(long)]
        cargo: PathBuf,
        #[arg(long)]
        rustc: PathBuf,
        #[arg(long)]
        rustdoc: PathBuf,
        #[arg(long)]
        home: PathBuf,
        #[arg(long)]
        cargo_home: PathBuf,
        #[arg(long)]
        target: PathBuf,
        #[arg(long)]
        temporary: PathBuf,
        #[arg(long)]
        config: PathBuf,
        #[arg(last = true, allow_hyphen_values = true)]
        cargo_args: Vec<OsString>,
    },
    /// Print the source-derived rehearsal tag for the clean current commit.
    Tag,
    /// Build and package one rehearsal binary with provenance and checksums.
    BuildBinary {
        #[arg(long)]
        target: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        tag: String,
    },
    /// Verify the complete two-target rehearsal asset set.
    VerifyAssets {
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        tag: String,
    },
    /// Create and verify a private draft in the fixed scratch repository.
    CreateDraft {
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        confirm_repository: String,
    },
    /// Revalidate the private scratch draft and exact assets.
    VerifyRemoteDraft {
        #[arg(long)]
        assets: PathBuf,
        #[arg(long)]
        tag: String,
    },
}

fn main() {
    let result = match Cli::parse().command {
        Command::IsolatedCargo {
            cargo,
            rustc,
            rustdoc,
            home,
            cargo_home,
            target,
            temporary,
            config,
            cargo_args,
        } => stab_release::execute_isolated_cargo(
            &cargo,
            &rustc,
            &rustdoc,
            &home,
            &cargo_home,
            &target,
            &temporary,
            &config,
            &cargo_args,
        ),
        Command::Tag => stab_release::rehearsal_tag().map(|tag| println!("{tag}")),
        Command::BuildBinary { target, out, tag } => {
            stab_release::build_rehearsal_binary(&target, &out, &tag)
        }
        Command::VerifyAssets { assets, tag } => {
            stab_release::verify_rehearsal_assets(&assets, &tag)
        }
        Command::CreateDraft {
            assets,
            tag,
            confirm_repository,
        } => stab_release::create_verified_rehearsal_draft(&assets, &tag, &confirm_repository),
        Command::VerifyRemoteDraft { assets, tag } => {
            stab_release::verify_rehearsal_draft(&assets, &tag)
        }
    };
    if let Err(error) = result {
        eprintln!("[stab-release-rehearsal] error: {error}");
        std::process::exit(1);
    }
}
