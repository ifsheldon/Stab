use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "stab-release")]
#[command(about = "Release preflight and artifact packaging for Stab")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate and package every coordinated product crate from a clean revision.
    Check {
        /// New report directory below target/releases/.
        #[arg(long)]
        out: PathBuf,
    },
    /// Print the source-owned crates.io publication order.
    PublishOrder,
    /// Publish the immutable reviewed package set and verify crates.io checksums.
    PublishReviewed {
        /// Existing release preflight directory below target/releases/.
        #[arg(long)]
        preflight: PathBuf,
        /// Exact release version confirming the irreversible operation.
        #[arg(long)]
        confirm_version: String,
    },
    /// Build and package one tagged stab binary with provenance and checksums.
    BuildBinary {
        /// Release target label, such as linux-aarch64.
        #[arg(long)]
        target: String,
        /// New artifact directory below the repository root.
        #[arg(long)]
        out: PathBuf,
        /// Annotated release tag that must resolve to the current clean revision.
        #[arg(long)]
        tag: String,
    },
    /// Verify the complete two-target release asset set before draft publication.
    VerifyAssets {
        /// Directory containing both targets' binaries, checksums, and manifests.
        #[arg(long)]
        assets: PathBuf,
        /// Annotated release tag that must resolve to the current clean revision.
        #[arg(long)]
        tag: String,
    },
}

fn main() {
    let result = match Cli::parse().command {
        Command::Check { out } => stab_release::check(&out),
        Command::PublishOrder => stab_release::print_publish_order(),
        Command::PublishReviewed {
            preflight,
            confirm_version,
        } => stab_release::publish_reviewed(&preflight, &confirm_version),
        Command::BuildBinary { target, out, tag } => {
            stab_release::build_binary(&target, &out, &tag)
        }
        Command::VerifyAssets { assets, tag } => stab_release::verify_assets(&assets, &tag),
    };
    if let Err(error) = result {
        eprintln!("[stab-release] error: {error}");
        std::process::exit(1);
    }
}
