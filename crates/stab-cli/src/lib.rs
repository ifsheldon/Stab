//! Development CLI entrypoints used by oracle compatibility tests.

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
use std::path::PathBuf;

mod agent;
mod analyze_errors;
mod batch_output;
mod convert;
mod detection;
mod diagnostics;
mod help;
mod input;
mod io_plan;
mod sample_dem;
mod streaming;

use agent::{CapabilitiesArgs, InspectArgs, PlanArgs, run_capabilities, run_inspect, run_plan};
use analyze_errors::{AnalyzeErrorsArgs, run_analyze_errors};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use convert::{ConvertArgs, run_convert};
use detection::{DetectArgs, M2dArgs, run_detect, run_m2d};
use diagnostics::{
    CliError, ErrorFormatArg, probe_error_format, write_cli_error, write_frame0_deprecation,
    write_prepend_observables_deprecation,
};
use help::{HelpArgs, run_help};
use input::{read_limited_input_file, read_limited_stdin};
use io_plan::{FileRole, PendingIo};
use sample_dem::{SampleDemArgs, run_sample_dem};
use stab_core::{
    BitPlane64Batch, Circuit, CircuitError, CircuitItem, CircuitResult, CodeDistance,
    ColorCodeParams, ColorCodeTask, GeneratedCircuit, MeasurementBatchView, MeasurementSink,
    Probability, RandomPolicy, ReferenceSampleMode, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, RunError, SampleFormat, SamplingCompiler, SamplingSession, Seed, ShotCount,
    SurfaceCodeParams, SurfaceCodeTask,
    advanced::records::{MeasureRecordWriter, validate_ptb64_shot_count},
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};

pub(crate) const MAX_CIRCUIT_INPUT_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_CONVERT_INPUT_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Parser)]
#[command(
    name = "stab",
    version,
    disable_help_subcommand = true,
    about = "A Rust implementation of Stim-compatible core workflows."
)]
struct Cli {
    /// Selects human-readable or JSON Lines diagnostics.
    #[arg(
        long = "error-format",
        value_enum,
        default_value_t = ErrorFormatArg::Human,
        global = true
    )]
    error_format: ErrorFormatArg,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints Stab-native command, format, and gate help.
    Help(HelpArgs),

    /// Reports machine-readable product capabilities.
    Capabilities(CapabilitiesArgs),

    /// Parses and summarizes a circuit or detector error model without executing it.
    Inspect(InspectArgs),

    /// Validates and describes an operation without executing it.
    Plan(PlanArgs),

    /// Generates example circuits.
    Gen(GenArgs),

    /// Converts supported result data between text formats.
    Convert(ConvertArgs),

    /// Samples measurements from a circuit.
    #[command(name = "sample")]
    Sample(SampleArgs),

    /// Samples detector events from a circuit.
    #[command(name = "detect")]
    Detect(DetectArgs),

    /// Converts measurements into detector events.
    #[command(name = "m2d")]
    M2d(M2dArgs),

    /// Converts a circuit into a detector error model.
    #[command(name = "analyze_errors")]
    AnalyzeErrors(AnalyzeErrorsArgs),

    /// Samples detection events from a detector error model.
    #[command(name = "sample_dem")]
    SampleDem(SampleDemArgs),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GeneratedCodeArg {
    #[value(name = "repetition_code")]
    Repetition,
    #[value(name = "surface_code")]
    Surface,
    #[value(name = "color_code")]
    Color,
}

impl GeneratedCodeArg {
    fn as_stim_name(self) -> &'static str {
        match self {
            Self::Repetition => "repetition_code",
            Self::Surface => "surface_code",
            Self::Color => "color_code",
        }
    }
}

#[derive(Debug, Args)]
struct GenArgs {
    /// Error-correcting code family to generate.
    #[arg(long, value_enum)]
    code: GeneratedCodeArg,

    /// Generated circuit task name.
    #[arg(long)]
    task: String,

    /// Code distance.
    #[arg(long)]
    distance: u32,

    /// Measurement rounds.
    #[arg(long, value_parser = parse_stim_u64)]
    rounds: u64,

    /// Depolarizing noise after Clifford gates.
    #[arg(long = "after_clifford_depolarization", default_value_t = 0.0)]
    after_clifford_depolarization: f64,

    /// Flip probability after reset gates.
    #[arg(long = "after_reset_flip_probability", default_value_t = 0.0)]
    after_reset_flip_probability: f64,

    /// Flip probability before measurement gates.
    #[arg(long = "before_measure_flip_probability", default_value_t = 0.0)]
    before_measure_flip_probability: f64,

    /// Depolarizing noise before each round starts.
    #[arg(long = "before_round_data_depolarization", default_value_t = 0.0)]
    before_round_data_depolarization: f64,

    /// Output path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Accepted for Stim compatibility and ignored by `stim gen`.
    #[arg(long = "in", hide = true)]
    _input: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RecordFormatArg {
    #[value(name = "01")]
    ZeroOne,
    #[value(name = "b8")]
    B8,
    #[value(name = "r8")]
    R8,
    #[value(name = "ptb64")]
    Ptb64,
    #[value(name = "hits")]
    Hits,
    #[value(name = "dets")]
    Dets,
    #[value(name = "stim")]
    Stim,
}

impl RecordFormatArg {
    fn name(self) -> &'static str {
        self.record_format()
            .map_or("stim", stab_core::RecordFormat::as_str)
    }

    fn record_format(self) -> Option<stab_core::RecordFormat> {
        match self {
            Self::ZeroOne => Some(stab_core::RecordFormat::ZeroOne),
            Self::B8 => Some(stab_core::RecordFormat::B8),
            Self::R8 => Some(stab_core::RecordFormat::R8),
            Self::Ptb64 => Some(stab_core::RecordFormat::Ptb64),
            Self::Hits => Some(stab_core::RecordFormat::Hits),
            Self::Dets => Some(stab_core::RecordFormat::Dets),
            Self::Stim => None,
        }
    }

    fn sample_format(self) -> Result<SampleFormat, CliError> {
        match self {
            Self::ZeroOne => Ok(SampleFormat::ZeroOne),
            Self::B8 => Ok(SampleFormat::B8),
            Self::R8 => Ok(SampleFormat::R8),
            Self::Hits => Ok(SampleFormat::Hits),
            Self::Dets => Ok(SampleFormat::Dets),
            Self::Ptb64 => Err(CliError::UnsupportedDetectionFormat { format: "ptb64" }),
            Self::Stim => Err(CliError::UnsupportedDetectionFormat { format: "stim" }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum SampleOutFormatArg {
    #[value(name = "01")]
    ZeroOne,
    #[value(name = "b8")]
    B8,
    #[value(name = "r8")]
    R8,
    #[value(name = "ptb64")]
    Ptb64,
    #[value(name = "hits")]
    Hits,
    #[value(name = "dets")]
    Dets,
}

impl SampleOutFormatArg {
    fn record_format(self) -> stab_core::RecordFormat {
        match self {
            Self::ZeroOne => stab_core::RecordFormat::ZeroOne,
            Self::B8 => stab_core::RecordFormat::B8,
            Self::R8 => stab_core::RecordFormat::R8,
            Self::Ptb64 => stab_core::RecordFormat::Ptb64,
            Self::Hits => stab_core::RecordFormat::Hits,
            Self::Dets => stab_core::RecordFormat::Dets,
        }
    }

    fn stream_writer(self) -> Option<MeasureRecordWriter> {
        let format = match self {
            Self::ZeroOne => SampleFormat::ZeroOne,
            Self::B8 => SampleFormat::B8,
            Self::R8 => SampleFormat::R8,
            Self::Hits => SampleFormat::Hits,
            Self::Dets => SampleFormat::Dets,
            Self::Ptb64 => return None,
        };
        Some(MeasureRecordWriter::new(format))
    }
}

#[derive(Debug, Args)]
struct SampleArgs {
    /// Number of shots to sample.
    #[arg(long, default_value_t = 1, value_parser = parse_stim_usize)]
    shots: usize,

    /// Input circuit path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output sample path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Output sample format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: SampleOutFormatArg,

    /// Partially deterministic random seed for noisy sampling.
    #[arg(long, value_parser = parse_stim_u64)]
    seed: Option<u64>,

    /// Assert the noiseless reference sample is all zeroes.
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,

    /// Disable reference-sample loop folding.
    #[arg(long = "skip_loop_folding")]
    skip_loop_folding: bool,

    /// Deprecated Stim alias for --skip_reference_sample.
    #[arg(long = "frame0", hide = true)]
    frame0: bool,
}

fn parse_stim_usize(value: &str) -> Result<usize, String> {
    let parsed = parse_stim_i64_compatible_u64(value)?;
    usize::try_from(parsed).map_err(|_| format!("{value:?} does not fit in usize"))
}

fn parse_stim_u64(value: &str) -> Result<u64, String> {
    parse_stim_i64_compatible_u64(value)
}

fn parse_stim_i64_compatible_u64(value: &str) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|error| format!("{value:?} is not a non-negative 64-bit integer: {error}"))?;
    if parsed > i64::MAX as u64 {
        return Err(format!("{value:?} is greater than Stim's i64 maximum"));
    }
    Ok(parsed)
}

/// Runs the CLI and returns a process exit code.
pub fn run_from<I, S, R, W, E>(args: I, mut input: R, mut stdout: W, mut stderr: E) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    let args = normalize_legacy_args(args);
    let error_format_probe = probe_error_format(&args);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            return write_clap_error(error, error_format_probe, stdout, stderr);
        }
    };
    let Cli {
        error_format,
        command,
    } = cli;

    let result = match command {
        Some(Command::Help(args)) => run_help(args, &mut stdout),
        Some(Command::Capabilities(args)) => run_capabilities(args, &mut stdout),
        Some(Command::Inspect(args)) => run_inspect(args, &mut input, &mut stdout),
        Some(Command::Plan(args)) => run_plan(args, &mut input, &mut stdout),
        Some(Command::Gen(args)) => run_gen(args, &mut stdout),
        Some(Command::Convert(args)) => run_convert(args, &mut input, &mut stdout),
        Some(Command::Sample(args)) => {
            run_sample(args, error_format, &mut input, &mut stdout, &mut stderr)
        }
        Some(Command::Detect(args)) => {
            run_detect(args, error_format, &mut input, &mut stdout, &mut stderr)
        }
        Some(Command::M2d(args)) => run_m2d(args, &mut input, &mut stdout),
        Some(Command::AnalyzeErrors(args)) => run_analyze_errors(args, &mut input, &mut stdout),
        Some(Command::SampleDem(args)) => run_sample_dem(args, &mut input, &mut stdout),
        None => {
            let error = Cli::command().error(
                ErrorKind::MissingSubcommand,
                "no command was given; try --help",
            );
            return write_clap_error(error, error_format, stdout, stderr);
        }
    };

    match result {
        Ok(()) => 0,
        Err(error) => {
            // Pinned Stim dies silently via SIGPIPE when its output pipe
            // closes, observable as status 141 with empty stderr (decision
            // D2), so broken-pipe-rooted failures exit 141 without a
            // diagnostic while every other I/O failure keeps its report.
            if error_chain_is_broken_pipe(&error) {
                return BROKEN_PIPE_EXIT_CODE;
            }
            if write_cli_error(&mut stderr, error_format, &error).is_err() {
                return 1;
            }
            1
        }
    }
}

const BROKEN_PIPE_EXIT_CODE: i32 = 141;

fn error_chain_is_broken_pipe(error: &CliError) -> bool {
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = source {
        if let Some(io_error) = current.downcast_ref::<std::io::Error>()
            && io_error.kind() == std::io::ErrorKind::BrokenPipe
        {
            return true;
        }
        source = current.source();
    }
    false
}

fn normalize_legacy_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.len() < 2 {
        return args;
    }

    let mut legacy_index = 1;
    while let Some(arg) = args.get(legacy_index) {
        let arg = arg.to_string_lossy();
        if arg == "--error-format" {
            legacy_index = legacy_index.saturating_add(2);
        } else if arg.starts_with("--error-format=") {
            legacy_index = legacy_index.saturating_add(1);
        } else {
            break;
        }
    }
    if legacy_index >= args.len() {
        return args;
    }
    relocate_single_legacy_mode_flag(&mut args, legacy_index);

    let legacy_arg = args
        .get(legacy_index)
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(topic) = legacy_arg.strip_prefix("--help=") {
        args.splice(
            legacy_index..legacy_index + 1,
            [OsString::from("help"), OsString::from(topic)],
        );
    } else if legacy_arg == "--help" {
        if let Some(arg) = args.get_mut(legacy_index) {
            *arg = OsString::from("help");
        }
    } else if legacy_arg == "--convert" {
        if let Some(arg) = args.get_mut(legacy_index) {
            *arg = OsString::from("convert");
        }
    } else if let Some(code) = legacy_arg.strip_prefix("--gen=") {
        args.splice(
            legacy_index..legacy_index + 1,
            [
                OsString::from("gen"),
                OsString::from("--code"),
                OsString::from(code),
            ],
        );
    } else if legacy_arg == "--gen" && args.len() >= 3 {
        if let Some(arg) = args.get_mut(legacy_index) {
            *arg = OsString::from("gen");
        }
        args.insert(legacy_index + 1, OsString::from("--code"));
    } else if let Some(shots) = legacy_arg.strip_prefix("--sample=") {
        args.splice(
            legacy_index..legacy_index + 1,
            [
                OsString::from("sample"),
                OsString::from("--shots"),
                OsString::from(shots),
            ],
        );
    } else if legacy_arg == "--sample" {
        if let Some(arg) = args.get_mut(legacy_index) {
            *arg = OsString::from("sample");
        }
        // An explicit --shots elsewhere in the vector keeps the shot count,
        // matching pinned Stim's `--shots 2 --sample` behavior.
        let has_explicit_shots = args.iter().any(|arg| {
            let arg = arg.to_string_lossy();
            arg == "--shots" || arg.starts_with("--shots=")
        });
        if !has_explicit_shots {
            args.insert(legacy_index + 1, OsString::from("--shots"));
            if args
                .get(legacy_index + 2)
                .map(|arg| arg.to_string_lossy().starts_with('-'))
                .unwrap_or(true)
            {
                args.insert(legacy_index + 2, OsString::from("1"));
            }
        }
    } else if let Some(shots) = legacy_arg.strip_prefix("--detect=") {
        args.splice(
            legacy_index..legacy_index + 1,
            [
                OsString::from("detect"),
                OsString::from("--shots"),
                OsString::from(shots),
            ],
        );
    } else if legacy_arg == "--detect" {
        if let Some(arg) = args.get_mut(legacy_index) {
            *arg = OsString::from("detect");
        }
        if args
            .get(legacy_index + 1)
            .map(|arg| !arg.to_string_lossy().starts_with('-'))
            .unwrap_or(false)
        {
            args.insert(legacy_index + 1, OsString::from("--shots"));
        }
    } else if legacy_arg == "--m2d"
        && let Some(arg) = args.get_mut(legacy_index)
    {
        *arg = OsString::from("m2d");
    } else if legacy_arg == "--analyze_errors"
        && let Some(arg) = args.get_mut(legacy_index)
    {
        *arg = OsString::from("analyze_errors");
    }
    args
}

fn is_legacy_mode_flag(arg: &str) -> bool {
    matches!(
        arg,
        "--convert" | "--sample" | "--detect" | "--m2d" | "--analyze_errors" | "--gen"
    ) || arg.starts_with("--gen=")
        || arg.starts_with("--sample=")
        || arg.starts_with("--detect=")
}

/// Pinned Stim accepts its legacy mode flag anywhere in the argument vector,
/// so when exactly one appears after other flags it moves (with its adjacent
/// shot-count value for bare `--sample`/`--detect`) to the normalization
/// position; zero or several mode flags leave the vector unchanged so the
/// parser keeps rejecting ambiguous invocations.
fn relocate_single_legacy_mode_flag(args: &mut Vec<OsString>, legacy_index: usize) {
    let boundary = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    if legacy_index >= boundary {
        return;
    }
    let at_index = args
        .get(legacy_index)
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !at_index.starts_with("--") || is_legacy_mode_flag(&at_index) {
        return;
    }
    let mode_positions = (legacy_index + 1..boundary)
        .filter(|&index| {
            args.get(index)
                .is_some_and(|arg| is_legacy_mode_flag(&arg.to_string_lossy()))
        })
        .collect::<Vec<_>>();
    let [position] = mode_positions.as_slice() else {
        return;
    };
    let mut end = position + 1;
    let takes_adjacent_value = args.get(*position).is_some_and(|arg| {
        matches!(
            arg.to_string_lossy().as_ref(),
            "--sample" | "--detect" | "--gen"
        )
    });
    if takes_adjacent_value
        && end < boundary
        && args
            .get(end)
            .is_some_and(|arg| !arg.to_string_lossy().starts_with('-'))
    {
        end += 1;
    }
    let moved = args.drain(*position..end).collect::<Vec<_>>();
    args.splice(legacy_index..legacy_index, moved);
}

fn run_gen<W>(args: GenArgs, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let circuit_text = generated_circuit_text(&args)?;
    write_output(args.output.as_ref(), stdout, circuit_text.as_bytes())
}

fn generated_circuit_text(args: &GenArgs) -> Result<String, CliError> {
    let rounds = RoundCount::try_new(args.rounds)?;
    let distance = CodeDistance::try_new(args.distance)?;
    let probabilities = GeneratorProbabilities::from_args(args)?;
    let generated = match args.code {
        GeneratedCodeArg::Repetition => {
            let params = probabilities.apply_repetition(RepetitionCodeParams::new(
                rounds,
                distance,
                parse_repetition_task(&args.task)?,
            )?);
            generate_repetition_code_circuit(&params)?
        }
        GeneratedCodeArg::Surface => {
            let params = probabilities.apply_surface(SurfaceCodeParams::new(
                rounds,
                distance,
                parse_surface_task(&args.task)?,
            )?);
            generate_surface_code_circuit(&params)?
        }
        GeneratedCodeArg::Color => {
            let params = probabilities.apply_color(ColorCodeParams::new(
                rounds,
                distance,
                parse_color_task(&args.task)?,
            )?);
            generate_color_code_circuit(&params)?
        }
    };
    Ok(format_generated_circuit(
        args.code.as_stim_name(),
        &args.task,
        rounds,
        distance,
        probabilities,
        &generated,
    ))
}

#[derive(Clone, Copy, Debug)]
struct GeneratorProbabilities {
    before_round_data_depolarization: Probability,
    before_measure_flip_probability: Probability,
    after_reset_flip_probability: Probability,
    after_clifford_depolarization: Probability,
}

impl GeneratorProbabilities {
    fn from_args(args: &GenArgs) -> Result<Self, CliError> {
        Ok(Self {
            before_round_data_depolarization: probability_arg(
                args.before_round_data_depolarization,
            )?,
            before_measure_flip_probability: probability_arg(args.before_measure_flip_probability)?,
            after_reset_flip_probability: probability_arg(args.after_reset_flip_probability)?,
            after_clifford_depolarization: probability_arg(args.after_clifford_depolarization)?,
        })
    }

    fn apply_repetition(self, params: RepetitionCodeParams) -> RepetitionCodeParams {
        params
            .with_before_round_data_depolarization(self.before_round_data_depolarization)
            .with_before_measure_flip_probability(self.before_measure_flip_probability)
            .with_after_reset_flip_probability(self.after_reset_flip_probability)
            .with_after_clifford_depolarization(self.after_clifford_depolarization)
    }

    fn apply_surface(self, params: SurfaceCodeParams) -> SurfaceCodeParams {
        params
            .with_before_round_data_depolarization(self.before_round_data_depolarization)
            .with_before_measure_flip_probability(self.before_measure_flip_probability)
            .with_after_reset_flip_probability(self.after_reset_flip_probability)
            .with_after_clifford_depolarization(self.after_clifford_depolarization)
    }

    fn apply_color(self, params: ColorCodeParams) -> ColorCodeParams {
        params
            .with_before_round_data_depolarization(self.before_round_data_depolarization)
            .with_before_measure_flip_probability(self.before_measure_flip_probability)
            .with_after_reset_flip_probability(self.after_reset_flip_probability)
            .with_after_clifford_depolarization(self.after_clifford_depolarization)
    }
}

fn format_generated_circuit(
    code_name: &str,
    task: &str,
    rounds: RoundCount,
    distance: CodeDistance,
    probabilities: GeneratorProbabilities,
    generated: &GeneratedCircuit,
) -> String {
    let mut out = String::new();
    out.push_str("# Generated ");
    out.push_str(code_name);
    out.push_str(" circuit.\n");
    out.push_str("# task: ");
    out.push_str(task);
    out.push('\n');
    out.push_str("# rounds: ");
    out.push_str(&rounds.get().to_string());
    out.push('\n');
    out.push_str("# distance: ");
    out.push_str(&distance.get().to_string());
    out.push('\n');
    write_probability_header(
        &mut out,
        "before_round_data_depolarization",
        probabilities.before_round_data_depolarization,
    );
    write_probability_header(
        &mut out,
        "before_measure_flip_probability",
        probabilities.before_measure_flip_probability,
    );
    write_probability_header(
        &mut out,
        "after_reset_flip_probability",
        probabilities.after_reset_flip_probability,
    );
    write_probability_header(
        &mut out,
        "after_clifford_depolarization",
        probabilities.after_clifford_depolarization,
    );
    out.push_str("# layout:\n");
    out.push_str(generated.layout_text());
    out.push_str(generated.hint_text());
    out.push_str(&generated.circuit().to_stim_string());
    out
}

fn parse_repetition_task(task: &str) -> Result<RepetitionCodeTask, CliError> {
    match task {
        "memory" => Ok(RepetitionCodeTask::Memory),
        _ => Err(CliError::UnsupportedRepetitionTask {
            task: task.to_string(),
        }),
    }
}

fn parse_surface_task(task: &str) -> Result<SurfaceCodeTask, CliError> {
    match task {
        "rotated_memory_x" => Ok(SurfaceCodeTask::RotatedMemoryX),
        "rotated_memory_z" => Ok(SurfaceCodeTask::RotatedMemoryZ),
        "unrotated_memory_x" => Ok(SurfaceCodeTask::UnrotatedMemoryX),
        "unrotated_memory_z" => Ok(SurfaceCodeTask::UnrotatedMemoryZ),
        _ => Err(CliError::UnsupportedSurfaceTask {
            task: task.to_string(),
        }),
    }
}

fn parse_color_task(task: &str) -> Result<ColorCodeTask, CliError> {
    match task {
        "memory_xyz" => Ok(ColorCodeTask::MemoryXyz),
        _ => Err(CliError::UnsupportedColorTask {
            task: task.to_string(),
        }),
    }
}

fn probability_arg(value: f64) -> CircuitResult<Probability> {
    Ok(Probability::try_new(value)?)
}

fn write_probability_header(out: &mut String, name: &str, value: Probability) {
    out.push_str("# ");
    out.push_str(name);
    out.push_str(": ");
    out.push_str(&value.stim_text().to_string());
    out.push('\n');
}

pub(crate) fn write_output<W>(
    path: Option<&PathBuf>,
    stdout: &mut W,
    output: &[u8],
) -> Result<(), CliError>
where
    W: Write,
{
    if let Some(path) = path {
        std::fs::write(path, output).map_err(|source| CliError::WritePath {
            path: path.clone(),
            source,
        })
    } else {
        stdout.write_all(output).map_err(CliError::WriteOutput)
    }
}

fn write_clap_error<W, E>(
    error: clap::Error,
    error_format: ErrorFormatArg,
    mut stdout: W,
    mut stderr: E,
) -> i32
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
            let result = match error_format {
                ErrorFormatArg::Human => write!(stderr, "{message}"),
                ErrorFormatArg::Json => diagnostics::write_clap_error(&mut stderr, &error),
            };
            if result.is_err() {
                return 1;
            }
            1
        }
    }
}

fn run_sample<R, W, E>(
    args: SampleArgs,
    error_format: ErrorFormatArg,
    input: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
    E: Write,
{
    if args.shots == 0 {
        PendingIo::reject_aliases_without_opening(
            [(FileRole::Input, args.input.as_deref())],
            [(FileRole::Output, args.output.as_deref())],
        )?;
        return Ok(());
    }
    if args.out_format == SampleOutFormatArg::Ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }
    let mut io = PendingIo::preflight(
        [(FileRole::Input, args.input.as_deref())],
        [(FileRole::Output, args.output.as_deref())],
    )?;
    if args.frame0 {
        write_frame0_deprecation(stderr, error_format).map_err(CliError::WriteOutput)?;
    }
    let input_bytes = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(
            &mut input_file,
            MAX_CIRCUIT_INPUT_BYTES,
            "sample circuit input",
        )?
    } else {
        read_limited_stdin(input, MAX_CIRCUIT_INPUT_BYTES, "sample circuit input")?
    };
    let circuit = Circuit::from_stim_bytes(&input_bytes)?;
    let plan = SamplingCompiler::new()
        .compile(&circuit)
        .map_err(CircuitError::from)?;
    let skip_reference_sample = args.skip_reference_sample || args.frame0;
    let visible_measurements = if args.shots == 1 && !skip_reference_sample {
        legacy_tableau_visible_measurements(&circuit)?
    } else {
        None
    };
    let random_policy = args.seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    });
    let reference_mode = if skip_reference_sample {
        ReferenceSampleMode::SkipReferenceSample
    } else {
        ReferenceSampleMode::UseReferenceSample
    };
    let mut session = plan
        .session_with_reference_mode(random_policy, reference_mode)
        .map_err(CircuitError::from)?;
    let shots = ShotCount::try_from(args.shots).map_err(CircuitError::from)?;
    let mut outputs = io.activate()?;
    if let Some(mut output) = outputs.take(FileRole::Output) {
        return write_sample_output(
            &mut session,
            shots,
            args.out_format,
            visible_measurements.as_deref(),
            &mut output,
        )
        .map_err(|source| CliError::SamplePath {
            path: output.path().to_path_buf(),
            source,
        });
    }
    write_sample_output(
        &mut session,
        shots,
        args.out_format,
        visible_measurements.as_deref(),
        stdout,
    )
    .map_err(CliError::SampleOutput)
}

pub(crate) fn parse_circuit_bytes(input: &[u8]) -> Result<Circuit, CliError> {
    Ok(Circuit::from_stim_bytes(input)?)
}

fn write_sample_output<W>(
    session: &mut SamplingSession,
    shots: ShotCount,
    format: SampleOutFormatArg,
    visible_measurements: Option<&[usize]>,
    output: &mut W,
) -> Result<(), RunError<std::io::Error>>
where
    W: Write,
{
    let mut sink = CliSampleSink {
        format,
        visible_measurements,
        filtered_record: visible_measurements.map(|indices| Vec::with_capacity(indices.len())),
        writer: format.stream_writer(),
        output,
    };
    session.run(shots, &mut sink).map(|_| ())
}

struct CliSampleSink<'a, W> {
    format: SampleOutFormatArg,
    visible_measurements: Option<&'a [usize]>,
    filtered_record: Option<Vec<bool>>,
    writer: Option<MeasureRecordWriter>,
    output: &'a mut W,
}

impl<W> MeasurementSink for CliSampleSink<'_, W>
where
    W: Write,
{
    type Error = std::io::Error;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        if self.format == SampleOutFormatArg::Ptb64 {
            if batch.shot_count() != 64 {
                return Err(std::io::Error::other(format!(
                    "ptb64 sample batch expected 64 shots, got {}",
                    batch.shot_count()
                )));
            }
            let converted;
            let planes = if let Some(bit_planes) = batch.bit_planes() {
                bit_planes
            } else {
                converted =
                    BitPlane64Batch::from_shot_major(batch.shot_major_records().ok_or_else(
                        || std::io::Error::other("measurement batch has no storage"),
                    )?)
                    .map_err(std::io::Error::other)?;
                converted.view()
            };
            for bit_index in 0..planes.bits_per_shot() {
                let plane = planes.plane(bit_index).map_err(std::io::Error::other)?;
                let word = plane.words().first().copied().ok_or_else(|| {
                    std::io::Error::other(
                        "ptb64 sample plane has no backing word for a 64-shot batch",
                    )
                })?;
                self.output.write_all(&word.to_le_bytes())?;
            }
            return Ok(());
        }

        let writer = self.writer.as_mut().ok_or_else(|| {
            std::io::Error::other("non-ptb64 sample sink has no result-format writer")
        })?;
        if let (Some(indices), Some(filtered_record)) =
            (self.visible_measurements, self.filtered_record.as_mut())
        {
            for shot_index in 0..batch.shot_count() {
                filtered_record.clear();
                for index in indices {
                    let bit = batch.get(shot_index, *index).ok_or_else(|| {
                        std::io::Error::other(format!(
                            "internal sample layout index {index} exceeded record width {}",
                            batch.width().get()
                        ))
                    })?;
                    filtered_record.push(bit);
                }
                writer.write_bits(filtered_record);
                writer.write_end();
            }
        } else if let Some(bit_planes) = batch.bit_planes() {
            writer
                .write_bit_plane_batch(bit_planes)
                .map_err(std::io::Error::other)?;
        } else {
            writer
                .write_packed_batch(
                    batch
                        .shot_major_records()
                        .ok_or_else(|| std::io::Error::other("measurement batch has no storage"))?,
                )
                .map_err(std::io::Error::other)?;
        }
        self.output.write_all(writer.buffered_bytes())?;
        writer
            .clear_buffered_bytes()
            .map_err(std::io::Error::other)?;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.output.flush()
    }
}

fn legacy_tableau_visible_measurements(circuit: &Circuit) -> Result<Option<Vec<usize>>, CliError> {
    // Stim v1.16's one-shot tableau CLI path records heralds for feedback but does not write them.
    if legacy_tableau_hidden_measurement_count(circuit)? == 0 {
        return Ok(None);
    }

    let mut visible = Vec::new();
    let mut measurement_index = 0usize;
    for instruction in circuit.iter_flattened_instructions() {
        if !instruction.gate().produces_measurements() {
            continue;
        }
        let produced = instruction.target_groups().len();
        let next_index = measurement_index
            .checked_add(produced)
            .ok_or(CliError::MeasurementCountOverflow)?;
        if !matches!(
            instruction.gate().canonical_name(),
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
        ) {
            visible.extend(measurement_index..next_index);
        }
        measurement_index = next_index;
    }
    Ok(Some(visible))
}

fn legacy_tableau_visible_measurement_count(circuit: &Circuit) -> Result<Option<usize>, CliError> {
    let hidden = legacy_tableau_hidden_measurement_count(circuit)?;
    if hidden == 0 {
        return Ok(None);
    }
    let visible = circuit
        .count_measurements()?
        .checked_sub(hidden)
        .ok_or(CliError::MeasurementCountOverflow)?;
    usize::try_from(visible)
        .map(Some)
        .map_err(|_| CliError::MeasurementCountOverflow)
}

fn legacy_tableau_hidden_measurement_count(circuit: &Circuit) -> Result<u64, CliError> {
    let mut hidden = 0u64;
    for item in circuit.items() {
        let contribution = match item {
            CircuitItem::Instruction(instruction)
                if matches!(
                    instruction.gate().canonical_name(),
                    "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
                ) =>
            {
                u64::try_from(instruction.target_groups().len())
                    .map_err(|_| CliError::MeasurementCountOverflow)?
            }
            CircuitItem::Instruction(_) => 0,
            CircuitItem::RepeatBlock(repeat) => {
                legacy_tableau_hidden_measurement_count(repeat.body())?
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or(CliError::MeasurementCountOverflow)?
            }
        };
        hidden = hidden
            .checked_add(contribution)
            .ok_or(CliError::MeasurementCountOverflow)?;
    }
    Ok(hidden)
}

#[cfg(test)]
mod tests;
