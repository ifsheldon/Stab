//! Development CLI entrypoints used by the M0 oracle smoke tests.

#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact diagnostics"
    )
)]

use std::ffi::OsString;
use std::io::{Read, Write};

use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use thiserror::Error;

#[derive(Debug, Parser)]
#[command(
    name = "stab",
    version,
    about = "A Rust implementation of Stim-compatible core workflows."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// M0 oracle-only measurement smoke shim; full sample support arrives in M8.
    #[command(name = "sample", hide = true)]
    Sample(SampleArgs),
}

#[derive(Debug, Args)]
struct SampleArgs {
    /// Number of shots to sample.
    #[arg(long, default_value_t = 1)]
    shots: usize,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("failed to read stdin: {0}")]
    ReadInput(std::io::Error),

    #[error("failed to write output: {0}")]
    WriteOutput(std::io::Error),

    #[error(
        "M0 smoke sampler only supports M and MZ instructions, found {instruction:?} on line {line}"
    )]
    UnsupportedInstruction { line: usize, instruction: String },

    #[error("measurement instruction on line {line} must include at least one qubit target")]
    MissingMeasurementTarget { line: usize },

    #[error(
        "M0 smoke sampler only supports unsigned integer qubit targets, found {target:?} on line {line}"
    )]
    InvalidMeasurementTarget { line: usize, target: String },

    #[error("measurement count overflowed")]
    MeasurementCountOverflow,
}

/// Runs the CLI and returns a process exit code.
pub fn run_from<I, S, R, W, E>(args: I, mut input: R, mut stdout: W, mut stderr: E) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
    R: Read,
    W: Write,
    E: Write,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            return write_clap_error(error, stdout, stderr);
        }
    };

    let result = match cli.command {
        Some(Command::Sample(args)) => run_sample(args, &mut input, &mut stdout),
        None => {
            let error = Cli::command().error(
                ErrorKind::MissingSubcommand,
                "no command was given; try --help",
            );
            return write_clap_error(error, stdout, stderr);
        }
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            if writeln!(stderr, "error: {error}").is_err() {
                return 1;
            }
            1
        }
    }
}

fn write_clap_error<W, E>(error: clap::Error, mut stdout: W, mut stderr: E) -> i32
where
    W: Write,
    E: Write,
{
    let message = error.to_string();
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            if write!(stdout, "{message}").is_err() {
                return 1;
            }
            0
        }
        _ => {
            if write!(stderr, "{message}").is_err() {
                return 1;
            }
            2
        }
    }
}

fn run_sample<R, W>(args: SampleArgs, input: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let mut circuit = String::new();
    input
        .read_to_string(&mut circuit)
        .map_err(CliError::ReadInput)?;
    let measurement_count = count_smoke_measurements(&circuit)?;
    for _ in 0..args.shots {
        for _ in 0..measurement_count {
            stdout.write_all(b"0").map_err(CliError::WriteOutput)?;
        }
        stdout.write_all(b"\n").map_err(CliError::WriteOutput)?;
    }
    Ok(())
}

fn count_smoke_measurements(circuit: &str) -> Result<usize, CliError> {
    let mut count = 0usize;
    for (line_index, raw_line) in circuit.lines().enumerate() {
        let line_number = line_index
            .checked_add(1)
            .ok_or(CliError::MeasurementCountOverflow)?;
        let line = raw_line
            .split_once('#')
            .map_or(raw_line, |(before_comment, _comment)| before_comment)
            .trim();
        if line.is_empty() || line == "TICK" {
            continue;
        }
        let mut parts = line.split_whitespace();
        let Some(instruction) = parts.next() else {
            continue;
        };
        if instruction != "M" && instruction != "MZ" {
            return Err(CliError::UnsupportedInstruction {
                line: line_number,
                instruction: instruction.to_string(),
            });
        }

        let mut targets_on_line = 0usize;
        for target in parts {
            if target.parse::<u64>().is_err() {
                return Err(CliError::InvalidMeasurementTarget {
                    line: line_number,
                    target: target.to_string(),
                });
            }
            targets_on_line = targets_on_line
                .checked_add(1)
                .ok_or(CliError::MeasurementCountOverflow)?;
        }
        if targets_on_line == 0 {
            return Err(CliError::MissingMeasurementTarget { line: line_number });
        }
        count = count
            .checked_add(targets_on_line)
            .ok_or(CliError::MeasurementCountOverflow)?;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::run_from;

    #[test]
    fn smoke_sampler_outputs_zero_measurements_for_each_shot() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            ["stab", "sample", "--shots", "2"],
            "M 0 1\n".as_bytes(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 0);
        assert_eq!(String::from_utf8(stdout).unwrap(), "00\n00\n");
        assert_eq!(String::from_utf8(stderr).unwrap(), "");
    }

    #[test]
    fn smoke_sampler_ignores_comments_and_ticks() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            ["stab", "sample", "--shots=1"],
            "# comment\nTICK\nMZ 2 # after\n".as_bytes(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 0);
        assert_eq!(String::from_utf8(stdout).unwrap(), "0\n");
        assert_eq!(String::from_utf8(stderr).unwrap(), "");
    }

    #[test]
    fn smoke_sampler_rejects_non_smoke_instructions() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(
            ["stab", "sample"],
            "H 0\nM 0\n".as_bytes(),
            &mut stdout,
            &mut stderr,
        );

        assert_eq!(status, 1);
        assert_eq!(String::from_utf8(stdout).unwrap(), "");
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("only supports M and MZ")
        );
    }

    #[test]
    fn smoke_sampler_is_hidden_from_public_help() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_from(["stab", "--help"], "".as_bytes(), &mut stdout, &mut stderr);

        assert_eq!(status, 0);
        assert!(!String::from_utf8(stdout).unwrap().contains("sample"));
        assert_eq!(String::from_utf8(stderr).unwrap(), "");
    }
}
