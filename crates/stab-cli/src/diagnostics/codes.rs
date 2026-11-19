//! Stable diagnostic codes of CLI errors and the library errors they wrap.

use stab_analysis::AnalysisError;
use stab_engine::{
    DemError, DemSamplingExecutionError, DetectionCompileError, DetectionError,
    DetectionExecutionError, RunError, SamplingCompileError, SamplingExecutionError,
};
use stab_model::ModelError;

use super::CliError;

pub(super) fn cli_error_code(error: &CliError) -> &'static str {
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
        CliError::ReplayInputNotRewindable { .. } => "replay-input-not-rewindable",
        CliError::InputTooLarge { .. } => "input-too-large",
        CliError::MissingRecordWidth => "missing-record-width",
        CliError::MissingRecordTypesForDets => "missing-record-types-for-dets",
        CliError::MissingConvertTypes => "missing-convert-types",
        CliError::UnknownConvertType { .. } => "unknown-convert-type",
        CliError::DuplicateConvertType { .. } => "duplicate-convert-type",
        CliError::IncompletePtb64OutputGroup { .. } => "incomplete-ptb64-output-group",
        CliError::UnknownHelpTopic { .. } => "unknown-help-topic",
        CliError::MeasurementCountOverflow => "measurement-count-overflow",
        CliError::ConvertStorageBudget { .. }
        | CliError::OutputStorageBudget { .. }
        | CliError::InputStorageBudget { .. } => "resource-limit-exceeded",
        CliError::CommandMemoryBudget { source, .. } => cli_error_code(source),
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
        ModelError::Cancelled(_) => CANCELLED_CODE,
    }
}

/// Stable diagnostic code of an operation stopped by its cancellation signal. The CLI passes
/// a never-cancelled signal, so only a library caller embedding these diagnostics observes it.
const CANCELLED_CODE: &str = "cancelled";

fn analysis_error_code(error: &AnalysisError) -> &'static str {
    match error {
        AnalysisError::Model(error) => model_error_code(error),
        AnalysisError::InvalidDomainValue { .. } => "invalid-domain-value",
        AnalysisError::InvalidTableauConversion { .. } => "invalid-tableau-conversion",
        AnalysisError::InvalidCircuitSimplification { .. } => "invalid-circuit-simplification",
        AnalysisError::InvalidResultFormat { .. } => "invalid-result-format",
        AnalysisError::InvalidDetectorErrorModel { .. } => "invalid-detector-error-model",
        AnalysisError::ResourceLimit(_) => "resource-limit-exceeded",
        AnalysisError::Cancelled(_) => CANCELLED_CODE,
    }
}

fn sampling_compile_error_code(error: &SamplingCompileError) -> &'static str {
    match error {
        SamplingCompileError::Model(error) => model_error_code(error),
        SamplingCompileError::Analysis(error) => analysis_error_code(error),
        SamplingCompileError::InvalidCircuit { .. } => "invalid-sampler-compilation",
        SamplingCompileError::CompiledBytesLimit { .. } => "resource-limit-exceeded",
        SamplingCompileError::Cancelled(_) => CANCELLED_CODE,
    }
}

fn sampling_execution_error_code(error: &SamplingExecutionError) -> &'static str {
    match error {
        SamplingExecutionError::InvalidSweepRecordWidth { .. } => "invalid-result-format",
        SamplingExecutionError::Cancelled(_) => CANCELLED_CODE,
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
        DetectionError::Cancelled(_) => CANCELLED_CODE,
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
        DetectionExecutionError::Cancelled(_) => CANCELLED_CODE,
        _ => "detection-execution-failed",
    }
}

fn dem_error_code(error: &DemError) -> &'static str {
    match error {
        DemError::Model(error) => model_error_code(error),
        DemError::InvalidSamplerCompilation { .. } => "invalid-sampler-compilation",
        DemError::InvalidResultFormat { .. } => "invalid-result-format",
        DemError::ResourceLimit(_) => "resource-limit-exceeded",
        DemError::Cancelled(_) => CANCELLED_CODE,
    }
}

fn dem_execution_error_code(error: &DemSamplingExecutionError) -> &'static str {
    match error {
        DemSamplingExecutionError::InvalidRequest(error) => dem_error_code(error),
        _ => "dem-sampling-execution-failed",
    }
}
