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
    /// Copy a tagged stab binary and write its SHA-256 sidecar.
    PackageBinary {
        /// Built stab executable below the repository root.
        #[arg(long)]
        binary: PathBuf,
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
}

fn main() {
    let result = match Cli::parse().command {
        Command::Check { out } => stab_release::check(&out),
        Command::PublishOrder => stab_release::print_publish_order(),
        Command::PackageBinary {
            binary,
            target,
            out,
            tag,
        } => stab_release::package_binary(&binary, &target, &out, &tag),
    };
    if let Err(error) = result {
        eprintln!("[stab-release] error: {error}");
        std::process::exit(1);
    }
}
