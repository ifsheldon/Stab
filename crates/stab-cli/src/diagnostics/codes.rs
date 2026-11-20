//! Stable diagnostic codes of CLI errors and the library errors they wrap.

use stab_analysis::{AnalysisError, CodeFamily};
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
        CliError::UnsupportedGenTask(error) => match error.family() {
            CodeFamily::Repetition => "unsupported-repetition-task",
            CodeFamily::Surface => "unsupported-surface-task",
            CodeFamily::Color => "unsupported-color-task",
        },
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
        ModelError::AllocationFailed { .. } => RESOURCE_LIMIT_CODE,
        ModelError::Validation(error) => error.code().as_str(),
        ModelError::Cancelled(_) => CANCELLED_CODE,
    }
}

/// Stable diagnostic code of an operation stopped by its cancellation signal. The CLI passes
/// a never-cancelled signal, so only a library caller embedding these diagnostics observes it.
const CANCELLED_CODE: &str = "cancelled";

/// Stable diagnostic code of a memory-budget refusal or of an allocation the allocator refused.
const RESOURCE_LIMIT_CODE: &str = "resource-limit-exceeded";

fn analysis_error_code(error: &AnalysisError) -> &'static str {
    match error {
        AnalysisError::Model(error) => model_error_code(error),
        AnalysisError::ResourceLimit(_) | AnalysisError::AllocationFailed { .. } => {
            RESOURCE_LIMIT_CODE
        }
        AnalysisError::Cancelled(_) => CANCELLED_CODE,
        AnalysisError::NonDeterministicTargets(_) => "non-deterministic-targets",
        AnalysisError::NonUnitaryOperation { .. } => "non-unitary-operation",
        AnalysisError::PauliNotPreserved { .. } => "pauli-not-preserved",
        AnalysisError::AntiHermitianPauliProduct { .. } => "anti-hermitian-pauli-product",
        AnalysisError::UnsupportedInput { .. } => "unsupported-analysis-input",
        AnalysisError::NoiseRequiresApproximation { .. } => "approximate-disjoint-errors-required",
        AnalysisError::UnanalyzableNoise(_) => "unanalyzable-noise",
        AnalysisError::ErrorDecompositionFailed { .. } => "error-decomposition-failed",
        AnalysisError::NonGraphlikeError => "non-graphlike-error",
        AnalysisError::NoLogicalErrorFound(_) => "no-logical-error-found",
        AnalysisError::InvalidRequest(_) => "invalid-analysis-request",
        AnalysisError::InvalidGeneratorParameter { .. } => "invalid-generator-parameter",
        AnalysisError::GateDataUnavailable { .. } => "gate-data-unavailable",
        AnalysisError::CountOverflow { .. } => "analysis-count-overflow",
        AnalysisError::NonFiniteCoordinate { .. } => "non-finite-coordinate",
        AnalysisError::MeasurementRecordOutOfRange { .. } => "measurement-record-out-of-range",
        AnalysisError::InternalInvariant { .. } => "analysis-invariant-failed",
    }
}

fn sampling_compile_error_code(error: &SamplingCompileError) -> &'static str {
    match error {
        SamplingCompileError::Model(error) => model_error_code(error),
        SamplingCompileError::Analysis(error) => analysis_error_code(error),
        SamplingCompileError::InvalidCircuit { .. } => "invalid-sampler-compilation",
        SamplingCompileError::CompiledBytesLimit { .. }
        | SamplingCompileError::StorageAllocation { .. } => RESOURCE_LIMIT_CODE,
        SamplingCompileError::Cancelled(_) => CANCELLED_CODE,
    }
}

fn sampling_execution_error_code(error: &SamplingExecutionError) -> &'static str {
    match error {
        SamplingExecutionError::InvalidSweepRecordWidth { .. } => "invalid-result-format",
        SamplingExecutionError::SessionStorageLimit { .. }
        | SamplingExecutionError::SessionStorageAllocation { .. } => RESOURCE_LIMIT_CODE,
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
        DetectionError::ResourceLimit(_) | DetectionError::StorageAllocation { .. } => {
            RESOURCE_LIMIT_CODE
        }
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
        DetectionExecutionError::SessionStorageLimit { .. }
        | DetectionExecutionError::SessionStorageAllocation { .. } => RESOURCE_LIMIT_CODE,
        DetectionExecutionError::Cancelled(_) => CANCELLED_CODE,
        _ => "detection-execution-failed",
    }
}

fn dem_error_code(error: &DemError) -> &'static str {
    match error {
        DemError::Model(error) => model_error_code(error),
        DemError::InvalidSamplerCompilation { .. } => "invalid-sampler-compilation",
        DemError::InvalidResultFormat { .. } => "invalid-result-format",
        DemError::ResourceLimit(_) | DemError::StorageAllocation { .. } => RESOURCE_LIMIT_CODE,
        DemError::Cancelled(_) => CANCELLED_CODE,
    }
}

fn dem_execution_error_code(error: &DemSamplingExecutionError) -> &'static str {
    match error {
        DemSamplingExecutionError::InvalidRequest(error) => dem_error_code(error),
        DemSamplingExecutionError::SessionStorageAllocation { .. } => RESOURCE_LIMIT_CODE,
        _ => "dem-sampling-execution-failed",
    }
}
