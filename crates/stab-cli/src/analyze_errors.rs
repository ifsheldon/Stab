use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_core::{ErrorAnalyzerOptions, Probability, circuit_to_detector_error_model};

use crate::{
    CliError,
    input::{read_limited_input_file, read_limited_stdin},
    io_plan::{FileRole, PendingIo},
    parse_circuit_bytes,
    streaming::OutputSink,
};

const MAX_ANALYZE_ERRORS_INPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Args)]
pub(crate) struct AnalyzeErrorsArgs {
    /// Input circuit path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detector error model path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Try to decompose composite errors into graphlike components.
    #[arg(long = "decompose_errors")]
    decompose_errors: bool,

    /// Keep decomposition from introducing graphlike remnant edges.
    #[arg(long = "block_decompose_from_introducing_remnant_edges")]
    block_decompose_from_introducing_remnant_edges: bool,

    /// Leave undecomposed errors in the output instead of failing decomposition.
    #[arg(long = "ignore_decomposition_failures")]
    ignore_decomposition_failures: bool,

    /// Preserve repeated circuit structure where possible.
    #[arg(long = "fold_loops")]
    fold_loops: bool,

    /// Permit gauge detectors during analysis.
    #[arg(long = "allow_gauge_detectors")]
    allow_gauge_detectors: bool,

    /// Approximate disjoint error channels during analysis, optionally limited to a threshold.
    #[arg(
        long = "approximate_disjoint_errors",
        num_args = 0..=1,
        default_missing_value = "1",
        value_parser = parse_probability_threshold,
    )]
    approximate_disjoint_errors: Option<Probability>,
}

pub(crate) fn run_analyze_errors<R, W>(
    args: AnalyzeErrorsArgs,
    input: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let mut io = PendingIo::preflight(
        [(FileRole::Input, args.input.as_deref())],
        [(FileRole::Output, args.output.as_deref())],
    )?;
    let mut input_file = io.take_input(FileRole::Input);
    let mut outputs = io.activate()?;
    let mut output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    let input_bytes = if let Some(input_file) = input_file.as_mut() {
        read_limited_input_file(
            input_file,
            MAX_ANALYZE_ERRORS_INPUT_BYTES,
            "analyze_errors input",
        )?
    } else {
        read_limited_stdin(
            input,
            MAX_ANALYZE_ERRORS_INPUT_BYTES,
            "analyze_errors input",
        )?
    };
    let circuit = parse_circuit_bytes(&input_bytes)?;
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: args.fold_loops,
            decompose_errors: args.decompose_errors,
            allow_gauge_detectors: args.allow_gauge_detectors,
            ignore_decomposition_failures: args.ignore_decomposition_failures,
            block_decomposition_from_introducing_remnant_edges: args
                .block_decompose_from_introducing_remnant_edges,
            approximate_disjoint_errors_threshold: args.approximate_disjoint_errors,
        },
    )?;
    output.write_with(|writer| writer.write_all(dem.to_dem_string().as_bytes()))
}

fn parse_probability_threshold(value: &str) -> Result<Probability, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("invalid probability threshold {value}"))?;
    Probability::try_new(parsed)
        .map_err(|_| format!("probability threshold {value} is not in [0, 1]"))
}
