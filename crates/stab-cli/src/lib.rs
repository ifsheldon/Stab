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
use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use convert::{ConvertArgs, run_convert};
use detection::{DetectArgs, M2dArgs, run_detect, run_m2d};
use diagnostics::{CliError, ErrorFormatArg, probe_error_format, write_cli_error};
use help::{HelpArgs, run_help};
use input::{read_limited_input_file, read_limited_stdin};
use io_plan::{FileRole, PendingIo, StandardStreams, StreamUse};
use sample_dem::{SampleDemArgs, run_sample_dem};
use stab_analysis::{
    CodeDistance, ColorCodeParams, ColorCodeTask, GeneratedCircuit, RepetitionCodeParams,
    RepetitionCodeTask, RoundCount, SurfaceCodeParams, SurfaceCodeTask,
    generate_color_code_circuit, generate_repetition_code_circuit, generate_surface_code_circuit,
};
use stab_engine::{
    LaneWidth, RandomPolicy, ReferenceSampleLoopPolicy, ReferenceSampleMode, RunError,
    SamplingCompiler, SamplingSession, Seed, ShotCount,
};
use stab_model::{
    Cancellation, Circuit, FoldedCircuitSummary, FoldedCount, GateKind, MemoryBudget,
    MemoryBudgetParseError, ModelResult, ParseLimits, Probability,
};
use stab_records::{
    DetsResultType, MeasurementBatchView, MeasurementSink, RecordBatchPart, RecordEncoder,
    RecordFormat, RecordPart, validate_ptb64_shot_count,
};

/// Stab-native admission budget shared by every command that reads, compiles, executes, or
/// materializes work. It bounds memory, not running time.
#[derive(Clone, Copy, Debug, Args)]
pub(crate) struct MemoryBudgetArg {
    /// Memory budget in bytes, with an optional KiB, MiB, or GiB suffix. Inputs, parsed models,
    /// compiled plans, sessions, and materialized outputs are each admitted against it before
    /// allocation.
    #[arg(
        long = "memory-budget",
        value_name = "SIZE",
        default_value = "1GiB",
        value_parser = parse_memory_budget
    )]
    memory_budget: MemoryBudget,
}

impl Default for MemoryBudgetArg {
    fn default() -> Self {
        Self {
            memory_budget: MemoryBudget::DEFAULT,
        }
    }
}

impl MemoryBudgetArg {
    pub(crate) const fn get(self) -> MemoryBudget {
        self.memory_budget
    }

    pub(crate) fn parse_limits(self) -> ParseLimits {
        ParseLimits::default().with_memory_budget(self.memory_budget)
    }

    /// The input byte cap: one past this is refused before the input is retained.
    pub(crate) const fn input_bytes(self) -> u64 {
        self.memory_budget.bytes()
    }

    /// The per-record text cap of streamed record inputs. A record's buffer doubles while it is
    /// framed, so half the budget keeps the buffer itself within the budget.
    pub(crate) fn record_bytes(self) -> usize {
        self.memory_budget.bytes_usize() / 2
    }
}

fn parse_memory_budget(value: &str) -> Result<MemoryBudget, String> {
    value
        .parse()
        .map_err(|error: MemoryBudgetParseError| error.to_string())
}

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

/// Returns the complete Clap command descriptor used by the `stab` binary.
///
/// Repository tooling uses this descriptor to derive CLI capability and compatibility checks from
/// the parser that actually owns command dispatch.
pub fn command_descriptor() -> clap::Command {
    Cli::command()
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

    #[command(flatten)]
    memory: MemoryBudgetArg,

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
        self.record_format().map_or("stim", RecordFormat::as_str)
    }

    fn record_format(self) -> Option<RecordFormat> {
        match self {
            Self::ZeroOne => Some(RecordFormat::ZeroOne),
            Self::B8 => Some(RecordFormat::B8),
            Self::R8 => Some(RecordFormat::R8),
            Self::Ptb64 => Some(RecordFormat::Ptb64),
            Self::Hits => Some(RecordFormat::Hits),
            Self::Dets => Some(RecordFormat::Dets),
            Self::Stim => None,
        }
    }

    fn required_record_format(self) -> Result<RecordFormat, CliError> {
        let Some(record_format) = self.record_format() else {
            return Err(CliError::UnsupportedDetectionFormat { format: "stim" });
        };
        Ok(record_format)
    }
}

fn result_record_format_parser() -> impl clap::builder::TypedValueParser<Value = RecordFormatArg> {
    PossibleValuesParser::new(["01", "b8", "r8", "ptb64", "hits", "dets"])
        .try_map(|value| RecordFormatArg::from_str(&value, false).map_err(std::io::Error::other))
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
    fn record_format(self) -> RecordFormat {
        match self {
            Self::ZeroOne => RecordFormat::ZeroOne,
            Self::B8 => RecordFormat::B8,
            Self::R8 => RecordFormat::R8,
            Self::Ptb64 => RecordFormat::Ptb64,
            Self::Hits => RecordFormat::Hits,
            Self::Dets => RecordFormat::Dets,
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
    ///
    /// The seeded output is a function of the circuit, the seed, and the total `--shots`, which
    /// selects the frame lane width: totals of 1-192 and 257-448 shots sample 64-lane batches and
    /// every other total samples 256-lane batches.
    #[arg(long, value_parser = parse_stim_u64)]
    seed: Option<u64>,

    /// Assert the noiseless reference sample is all zeroes.
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,

    /// Disable reference-sample loop folding.
    #[arg(long = "skip_loop_folding")]
    skip_loop_folding: bool,

    #[command(flatten)]
    memory: MemoryBudgetArg,
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

/// Runs the CLI over caller-provided streams and returns a process exit code.
///
/// The streams are treated as detached from the filesystem, so file-role
/// aliasing checks cover only paths named by flags. Use [`run_process`] for
/// the real process standard streams.
pub fn run_from<I, S, R, W, E>(args: I, input: R, stdout: W, stderr: E) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    run_with_streams(args, input, stdout, stderr, &StandardStreams::detached())
}

/// Runs the CLI over this process's arguments and standard streams.
///
/// When standard input or output is a regular file, commands also reject
/// that file aliasing a named input or output before truncating anything.
///
/// Standard output passes through the same 64 KiB buffer as output files, and every command
/// flushes it before reporting success, so a failure on the final bytes still exits non-zero.
pub fn run_process() -> i32 {
    let streams = StandardStreams::capture();
    run_with_streams(
        std::env::args_os(),
        std::io::stdin().lock(),
        io_plan::buffered_output(std::io::stdout().lock()),
        std::io::stderr().lock(),
        &streams,
    )
}

fn run_with_streams<I, S, R, W, E>(
    args: I,
    mut input: R,
    mut stdout: W,
    mut stderr: E,
    streams: &StandardStreams,
) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<OsString>,
    R: Read,
    W: Write,
    E: Write,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<OsString>>();
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
        Some(Command::Inspect(args)) => run_inspect(args, &mut input, &mut stdout, streams),
        Some(Command::Plan(args)) => run_plan(args, &mut input, &mut stdout, streams),
        Some(Command::Gen(args)) => run_gen(args, &mut stdout),
        Some(Command::Convert(args)) => run_convert(args, &mut input, &mut stdout, streams),
        Some(Command::Sample(args)) => run_sample(args, &mut input, &mut stdout, streams),
        Some(Command::Detect(args)) => run_detect(args, &mut input, &mut stdout, streams),
        Some(Command::M2d(args)) => run_m2d(args, &mut input, &mut stdout, streams),
        Some(Command::AnalyzeErrors(args)) => {
            run_analyze_errors(args, &mut input, &mut stdout, streams)
        }
        Some(Command::SampleDem(args)) => run_sample_dem(args, &mut input, &mut stdout, streams),
        None => {
            let error = Cli::command().error(
                ErrorKind::MissingSubcommand,
                "no command was given; try --help",
            );
            return write_clap_error(error, error_format, stdout, stderr);
        }
    };

    // Buffered stdout can hold the final bytes until drop, where a write error
    // would be lost, so a command succeeds only after its output is flushed.
    let result = result.and_then(|()| stdout.flush().map_err(CliError::WriteOutput));
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
            let params = probabilities
                .apply_repetition(RepetitionCodeParams::new(
                    rounds,
                    distance,
                    parse_repetition_task(&args.task)?,
                )?)
                .with_memory_budget(args.memory.get());
            generate_repetition_code_circuit(&params)?
        }
        GeneratedCodeArg::Surface => {
            let params = probabilities
                .apply_surface(SurfaceCodeParams::new(
                    rounds,
                    distance,
                    parse_surface_task(&args.task)?,
                )?)
                .with_memory_budget(args.memory.get());
            generate_surface_code_circuit(&params)?
        }
        GeneratedCodeArg::Color => {
            let params = probabilities
                .apply_color(ColorCodeParams::new(
                    rounds,
                    distance,
                    parse_color_task(&args.task)?,
                )?)
                .with_memory_budget(args.memory.get());
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

fn probability_arg(value: f64) -> ModelResult<Probability> {
    Probability::try_new(value)
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

fn run_sample<R, W>(
    args: SampleArgs,
    input: &mut R,
    stdout: &mut W,
    streams: &StandardStreams,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
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
    io.reject_standard_stream_aliases(
        streams,
        StreamUse {
            reads_stdin: args.input.is_none(),
            writes_stdout: args.output.is_none(),
        },
    )?;
    let input_bytes = if let Some(mut input_file) = io.take_input(FileRole::Input) {
        read_limited_input_file(
            &mut input_file,
            args.memory.input_bytes(),
            "sample circuit input",
        )?
    } else {
        read_limited_stdin(input, args.memory.input_bytes(), "sample circuit input")?
    };
    let circuit = parse_circuit_bytes(&input_bytes, args.memory)?;
    let reference_sample_loop_policy = if args.skip_loop_folding {
        ReferenceSampleLoopPolicy::Iterate
    } else {
        ReferenceSampleLoopPolicy::Fold
    };
    let plan = SamplingCompiler::new()
        .memory_budget(args.memory.get())
        .reference_sample_loop_policy(reference_sample_loop_policy)
        .compile(&circuit)
        .map_err(CliError::from)?;
    let skip_reference_sample = args.skip_reference_sample;
    let visible_measurements = if args.shots == 1 && !skip_reference_sample {
        one_shot_visible_measurements(&circuit, args.memory.get())?
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
    let shots = ShotCount::try_from(args.shots).map_err(CliError::from)?;
    // The CLI knows the whole request up front, so a small request samples narrow batches; the
    // seeded stream is therefore a function of the seed and the total shot count.
    let mut session = plan
        .session_with_lane_width(
            random_policy,
            reference_mode,
            LaneWidth::for_total_shots(shots),
        )
        .map_err(CliError::from)?;
    let output_width = visible_measurements
        .as_ref()
        .map_or(plan.measurement_width().get(), Vec::len);
    let encoder = RecordEncoder::try_new(args.out_format.record_format(), output_width)?;
    let mut outputs = io.activate()?;
    if let Some(mut output) = outputs.take(FileRole::Output) {
        return write_sample_output(
            &mut session,
            shots,
            visible_measurements.as_deref(),
            encoder,
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
        visible_measurements.as_deref(),
        encoder,
        stdout,
    )
    .map_err(CliError::SampleOutput)
}

pub(crate) fn parse_circuit_bytes(
    input: &[u8],
    memory: MemoryBudgetArg,
) -> Result<Circuit, CliError> {
    Ok(Circuit::from_stim_bytes_with_limits(
        input,
        memory.parse_limits(),
    )?)
}

fn write_sample_output<W>(
    session: &mut SamplingSession,
    shots: ShotCount,
    visible_measurements: Option<&[usize]>,
    encoder: RecordEncoder,
    output: &mut W,
) -> Result<(), RunError<std::io::Error>>
where
    W: Write,
{
    let mut sink = CliSampleSink {
        visible_measurements,
        filtered_record: Vec::new(),
        encoder,
        output,
    };
    session.run(shots, &mut sink).map(|_| ())
}

/// Routes sampled measurements to the shared encoder.
///
/// The only routing decision is Stim's one-shot herald rule: when `visible_measurements` is
/// present, each record keeps just those measurement columns.
struct CliSampleSink<'a, W> {
    visible_measurements: Option<&'a [usize]>,
    filtered_record: Vec<u64>,
    encoder: RecordEncoder,
    output: &'a mut W,
}

impl<W> CliSampleSink<'_, W>
where
    W: Write,
{
    fn write_visible_columns(
        &mut self,
        batch: MeasurementBatchView<'_>,
        indices: &[usize],
    ) -> std::io::Result<()> {
        for shot_index in 0..batch.shot_count() {
            self.filtered_record.clear();
            self.filtered_record.resize(indices.len().div_ceil(64), 0);
            for (output_index, index) in indices.iter().enumerate() {
                let bit = batch.get(shot_index, *index).ok_or_else(|| {
                    std::io::Error::other(format!(
                        "internal sample layout index {index} exceeded record width {}",
                        batch.width().get()
                    ))
                })?;
                if bit && let Some(word) = self.filtered_record.get_mut(output_index / 64) {
                    *word |= 1 << (output_index % 64);
                }
            }
            let bits = stab_bits::BitSlice::new(&self.filtered_record, indices.len())
                .map_err(std::io::Error::other)?;
            self.encoder
                .write_record(
                    &[RecordPart::new(DetsResultType::Measurement, bits)],
                    self.output,
                )
                .map_err(sample_write_io)?;
        }
        Ok(())
    }
}

impl<W> MeasurementSink for CliSampleSink<'_, W>
where
    W: Write,
{
    type Error = std::io::Error;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        if let Some(indices) = self.visible_measurements {
            return self.write_visible_columns(batch, indices);
        }
        let result = self.encoder.write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                batch.records(),
            )],
            self.output,
        );
        result.map_err(sample_write_io)
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        let pending = self.encoder.pending_group_records();
        if pending != 0 {
            return Err(std::io::Error::other(format!(
                "ptb64 sample output ended with {pending} records outside a complete 64-record group"
            )));
        }
        self.output.flush()
    }
}

fn sample_write_io(error: stab_records::RecordWriteError) -> std::io::Error {
    match error {
        stab_records::RecordWriteError::Io(error) => error,
        stab_records::RecordWriteError::Format(error) => std::io::Error::other(error),
    }
}

fn one_shot_visible_measurements(
    circuit: &Circuit,
    budget: MemoryBudget,
) -> Result<Option<Vec<usize>>, CliError> {
    // Stim v1.16's one-shot tableau CLI path records heralds for feedback but does not write them.
    let Some(visible_count) = one_shot_visible_measurement_count(circuit)? else {
        return Ok(None);
    };
    let required = (visible_count as u128).saturating_mul(size_of::<usize>() as u128);
    if !budget.admits(required) {
        return Err(stab_engine::SamplingExecutionError::SessionStorageLimit {
            estimated_bytes: required,
            limit_bytes: budget.bytes(),
        }
        .into());
    }
    let mut visible = Vec::new();
    visible.try_reserve_exact(visible_count).map_err(|error| {
        stab_engine::SamplingExecutionError::SessionStorageAllocation {
            message: format!("visible measurement index list: {error}"),
        }
    })?;
    let mut measurement_index = 0usize;
    for instruction in circuit.iter_flattened_instructions() {
        if !instruction.gate().produces_measurements() {
            continue;
        }
        let produced = instruction.target_groups().len();
        let next_index = measurement_index
            .checked_add(produced)
            .ok_or(CliError::MeasurementCountOverflow)?;
        if !instruction.gate().produces_heralded_results() {
            visible.extend(measurement_index..next_index);
        }
        measurement_index = next_index;
    }
    Ok(Some(visible))
}

fn one_shot_visible_measurement_count(circuit: &Circuit) -> Result<Option<usize>, CliError> {
    let hidden = hidden_herald_measurement_count(circuit)?;
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

fn hidden_herald_measurement_count(circuit: &Circuit) -> Result<u64, CliError> {
    let summary = FoldedCircuitSummary::of(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
    )
    .map_err(stab_model::ModelError::from)?;
    GateKind::ALL
        .into_iter()
        .filter(|kind| kind.gate().produces_heralded_results())
        .fold(FoldedCount::ZERO, |hidden, kind| {
            hidden.plus(summary.measurements_of(kind))
        })
        .exact()
        .ok_or(CliError::MeasurementCountOverflow)
}

#[cfg(test)]
mod tests;
