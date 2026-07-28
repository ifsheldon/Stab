use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::{Args, CommandFactory, Subcommand, ValueEnum};
use serde::Serialize;
use stab_core::{
    CapabilitySet, Circuit, CompilationRequestFingerprint, DetectorErrorModel, Estimate,
    GateArgumentRule, GateCategory, GateTargetGroupKind, GateTargetRule, ModelFingerprint,
    ParseLimits, RecordFormat, ResourceEstimate, estimate_sampling_request,
    result_formats::validate_ptb64_shot_count,
};

use crate::{
    Cli, CliError, CompiledSampler, FileRole, MAX_CIRCUIT_INPUT_BYTES, PendingIo,
    SampleOutFormatArg, legacy_tableau_visible_measurement_count, parse_stim_u64, parse_stim_usize,
    read_limited_input_file, read_limited_stdin,
};

const AGENT_OUTPUT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum AgentOutputFormatArg {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Args)]
pub(crate) struct CapabilitiesArgs {
    /// Selects concise human text or schema-version-1 JSON.
    #[arg(long, value_enum, default_value_t = AgentOutputFormatArg::Human)]
    format: AgentOutputFormatArg,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum InspectModelTypeArg {
    #[value(name = "stim")]
    Circuit,
    #[value(name = "dem")]
    Dem,
}

#[derive(Debug, Args)]
pub(crate) struct InspectArgs {
    /// Circuit or detector error model path. Defaults to stdin.
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    /// Input model type. Required for stdin or paths without a .stim or .dem extension.
    #[arg(long = "type", value_enum)]
    model_type: Option<InspectModelTypeArg>,

    /// Selects concise human text or schema-version-1 JSON.
    #[arg(long, value_enum, default_value_t = AgentOutputFormatArg::Human)]
    format: AgentOutputFormatArg,
}

#[derive(Debug, Args)]
pub(crate) struct PlanArgs {
    #[command(subcommand)]
    command: PlanCommand,
}

#[derive(Debug, Subcommand)]
enum PlanCommand {
    /// Validates and describes a measurement-sampling request without executing shots.
    Sample(PlanSampleArgs),
}

#[derive(Debug, Args)]
struct PlanSampleArgs {
    /// Circuit path. Defaults to stdin.
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,

    /// Number of shots described by the plan.
    #[arg(long, default_value_t = 1, value_parser = parse_stim_usize)]
    shots: usize,

    /// Planned result-record encoding.
    #[arg(long = "out_format", value_enum, default_value = "01")]
    out_format: SampleOutFormatArg,

    /// Partially deterministic random seed recorded in the run configuration.
    #[arg(long, value_parser = parse_stim_u64)]
    seed: Option<u64>,

    /// Assert the noiseless reference sample is all zeroes.
    #[arg(long = "skip_reference_sample")]
    skip_reference_sample: bool,

    /// Accepted compatibility no-op, reported separately from compilation identity.
    #[arg(long = "skip_loop_folding")]
    skip_loop_folding: bool,

    /// Selects concise human text or schema-version-1 JSON.
    #[arg(long, value_enum, default_value_t = AgentOutputFormatArg::Human)]
    format: AgentOutputFormatArg,
}

pub(crate) fn run_capabilities<W>(args: CapabilitiesArgs, stdout: &mut W) -> Result<(), CliError>
where
    W: Write,
{
    let report = CapabilitiesReport::current();
    write_agent_output(
        stdout,
        args.format,
        &report,
        &render_capabilities_human(&report),
    )
}

pub(crate) fn run_inspect<R, W>(
    args: InspectArgs,
    stdin: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let model_type = resolve_model_type(args.model_type, args.input.as_deref())?;
    let input = read_agent_input(
        args.input.as_deref(),
        stdin,
        "inspect model input",
        MAX_CIRCUIT_INPUT_BYTES,
    )?;
    let report = match model_type {
        InspectModelTypeArg::Circuit => InspectReport::for_circuit(&input)?,
        InspectModelTypeArg::Dem => InspectReport::for_dem(&input)?,
    };
    write_agent_output(stdout, args.format, &report, &render_inspect_human(&report))
}

pub(crate) fn run_plan<R, W>(args: PlanArgs, stdin: &mut R, stdout: &mut W) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    match args.command {
        PlanCommand::Sample(args) => run_plan_sample(args, stdin, stdout),
    }
}

fn run_plan_sample<R, W>(
    args: PlanSampleArgs,
    stdin: &mut R,
    stdout: &mut W,
) -> Result<(), CliError>
where
    R: Read,
    W: Write,
{
    let output_format = args.out_format.record_format();
    if output_format == RecordFormat::Ptb64 {
        validate_ptb64_shot_count(args.shots)?;
    }

    let input = read_agent_input(
        args.input.as_deref(),
        stdin,
        "plan sample circuit input",
        MAX_CIRCUIT_INPUT_BYTES,
    )?;
    let circuit = Circuit::from_stim_bytes(&input)?;

    // Compilation is intentionally performed for validation. No sampling method is called.
    let _validated_sampler = CompiledSampler::compile(&circuit)?;
    let request_fingerprint = CompilationRequestFingerprint::for_sampling(&circuit);
    let mut estimates = ResourceEstimateReport::from(estimate_sampling_request(
        &circuit,
        args.shots,
        output_format,
    ));

    let visible_measurements = if args.shots == 1 && !args.skip_reference_sample {
        legacy_tableau_visible_measurement_count(&circuit)?
    } else {
        None
    };
    if let Some(visible_measurement_count) = visible_measurements {
        estimates.output_bytes = EstimateReport::from(Estimate::from(
            output_format.estimate_output_bytes(args.shots, visible_measurement_count),
        ));
    }

    let report = SamplePlanReport {
        schema_version: AGENT_OUTPUT_SCHEMA_VERSION,
        operation: "sample",
        executes: false,
        source: SourceReport::new(&input),
        model: ModelIdentityReport::from(circuit.fingerprint()),
        compilation: CompilationReport {
            request_fingerprint: CompilationFingerprintReport::from(request_fingerprint),
            compiler_schema_version: request_fingerprint.compiler_schema_version(),
            normalized_options: Vec::new(),
            configurable_limits: Vec::new(),
            selectable_backend: None,
            validated: true,
        },
        run: SampleRunReport {
            shots: args.shots,
            random_policy: if args.seed.is_some() {
                "seeded"
            } else {
                "entropy"
            },
            seed: args.seed,
            reference_mode: if args.skip_reference_sample {
                "skip-reference-sample"
            } else {
                "normal"
            },
            output_format: output_format.as_str(),
            skip_loop_folding_requested: args.skip_loop_folding,
            skip_loop_folding_effect: "accepted-no-op",
        },
        estimates,
    };
    write_agent_output(
        stdout,
        args.format,
        &report,
        &render_sample_plan_human(&report),
    )
}

fn read_agent_input<R>(
    path: Option<&Path>,
    stdin: &mut R,
    kind: &'static str,
    limit: u64,
) -> Result<Vec<u8>, CliError>
where
    R: Read,
{
    let mut io = PendingIo::preflight(
        [(FileRole::Input, path)],
        std::iter::empty::<(FileRole, Option<&Path>)>(),
    )?;
    if path.is_some() {
        let mut input = io
            .take_input(FileRole::Input)
            .ok_or(CliError::IoPlanInvariant {
                message: "agent command lost its retained input before reading",
            })?;
        read_limited_input_file(&mut input, limit, kind)
    } else {
        read_limited_stdin(stdin, limit, kind)
    }
}

fn resolve_model_type(
    requested: Option<InspectModelTypeArg>,
    path: Option<&Path>,
) -> Result<InspectModelTypeArg, CliError> {
    if let Some(requested) = requested {
        return Ok(requested);
    }
    let Some(path) = path else {
        return Err(CliError::MissingInspectModelType);
    };
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("stim") => Ok(InspectModelTypeArg::Circuit),
        Some("dem") => Ok(InspectModelTypeArg::Dem),
        _ => Err(CliError::UnknownInspectModelType {
            path: path.to_path_buf(),
        }),
    }
}

pub(crate) fn command_descriptions() -> Vec<(String, String)> {
    let command = Cli::command();
    let mut descriptions = Vec::new();
    collect_command_descriptions("", &command, &mut descriptions);
    descriptions
}

fn collect_command_descriptions(
    prefix: &str,
    command: &clap::Command,
    descriptions: &mut Vec<(String, String)>,
) {
    for subcommand in command.get_subcommands() {
        let name = if prefix.is_empty() {
            subcommand.get_name().to_string()
        } else {
            format!("{prefix} {}", subcommand.get_name())
        };
        let summary = subcommand
            .get_about()
            .map(ToString::to_string)
            .unwrap_or_default();
        descriptions.push((name.clone(), summary));
        collect_command_descriptions(&name, subcommand, descriptions);
    }
}

#[derive(Serialize)]
struct CapabilitiesReport {
    schema_version: u16,
    stab_version: &'static str,
    stim_compatibility_version: &'static str,
    commands: Vec<CommandReport>,
    dialects: Vec<DialectReport>,
    gates: Vec<GateReport>,
    codecs: Vec<CodecReport>,
    compilers: Vec<CompilerReport>,
    selectable_backends: Vec<&'static str>,
}

impl CapabilitiesReport {
    fn current() -> Self {
        let capabilities = CapabilitySet::current();
        Self {
            schema_version: CapabilitySet::SCHEMA_VERSION,
            stab_version: env!("CARGO_PKG_VERSION"),
            stim_compatibility_version: CapabilitySet::STIM_COMPATIBILITY_VERSION,
            commands: command_descriptions()
                .into_iter()
                .map(|(name, summary)| CommandReport { name, summary })
                .collect(),
            dialects: capabilities
                .dialects()
                .map(|dialect| {
                    let limits = capabilities.default_parse_limits(dialect);
                    DialectReport {
                        name: dialect.as_str(),
                        default_parse_limits: ParseLimitsReport {
                            source_lines: limits.source_line_limit().get(),
                            repeat_nesting: limits.repeat_nesting_limit().get(),
                        },
                    }
                })
                .collect(),
            gates: capabilities
                .gates()
                .map(|gate| GateReport {
                    canonical_name: gate.canonical_name(),
                    aliases: gate.aliases(),
                    category: gate_category_name(gate.category()),
                    argument_rule: GateRuleReport::from(gate.argument_rule()),
                    target_rule: gate_target_rule_name(gate.target_rule()),
                    target_grouping: gate_target_group_name(gate.target_group_kind()),
                    support_scope: "accepted-circuit-syntax",
                })
                .collect(),
            codecs: capabilities
                .codecs()
                .map(|codec| CodecReport {
                    name: codec.format().as_str(),
                    encoding: codec.format().encoding().as_str(),
                    can_decode: codec.can_decode(),
                    can_encode: codec.can_encode(),
                    requires_typed_layout: codec.requires_typed_layout(),
                    records_per_group: codec.format().records_per_group(),
                })
                .collect(),
            compilers: capabilities
                .compilation_operations()
                .map(|compiler| CompilerReport {
                    operation: compiler.operation().as_str(),
                    input_dialect: compiler.input_dialect().as_str(),
                    compiler_schema_version: compiler.compiler_schema_version(),
                    request_fingerprint_schema_version: compiler
                        .request_fingerprint_schema_version(),
                    configurable_limits: compiler.has_configurable_limits(),
                    backend_selection: compiler.supports_backend_selection(),
                })
                .collect(),
            selectable_backends: capabilities.selectable_backend_ids().collect(),
        }
    }
}

#[derive(Serialize)]
struct CommandReport {
    name: String,
    summary: String,
}

#[derive(Serialize)]
struct DialectReport {
    name: &'static str,
    default_parse_limits: ParseLimitsReport,
}

#[derive(Serialize)]
struct ParseLimitsReport {
    source_lines: usize,
    repeat_nesting: usize,
}

#[derive(Serialize)]
struct GateReport {
    canonical_name: &'static str,
    aliases: &'static [&'static str],
    category: &'static str,
    argument_rule: GateRuleReport,
    target_rule: &'static str,
    target_grouping: &'static str,
    support_scope: &'static str,
}

#[derive(Serialize)]
struct GateRuleReport {
    kind: &'static str,
    count: Option<usize>,
}

impl From<GateArgumentRule> for GateRuleReport {
    fn from(rule: GateArgumentRule) -> Self {
        match rule {
            GateArgumentRule::Exact(count) => Self {
                kind: "exact",
                count: Some(count),
            },
            GateArgumentRule::Any => Self {
                kind: "any",
                count: None,
            },
            GateArgumentRule::OptionalProbability => Self {
                kind: "optional-probability",
                count: None,
            },
            GateArgumentRule::ProbabilityList(count) => Self {
                kind: "probability-list",
                count: Some(count),
            },
            GateArgumentRule::AnyProbabilityList => Self {
                kind: "any-probability-list",
                count: None,
            },
            GateArgumentRule::UnsignedInteger => Self {
                kind: "unsigned-integer",
                count: None,
            },
        }
    }
}

#[derive(Serialize)]
struct CodecReport {
    name: &'static str,
    encoding: &'static str,
    can_decode: bool,
    can_encode: bool,
    requires_typed_layout: bool,
    records_per_group: usize,
}

#[derive(Serialize)]
struct CompilerReport {
    operation: &'static str,
    input_dialect: &'static str,
    compiler_schema_version: u16,
    request_fingerprint_schema_version: u16,
    configurable_limits: bool,
    backend_selection: bool,
}

#[derive(Serialize)]
struct InspectReport {
    schema_version: u16,
    executes: bool,
    source: SourceReport,
    parse_estimate: ResourceEstimateReport,
    model: InspectedModelReport,
}

impl InspectReport {
    fn for_circuit(input: &[u8]) -> Result<Self, CliError> {
        let circuit = Circuit::from_stim_bytes(input)?;
        Ok(Self {
            schema_version: AGENT_OUTPUT_SCHEMA_VERSION,
            executes: false,
            source: SourceReport::new(input),
            parse_estimate: ResourceEstimateReport::from(
                ParseLimits::default().estimate_bytes(input),
            ),
            model: InspectedModelReport::StimCircuit {
                fingerprint: ModelIdentityReport::from(circuit.fingerprint()),
                top_level_items: circuit.items().len(),
                qubits: circuit.count_qubits(),
                measurements: circuit.count_measurements()?,
                detectors: circuit.count_detectors()?,
                observables: circuit.count_observables()?,
                sweep_bits: circuit.count_sweep_bits()?,
            },
        })
    }

    fn for_dem(input: &[u8]) -> Result<Self, CliError> {
        let model = DetectorErrorModel::from_dem_bytes(input)?;
        Ok(Self {
            schema_version: AGENT_OUTPUT_SCHEMA_VERSION,
            executes: false,
            source: SourceReport::new(input),
            parse_estimate: ResourceEstimateReport::from(
                ParseLimits::default().estimate_bytes(input),
            ),
            model: InspectedModelReport::DetectorErrorModel {
                fingerprint: ModelIdentityReport::from(model.fingerprint()),
                top_level_items: model.items().len(),
                detectors: model.count_detectors()?,
                observables: model.count_observables()?,
            },
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "dialect", rename_all = "kebab-case")]
enum InspectedModelReport {
    StimCircuit {
        fingerprint: ModelIdentityReport,
        top_level_items: usize,
        qubits: usize,
        measurements: u64,
        detectors: u64,
        observables: u64,
        sweep_bits: u64,
    },
    DetectorErrorModel {
        fingerprint: ModelIdentityReport,
        top_level_items: usize,
        detectors: u64,
        observables: u64,
    },
}

#[derive(Serialize)]
struct SourceReport {
    bytes: usize,
    physical_lines: usize,
}

impl SourceReport {
    fn new(input: &[u8]) -> Self {
        Self {
            bytes: input.len(),
            physical_lines: if input.is_empty() {
                0
            } else {
                input.iter().filter(|byte| **byte == b'\n').count()
                    + usize::from(input.last() != Some(&b'\n'))
            },
        }
    }
}

#[derive(Serialize)]
struct ModelIdentityReport {
    schema_version: u16,
    algorithm: &'static str,
    dialect: &'static str,
    digest: String,
}

impl From<ModelFingerprint> for ModelIdentityReport {
    fn from(fingerprint: ModelFingerprint) -> Self {
        Self {
            schema_version: fingerprint.schema_version(),
            algorithm: ModelFingerprint::ALGORITHM,
            dialect: fingerprint.dialect().as_str(),
            digest: fingerprint.digest_hex(),
        }
    }
}

#[derive(Serialize)]
struct SamplePlanReport {
    schema_version: u16,
    operation: &'static str,
    executes: bool,
    source: SourceReport,
    model: ModelIdentityReport,
    compilation: CompilationReport,
    run: SampleRunReport,
    estimates: ResourceEstimateReport,
}

#[derive(Serialize)]
struct CompilationReport {
    request_fingerprint: CompilationFingerprintReport,
    compiler_schema_version: u16,
    normalized_options: Vec<&'static str>,
    configurable_limits: Vec<&'static str>,
    selectable_backend: Option<&'static str>,
    validated: bool,
}

#[derive(Serialize)]
struct CompilationFingerprintReport {
    schema_version: u16,
    algorithm: &'static str,
    digest: String,
}

impl From<CompilationRequestFingerprint> for CompilationFingerprintReport {
    fn from(fingerprint: CompilationRequestFingerprint) -> Self {
        Self {
            schema_version: fingerprint.schema_version(),
            algorithm: CompilationRequestFingerprint::ALGORITHM,
            digest: fingerprint.digest_hex(),
        }
    }
}

#[derive(Serialize)]
struct SampleRunReport {
    shots: usize,
    random_policy: &'static str,
    seed: Option<u64>,
    reference_mode: &'static str,
    output_format: &'static str,
    skip_loop_folding_requested: bool,
    skip_loop_folding_effect: &'static str,
}

#[derive(Serialize)]
struct ResourceEstimateReport {
    input_bytes: EstimateReport,
    input_items: EstimateReport,
    expanded_operations: EstimateReport,
    folded_traversal: EstimateReport,
    scratch_bytes: EstimateReport,
    resident_bytes: EstimateReport,
    output_bytes: EstimateReport,
    work_units: EstimateReport,
}

impl From<ResourceEstimate> for ResourceEstimateReport {
    fn from(estimate: ResourceEstimate) -> Self {
        Self {
            input_bytes: EstimateReport::from(estimate.input_bytes()),
            input_items: EstimateReport::from(estimate.input_items()),
            expanded_operations: EstimateReport::from(estimate.expanded_operations()),
            folded_traversal: EstimateReport::from(estimate.folded_traversal()),
            scratch_bytes: EstimateReport::from(estimate.scratch_bytes()),
            resident_bytes: EstimateReport::from(estimate.resident_bytes()),
            output_bytes: EstimateReport::from(estimate.output_bytes()),
            work_units: EstimateReport::from(estimate.work_units()),
        }
    }
}

#[derive(Serialize)]
struct EstimateReport {
    class: &'static str,
    value: Option<usize>,
}

impl From<Estimate<usize>> for EstimateReport {
    fn from(estimate: Estimate<usize>) -> Self {
        match estimate {
            Estimate::Exact(value) => Self {
                class: "exact",
                value: Some(value),
            },
            Estimate::UpperBound(value) => Self {
                class: "upper-bound",
                value: Some(value),
            },
            Estimate::Unknown => Self {
                class: "unknown",
                value: None,
            },
        }
    }
}

fn write_agent_output<W, T>(
    stdout: &mut W,
    format: AgentOutputFormatArg,
    report: &T,
    human: &str,
) -> Result<(), CliError>
where
    W: Write,
    T: Serialize,
{
    match format {
        AgentOutputFormatArg::Human => stdout
            .write_all(human.as_bytes())
            .map_err(CliError::WriteOutput),
        AgentOutputFormatArg::Json => {
            let mut output =
                serde_json::to_vec_pretty(report).map_err(CliError::SerializeAgentOutput)?;
            output.push(b'\n');
            stdout.write_all(&output).map_err(CliError::WriteOutput)
        }
    }
}

fn render_capabilities_human(report: &CapabilitiesReport) -> String {
    format!(
        "Stab {} capabilities (Stim {})\ncommands: {}\ndialects: {}\ngates: {}\nresult codecs: {}\ncompilers: {}\nselectable backends: {}\n",
        report.stab_version,
        report.stim_compatibility_version,
        report.commands.len(),
        report.dialects.len(),
        report.gates.len(),
        report.codecs.len(),
        report.compilers.len(),
        report.selectable_backends.len()
    )
}

fn render_inspect_human(report: &InspectReport) -> String {
    match &report.model {
        InspectedModelReport::StimCircuit {
            fingerprint,
            top_level_items,
            qubits,
            measurements,
            detectors,
            observables,
            sweep_bits,
        } => format!(
            "stim circuit\nfingerprint: {}\nsource bytes: {}\nphysical lines: {}\ntop-level items: {}\nqubits: {}\nmeasurements: {}\ndetectors: {}\nobservables: {}\nsweep bits: {}\nexecutes: no\n",
            fingerprint.digest,
            report.source.bytes,
            report.source.physical_lines,
            top_level_items,
            qubits,
            measurements,
            detectors,
            observables,
            sweep_bits
        ),
        InspectedModelReport::DetectorErrorModel {
            fingerprint,
            top_level_items,
            detectors,
            observables,
        } => format!(
            "detector error model\nfingerprint: {}\nsource bytes: {}\nphysical lines: {}\ntop-level items: {}\ndetectors: {}\nobservables: {}\nexecutes: no\n",
            fingerprint.digest,
            report.source.bytes,
            report.source.physical_lines,
            top_level_items,
            detectors,
            observables
        ),
    }
}

fn render_sample_plan_human(report: &SamplePlanReport) -> String {
    format!(
        "sample plan\nmodel fingerprint: {}\nrequest fingerprint: {}\nvalidated: yes\nexecutes: no\nshots: {}\noutput format: {}\nreference mode: {}\noutput bytes: {}{}\nselectable backend: none\n",
        report.model.digest,
        report.compilation.request_fingerprint.digest,
        report.run.shots,
        report.run.output_format,
        report.run.reference_mode,
        report
            .estimates
            .output_bytes
            .value
            .map_or_else(|| "unknown".to_string(), |value| value.to_string()),
        if report.run.skip_loop_folding_requested {
            "\nskip_loop_folding: accepted no-op"
        } else {
            ""
        }
    )
}

fn gate_category_name(category: GateCategory) -> &'static str {
    match category {
        GateCategory::Annotation => "annotation",
        GateCategory::ControlFlow => "control-flow",
        GateCategory::Collapsing => "collapsing",
        GateCategory::Controlled => "controlled",
        GateCategory::HadamardLike => "hadamard-like",
        GateCategory::Noise => "noise",
        GateCategory::HeraldedNoise => "heralded-noise",
        GateCategory::Pauli => "pauli",
        GateCategory::Period3 => "period-3",
        GateCategory::Period4 => "period-4",
        GateCategory::ParityPhasing => "parity-phasing",
        GateCategory::PauliProduct => "pauli-product",
        GateCategory::Swap => "swap",
        GateCategory::PairMeasurement => "pair-measurement",
    }
}

fn gate_target_rule_name(rule: GateTargetRule) -> &'static str {
    match rule {
        GateTargetRule::None => "none",
        GateTargetRule::AnySingleQubit => "any-single-qubit",
        GateTargetRule::MeasurementQubits => "measurement-qubits",
        GateTargetRule::MeasurementPads => "measurement-pads",
        GateTargetRule::PlainPairs => "plain-pairs",
        GateTargetRule::ClassicalControlPairs => "classical-control-pairs",
        GateTargetRule::MeasurementPairs => "measurement-pairs",
        GateTargetRule::RecOnly => "measurement-record-only",
        GateTargetRule::RecOrPauli => "measurement-record-or-pauli",
        GateTargetRule::QubitCoords => "qubit-coordinates",
        GateTargetRule::PauliProducts => "pauli-products",
        GateTargetRule::PauliList => "pauli-list",
    }
}

fn gate_target_group_name(grouping: GateTargetGroupKind) -> &'static str {
    match grouping {
        GateTargetGroupKind::None => "none",
        GateTargetGroupKind::Singles => "singles",
        GateTargetGroupKind::Pairs => "pairs",
        GateTargetGroupKind::PauliProducts => "pauli-products",
        GateTargetGroupKind::AllTargets => "all-targets",
    }
}
