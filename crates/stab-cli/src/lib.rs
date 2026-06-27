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
mod input;
mod sample_dem;

use analyze_errors::{AnalyzeErrorsArgs, run_analyze_errors};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
pub(crate) use input::read_limited_input;
use sample_dem::{SampleDemArgs, run_sample_dem};
use stab_core::{
    Circuit, CircuitResult, CodeDistance, ColorCodeParams, ColorCodeTask, CompiledSampler,
    DetectionConversionOptions, DetectionConversionOutput, DetectionObservableOutputMode,
    GeneratedCircuit, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    SampleFormat, SurfaceCodeParams, SurfaceCodeTask, convert_measurements_to_detection_events,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
    measurement_record_count, result_formats::read_measurement_records, sample_detection_events,
    validate_detection_sampling_circuit, write_detection_records, write_observable_records,
};
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
    #[arg(long)]
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum RecordFormatArg {
    #[value(name = "01")]
    ZeroOne,
    #[value(name = "b8")]
    B8,
    #[value(name = "r8")]
    R8,
    #[value(name = "hits")]
    Hits,
    #[value(name = "dets")]
    Dets,
    #[value(name = "stim")]
    Stim,
}

impl RecordFormatArg {
    fn sample_format(self) -> Result<SampleFormat, CliError> {
        match self {
            Self::ZeroOne => Ok(SampleFormat::ZeroOne),
            Self::B8 => Ok(SampleFormat::B8),
            Self::R8 => Ok(SampleFormat::R8),
            Self::Hits => Ok(SampleFormat::Hits),
            Self::Dets => Ok(SampleFormat::Dets),
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
struct ConvertArgs {
    /// Input record format.
    #[arg(long = "in_format", value_enum)]
    in_format: RecordFormatArg,

    /// Output record format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: RecordFormatArg,

    /// Input path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Number of measurement bits per shot.
    #[arg(long = "num_measurements", default_value_t = 0)]
    num_measurements: usize,

    /// Number of detector bits per shot.
    #[arg(long = "num_detectors", default_value_t = 0)]
    num_detectors: usize,

    /// Number of observable bits per shot.
    #[arg(long = "num_observables", default_value_t = 0)]
    num_observables: usize,

    /// Raw bits per shot when no value type is known.
    #[arg(long = "bits_per_shot", default_value_t = 0)]
    bits_per_shot: usize,
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

#[derive(Debug, Args)]
struct DetectArgs {
    /// Number of shots to sample.
    #[arg(long, default_value_t = 1, value_parser = parse_stim_usize)]
    shots: usize,

    /// Input circuit path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detection-event path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Output detection-event format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: SampleOutFormatArg,

    /// Partially deterministic random seed for noisy detection.
    #[arg(long, value_parser = parse_stim_u64)]
    seed: Option<u64>,

    /// Append observable flips after detector-event bits.
    #[arg(long = "append_observables")]
    append_observables: bool,

    /// Deprecated Stim alias that writes observable flips before detector bits.
    #[arg(long = "prepend_observables", hide = true)]
    prepend_observables: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,
}

#[derive(Debug, Args)]
struct M2dArgs {
    /// Circuit path used to interpret measurement records.
    #[arg(long)]
    circuit: PathBuf,

    /// Input measurement path. Defaults to stdin.
    #[arg(long = "in")]
    input: Option<PathBuf>,

    /// Output detection-event path. Defaults to stdout.
    #[arg(long = "out")]
    output: Option<PathBuf>,

    /// Input measurement format.
    #[arg(long = "in_format", value_enum)]
    in_format: RecordFormatArg,

    /// Output detection-event format.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: RecordFormatArg,

    /// Append observable flips after detector-event bits.
    #[arg(long = "append_observables")]
    append_observables: bool,

    /// Compare measurements directly instead of subtracting the circuit reference sample.
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,

    /// Optional separate observable-flip output path.
    #[arg(long = "obs_out")]
    obs_output: Option<PathBuf>,

    /// Separate observable-flip output format.
    #[arg(long = "obs_out_format", value_enum, default_value = "01")]
    obs_out_format: RecordFormatArg,

    /// Stim compatibility flag for externally transformed circuits.
    #[arg(long = "ran_without_feedback")]
    ran_without_feedback: bool,
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
        "unsupported conversion; supported conversions are 01 input to 01 or dets output, and stim input to stim output"
    )]
    UnsupportedConversion,

    #[error("format {format} is not supported for detection data")]
    UnsupportedDetectionFormat { format: &'static str },

    #[error(
        "--ran_without_feedback is not supported until feedback-removal conversion is implemented"
    )]
    UnsupportedRanWithoutFeedback,

    #[error("cannot combine --prepend_observables, --append_observables, or --obs_out")]
    ConflictingObservableRouting,

    #[error("replay error input has {actual} records but --shots requested {expected}")]
    ReplayErrorRecordCountMismatch { expected: usize, actual: usize },

    #[error("{kind} is too large; limit is {limit} bytes")]
    InputTooLarge { kind: &'static str, limit: u64 },

    #[error("not enough information given to parse input file")]
    MissingRecordWidth,

    #[error("input is not valid UTF-8 text")]
    InvalidUtf8Input,

    #[error("01 record on line {line} has {actual} bits but expected {expected}")]
    RecordWidthMismatch {
        line: usize,
        actual: usize,
        expected: usize,
    },

    #[error("01 record on line {line} contains invalid character {character:?}")]
    InvalidZeroOneCharacter { line: usize, character: char },

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
    if let Some(code) = legacy_arg.strip_prefix("--gen=") {
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

fn run_convert<R, W>(args: ConvertArgs, stdin: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    match (args.in_format, args.out_format) {
        (RecordFormatArg::ZeroOne, RecordFormatArg::ZeroOne | RecordFormatArg::Dets) => {
            run_convert_zero_one(args, stdin, stdout)
        }
        (RecordFormatArg::Stim, RecordFormatArg::Stim) => run_convert_stim(args, stdin, stdout),
        _ => Err(CliError::UnsupportedConversion),
    }
}

fn run_convert_zero_one<R, W>(
    args: ConvertArgs,
    stdin: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let width = convert_record_width(&args)?;
    let input = read_input(args.input.as_ref(), stdin)?;
    let records = parse_zero_one_records(&input, width)?;
    let output = match args.out_format {
        RecordFormatArg::ZeroOne => write_zero_one_records(&records),
        RecordFormatArg::Dets => write_dets_records(&records, &args),
        RecordFormatArg::B8
        | RecordFormatArg::R8
        | RecordFormatArg::Hits
        | RecordFormatArg::Stim => {
            return Err(CliError::UnsupportedConversion);
        }
    };
    write_output(args.output.as_ref(), stdout, output.as_bytes())
}

fn run_convert_stim<R, W>(args: ConvertArgs, stdin: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let input = read_input(args.input.as_ref(), stdin)?;
    let text = std::str::from_utf8(&input).map_err(|_| CliError::InvalidUtf8Input)?;
    let circuit = Circuit::from_stim_str(text)?;
    let output = circuit.to_stim_string();
    write_output(args.output.as_ref(), stdout, output.as_bytes())
}

fn convert_record_width(args: &ConvertArgs) -> Result<usize, CliError> {
    let typed_width = args
        .num_measurements
        .checked_add(args.num_detectors)
        .and_then(|value| value.checked_add(args.num_observables))
        .ok_or(CliError::MeasurementCountOverflow)?;
    if typed_width > 0 {
        Ok(typed_width)
    } else if args.bits_per_shot > 0 {
        Ok(args.bits_per_shot)
    } else {
        Err(CliError::MissingRecordWidth)
    }
}

pub(crate) fn read_input<R>(path: Option<&PathBuf>, stdin: &mut R) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    if let Some(path) = path {
        std::fs::read(path).map_err(|source| CliError::ReadPath {
            path: path.clone(),
            source,
        })
    } else {
        let mut input = Vec::new();
        stdin.read_to_end(&mut input).map_err(CliError::ReadInput)?;
        Ok(input)
    }
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

fn parse_zero_one_records(input: &[u8], width: usize) -> Result<Vec<Vec<bool>>, CliError> {
    let text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    let mut records = Vec::new();
    for (line_index, raw_line) in text.lines().enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let line_number = line_index
            .checked_add(1)
            .ok_or(CliError::MeasurementCountOverflow)?;
        let mut record = Vec::with_capacity(width);
        for character in line.chars() {
            match character {
                '0' => record.push(false),
                '1' => record.push(true),
                _ => {
                    return Err(CliError::InvalidZeroOneCharacter {
                        line: line_number,
                        character,
                    });
                }
            }
        }
        if record.len() != width {
            return Err(CliError::RecordWidthMismatch {
                line: line_number,
                actual: record.len(),
                expected: width,
            });
        }
        records.push(record);
    }
    Ok(records)
}

fn write_zero_one_records(records: &[Vec<bool>]) -> String {
    let mut out = String::new();
    for record in records {
        for bit in record {
            out.push(if *bit { '1' } else { '0' });
        }
        out.push('\n');
    }
    out
}

fn write_dets_records(records: &[Vec<bool>], args: &ConvertArgs) -> String {
    let mut out = String::new();
    for record in records {
        out.push_str("shot");
        let mut offset = 0usize;
        write_dets_record_type(&mut out, record, offset, args.num_measurements, 'M');
        offset += args.num_measurements;
        write_dets_record_type(&mut out, record, offset, args.num_detectors, 'D');
        offset += args.num_detectors;
        write_dets_record_type(&mut out, record, offset, args.num_observables, 'L');
        out.push('\n');
    }
    out
}

fn write_dets_record_type(
    out: &mut String,
    record: &[bool],
    offset: usize,
    count: usize,
    prefix: char,
) {
    for index in 0..count {
        if record.get(offset + index).copied().unwrap_or(false) {
            out.push(' ');
            out.push(prefix);
            out.push_str(&index.to_string());
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
    let input_bytes = read_input(args.input.as_ref(), input)?;
    let circuit_text = std::str::from_utf8(&input_bytes).map_err(|_| CliError::InvalidUtf8Input)?;
    let circuit = Circuit::from_stim_str(circuit_text)?;
    let sampler = CompiledSampler::compile(&circuit)?;
    let skip_reference_sample = args.skip_reference_sample || args.frame0;
    let output = match args.out_format {
        SampleOutFormatArg::ZeroOne => sampler.sample_bytes_with_seed_and_reference_mode(
            args.shots,
            SampleFormat::ZeroOne,
            args.seed,
            skip_reference_sample,
        ),
        SampleOutFormatArg::B8 => sampler.sample_bytes_with_seed_and_reference_mode(
            args.shots,
            SampleFormat::B8,
            args.seed,
            skip_reference_sample,
        ),
        SampleOutFormatArg::R8 => sampler.sample_bytes_with_seed_and_reference_mode(
            args.shots,
            SampleFormat::R8,
            args.seed,
            skip_reference_sample,
        ),
        SampleOutFormatArg::Ptb64 => sampler.sample_ptb64_bytes_with_seed_and_reference_mode(
            args.shots,
            args.seed,
            skip_reference_sample,
        )?,
        SampleOutFormatArg::Hits => sampler.sample_bytes_with_seed_and_reference_mode(
            args.shots,
            SampleFormat::Hits,
            args.seed,
            skip_reference_sample,
        ),
        SampleOutFormatArg::Dets => sampler.sample_bytes_with_seed_and_reference_mode(
            args.shots,
            SampleFormat::Dets,
            args.seed,
            skip_reference_sample,
        ),
    };
    write_output(args.output.as_ref(), stdout, &output)
}

fn run_detect<R, W, E>(
    args: DetectArgs,
    input: &mut R,
    stdout: &mut W,
    stderr: &mut E,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
    E: Write,
{
    if args.prepend_observables {
        writeln!(
            stderr,
            "[DEPRECATION] Avoid using `--prepend_observables`. Data readers assume observables are appended, not prepended."
        )
        .map_err(CliError::WriteOutput)?;
    }
    validate_detect_observable_routing(&args)?;
    if args.shots == 0 {
        write_output(args.output.as_ref(), stdout, &[])?;
        return write_empty_observables(args.obs_output.as_ref(), stdout);
    }
    let input_bytes = read_input(args.input.as_ref(), input)?;
    let circuit = parse_circuit_bytes(&input_bytes)?;
    validate_detection_sampling_circuit(&circuit)?;
    let detection_output = sample_detection_events(&circuit, args.shots, args.seed)?;
    let observable_mode = detect_observable_output_mode(&args);
    let output = write_detection_records(
        &detection_output,
        observable_mode,
        args.out_format.sample_format()?,
    )?;
    write_output(args.output.as_ref(), stdout, &output)?;
    write_optional_observables(
        args.obs_output.as_ref(),
        args.obs_out_format,
        stdout,
        &detection_output,
    )
}

fn run_m2d<R, W>(args: M2dArgs, input: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    if args.ran_without_feedback {
        return Err(CliError::UnsupportedRanWithoutFeedback);
    }
    let circuit_bytes = std::fs::read(&args.circuit).map_err(|source| CliError::ReadPath {
        path: args.circuit.clone(),
        source,
    })?;
    let circuit = parse_circuit_bytes(&circuit_bytes)?;
    let measurement_width = measurement_record_count(&circuit)?;
    let input_bytes =
        read_limited_input(args.input.as_ref(), input, MAX_M2D_INPUT_BYTES, "m2d input")?;
    let measurements = read_measurement_records(
        &input_bytes,
        args.in_format.sample_format()?,
        measurement_width,
    )?;
    let detection_output = convert_measurements_to_detection_events(
        &circuit,
        &measurements,
        DetectionConversionOptions {
            skip_reference_sample: args.skip_reference_sample,
        },
    )?;
    let output = write_detection_records(
        &detection_output,
        observable_output_mode(args.append_observables),
        args.out_format.sample_format()?,
    )?;
    write_output(args.output.as_ref(), stdout, &output)?;
    write_optional_observables(
        args.obs_output.as_ref(),
        args.obs_out_format,
        stdout,
        &detection_output,
    )
}

const MAX_M2D_INPUT_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn parse_circuit_bytes(input: &[u8]) -> Result<Circuit, CliError> {
    let circuit_text = std::str::from_utf8(input).map_err(|_| CliError::InvalidUtf8Input)?;
    Ok(Circuit::from_stim_str(circuit_text)?)
}

fn validate_detect_observable_routing(args: &DetectArgs) -> Result<(), CliError> {
    let effective_prepend = args.prepend_observables
        || (args.out_format == SampleOutFormatArg::Dets && !args.append_observables);
    let selected_routes = usize::from(effective_prepend)
        + usize::from(args.append_observables)
        + usize::from(args.obs_output.is_some());
    if selected_routes > 1 {
        return Err(CliError::ConflictingObservableRouting);
    }
    Ok(())
}

fn detect_observable_output_mode(args: &DetectArgs) -> DetectionObservableOutputMode {
    if args.append_observables {
        DetectionObservableOutputMode::Append
    } else if args.prepend_observables || args.out_format == SampleOutFormatArg::Dets {
        DetectionObservableOutputMode::Prepend
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
}

fn observable_output_mode(append_observables: bool) -> DetectionObservableOutputMode {
    if append_observables {
        DetectionObservableOutputMode::Append
    } else {
        DetectionObservableOutputMode::DetectorsOnly
    }
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

fn write_optional_observables<W>(
    output_path: Option<&PathBuf>,
    format: RecordFormatArg,
    stdout: &mut W,
    detection_output: &DetectionConversionOutput,
) -> Result<(), CliError>
where
    W: Write,
{
    let Some(output_path) = output_path else {
        return Ok(());
    };
    let output = write_observable_records(detection_output, format.sample_format()?)?;
    write_output(Some(output_path), stdout, &output)
}

#[cfg(test)]
mod tests;
