use std::io::{Read, Write};
use std::path::PathBuf;

use clap::Args;
use stab_analysis::{
    DecompositionFailure, ErrorAnalyzerOptions, ErrorDecomposition, GaugeDetectors, LoopFolding,
    RemnantEdges, circuit_to_detector_error_model,
};
use stab_model::{MemoryBudget, Probability};

use crate::{
    CliError, MemoryBudgetArg, MemoryLedger,
    input::{read_limited_input_file, read_limited_stdin},
    io_plan::{FileRole, PendingIo, StandardStreams, StreamUse},
    output_storage::{EncodedBatches, OutputStorage},
    streaming::OutputSink,
};

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

    #[command(flatten)]
    memory: MemoryBudgetArg,
}

impl AnalyzeErrorsArgs {
    /// Maps the flags onto the analyzer's options. Like Stim v1.16.0, the two decomposition
    /// refinements are accepted without `--decompose_errors` and then have no effect.
    fn analyzer_options(&self, memory_budget: MemoryBudget) -> ErrorAnalyzerOptions {
        ErrorAnalyzerOptions {
            loops: if self.fold_loops {
                LoopFolding::Fold
            } else {
                LoopFolding::Unroll
            },
            decomposition: self.decompose_errors.then_some(ErrorDecomposition {
                on_failure: if self.ignore_decomposition_failures {
                    DecompositionFailure::Ignore
                } else {
                    DecompositionFailure::Fail
                },
                remnant_edges: if self.block_decompose_from_introducing_remnant_edges {
                    RemnantEdges::Block
                } else {
                    RemnantEdges::Allow
                },
            }),
            gauge_detectors: if self.allow_gauge_detectors {
                GaugeDetectors::Allow
            } else {
                GaugeDetectors::Reject
            },
            approximate_disjoint_errors: self.approximate_disjoint_errors,
            memory_budget,
            cancellation: stab_model::Cancellation::never(),
        }
    }
}

pub(crate) fn run_analyze_errors<R, W>(
    args: AnalyzeErrorsArgs,
    input: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let mut io = PendingIo::preflight(
        [(FileRole::Input, args.input.as_deref())],
        [(FileRole::Output, args.output.as_deref())],
    )?;
    io.reject_standard_stream_aliases(
        streams,
        StreamUse {
            reads_stdin: args.input.is_none(),
            writes_stdout: args.output.is_none(),
        },
    )?;
    let mut ledger = MemoryLedger::new(args.memory);
    let input_cap = ledger.stage().input_bytes();
    let mut input_file = io.take_input(FileRole::Input);
    let input_bytes = if let Some(input_file) = input_file.as_mut() {
        read_limited_input_file(input_file, input_cap, "analyze_errors input")?
    } else {
        read_limited_stdin(input, input_cap, "analyze_errors input")?
    };
    let (circuit, circuit_hold) = ledger.parse_circuit(input_bytes)?;
    // A named `--out` file's write buffer coexists with the model the analysis returns, so it is
    // held before the analysis admits that model against what is left.
    let _output_hold = OutputStorage::new("analyze_errors", EncodedBatches::ShotMajor, None)
        .file_buffer(args.output.is_some())
        .admit(&mut ledger)?;
    let analysis = ledger.stage();
    let dem = circuit_to_detector_error_model(&circuit, &args.analyzer_options(analysis.get()))
        .map_err(|error| analysis.refusal(error))?;
    drop(circuit);
    ledger.release(circuit_hold);
    let mut outputs = io.activate()?;
    let mut output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    output.write_with(|mut writer| {
        // A model with no items prints no text; pinned Stim still emits one newline.
        if dem.items().is_empty() {
            writer.write_all(b"\n")
        } else {
            dem.write_dem_bytes(&mut writer)
        }
    })?;
    output.flush()
}

fn parse_probability_threshold(value: &str) -> Result<Probability, String> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| format!("invalid probability threshold {value}"))?;
    Probability::try_new(parsed)
        .map_err(|_| format!("probability threshold {value} is not in [0, 1]"))
}
