use std::fmt::{Display, Formatter};

use crate::ByteSpan;

/// Whether a resource estimate is exact, an upper bound, or unavailable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EstimateClass {
    Exact,
    UpperBound,
    Unknown,
}

/// A resource quantity together with the strength of the estimate.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Estimate<T> {
    Exact(T),
    UpperBound(T),
    #[default]
    Unknown,
}

impl<T> Estimate<T> {
    pub const fn class(&self) -> EstimateClass {
        match self {
            Self::Exact(_) => EstimateClass::Exact,
            Self::UpperBound(_) => EstimateClass::UpperBound,
            Self::Unknown => EstimateClass::Unknown,
        }
    }

    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Exact(value) | Self::UpperBound(value) => Some(value),
            Self::Unknown => None,
        }
    }
}

impl<T> From<stab_records::EncodedSizeEstimate<T>> for Estimate<T> {
    fn from(estimate: stab_records::EncodedSizeEstimate<T>) -> Self {
        match estimate {
            stab_records::EncodedSizeEstimate::Exact(value) => Self::Exact(value),
            stab_records::EncodedSizeEstimate::Unknown => Self::Unknown,
        }
    }
}

/// Cheap resource information collected without executing the described operation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ResourceEstimate {
    input_bytes: Estimate<usize>,
    input_items: Estimate<usize>,
    expanded_operations: Estimate<usize>,
    folded_traversal: Estimate<usize>,
    scratch_bytes: Estimate<usize>,
    resident_bytes: Estimate<usize>,
    output_bytes: Estimate<usize>,
    work_units: Estimate<usize>,
}

impl ResourceEstimate {
    pub(crate) fn for_text_parse(input: &str) -> Self {
        Self::for_model_bytes(input.as_bytes())
    }

    pub(crate) fn for_model_bytes(input: &[u8]) -> Self {
        let physical_lines = if input.is_empty() {
            0
        } else {
            input.iter().filter(|byte| **byte == b'\n').count()
                + usize::from(input.last() != Some(&b'\n'))
        };
        Self {
            input_bytes: Estimate::Exact(input.len()),
            input_items: Estimate::Exact(physical_lines),
            ..Self::default()
        }
    }

    pub(crate) const fn for_sampling_request(
        input_items: Estimate<usize>,
        expanded_operations: Estimate<usize>,
        folded_traversal: Estimate<usize>,
        output_bytes: Estimate<usize>,
    ) -> Self {
        Self {
            input_items,
            expanded_operations,
            folded_traversal,
            output_bytes,
            input_bytes: Estimate::Unknown,
            scratch_bytes: Estimate::Unknown,
            resident_bytes: Estimate::Unknown,
            work_units: Estimate::Unknown,
        }
    }

    pub const fn input_bytes(&self) -> Estimate<usize> {
        self.input_bytes
    }

    pub const fn input_items(&self) -> Estimate<usize> {
        self.input_items
    }

    pub const fn expanded_operations(&self) -> Estimate<usize> {
        self.expanded_operations
    }

    pub const fn folded_traversal(&self) -> Estimate<usize> {
        self.folded_traversal
    }

    pub const fn scratch_bytes(&self) -> Estimate<usize> {
        self.scratch_bytes
    }

    pub const fn resident_bytes(&self) -> Estimate<usize> {
        self.resident_bytes
    }

    pub const fn output_bytes(&self) -> Estimate<usize> {
        self.output_bytes
    }

    pub const fn work_units(&self) -> Estimate<usize> {
        self.work_units
    }
}

/// Operation whose configurable resource budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitParse,
    DetectorErrorModelParse,
    CircuitFlatten,
    DetectionConversion,
    DetectorErrorModelFlatten,
    DetectorErrorModelSampling,
    LogicalErrorSearch,
    SatMaterialization,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitParse => "circuit-parse",
            Self::DetectorErrorModelParse => "detector-error-model-parse",
            Self::CircuitFlatten => "circuit-flatten",
            Self::DetectionConversion => "detection-conversion",
            Self::DetectorErrorModelFlatten => "detector-error-model-flatten",
            Self::DetectorErrorModelSampling => "detector-error-model-sampling",
            Self::LogicalErrorSearch => "logical-error-search",
            Self::SatMaterialization => "sat-materialization",
        }
    }
}

/// Resource dimension whose configurable budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    SourceLines,
    RepeatNesting,
    ExpandedOperations,
    RecordBits,
    MaterializedBits,
    RepeatCount,
    RepeatIterations,
    MaterializedUnits,
    MaterializedBytes,
    SampledErrorApplications,
    ReplayWorkUnits,
    CompiledTerms,
    ErrorMechanisms,
    TargetOccurrences,
    ArgumentValues,
    ErrorTargetOccurrencesPerMechanism,
    TotalErrorTargetOccurrences,
    EffectiveDetectorNodes,
    UniqueGraphEdges,
    StoredGraphTerms,
    HyperedgeDegree,
    HyperedgeIncidences,
    SearchStates,
    SearchTransitions,
    SearchStateTerms,
    StoredSearchStateTerms,
    Variables,
    Clauses,
    ClauseLiterals,
    OutputBytes,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceLines => "source-lines",
            Self::RepeatNesting => "repeat-nesting",
            Self::ExpandedOperations => "expanded-operations",
            Self::RecordBits => "record-bits",
            Self::MaterializedBits => "materialized-bits",
            Self::RepeatCount => "repeat-count",
            Self::RepeatIterations => "repeat-iterations",
            Self::MaterializedUnits => "materialized-units",
            Self::MaterializedBytes => "materialized-bytes",
            Self::SampledErrorApplications => "sampled-error-applications",
            Self::ReplayWorkUnits => "replay-work-units",
            Self::CompiledTerms => "compiled-terms",
            Self::ErrorMechanisms => "error-mechanisms",
            Self::TargetOccurrences => "target-occurrences",
            Self::ArgumentValues => "argument-values",
            Self::ErrorTargetOccurrencesPerMechanism => "error-target-occurrences-per-mechanism",
            Self::TotalErrorTargetOccurrences => "total-error-target-occurrences",
            Self::EffectiveDetectorNodes => "effective-detector-nodes",
            Self::UniqueGraphEdges => "unique-graph-edges",
            Self::StoredGraphTerms => "stored-graph-terms",
            Self::HyperedgeDegree => "hyperedge-degree",
            Self::HyperedgeIncidences => "hyperedge-incidences",
            Self::SearchStates => "search-states",
            Self::SearchTransitions => "search-transitions",
            Self::SearchStateTerms => "search-state-terms",
            Self::StoredSearchStateTerms => "stored-search-state-terms",
            Self::Variables => "variables",
            Self::Clauses => "clauses",
            Self::ClauseLiterals => "clause-literals",
            Self::OutputBytes => "output-bytes",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DetectionRecordLimitSubject {
    DetectionRecord,
    MeasurementRecord,
    SweepRecord,
    ObservableCount,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DetectionBufferLimitSubject {
    MeasurementSamples,
    DetectionRecords,
    SweepRecords,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SatMaterializationResource {
    RepeatCount,
    ExpandedInstructions,
    ErrorMechanisms,
    TargetOccurrences,
    Variables,
    Clauses,
    ClauseLiterals,
    OutputBytes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum LogicalErrorSearchResource {
    RepeatCount,
    ExpandedErrorMechanisms,
    ErrorTargetOccurrencesPerMechanism,
    TotalErrorTargetOccurrences,
    EffectiveDetectorNodes,
    UniqueGraphEdges,
    StoredGraphTerms,
    HyperedgeDegree,
    HyperedgeIncidences,
    SearchStates,
    SearchTransitions,
    SearchStateTerms,
    StoredSearchStateTerms,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ResourceLimitCause {
    CircuitSourceLines,
    CircuitRepeatNesting {
        source_line: usize,
    },
    DetectorErrorModelSourceLines,
    DetectorErrorModelRepeatNesting,
    CircuitFlattenRepeatNesting,
    CircuitFlattenExpandedOperations,
    CircuitFlattenTargetOccurrences,
    CircuitFlattenArgumentValues,
    CircuitFlattenMaterializedBytes,
    CircuitFlattenMaterializedUnits,
    DetectionRecordBits {
        subject: DetectionRecordLimitSubject,
    },
    DetectionMaterializedBits {
        subject: DetectionBufferLimitSubject,
    },
    DetectionRepeatNesting,
    DetectionRepeatCount,
    DetectionExpandedInstructions,
    DetectionRepeatIterations,
    DetectionCompiledTerms,
    DetectionCompiledBytes,
    DetectorErrorModelFlattenRepeatCount,
    DetectorErrorModelFlattenExpandedInstructions,
    DetectorErrorModelFlattenRepeatIterations,
    DetectorErrorModelFlattenTargetOccurrences,
    DetectorErrorModelFlattenArgumentValues,
    DetectorErrorModelFlattenMaterializedUnits,
    DetectorErrorModelFlattenMaterializedBytes,
    DetectorErrorModelSampledErrorApplications,
    DetectorErrorModelReplayWorkUnits,
    DetectorErrorModelMaterializedUnits,
    DetectorErrorModelMaterializedBytes,
    SatMaterialization {
        resource: SatMaterializationResource,
    },
    LogicalErrorSearch {
        context: &'static str,
        resource: LogicalErrorSearchResource,
    },
    DemTraversalRepeatIterations {
        operation: ResourceOperation,
        context: &'static str,
    },
}

/// Typed resource-admission failure with compatibility-preserving human output.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    cause: ResourceLimitCause,
    actual: u64,
    limit: u64,
    span: Option<ByteSpan>,
}

impl ResourceLimitError {
    pub(crate) const fn circuit_source_lines(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitSourceLines,
            actual: actual as u64,
            limit: limit as u64,
            span: Some(span),
        }
    }

    pub(crate) const fn circuit_repeat_nesting(
        source_line: usize,
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitRepeatNesting { source_line },
            actual: actual as u64,
            limit: limit as u64,
            span: Some(span),
        }
    }

    pub(crate) const fn dem_source_lines(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelSourceLines,
            actual: actual as u64,
            limit: limit as u64,
            span: Some(span),
        }
    }

    pub(crate) const fn dem_repeat_nesting(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelRepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
            span: Some(span),
        }
    }

    pub(crate) const fn circuit_flatten_expanded_operations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenExpandedOperations,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn circuit_flatten_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenRepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenMaterializedUnits,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn circuit_flatten_target_occurrences(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenTargetOccurrences,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn circuit_flatten_argument_values(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenArgumentValues,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenMaterializedBytes,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_record_bits(
        subject: DetectionRecordLimitSubject,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionRecordBits { subject },
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_materialized_bits(
        subject: DetectionBufferLimitSubject,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionMaterializedBits { subject },
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_repeat_count(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionRepeatCount,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionRepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn detection_expanded_instructions(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionExpandedInstructions,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_repeat_iterations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionRepeatIterations,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_compiled_terms(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionCompiledTerms,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn detection_compiled_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectionCompiledBytes,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_repeat_count(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenRepeatCount,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_expanded_instructions(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenExpandedInstructions,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_repeat_iterations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenRepeatIterations,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_target_occurrences(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenTargetOccurrences,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_argument_values(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenArgumentValues,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_materialized_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenMaterializedBytes,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelFlattenMaterializedUnits,
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_sampled_error_applications(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelSampledErrorApplications,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn dem_replay_work_units(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelReplayWorkUnits,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn dem_materialized_units(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelMaterializedUnits,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn dem_materialized_bytes(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelMaterializedBytes,
            actual: actual as u64,
            limit: limit as u64,
            span: None,
        }
    }

    pub(crate) const fn sat_materialization(
        resource: SatMaterializationResource,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::SatMaterialization { resource },
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn logical_error_search(
        context: &'static str,
        resource: LogicalErrorSearchResource,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::LogicalErrorSearch { context, resource },
            actual,
            limit,
            span: None,
        }
    }

    pub(crate) const fn dem_traversal_repeat_iterations(
        operation: ResourceOperation,
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::DemTraversalRepeatIterations { operation, context },
            actual,
            limit,
            span: None,
        }
    }

    pub const fn code(&self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn operation(&self) -> ResourceOperation {
        match self.cause {
            ResourceLimitCause::CircuitSourceLines
            | ResourceLimitCause::CircuitRepeatNesting { .. } => ResourceOperation::CircuitParse,
            ResourceLimitCause::DetectorErrorModelSourceLines
            | ResourceLimitCause::DetectorErrorModelRepeatNesting => {
                ResourceOperation::DetectorErrorModelParse
            }
            ResourceLimitCause::CircuitFlattenRepeatNesting
            | ResourceLimitCause::CircuitFlattenExpandedOperations
            | ResourceLimitCause::CircuitFlattenTargetOccurrences
            | ResourceLimitCause::CircuitFlattenArgumentValues
            | ResourceLimitCause::CircuitFlattenMaterializedBytes
            | ResourceLimitCause::CircuitFlattenMaterializedUnits => {
                ResourceOperation::CircuitFlatten
            }
            ResourceLimitCause::DetectionRecordBits { .. }
            | ResourceLimitCause::DetectionMaterializedBits { .. }
            | ResourceLimitCause::DetectionRepeatNesting
            | ResourceLimitCause::DetectionRepeatCount
            | ResourceLimitCause::DetectionExpandedInstructions
            | ResourceLimitCause::DetectionRepeatIterations
            | ResourceLimitCause::DetectionCompiledTerms
            | ResourceLimitCause::DetectionCompiledBytes => ResourceOperation::DetectionConversion,
            ResourceLimitCause::DetectorErrorModelFlattenRepeatCount
            | ResourceLimitCause::DetectorErrorModelFlattenExpandedInstructions
            | ResourceLimitCause::DetectorErrorModelFlattenRepeatIterations
            | ResourceLimitCause::DetectorErrorModelFlattenTargetOccurrences
            | ResourceLimitCause::DetectorErrorModelFlattenArgumentValues
            | ResourceLimitCause::DetectorErrorModelFlattenMaterializedUnits
            | ResourceLimitCause::DetectorErrorModelFlattenMaterializedBytes => {
                ResourceOperation::DetectorErrorModelFlatten
            }
            ResourceLimitCause::DetectorErrorModelSampledErrorApplications
            | ResourceLimitCause::DetectorErrorModelReplayWorkUnits
            | ResourceLimitCause::DetectorErrorModelMaterializedUnits
            | ResourceLimitCause::DetectorErrorModelMaterializedBytes => {
                ResourceOperation::DetectorErrorModelSampling
            }
            ResourceLimitCause::SatMaterialization { .. } => ResourceOperation::SatMaterialization,
            ResourceLimitCause::LogicalErrorSearch { .. } => ResourceOperation::LogicalErrorSearch,
            ResourceLimitCause::DemTraversalRepeatIterations { operation, .. } => operation,
        }
    }

    pub const fn resource(&self) -> ResourceKind {
        match self.cause {
            ResourceLimitCause::CircuitSourceLines
            | ResourceLimitCause::DetectorErrorModelSourceLines => ResourceKind::SourceLines,
            ResourceLimitCause::CircuitRepeatNesting { .. }
            | ResourceLimitCause::DetectorErrorModelRepeatNesting
            | ResourceLimitCause::CircuitFlattenRepeatNesting
            | ResourceLimitCause::DetectionRepeatNesting => ResourceKind::RepeatNesting,
            ResourceLimitCause::CircuitFlattenExpandedOperations
            | ResourceLimitCause::DetectionExpandedInstructions
            | ResourceLimitCause::DetectorErrorModelFlattenExpandedInstructions => {
                ResourceKind::ExpandedOperations
            }
            ResourceLimitCause::CircuitFlattenTargetOccurrences
            | ResourceLimitCause::DetectorErrorModelFlattenTargetOccurrences => {
                ResourceKind::TargetOccurrences
            }
            ResourceLimitCause::CircuitFlattenArgumentValues
            | ResourceLimitCause::DetectorErrorModelFlattenArgumentValues => {
                ResourceKind::ArgumentValues
            }
            ResourceLimitCause::CircuitFlattenMaterializedBytes
            | ResourceLimitCause::DetectorErrorModelFlattenMaterializedBytes => {
                ResourceKind::MaterializedBytes
            }
            ResourceLimitCause::DetectionRecordBits { .. } => ResourceKind::RecordBits,
            ResourceLimitCause::DetectionMaterializedBits { .. } => ResourceKind::MaterializedBits,
            ResourceLimitCause::DetectionRepeatCount
            | ResourceLimitCause::DetectorErrorModelFlattenRepeatCount => ResourceKind::RepeatCount,
            ResourceLimitCause::DetectionRepeatIterations
            | ResourceLimitCause::DetectorErrorModelFlattenRepeatIterations => {
                ResourceKind::RepeatIterations
            }
            ResourceLimitCause::DetectionCompiledTerms => ResourceKind::CompiledTerms,
            ResourceLimitCause::DetectionCompiledBytes => ResourceKind::MaterializedBytes,
            ResourceLimitCause::DetectorErrorModelSampledErrorApplications => {
                ResourceKind::SampledErrorApplications
            }
            ResourceLimitCause::DetectorErrorModelReplayWorkUnits => ResourceKind::ReplayWorkUnits,
            ResourceLimitCause::CircuitFlattenMaterializedUnits
            | ResourceLimitCause::DetectorErrorModelFlattenMaterializedUnits
            | ResourceLimitCause::DetectorErrorModelMaterializedUnits => {
                ResourceKind::MaterializedUnits
            }
            ResourceLimitCause::DetectorErrorModelMaterializedBytes => {
                ResourceKind::MaterializedBytes
            }
            ResourceLimitCause::SatMaterialization { resource } => match resource {
                SatMaterializationResource::RepeatCount => ResourceKind::RepeatCount,
                SatMaterializationResource::ExpandedInstructions => {
                    ResourceKind::ExpandedOperations
                }
                SatMaterializationResource::ErrorMechanisms => ResourceKind::ErrorMechanisms,
                SatMaterializationResource::TargetOccurrences => ResourceKind::TargetOccurrences,
                SatMaterializationResource::Variables => ResourceKind::Variables,
                SatMaterializationResource::Clauses => ResourceKind::Clauses,
                SatMaterializationResource::ClauseLiterals => ResourceKind::ClauseLiterals,
                SatMaterializationResource::OutputBytes => ResourceKind::OutputBytes,
            },
            ResourceLimitCause::LogicalErrorSearch { resource, .. } => match resource {
                LogicalErrorSearchResource::RepeatCount => ResourceKind::RepeatCount,
                LogicalErrorSearchResource::ExpandedErrorMechanisms => {
                    ResourceKind::ErrorMechanisms
                }
                LogicalErrorSearchResource::ErrorTargetOccurrencesPerMechanism => {
                    ResourceKind::ErrorTargetOccurrencesPerMechanism
                }
                LogicalErrorSearchResource::TotalErrorTargetOccurrences => {
                    ResourceKind::TotalErrorTargetOccurrences
                }
                LogicalErrorSearchResource::EffectiveDetectorNodes => {
                    ResourceKind::EffectiveDetectorNodes
                }
                LogicalErrorSearchResource::UniqueGraphEdges => ResourceKind::UniqueGraphEdges,
                LogicalErrorSearchResource::StoredGraphTerms => ResourceKind::StoredGraphTerms,
                LogicalErrorSearchResource::HyperedgeDegree => ResourceKind::HyperedgeDegree,
                LogicalErrorSearchResource::HyperedgeIncidences => {
                    ResourceKind::HyperedgeIncidences
                }
                LogicalErrorSearchResource::SearchStates => ResourceKind::SearchStates,
                LogicalErrorSearchResource::SearchTransitions => ResourceKind::SearchTransitions,
                LogicalErrorSearchResource::SearchStateTerms => ResourceKind::SearchStateTerms,
                LogicalErrorSearchResource::StoredSearchStateTerms => {
                    ResourceKind::StoredSearchStateTerms
                }
            },
            ResourceLimitCause::DemTraversalRepeatIterations { .. } => {
                ResourceKind::RepeatIterations
            }
        }
    }

    pub const fn actual(&self) -> u64 {
        self.actual
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }

    pub const fn span(&self) -> Option<ByteSpan> {
        self.span
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.cause {
            ResourceLimitCause::CircuitSourceLines => write!(
                formatter,
                "failed to parse line {}: circuit input has more than {} lines",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitRepeatNesting { source_line } => write!(
                formatter,
                "failed to parse line {source_line}: repeat nesting exceeds current limit {}",
                self.limit
            ),
            ResourceLimitCause::DetectorErrorModelSourceLines => write!(
                formatter,
                "invalid detector error model: DEM input has more than {} lines",
                self.limit
            ),
            ResourceLimitCause::DetectorErrorModelRepeatNesting => write!(
                formatter,
                "invalid detector error model: DEM repeat nesting exceeds current limit {}",
                self.limit
            ),
            ResourceLimitCause::CircuitFlattenRepeatNesting => write!(
                formatter,
                "invalid flattened circuit repeat nesting value {} exceeds fixed safety limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitFlattenExpandedOperations => write!(
                formatter,
                "invalid flattened circuit operation count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitFlattenTargetOccurrences => write!(
                formatter,
                "invalid flattened circuit target count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitFlattenArgumentValues => write!(
                formatter,
                "invalid flattened circuit argument count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitFlattenMaterializedBytes => write!(
                formatter,
                "invalid flattened circuit would require at least {} materialized bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::CircuitFlattenMaterializedUnits => write!(
                formatter,
                "invalid flattened circuit instruction vector would require {} materialized units; platform limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectionRecordBits { subject } => match subject {
                DetectionRecordLimitSubject::DetectionRecord => write!(
                    formatter,
                    "invalid result format data: detection record width {} exceeds current limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::MeasurementRecord => write!(
                    formatter,
                    "invalid result format data: measurement record width {} exceeds current detection conversion limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::SweepRecord => write!(
                    formatter,
                    "invalid result format data: sweep bit width {} exceeds current detection conversion limit {}",
                    self.actual, self.limit
                ),
                DetectionRecordLimitSubject::ObservableCount => write!(
                    formatter,
                    "invalid result format data: observable id {} exceeds current detection record limit {}",
                    self.actual.saturating_sub(1),
                    self.limit
                ),
            },
            ResourceLimitCause::DetectionMaterializedBits { subject } => {
                let subject = match subject {
                    DetectionBufferLimitSubject::MeasurementSamples => "measurement samples",
                    DetectionBufferLimitSubject::DetectionRecords => "detection records",
                    DetectionBufferLimitSubject::SweepRecords => "sweep records",
                };
                write!(
                    formatter,
                    "invalid result format data: {subject} would require {} buffered bits; current limit is {}",
                    self.actual, self.limit
                )
            }
            ResourceLimitCause::DetectionRepeatNesting => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion repeat nesting {} exceeds fixed safety limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectionRepeatCount => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion currently supports repeat counts up to {}, got {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectionExpandedInstructions => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would execute {} expanded instructions; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectionRepeatIterations => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would execute {} repeat iterations; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectionCompiledTerms => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would retain {} measurement-reference terms; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectionCompiledBytes => write!(
                formatter,
                "cannot compile circuit sampler: detection conversion would require at least {} compiled bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelFlattenRepeatCount => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports repeat counts up to {}, got {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectorErrorModelFlattenExpandedInstructions => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} expanded instructions, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectorErrorModelFlattenRepeatIterations => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} expanded repeat iterations, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectorErrorModelFlattenTargetOccurrences => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} target occurrences, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectorErrorModelFlattenArgumentValues => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} argument values, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DetectorErrorModelFlattenMaterializedBytes => write!(
                formatter,
                "invalid detector error model: DEM flattened would require at least {} materialized bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelFlattenMaterializedUnits => write!(
                formatter,
                "invalid detector error model: DEM flattened instruction vector would require {} materialized units; platform limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelSampledErrorApplications => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would apply {} sampled errors; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelReplayWorkUnits => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would require {} buffered units; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelMaterializedUnits => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would require {} buffered units; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DetectorErrorModelMaterializedBytes => write!(
                formatter,
                "cannot compile circuit sampler: DEM sampler would require at least {} materialized bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::SatMaterialization { resource } => match resource {
                SatMaterializationResource::RepeatCount => write!(
                    formatter,
                    "invalid detector error model: DEM SAT problem generation currently supports repeat counts up to {}, got {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::ExpandedInstructions => write!(
                    formatter,
                    "invalid detector error model: DEM SAT problem generation currently supports at most {} expanded instructions, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::ErrorMechanisms => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} error mechanisms, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::TargetOccurrences => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} target occurrences, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::Variables => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} variables, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::Clauses => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} clauses, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::ClauseLiterals => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} clause literals, got at least {}",
                    self.limit, self.actual
                ),
                SatMaterializationResource::OutputBytes => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} WDIMACS output bytes, got at least {}",
                    self.limit, self.actual
                ),
            },
            ResourceLimitCause::LogicalErrorSearch { context, resource } => match resource {
                LogicalErrorSearchResource::RepeatCount => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports repeat counts up to {}, got {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::ExpandedErrorMechanisms => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} expanded nonzero error mechanisms, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::ErrorTargetOccurrencesPerMechanism => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} target occurrences per nonzero error mechanism, got {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::TotalErrorTargetOccurrences => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} total target occurrences across expanded nonzero error mechanisms, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::EffectiveDetectorNodes => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} effective detector nodes, got {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::UniqueGraphEdges => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} unique graph edges, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::StoredGraphTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} stored graph payload terms, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::HyperedgeDegree => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports edges with at most {} detectors, got {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::HyperedgeIncidences => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} edge incidences, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::SearchStates => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} search states, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::SearchTransitions => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} search transitions, got at least {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::SearchStateTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} detector and observable terms per search state, got {}",
                    self.limit, self.actual
                ),
                LogicalErrorSearchResource::StoredSearchStateTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} stored detector and observable search-state terms, got at least {}",
                    self.limit, self.actual
                ),
            },
            ResourceLimitCause::DemTraversalRepeatIterations { context, .. } => write!(
                formatter,
                "invalid detector error model: DEM {context} traversal currently supports at most {} expanded repeat iterations, got at least {}",
                self.limit, self.actual
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
