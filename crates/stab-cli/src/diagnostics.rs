use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Value, json};
use stab_analysis::{AnalysisError, ResourceLimitError as AnalysisResourceLimitError};
use stab_engine::{
    DemError, DemResourceKind, DemResourceLimitError, DemSamplingExecutionError,
    DetectionCompileError, DetectionError, DetectionExecutionError, DetectionRecordLimitSubject,
    DetectionResourceKind, DetectionResourceLimitError, RunError, SamplingCompileError,
    SamplingExecutionError,
};
use stab_model::{
    ByteSpan as ModelByteSpan, DiagnosticSeverity, ModelDialect, ModelError, ParseError,
    ParseErrorContext, ResourceLimitError as ModelResourceLimitError, ValidationError,
};
use stab_records::{ByteSpan as RecordByteSpan, DetsResultType, FormatError, FormatErrorContext};
use thiserror::Error;

const JSON_DIAGNOSTIC_SCHEMA_VERSION: u8 = 1;

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
    WriteOutput(#[source] std::io::Error),

    #[error("failed to write {path}: {source}")]
    WritePath {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("sampling failed while producing output: {0}")]
    SampleOutput(#[source] RunError<std::io::Error>),

    #[error("sampling failed while writing {path}: {source}")]
    SamplePath {
        path: PathBuf,
        #[source]
        source: RunError<std::io::Error>,
    },

    #[error("file roles {first} and {second} refer to the same file")]
    ConflictingFileRoles {
        first: &'static str,
        second: &'static str,
    },

    #[error("internal CLI I/O plan invariant failed: {message}")]
    IoPlanInvariant { message: &'static str },

    #[error("cannot infer model type from stdin; pass --type=stim or --type=dem")]
    MissingInspectModelType,

    #[error("cannot infer model type from path {path}; pass --type=stim or --type=dem")]
    UnknownInspectModelType { path: PathBuf },

    #[error("failed to serialize agent output: {0}")]
    SerializeAgentOutput(serde_json::Error),

    #[error("internal agent-output contract invariant failed: {message}")]
    AgentOutputInvariant { message: &'static str },

    #[error(transparent)]
    Model(#[from] ModelError),

    #[error(transparent)]
    Analysis(#[from] AnalysisError),

    #[error("invalid result format data: {0}")]
    Record(#[from] FormatError),

    #[error(transparent)]
    SamplingCompile(#[from] SamplingCompileError),

    #[error(transparent)]
    SamplingExecution(#[from] SamplingExecutionError),

    #[error(transparent)]
    Detection(#[from] DetectionError),

    #[error(transparent)]
    DetectionCompile(#[from] DetectionCompileError),

    #[error(transparent)]
    DetectionExecution(#[from] DetectionExecutionError),

    #[error(transparent)]
    Dem(#[from] DemError),

    #[error(transparent)]
    DemSamplingExecution(#[from] DemSamplingExecutionError),

    #[error("invalid result format data: {source}")]
    InputRecord {
        byte_offset: usize,
        #[source]
        source: FormatError,
    },

    #[error("{kind} byte offset overflowed")]
    InputByteOffsetOverflow { kind: &'static str },

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

    #[error("{flag} is not valid for stim-to-stim conversion")]
    UnsupportedStimConversionOption { flag: &'static str },

    #[error("format {format} is not supported for detection data")]
    UnsupportedDetectionFormat { format: &'static str },

    #[error("cannot combine --obs_out with detector output that includes observables")]
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

    #[error("measurement count overflowed")]
    MeasurementCountOverflow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum ErrorFormatArg {
    #[default]
    Human,
    Json,
}

pub(crate) fn probe_error_format(args: &[OsString]) -> ErrorFormatArg {
    let mut occurrences = 0usize;
    let mut requested_json = false;
    let mut index = 1usize;
    while let Some(argument) = args.get(index) {
        let argument = argument.to_string_lossy();
        if argument == "--" {
            break;
        }
        if argument == "--error-format" {
            occurrences = occurrences.saturating_add(1);
            if args.get(index + 1).is_some_and(|value| value == "json") {
                requested_json = true;
            }
            index = index.saturating_add(2);
            continue;
        }
        if let Some(value) = argument.strip_prefix("--error-format=") {
            occurrences = occurrences.saturating_add(1);
            requested_json = value == "json";
        }
        index = index.saturating_add(1);
    }
    if occurrences == 1 && requested_json {
        ErrorFormatArg::Json
    } else {
        ErrorFormatArg::Human
    }
}

pub(crate) fn write_cli_error(
    writer: &mut impl Write,
    format: ErrorFormatArg,
    error: &CliError,
) -> io::Result<()> {
    match format {
        ErrorFormatArg::Human => writeln!(writer, "error: {error}"),
        ErrorFormatArg::Json => write_json(writer, &cli_error_diagnostic(error)),
    }
}

pub(crate) fn write_clap_error(writer: &mut impl Write, error: &clap::Error) -> io::Result<()> {
    let rendered = error.to_string();
    let message = rendered
        .strip_prefix("error: ")
        .unwrap_or(&rendered)
        .trim_end()
        .to_string();
    write_json(
        writer,
        &JsonDiagnostic {
            schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
            code: clap_error_code(error.kind()),
            severity: DiagnosticSeverity::Error.as_str(),
            message,
            span: None,
            labels: Vec::new(),
            help: None,
            context: json!({
                "clap_error_kind": clap_error_kind(error.kind()),
            }),
        },
    )
}

#[derive(Serialize)]
struct JsonDiagnostic {
    schema_version: u8,
    code: &'static str,
    severity: &'static str,
    message: String,
    span: Option<JsonSpan>,
    labels: Vec<Value>,
    help: Option<&'static str>,
    context: Value,
}

#[derive(Serialize)]
struct JsonSpan {
    byte_start: usize,
    byte_length: usize,
}

impl From<ModelByteSpan> for JsonSpan {
    fn from(span: ModelByteSpan) -> Self {
        Self {
            byte_start: span.byte_start(),
            byte_length: span.byte_length(),
        }
    }
}

impl From<RecordByteSpan> for JsonSpan {
    fn from(span: RecordByteSpan) -> Self {
        Self {
            byte_start: span.byte_start(),
            byte_length: span.byte_length(),
        }
    }
}

fn write_json(writer: &mut impl Write, diagnostic: &JsonDiagnostic) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, diagnostic).map_err(io::Error::other)?;
    writer.write_all(b"\n")
}

fn cli_error_diagnostic(error: &CliError) -> JsonDiagnostic {
    if let Some((parse_error, byte_offset)) = parse_error_with_offset(error) {
        return JsonDiagnostic {
            schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
            code: parse_error.code().as_str(),
            severity: parse_error.severity().as_str(),
            message: parse_error.message().to_string(),
            span: shifted_model_span(parse_error.span(), byte_offset),
            labels: Vec::new(),
            help: cli_error_help(error),
            context: parse_error_context(parse_error),
        };
    }
    if let Some(resource) = resource_diagnostic(error) {
        return JsonDiagnostic {
            schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
            code: "resource-limit-exceeded",
            severity: DiagnosticSeverity::Error.as_str(),
            message: resource.message(),
            span: resource.span(),
            labels: Vec::new(),
            help: cli_error_help(error),
            context: resource.context(),
        };
    }
    let (format_error, byte_offset) = format_error_with_offset(error).unzip();
    JsonDiagnostic {
        schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
        code: format_error.map_or_else(|| cli_error_code(error), |error| error.code().as_str()),
        severity: DiagnosticSeverity::Error.as_str(),
        message: format_error
            .map_or_else(|| error.to_string(), |error| error.message().to_string()),
        span: format_error
            .and_then(FormatError::span)
            .and_then(|span| shifted_record_span(span, byte_offset.unwrap_or(0))),
        labels: Vec::new(),
        help: cli_error_help(error),
        context: format_error.map_or_else(|| cli_error_context(error), format_error_context),
    }
}

fn cli_error_code(error: &CliError) -> &'static str {
    match error {
        CliError::ReadInput(_) => "stdin-read-failed",
        CliError::ReadPath { .. } => "path-read-failed",
        CliError::WriteOutput(_) => "stdout-write-failed",
        CliError::WritePath { .. } => "path-write-failed",
        CliError::SampleOutput(source) => sample_run_error_code(source, false),
        CliError::SamplePath { source, .. } => sample_run_error_code(source, true),
        CliError::ConflictingFileRoles { .. } => "conflicting-file-roles",
        CliError::IoPlanInvariant { .. } => "io-plan-invariant-failed",
        CliError::MissingInspectModelType => "missing-inspect-model-type",
        CliError::UnknownInspectModelType { .. } => "unknown-inspect-model-type",
        CliError::SerializeAgentOutput(_) => "agent-output-serialization-failed",
        CliError::AgentOutputInvariant { .. } => "agent-output-invariant-failed",
        CliError::Model(error) => model_error_code(error),
        CliError::Analysis(error) => analysis_error_code(error),
        CliError::Record(error) | CliError::InputRecord { source: error, .. } => {
            error.code().as_str()
        }
        CliError::SamplingCompile(error) => sampling_compile_error_code(error),
        CliError::SamplingExecution(error) => sampling_execution_error_code(error),
        CliError::Detection(error) => detection_error_code(error),
        CliError::DetectionCompile(error) => detection_compile_error_code(error),
        CliError::DetectionExecution(error) => detection_execution_error_code(error),
        CliError::Dem(error) => dem_error_code(error),
        CliError::DemSamplingExecution(error) => dem_execution_error_code(error),
        CliError::InputByteOffsetOverflow { .. } => "input-byte-offset-overflow",
        CliError::UnsupportedRepetitionTask { .. } => "unsupported-repetition-task",
        CliError::UnsupportedSurfaceTask { .. } => "unsupported-surface-task",
        CliError::UnsupportedColorTask { .. } => "unsupported-color-task",
        CliError::UnsupportedConversion => "unsupported-conversion",
        CliError::UnsupportedStimConversionOption { .. } => "unsupported-stim-conversion-option",
        CliError::UnsupportedDetectionFormat { .. } => "unsupported-detection-format",
        CliError::ConflictingObservableRouting => "conflicting-observable-routing",
        CliError::ReplayErrorRecordCountMismatch { .. } => "replay-record-count-mismatch",
        CliError::InputTooLarge { .. } => "input-too-large",
        CliError::MissingRecordWidth => "missing-record-width",
        CliError::MissingRecordTypesForDets => "missing-record-types-for-dets",
        CliError::MissingConvertTypes => "missing-convert-types",
        CliError::UnknownConvertType { .. } => "unknown-convert-type",
        CliError::DuplicateConvertType { .. } => "duplicate-convert-type",
        CliError::IncompletePtb64OutputGroup { .. } => "incomplete-ptb64-output-group",
        CliError::UnknownHelpTopic { .. } => "unknown-help-topic",
        CliError::MeasurementCountOverflow => "measurement-count-overflow",
    }
}

fn sample_run_error_code(error: &RunError<std::io::Error>, writes_path: bool) -> &'static str {
    match error {
        RunError::Engine { .. } => "sampling-execution-failed",
        RunError::Sink { .. } if writes_path => "path-write-failed",
        RunError::Sink { .. } => "stdout-write-failed",
    }
}

fn model_error_code(error: &ModelError) -> &'static str {
    match error {
        ModelError::Parse(error) => error.code().as_str(),
        ModelError::ResourceLimit(error) => error.code(),
        ModelError::Validation(error) => error.code().as_str(),
    }
}

fn analysis_error_code(error: &AnalysisError) -> &'static str {
    match error {
        AnalysisError::Model(error) => model_error_code(error),
        AnalysisError::InvalidDomainValue { .. } => "invalid-domain-value",
        AnalysisError::InvalidTableauConversion { .. } => "invalid-tableau-conversion",
        AnalysisError::InvalidCircuitSimplification { .. } => "invalid-circuit-simplification",
        AnalysisError::InvalidResultFormat { .. } => "invalid-result-format",
        AnalysisError::InvalidDetectorErrorModel { .. } => "invalid-detector-error-model",
        AnalysisError::ResourceLimit(_) => "resource-limit-exceeded",
    }
}

fn sampling_compile_error_code(error: &SamplingCompileError) -> &'static str {
    match error {
        SamplingCompileError::Model(error) => model_error_code(error),
        SamplingCompileError::Analysis(error) => analysis_error_code(error),
        SamplingCompileError::InvalidCircuit { .. } => "invalid-sampler-compilation",
        SamplingCompileError::ExpandedOperationLimit { .. } => "resource-limit-exceeded",
    }
}

fn sampling_execution_error_code(error: &SamplingExecutionError) -> &'static str {
    match error {
        SamplingExecutionError::InvalidSweepRecordWidth { .. } => "invalid-result-format",
        _ => "sampling-execution-failed",
    }
}

fn detection_error_code(error: &DetectionError) -> &'static str {
    match error {
        DetectionError::Model(error) => model_error_code(error),
        DetectionError::Analysis(error) => analysis_error_code(error),
        DetectionError::InvalidSamplerCompilation { .. } => "invalid-sampler-compilation",
        DetectionError::InvalidResultFormat { .. } => "invalid-result-format",
        DetectionError::ResourceLimit(_) => "resource-limit-exceeded",
    }
}

fn detection_compile_error_code(error: &DetectionCompileError) -> &'static str {
    match error {
        DetectionCompileError::InvalidCircuit(error) => detection_error_code(error),
    }
}

fn detection_execution_error_code(error: &DetectionExecutionError) -> &'static str {
    match error {
        DetectionExecutionError::Conversion(error) => detection_error_code(error),
        DetectionExecutionError::Sampling(error) => sampling_execution_error_code(error),
        _ => "detection-execution-failed",
    }
}

fn dem_error_code(error: &DemError) -> &'static str {
    match error {
        DemError::Model(error) => model_error_code(error),
        DemError::InvalidSamplerCompilation { .. } => "invalid-sampler-compilation",
        DemError::InvalidResultFormat { .. } => "invalid-result-format",
        DemError::ResourceLimit(_) => "resource-limit-exceeded",
    }
}

fn dem_execution_error_code(error: &DemSamplingExecutionError) -> &'static str {
    match error {
        DemSamplingExecutionError::InvalidRequest(error) => dem_error_code(error),
        _ => "dem-sampling-execution-failed",
    }
}

fn cli_error_help(error: &CliError) -> Option<&'static str> {
    match error {
        CliError::MissingRecordWidth => Some("Provide layout counts or `--bits_per_shot`."),
        CliError::MissingConvertTypes => Some("Add `--types` with unique M, D, or L letters."),
        CliError::UnknownHelpTopic { .. } => Some("Run `stab help commands` to list help topics."),
        CliError::MissingInspectModelType | CliError::UnknownInspectModelType { .. } => {
            Some("Pass `--type=stim` for a circuit or `--type=dem` for a detector error model.")
        }
        _ => None,
    }
}

fn cli_error_context(error: &CliError) -> Value {
    match error {
        CliError::ReadInput(_) | CliError::WriteOutput(_) => json!({}),
        CliError::ReadPath { path, .. } | CliError::WritePath { path, .. } => json!({
            "path": path.to_string_lossy(),
        }),
        CliError::SampleOutput(source) => sample_run_error_context(source, None),
        CliError::SamplePath { path, source } => {
            sample_run_error_context(source, Some(path.as_path()))
        }
        CliError::ConflictingFileRoles { first, second } => json!({
            "first_role": first,
            "second_role": second,
        }),
        CliError::IoPlanInvariant { message } => json!({
            "invariant": message,
        }),
        CliError::MissingInspectModelType => json!({
            "input": "stdin",
        }),
        CliError::UnknownInspectModelType { path } => json!({
            "path": path.to_string_lossy(),
        }),
        CliError::SerializeAgentOutput(_) => json!({}),
        CliError::AgentOutputInvariant { message } => json!({
            "invariant": message,
        }),
        CliError::Model(error) => model_error_context(error),
        CliError::Analysis(error) => analysis_error_context(error),
        CliError::Record(error) | CliError::InputRecord { source: error, .. } => {
            format_error_context(error)
        }
        CliError::SamplingCompile(error) => sampling_compile_error_context(error),
        CliError::SamplingExecution(_) => json!({}),
        CliError::Detection(error) => detection_error_context(error),
        CliError::DetectionCompile(error) => match error {
            DetectionCompileError::InvalidCircuit(error) => detection_error_context(error),
        },
        CliError::DetectionExecution(error) => detection_execution_error_context(error),
        CliError::Dem(error) => dem_error_context(error),
        CliError::DemSamplingExecution(error) => match error {
            DemSamplingExecutionError::InvalidRequest(error) => dem_error_context(error),
            _ => json!({}),
        },
        CliError::InputByteOffsetOverflow { kind } => json!({
            "input_kind": kind,
        }),
        CliError::UnsupportedRepetitionTask { task }
        | CliError::UnsupportedSurfaceTask { task }
        | CliError::UnsupportedColorTask { task } => json!({
            "task": task,
        }),
        CliError::UnsupportedConversion
        | CliError::ConflictingObservableRouting
        | CliError::MissingRecordWidth
        | CliError::MissingRecordTypesForDets
        | CliError::MissingConvertTypes
        | CliError::MeasurementCountOverflow => json!({}),
        CliError::UnsupportedStimConversionOption { flag } => json!({
            "flag": flag,
        }),
        CliError::UnsupportedDetectionFormat { format } => json!({
            "format": format,
        }),
        CliError::ReplayErrorRecordCountMismatch { expected, actual } => json!({
            "expected_records": expected,
            "actual_records": actual,
        }),
        CliError::InputTooLarge { kind, limit } => json!({
            "input_kind": kind,
            "limit_bytes": limit,
        }),
        CliError::UnknownConvertType { result_type }
        | CliError::DuplicateConvertType { result_type } => json!({
            "result_type": result_type.to_string(),
        }),
        CliError::IncompletePtb64OutputGroup { count } => json!({
            "trailing_records": count,
        }),
        CliError::UnknownHelpTopic { topic } => json!({
            "topic": topic,
        }),
    }
}

fn sample_run_error_context(error: &RunError<std::io::Error>, path: Option<&Path>) -> Value {
    let progress = error.progress();
    let mut context = match error {
        RunError::Engine { .. } => json!({
            "failure_kind": "engine",
            "committed_shots": progress.committed_shots().get(),
            "attempted_batch_shots": progress.attempted_batch_shots().get(),
        }),
        RunError::Sink { phase, .. } => json!({
            "failure_kind": "sink",
            "sink_phase": phase.as_str(),
            "committed_shots": progress.committed_shots().get(),
            "attempted_batch_shots": progress.attempted_batch_shots().get(),
        }),
    };
    if let (Some(path), Value::Object(fields)) = (path, &mut context) {
        fields.insert(
            "path".to_owned(),
            Value::String(path.to_string_lossy().into_owned()),
        );
    }
    context
}

fn format_error_with_offset(error: &CliError) -> Option<(&FormatError, usize)> {
    match error {
        CliError::Record(error) => Some((error, 0)),
        CliError::InputRecord {
            byte_offset,
            source,
        } => Some((source, *byte_offset)),
        _ => None,
    }
}

fn parse_error_with_offset(error: &CliError) -> Option<(&ParseError, usize)> {
    model_error(error)?.parse_error().map(|error| (error, 0))
}

fn model_error(error: &CliError) -> Option<&ModelError> {
    match error {
        CliError::Model(error) => Some(error),
        CliError::Analysis(error) => model_error_from_analysis(error),
        CliError::SamplingCompile(error) => match error {
            SamplingCompileError::Model(error) => Some(error),
            SamplingCompileError::Analysis(error) => model_error_from_analysis(error),
            SamplingCompileError::InvalidCircuit { .. }
            | SamplingCompileError::ExpandedOperationLimit { .. } => None,
        },
        CliError::Detection(error) => model_error_from_detection(error),
        CliError::DetectionCompile(DetectionCompileError::InvalidCircuit(error)) => {
            model_error_from_detection(error)
        }
        CliError::DetectionExecution(DetectionExecutionError::Conversion(error)) => {
            model_error_from_detection(error)
        }
        CliError::Dem(error) => model_error_from_dem(error),
        CliError::DemSamplingExecution(DemSamplingExecutionError::InvalidRequest(error)) => {
            model_error_from_dem(error)
        }
        _ => None,
    }
}

fn model_error_from_analysis(error: &AnalysisError) -> Option<&ModelError> {
    match error {
        AnalysisError::Model(error) => Some(error),
        _ => None,
    }
}

fn model_error_from_detection(error: &DetectionError) -> Option<&ModelError> {
    match error {
        DetectionError::Model(error) => Some(error),
        DetectionError::Analysis(error) => model_error_from_analysis(error),
        _ => None,
    }
}

fn model_error_from_dem(error: &DemError) -> Option<&ModelError> {
    match error {
        DemError::Model(error) => Some(error),
        _ => None,
    }
}

fn shifted_model_span(span: ModelByteSpan, byte_offset: usize) -> Option<JsonSpan> {
    let byte_start = span.byte_start().checked_add(byte_offset)?;
    Some(JsonSpan {
        byte_start,
        byte_length: span.byte_length(),
    })
}

fn shifted_record_span(span: RecordByteSpan, byte_offset: usize) -> Option<JsonSpan> {
    let byte_start = span.byte_start().checked_add(byte_offset)?;
    Some(JsonSpan {
        byte_start,
        byte_length: span.byte_length(),
    })
}

fn model_error_context(error: &ModelError) -> Value {
    match error {
        ModelError::Parse(error) => parse_error_context(error),
        ModelError::ResourceLimit(error) => model_resource_context(error),
        ModelError::Validation(error) => validation_error_context(error),
    }
}

fn validation_error_context(error: &ValidationError) -> Value {
    match error {
        ValidationError::UnknownGate(gate) => json!({
            "gate": gate,
        }),
        ValidationError::InvalidDomainValue { kind, value } => json!({
            "domain": kind,
            "value": value,
        }),
        ValidationError::InvalidArgumentCount {
            gate,
            expected,
            actual,
        } => json!({
            "gate": gate,
            "expected": expected,
            "actual": actual,
        }),
        ValidationError::InvalidArgument { gate, argument } => json!({
            "gate": gate,
            "argument": argument,
        }),
        ValidationError::InvalidTarget { gate, target } => json!({
            "gate": gate,
            "target": target,
        }),
        ValidationError::InvalidTargetCount { gate, count } => json!({
            "gate": gate,
            "count": count,
        }),
        ValidationError::DetectorIndexOutOfRange {
            index,
            detector_count,
        } => json!({
            "index": index,
            "detector_count": detector_count,
        }),
        _ => json!({}),
    }
}

fn analysis_error_context(error: &AnalysisError) -> Value {
    match error {
        AnalysisError::Model(error) => model_error_context(error),
        AnalysisError::InvalidDomainValue { kind, value } => json!({
            "domain": kind,
            "value": value,
        }),
        AnalysisError::ResourceLimit(error) => analysis_resource_context(error),
        _ => json!({}),
    }
}

fn sampling_compile_error_context(error: &SamplingCompileError) -> Value {
    match error {
        SamplingCompileError::Model(error) => model_error_context(error),
        SamplingCompileError::Analysis(error) => analysis_error_context(error),
        SamplingCompileError::InvalidCircuit { .. } => json!({}),
        SamplingCompileError::ExpandedOperationLimit { actual, limit } => {
            let mut context = json!({
                "resource": "expanded-operations-per-shot",
                "actual": actual.value().to_string(),
                "limit": limit.to_string(),
            });
            if actual.is_lower_bound()
                && let Value::Object(fields) = &mut context
            {
                fields.insert("actual_is_lower_bound".to_owned(), Value::Bool(true));
            }
            context
        }
    }
}

fn detection_error_context(error: &DetectionError) -> Value {
    match error {
        DetectionError::Model(error) => model_error_context(error),
        DetectionError::Analysis(error) => analysis_error_context(error),
        DetectionError::ResourceLimit(error) => detection_resource_context(error),
        _ => json!({}),
    }
}

fn detection_execution_error_context(error: &DetectionExecutionError) -> Value {
    match error {
        DetectionExecutionError::Conversion(error) => detection_error_context(error),
        DetectionExecutionError::Sampling(SamplingExecutionError::InvalidSweepRecordWidth {
            expected,
            actual,
        }) => json!({
            "expected_bits": expected,
            "actual_bits": actual,
        }),
        _ => json!({}),
    }
}

fn dem_error_context(error: &DemError) -> Value {
    match error {
        DemError::Model(error) => model_error_context(error),
        DemError::ResourceLimit(error) => dem_resource_context(error),
        _ => json!({}),
    }
}

fn parse_error_context(error: &ParseError) -> Value {
    match error.context() {
        ParseErrorContext::Model { dialect } => json!({
            "dialect": model_dialect_name(*dialect),
        }),
        ParseErrorContext::Utf8 {
            dialect,
            valid_up_to,
            error_length,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "valid_up_to": valid_up_to,
            "error_length": error_length,
        }),
        ParseErrorContext::Instruction {
            dialect,
            instruction,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "instruction": instruction,
        }),
        ParseErrorContext::DomainValue {
            dialect,
            kind,
            value,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "domain": kind,
            "value": value,
        }),
        ParseErrorContext::ArgumentCount {
            dialect,
            instruction,
            expected,
            actual,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "instruction": instruction,
            "expected": expected,
            "actual": actual,
        }),
        ParseErrorContext::Argument {
            dialect,
            instruction,
            argument,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "instruction": instruction,
            "argument": argument,
        }),
        ParseErrorContext::Target {
            dialect,
            instruction,
            target,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "instruction": instruction,
            "target": target,
        }),
        ParseErrorContext::TargetCount {
            dialect,
            instruction,
            actual,
        } => json!({
            "dialect": model_dialect_name(*dialect),
            "instruction": instruction,
            "actual": actual,
        }),
        context => json!({
            "dialect": model_dialect_name(context.dialect()),
        }),
    }
}

enum ResourceDiagnostic<'a> {
    Model(&'a ModelResourceLimitError),
    Analysis(&'a AnalysisResourceLimitError),
    Detection(&'a DetectionResourceLimitError),
    Dem(&'a DemResourceLimitError),
}

impl ResourceDiagnostic<'_> {
    fn message(&self) -> String {
        match self {
            Self::Model(error) => error.to_string(),
            Self::Analysis(error) => error.to_string(),
            Self::Detection(error) => error.to_string(),
            Self::Dem(error) => error.to_string(),
        }
    }

    fn span(&self) -> Option<JsonSpan> {
        match self {
            Self::Model(error) => shifted_model_span(error.span(), 0),
            Self::Analysis(_) | Self::Detection(_) | Self::Dem(_) => None,
        }
    }

    fn context(&self) -> Value {
        match self {
            Self::Model(error) => model_resource_context(error),
            Self::Analysis(error) => analysis_resource_context(error),
            Self::Detection(error) => detection_resource_context(error),
            Self::Dem(error) => dem_resource_context(error),
        }
    }
}

fn resource_diagnostic(error: &CliError) -> Option<ResourceDiagnostic<'_>> {
    if let Some(error) = model_error(error).and_then(ModelError::resource_limit_error) {
        return Some(ResourceDiagnostic::Model(error));
    }
    if let Some(error) = analysis_error(error).and_then(AnalysisError::resource_limit_error) {
        return Some(ResourceDiagnostic::Analysis(error));
    }
    if let Some(error) = detection_resource_error(error) {
        return Some(ResourceDiagnostic::Detection(error));
    }
    dem_resource_error(error).map(ResourceDiagnostic::Dem)
}

fn analysis_error(error: &CliError) -> Option<&AnalysisError> {
    match error {
        CliError::Analysis(error) => Some(error),
        CliError::SamplingCompile(SamplingCompileError::Analysis(error)) => Some(error),
        CliError::Detection(DetectionError::Analysis(error))
        | CliError::DetectionCompile(DetectionCompileError::InvalidCircuit(
            DetectionError::Analysis(error),
        ))
        | CliError::DetectionExecution(DetectionExecutionError::Conversion(
            DetectionError::Analysis(error),
        )) => Some(error),
        _ => None,
    }
}

fn detection_resource_error(error: &CliError) -> Option<&DetectionResourceLimitError> {
    match error {
        CliError::Detection(DetectionError::ResourceLimit(error))
        | CliError::DetectionCompile(DetectionCompileError::InvalidCircuit(
            DetectionError::ResourceLimit(error),
        ))
        | CliError::DetectionExecution(DetectionExecutionError::Conversion(
            DetectionError::ResourceLimit(error),
        )) => Some(error),
        _ => None,
    }
}

fn dem_resource_error(error: &CliError) -> Option<&DemResourceLimitError> {
    match error {
        CliError::Dem(DemError::ResourceLimit(error))
        | CliError::DemSamplingExecution(DemSamplingExecutionError::InvalidRequest(
            DemError::ResourceLimit(error),
        )) => Some(error),
        _ => None,
    }
}

fn model_resource_context(error: &ModelResourceLimitError) -> Value {
    json!({
        "operation": error.operation().as_str(),
        "resource": error.resource().as_str(),
        "actual": error.actual(),
        "limit": error.limit(),
    })
}

fn analysis_resource_context(error: &AnalysisResourceLimitError) -> Value {
    json!({
        "operation": error.operation().as_str(),
        "resource": error.resource().as_str(),
        "actual": error.actual(),
        "limit": error.limit(),
    })
}

fn detection_resource_context(error: &DetectionResourceLimitError) -> Value {
    let resource = match error.kind() {
        DetectionResourceKind::RecordBits(_) => "record-bits",
        DetectionResourceKind::SamplingExpandedOperations => "expanded-operations-per-shot",
        DetectionResourceKind::RepeatNesting => "repeat-nesting",
        DetectionResourceKind::ExpandedInstructions => "expanded-operations",
        DetectionResourceKind::RepeatIterations => "repeat-iterations",
        DetectionResourceKind::CompiledTerms => "compiled-terms",
        DetectionResourceKind::CompiledBytes => "materialized-bytes",
    };
    let subject = match error.kind() {
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::DetectionRecord) => {
            Some("detection-record")
        }
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::MeasurementRecord) => {
            Some("measurement-record")
        }
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::SweepRecord) => {
            Some("sweep-record")
        }
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::ObservableCount) => {
            Some("observable-count")
        }
        _ => None,
    };
    let operation = if matches!(
        error.kind(),
        DetectionResourceKind::SamplingExpandedOperations
    ) {
        "detection-sampling"
    } else {
        "detection-conversion"
    };
    let mut context = json!({
        "operation": operation,
        "resource": resource,
        "subject": subject,
        "actual": error.actual(),
        "limit": error.limit(),
    });
    if error.actual_is_lower_bound()
        && let Value::Object(fields) = &mut context
    {
        fields.insert("actual_is_lower_bound".to_owned(), Value::Bool(true));
    }
    context
}

fn dem_resource_context(error: &DemResourceLimitError) -> Value {
    let resource = match error.kind() {
        DemResourceKind::SampledErrorApplications => "sampled-error-applications",
        DemResourceKind::ReplayWorkUnits => "replay-work-units",
        DemResourceKind::ActiveBatchBytes => "active-batch-bytes",
    };
    json!({
        "operation": "detector-error-model-sampling",
        "resource": resource,
        "actual": error.actual(),
        "limit": error.limit(),
    })
}

fn model_dialect_name(dialect: ModelDialect) -> &'static str {
    dialect.as_str()
}

fn format_error_context(error: &FormatError) -> Value {
    match error.context() {
        FormatErrorContext::None => json!({}),
        FormatErrorContext::RecordWidth {
            actual_bits,
            expected_bits,
        } => json!({
            "actual_bits": actual_bits,
            "expected_bits": expected_bits,
        }),
        FormatErrorContext::MinimumRecordWidth {
            actual_bits,
            minimum_bits,
        } => json!({
            "actual_bits": actual_bits,
            "minimum_bits": minimum_bits,
        }),
        FormatErrorContext::InvalidByte { byte } => json!({
            "byte": byte,
        }),
        FormatErrorContext::Index {
            result_type,
            index,
            exclusive_bound,
        } => {
            let result_type = result_type.map(dets_result_type_name);
            json!({
                "result_type": result_type,
                "index": index,
                "exclusive_bound": exclusive_bound,
            })
        }
        FormatErrorContext::InputLengthMultiple {
            actual_bytes,
            byte_multiple,
        } => json!({
            "actual_bytes": actual_bytes,
            "byte_multiple": byte_multiple,
        }),
        FormatErrorContext::MinimumInputLength {
            actual_bytes,
            minimum_bytes,
        } => json!({
            "actual_bytes": actual_bytes,
            "minimum_bytes": minimum_bytes,
        }),
        FormatErrorContext::RunLength {
            decoded_bits,
            expected_bits,
        } => json!({
            "decoded_bits": decoded_bits,
            "expected_bits": expected_bits,
        }),
    }
}

fn dets_result_type_name(result_type: DetsResultType) -> &'static str {
    match result_type {
        DetsResultType::Measurement => "measurement",
        DetsResultType::Detector => "detector",
        DetsResultType::Observable => "observable",
    }
}

fn clap_error_code(kind: clap::error::ErrorKind) -> &'static str {
    match kind {
        clap::error::ErrorKind::InvalidValue => "cli-invalid-value",
        clap::error::ErrorKind::UnknownArgument => "cli-unknown-argument",
        clap::error::ErrorKind::InvalidSubcommand => "cli-invalid-subcommand",
        clap::error::ErrorKind::NoEquals => "cli-no-equals",
        clap::error::ErrorKind::ValueValidation => "cli-value-validation",
        clap::error::ErrorKind::TooManyValues => "cli-too-many-values",
        clap::error::ErrorKind::TooFewValues => "cli-too-few-values",
        clap::error::ErrorKind::WrongNumberOfValues => "cli-wrong-number-of-values",
        clap::error::ErrorKind::ArgumentConflict => "cli-argument-conflict",
        clap::error::ErrorKind::MissingRequiredArgument => "cli-missing-required-argument",
        clap::error::ErrorKind::MissingSubcommand => "cli-missing-subcommand",
        clap::error::ErrorKind::DisplayHelp => "cli-display-help",
        clap::error::ErrorKind::DisplayVersion => "cli-display-version",
        clap::error::ErrorKind::Io => "cli-io-failed",
        clap::error::ErrorKind::Format => "cli-format-failed",
        _ => "cli-argument-error",
    }
}

fn clap_error_kind(kind: clap::error::ErrorKind) -> &'static str {
    match kind {
        clap::error::ErrorKind::InvalidValue => "invalid-value",
        clap::error::ErrorKind::UnknownArgument => "unknown-argument",
        clap::error::ErrorKind::InvalidSubcommand => "invalid-subcommand",
        clap::error::ErrorKind::NoEquals => "no-equals",
        clap::error::ErrorKind::ValueValidation => "value-validation",
        clap::error::ErrorKind::TooManyValues => "too-many-values",
        clap::error::ErrorKind::TooFewValues => "too-few-values",
        clap::error::ErrorKind::WrongNumberOfValues => "wrong-number-of-values",
        clap::error::ErrorKind::ArgumentConflict => "argument-conflict",
        clap::error::ErrorKind::MissingRequiredArgument => "missing-required-argument",
        clap::error::ErrorKind::MissingSubcommand => "missing-subcommand",
        clap::error::ErrorKind::DisplayHelp => "display-help",
        clap::error::ErrorKind::DisplayVersion => "display-version",
        clap::error::ErrorKind::Io => "io",
        clap::error::ErrorKind::Format => "format",
        _ => "other",
    }
}
