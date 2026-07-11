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

mod analyze_errors;
mod convert;
mod detection;
mod help;
mod input;
mod sample_dem;
mod streaming;

use analyze_errors::{AnalyzeErrorsArgs, run_analyze_errors};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use convert::{ConvertArgs, run_convert};
use detection::{DetectArgs, M2dArgs, run_detect, run_m2d};
use help::{HelpArgs, run_help};
pub(crate) use input::read_limited_input;
use sample_dem::{SampleDemArgs, run_sample_dem};
use stab_core::{
    Circuit, CircuitItem, CircuitResult, CodeDistance, ColorCodeParams, ColorCodeTask,
    CompiledSampler, GeneratedCircuit, Probability, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, SampleFormat, SurfaceCodeParams, SurfaceCodeTask, generate_color_code_circuit,
    generate_repetition_code_circuit, generate_surface_code_circuit,
    result_formats::{MeasureRecordWriter, validate_ptb64_shot_count},
};
use streaming::write_ptb64_group;
use thiserror::Error;

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
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints Stab-native command, format, and gate help.
    Help(HelpArgs),

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
        match self {
            Self::ZeroOne => "01",
            Self::B8 => "b8",
            Self::R8 => "r8",
            Self::Ptb64 => "ptb64",
            Self::Hits => "hits",
            Self::Dets => "dets",
            Self::Stim => "stim",
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
    fn sample_format(self) -> Result<SampleFormat, CliError> {
        match self {
            Self::ZeroOne => Ok(SampleFormat::ZeroOne),
            Self::B8 => Ok(SampleFormat::B8),
            Self::R8 => Ok(SampleFormat::R8),
            Self::Hits => Ok(SampleFormat::Hits),
            Self::Dets => Ok(SampleFormat::Dets),
            Self::Ptb64 => Err(CliError::UnsupportedDetectionFormat { format: "ptb64" }),
        }
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

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("failed to read stdin: {0}")]
    ReadInput(std::io::Error),

    #[error("failed to read {path}: {source}")]
    ReadPath {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to write output: {0}")]
    WriteOutput(std::io::Error),

    #[error("failed to write {path}: {source}")]
    WritePath {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("{0}")]
    Circuit(#[from] stab_core::CircuitError),

    #[error("unsupported repetition_code task {task:?}; expected memory")]
    UnsupportedRepetitionTask { task: String },

    #[error(
        "unsupported surface_code task {task:?}; expected rotated_memory_x, rotated_memory_z, unrotated_memory_x, or unrotated_memory_z"
    )]
    UnsupportedSurfaceTask { task: String },

    #[error("unsupported color_code task {task:?}; expected memory_xyz")]
    UnsupportedColorTask { task: String },

    #[error(
        "unsupported conversion; supported conversions are result formats 01, b8, r8, hits, dets, and ptb64 with explicit layout information, plus stim input to stim output"
    )]
    UnsupportedConversion,

    #[error("format {format} is not supported for detection data")]
    UnsupportedDetectionFormat { format: &'static str },

    #[error("cannot combine --prepend_observables, --append_observables, or --obs_out")]
    ConflictingObservableRouting,

    #[error("replay error input has {actual} records but --shots requested {expected}")]
    ReplayErrorRecordCountMismatch { expected: usize, actual: usize },

    #[error("{kind} is too large; limit is {limit} bytes")]
    InputTooLarge { kind: &'static str, limit: u64 },

    #[error("not enough information given to parse input file")]
    MissingRecordWidth,

    #[error(
        "not enough information given to parse input file to write to dets; provide explicit measurement, detector, or observable counts"
    )]
    MissingRecordTypesForDets,

    #[error("--circuit requires --types to select M, D, or L records")]
    MissingConvertTypes,

    #[error("--types contains unknown result type {result_type:?}; expected M, D, or L")]
    UnknownConvertType { result_type: char },

    #[error("--types contains duplicate result type {result_type}")]
    DuplicateConvertType { result_type: char },

    #[error("ptb64 output requires records in groups of 64; got trailing group of {count}")]
    IncompletePtb64OutputGroup { count: usize },

    #[error("unrecognized help topic {topic:?}")]
    UnknownHelpTopic { topic: String },

    #[error("input is not valid UTF-8 text")]
    InvalidUtf8Input,

    #[error("measurement count overflowed")]
    MeasurementCountOverflow,
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
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            return write_clap_error(error, stdout, stderr);
        }
    };

    let result = match cli.command {
        Some(Command::Help(args)) => run_help(args, &mut stdout),
        Some(Command::Gen(args)) => run_gen(args, &mut stdout),
        Some(Command::Convert(args)) => run_convert(args, &mut input, &mut stdout),
        Some(Command::Sample(args)) => run_sample(args, &mut input, &mut stdout, &mut stderr),
        Some(Command::Detect(args)) => run_detect(args, &mut input, &mut stdout, &mut stderr),
        Some(Command::M2d(args)) => run_m2d(args, &mut input, &mut stdout),
        Some(Command::AnalyzeErrors(args)) => run_analyze_errors(args, &mut input, &mut stdout),
        Some(Command::SampleDem(args)) => run_sample_dem(args, &mut input, &mut stdout),
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

fn normalize_legacy_args<I, S>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.len() < 2 {
        return args;
    }

    let legacy_arg = args
        .get(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .unwrap_or_default();
    if let Some(topic) = legacy_arg.strip_prefix("--help=") {
        args.splice(1..2, [OsString::from("help"), OsString::from(topic)]);
    } else if legacy_arg == "--help" {
        if let Some(arg) = args.get_mut(1) {
            *arg = OsString::from("help");
        }
    } else if legacy_arg == "--convert" {
        if let Some(arg) = args.get_mut(1) {
            *arg = OsString::from("convert");
        }
    } else if let Some(code) = legacy_arg.strip_prefix("--gen=") {
        args.splice(
            1..2,
            [
                OsString::from("gen"),
                OsString::from("--code"),
                OsString::from(code),
            ],
        );
    } else if legacy_arg == "--gen" && args.len() >= 3 {
        if let Some(arg) = args.get_mut(1) {
            *arg = OsString::from("gen");
        }
        args.insert(2, OsString::from("--code"));
    } else if let Some(shots) = legacy_arg.strip_prefix("--sample=") {
        args.splice(
            1..2,
            [
                OsString::from("sample"),
                OsString::from("--shots"),
                OsString::from(shots),
            ],
        );
    } else if legacy_arg == "--sample" {
        if let Some(arg) = args.get_mut(1) {
            *arg = OsString::from("sample");
        }
        args.insert(2, OsString::from("--shots"));
        if args
            .get(3)
            .map(|arg| arg.to_string_lossy().starts_with('-'))
            .unwrap_or(true)
        {
            args.insert(3, OsString::from("1"));
        }
    } else if let Some(shots) = legacy_arg.strip_prefix("--detect=") {
        args.splice(
            1..2,
            [
                OsString::from("detect"),
                OsString::from("--shots"),
                OsString::from(shots),
            ],
        );
    } else if legacy_arg == "--detect" {
        if let Some(arg) = args.get_mut(1) {
            *arg = OsString::from("detect");
        }
        if args
            .get(2)
            .map(|arg| !arg.to_string_lossy().starts_with('-'))
            .unwrap_or(false)
        {
            args.insert(2, OsString::from("--shots"));
        }
    } else if legacy_arg == "--m2d"
        && let Some(arg) = args.get_mut(1)
    {
        *arg = OsString::from("m2d");
    } else if legacy_arg == "--analyze_errors"
        && let Some(arg) = args.get_mut(1)
    {
        *arg = OsString::from("analyze_errors");
    }
    args
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
    Probability::try_new(value)
}

fn write_probability_header(out: &mut String, name: &str, value: Probability) {
    out.push_str("# ");
    out.push_str(name);
    out.push_str(": ");
    out.push_str(&value.get().to_string());
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
            1
        }
    }
}

fn run_sample<R, W, E>(
    args: SampleArgs,
    input: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
    E: Write,
{
    if args.frame0 {
        writeln!(
            stderr,
            "[DEPRECATION] Use `--skip_reference_sample` instead of `--frame0`"
        )
        .map_err(CliError::WriteOutput)?;
    }
    let input_bytes = read_limited_input(
        args.input.as_ref(),
        input,
        MAX_CIRCUIT_INPUT_BYTES,
        "sample circuit input",
    )?;
    let circuit_text = std::str::from_utf8(&input_bytes).map_err(|_| CliError::InvalidUtf8Input)?;
    let circuit = Circuit::from_stim_str(circuit_text)?;
    let sampler = CompiledSampler::compile(&circuit)?;
    let skip_reference_sample = args.skip_reference_sample || args.frame0;
    let visible_measurements = if args.shots == 1 && !skip_reference_sample {
        legacy_tableau_visible_measurements(&circuit)?
    } else {
        None
    };
    if args.out_format == SampleOutFormatArg::Ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }
    if let Some(output_path) = args.output.as_ref() {
        let mut output =
            std::fs::File::create(output_path).map_err(|source| CliError::WritePath {
                path: output_path.clone(),
                source,
            })?;
        return write_sample_output(
            &sampler,
            args.shots,
            args.out_format,
            args.seed,
            skip_reference_sample,
            visible_measurements.as_deref(),
            &mut output,
        )
        .map_err(|source| CliError::WritePath {
            path: output_path.clone(),
            source,
        });
    }
    write_sample_output(
        &sampler,
        args.shots,
        args.out_format,
        args.seed,
        skip_reference_sample,
        visible_measurements.as_deref(),
        stdout,
    )
    .map_err(CliError::WriteOutput)
}

pub(crate) fn parse_circuit_bytes(input: &[u8]) -> Result<Circuit, CliError> {
    let circuit_text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    Ok(Circuit::from_stim_str(circuit_text)?)
}

fn write_sample_output<W>(
    sampler: &CompiledSampler,
    shots: usize,
    format: SampleOutFormatArg,
    seed: Option<u64>,
    skip_reference_sample: bool,
    visible_measurements: Option<&[usize]>,
    output: &mut W,
) -> std::io::Result<()>
where
    W: Write,
{
    match format {
        SampleOutFormatArg::Ptb64 => {
            write_ptb64_sample_output(sampler, shots, seed, skip_reference_sample, output)
        }
        _ => write_record_sample_output(
            sampler,
            shots,
            format.sample_format().map_err(std::io::Error::other)?,
            seed,
            skip_reference_sample,
            visible_measurements,
            output,
        ),
    }
}

fn write_record_sample_output<W>(
    sampler: &CompiledSampler,
    shots: usize,
    format: SampleFormat,
    seed: Option<u64>,
    skip_reference_sample: bool,
    visible_measurements: Option<&[usize]>,
    output: &mut W,
) -> std::io::Result<()>
where
    W: Write,
{
    let mut filtered_record = visible_measurements.map(|indices| Vec::with_capacity(indices.len()));
    sampler.for_each_sample_with_seed_and_reference_mode(
        shots,
        seed,
        skip_reference_sample,
        |record| {
            let record = if let (Some(indices), Some(filtered_record)) =
                (visible_measurements, filtered_record.as_mut())
            {
                filtered_record.clear();
                for index in indices {
                    filtered_record.push(*record.get(*index).ok_or_else(|| {
                        std::io::Error::other(format!(
                            "internal sample layout index {index} exceeded record width {}",
                            record.len()
                        ))
                    })?);
                }
                filtered_record.as_slice()
            } else {
                record
            };
            let mut writer = MeasureRecordWriter::new(format);
            writer.write_bits(record);
            writer.write_end();
            output.write_all(&writer.into_bytes())
        },
    )
}

fn legacy_tableau_visible_measurements(circuit: &Circuit) -> Result<Option<Vec<usize>>, CliError> {
    // Stim v1.16's one-shot tableau CLI path records heralds for feedback but does not write them.
    if !circuit_contains_heralded_records(circuit) {
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

fn circuit_contains_heralded_records(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => matches!(
            instruction.gate().canonical_name(),
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
        ),
        CircuitItem::RepeatBlock(repeat) => circuit_contains_heralded_records(repeat.body()),
    })
}

fn write_ptb64_sample_output<W>(
    sampler: &CompiledSampler,
    shots: usize,
    seed: Option<u64>,
    skip_reference_sample: bool,
    output: &mut W,
) -> std::io::Result<()>
where
    W: Write,
{
    let mut group = Vec::with_capacity(64);
    sampler.for_each_sample_with_seed_and_reference_mode(
        shots,
        seed,
        skip_reference_sample,
        |record| {
            group.push(record.to_vec());
            if group.len() == 64 {
                write_ptb64_group(&group, output)?;
                group.clear();
            }
            Ok::<(), std::io::Error>(())
        },
    )?;
    debug_assert!(group.is_empty());
    Ok(())
}

fn write_empty_observables<W>(output_path: Option<&PathBuf>, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    write_output(Some(output_path), stdout, &[])
}

#[cfg(test)]
mod tests;
