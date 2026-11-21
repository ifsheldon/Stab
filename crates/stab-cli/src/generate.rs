//! `stab gen`: generates Stim's example error-correction circuits.

use std::io::Write;
use std::path::PathBuf;

use clap::Args;
use clap::builder::{PossibleValuesParser, TypedValueParser as _};
use stab_analysis::{
    CircuitGenerationOptions, CodeCircuitParams, CodeDistance, CodeFamily, GeneratedCircuit,
    RoundCount,
};
use stab_model::{ModelResult, Probability};

use crate::io_plan::{FileRole, PendingIo};
use crate::ledger::Hold;
use crate::output_storage::{EncodedBatches, OutputStorage};
use crate::streaming::OutputSink;
use crate::{CliError, MemoryBudgetArg, MemoryLedger, parse_stim_u64};

/// Parses `--code` against the family names `stab-analysis` owns, listing them in `--help` and
/// in the refusal of an unknown name.
fn code_family_parser() -> impl clap::builder::TypedValueParser<Value = CodeFamily> {
    PossibleValuesParser::new(CodeFamily::ALL.map(CodeFamily::as_str))
        .try_map(|name| name.parse::<CodeFamily>())
}

#[derive(Debug, Args)]
pub(crate) struct GenArgs {
    /// Error-correcting code family to generate.
    #[arg(long, value_parser = code_family_parser())]
    code: CodeFamily,

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

pub(crate) fn run_gen<W>(args: GenArgs, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let mut ledger = MemoryLedger::new(args.memory);
    let (params, generated, _output_hold) = generate_circuit(&args, &mut ledger)?;
    // The circuit text streams to its destination instead of being rendered into a `String`
    // first, which for large codes held one or two extra copies of the whole text. A named
    // output goes through the same preflight as every other command's `--out`, but only after
    // generation succeeds, so a rejected request never creates or truncates it, as in Stim.
    let mut outputs =
        PendingIo::preflight_outputs([(FileRole::Output, args.output.as_deref())])?.activate()?;
    let mut output = OutputSink::from_output(outputs.take(FileRole::Output), stdout);
    output.write_with(|out| write_generated_circuit(out, &params, &generated))?;
    output.flush()
}

/// Validates the arguments, holds a named output's write buffer, and generates the circuit
/// against the budget left beside that buffer.
///
/// Arguments are checked in pinned Stim's order: rounds, distance, noise strengths, the task
/// within its family, and then the family's shape constraints.
fn generate_circuit(
    args: &GenArgs,
    ledger: &mut MemoryLedger,
) -> Result<(CodeCircuitParams, GeneratedCircuit, Hold), CliError> {
    let rounds = RoundCount::try_new(args.rounds)?;
    let distance = CodeDistance::try_new(args.distance)?;
    let before_round_data_depolarization = probability_arg(args.before_round_data_depolarization)?;
    let before_measure_flip_probability = probability_arg(args.before_measure_flip_probability)?;
    let after_reset_flip_probability = probability_arg(args.after_reset_flip_probability)?;
    let after_clifford_depolarization = probability_arg(args.after_clifford_depolarization)?;
    let task = args.code.parse_task(&args.task)?;
    let params = CodeCircuitParams::try_new(task, distance, rounds)?
        .with_before_round_data_depolarization(before_round_data_depolarization)
        .with_before_measure_flip_probability(before_measure_flip_probability)
        .with_after_reset_flip_probability(after_reset_flip_probability)
        .with_after_clifford_depolarization(after_clifford_depolarization);
    // A named `--out` file's write buffer coexists with the generated circuit, so it is held
    // before generation admits the circuit against what is left.
    let output_hold = OutputStorage::new("gen", EncodedBatches::ShotMajor, None)
        .file_buffer(args.output.is_some())
        .admit(ledger)?;
    let stage = ledger.stage();
    let options = CircuitGenerationOptions {
        memory_budget: stage.get(),
    };
    let generated = params
        .generate(&options)
        .map_err(|error| stage.refusal(error))?;
    Ok((params, generated, output_hold))
}

/// Writes the header that echoes the parameters, the layout diagram, its legend, and the
/// circuit, as pinned Stim does.
fn write_generated_circuit(
    out: &mut dyn Write,
    params: &CodeCircuitParams,
    generated: &GeneratedCircuit,
) -> std::io::Result<()> {
    writeln!(out, "# Generated {} circuit.", params.task().family())?;
    writeln!(out, "# task: {}", params.task())?;
    writeln!(out, "# rounds: {}", params.rounds().get())?;
    writeln!(out, "# distance: {}", params.distance().get())?;
    for (name, value) in [
        (
            "before_round_data_depolarization",
            params.before_round_data_depolarization(),
        ),
        (
            "before_measure_flip_probability",
            params.before_measure_flip_probability(),
        ),
        (
            "after_reset_flip_probability",
            params.after_reset_flip_probability(),
        ),
        (
            "after_clifford_depolarization",
            params.after_clifford_depolarization(),
        ),
    ] {
        writeln!(out, "# {name}: {}", value.stim_text())?;
    }
    out.write_all(b"# layout:\n")?;
    out.write_all(generated.layout_text().as_bytes())?;
    out.write_all(generated.hint_text().as_bytes())?;
    let mut out = out;
    generated.circuit().write_stim_io(&mut out)
}

fn probability_arg(value: f64) -> ModelResult<Probability> {
    Probability::try_new(value)
}
