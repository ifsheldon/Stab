use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use clap::ValueEnum;
use serde::Serialize;
use serde_json::{Value, json};
use stab_core::{
    ByteSpan, CircuitError, DetsResultType, DiagnosticSeverity, FormatError, FormatErrorContext,
};
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
    WriteOutput(std::io::Error),

    #[error("failed to write {path}: {source}")]
    WritePath {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("file roles {first} and {second} refer to the same file")]
    ConflictingFileRoles {
        first: &'static str,
        second: &'static str,
    },

    #[error("internal CLI I/O plan invariant failed: {message}")]
    IoPlanInvariant { message: &'static str },

    #[error("{0}")]
    Circuit(#[from] CircuitError),

    #[error("{source}")]
    InputRecord {
        byte_offset: usize,
        #[source]
        source: CircuitError,
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

pub(crate) fn write_frame0_deprecation(
    writer: &mut impl Write,
    format: ErrorFormatArg,
) -> io::Result<()> {
    match format {
        ErrorFormatArg::Human => writeln!(
            writer,
            "[DEPRECATION] Use `--skip_reference_sample` instead of `--frame0`"
        ),
        ErrorFormatArg::Json => write_json(
            writer,
            &JsonDiagnostic {
                schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
                code: "deprecated-frame0",
                severity: DiagnosticSeverity::Warning.as_str(),
                message: "`--frame0` is deprecated".to_string(),
                span: None,
                labels: Vec::new(),
                help: Some("Use `--skip_reference_sample` instead."),
                context: json!({
                    "flag": "--frame0",
                    "replacement": "--skip_reference_sample",
                }),
            },
        ),
    }
}

pub(crate) fn write_prepend_observables_deprecation(
    writer: &mut impl Write,
    format: ErrorFormatArg,
) -> io::Result<()> {
    const MESSAGE: &str = "Avoid using `--prepend_observables`. Data readers assume observables are appended, not prepended.";
    match format {
        ErrorFormatArg::Human => writeln!(writer, "[DEPRECATION] {MESSAGE}"),
        ErrorFormatArg::Json => write_json(
            writer,
            &JsonDiagnostic {
                schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
                code: "deprecated-prepend-observables",
                severity: DiagnosticSeverity::Warning.as_str(),
                message: "`--prepend_observables` is deprecated".to_string(),
                span: None,
                labels: Vec::new(),
                help: Some("Use appended observables or `--obs_out` instead."),
                context: json!({
                    "flag": "--prepend_observables",
                    "replacement": "--obs_out",
                }),
            },
        ),
    }
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

impl From<ByteSpan> for JsonSpan {
    fn from(span: ByteSpan) -> Self {
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
    let (format_error, byte_offset) = format_error_with_offset(error).unzip();
    JsonDiagnostic {
        schema_version: JSON_DIAGNOSTIC_SCHEMA_VERSION,
        code: format_error.map_or_else(|| cli_error_code(error), |error| error.code().as_str()),
        severity: DiagnosticSeverity::Error.as_str(),
        message: format_error
            .map_or_else(|| error.to_string(), |error| error.message().to_string()),
        span: format_error
            .and_then(FormatError::span)
            .and_then(|span| shifted_span(span, byte_offset.unwrap_or(0))),
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
        CliError::ConflictingFileRoles { .. } => "conflicting-file-roles",
        CliError::IoPlanInvariant { .. } => "io-plan-invariant-failed",
        CliError::Circuit(error) => circuit_error_code(error),
        CliError::InputRecord { source, .. } => circuit_error_code(source),
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
        CliError::InvalidUtf8Input => "invalid-utf8-input",
        CliError::MeasurementCountOverflow => "measurement-count-overflow",
    }
}

fn circuit_error_code(error: &CircuitError) -> &'static str {
    match error {
        CircuitError::UnknownGate(_) => "unknown-gate",
        CircuitError::InvalidDomainValue { .. } => "invalid-domain-value",
        CircuitError::ParseLine { .. } => "circuit-parse-line",
        CircuitError::InvalidArgumentCount { .. } => "invalid-gate-argument-count",
        CircuitError::InvalidArgument { .. } => "invalid-gate-argument",
        CircuitError::InvalidTarget { .. } => "invalid-gate-target",
        CircuitError::InvalidTargetCount { .. } => "invalid-gate-target-count",
        CircuitError::InvalidTableauConversion { .. } => "invalid-tableau-conversion",
        CircuitError::InvalidCircuitSimplification { .. } => "invalid-circuit-simplification",
        CircuitError::InvalidSamplerCompilation { .. } => "invalid-sampler-compilation",
        CircuitError::InvalidResultFormat(error) => error.code().as_str(),
        CircuitError::ResourceLimit(error) => error.code(),
        CircuitError::CircuitIo { .. } => "circuit-io-failed",
        CircuitError::InvalidDetectorErrorModel { .. } => "invalid-detector-error-model",
        CircuitError::UnterminatedRepeatBlock => "unterminated-repeat-block",
        CircuitError::UnexpectedRepeatTerminator => "unexpected-repeat-terminator",
    }
}

fn cli_error_help(error: &CliError) -> Option<&'static str> {
    match error {
        CliError::MissingRecordWidth => Some("Provide layout counts or `--bits_per_shot`."),
        CliError::MissingConvertTypes => Some("Add `--types` with unique M, D, or L letters."),
        CliError::UnknownHelpTopic { .. } => Some("Run `stab help commands` to list help topics."),
        _ => None,
    }
}

fn cli_error_context(error: &CliError) -> Value {
    match error {
        CliError::ReadInput(_) | CliError::WriteOutput(_) => json!({}),
        CliError::ReadPath { path, .. } | CliError::WritePath { path, .. } => json!({
            "path": path.to_string_lossy(),
        }),
        CliError::ConflictingFileRoles { first, second } => json!({
            "first_role": first,
            "second_role": second,
        }),
        CliError::IoPlanInvariant { message } => json!({
            "invariant": message,
        }),
        CliError::Circuit(error) => circuit_error_context(error),
        CliError::InputRecord { source, .. } => circuit_error_context(source),
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
        | CliError::InvalidUtf8Input
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

fn format_error_with_offset(error: &CliError) -> Option<(&FormatError, usize)> {
    match error {
        CliError::Circuit(error) => error.format_error().map(|error| (error, 0)),
        CliError::InputRecord {
            byte_offset,
            source,
        } => source.format_error().map(|error| (error, *byte_offset)),
        _ => None,
    }
}

fn shifted_span(span: ByteSpan, byte_offset: usize) -> Option<JsonSpan> {
    let byte_start = span.byte_start().checked_add(byte_offset)?;
    Some(JsonSpan {
        byte_start,
        byte_length: span.byte_length(),
    })
}

fn circuit_error_context(error: &CircuitError) -> Value {
    match error {
        CircuitError::UnknownGate(gate) => json!({
            "gate": gate,
        }),
        CircuitError::InvalidDomainValue { kind, value } => json!({
            "domain": kind,
            "value": value,
        }),
        CircuitError::ParseLine { line, .. } => json!({
            "line": line,
        }),
        CircuitError::InvalidArgumentCount {
            gate,
            expected,
            actual,
        } => json!({
            "gate": gate,
            "expected": expected,
            "actual": actual,
        }),
        CircuitError::InvalidArgument { gate, argument } => json!({
            "gate": gate,
            "argument": argument,
        }),
        CircuitError::InvalidTarget { gate, target } => json!({
            "gate": gate,
            "target": target,
        }),
        CircuitError::InvalidTargetCount { gate, count } => json!({
            "gate": gate,
            "count": count,
        }),
        CircuitError::InvalidTableauConversion { .. }
        | CircuitError::InvalidCircuitSimplification { .. }
        | CircuitError::InvalidSamplerCompilation { .. }
        | CircuitError::InvalidDetectorErrorModel { .. }
        | CircuitError::UnterminatedRepeatBlock
        | CircuitError::UnexpectedRepeatTerminator => json!({}),
        CircuitError::InvalidResultFormat(error) => format_error_context(error),
        CircuitError::ResourceLimit(error) => json!({
            "operation": error.operation().as_str(),
            "resource": error.resource().as_str(),
            "actual": error.actual(),
            "limit": error.limit(),
        }),
        CircuitError::CircuitIo {
            operation, kind, ..
        } => json!({
            "operation": operation,
            "io_error_kind": io_error_kind_name(*kind),
        }),
    }
}

fn io_error_kind_name(kind: io::ErrorKind) -> &'static str {
    match kind {
        io::ErrorKind::NotFound => "not-found",
        io::ErrorKind::PermissionDenied => "permission-denied",
        io::ErrorKind::ConnectionRefused => "connection-refused",
        io::ErrorKind::ConnectionReset => "connection-reset",
        io::ErrorKind::ConnectionAborted => "connection-aborted",
        io::ErrorKind::NotConnected => "not-connected",
        io::ErrorKind::AddrInUse => "address-in-use",
        io::ErrorKind::AddrNotAvailable => "address-not-available",
        io::ErrorKind::BrokenPipe => "broken-pipe",
        io::ErrorKind::AlreadyExists => "already-exists",
        io::ErrorKind::WouldBlock => "would-block",
        io::ErrorKind::InvalidInput => "invalid-input",
        io::ErrorKind::InvalidData => "invalid-data",
        io::ErrorKind::TimedOut => "timed-out",
        io::ErrorKind::WriteZero => "write-zero",
        io::ErrorKind::Interrupted => "interrupted",
        io::ErrorKind::Unsupported => "unsupported",
        io::ErrorKind::UnexpectedEof => "unexpected-eof",
        io::ErrorKind::OutOfMemory => "out-of-memory",
        _ => "other",
    }
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
