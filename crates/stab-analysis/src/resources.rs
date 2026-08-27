use std::fmt::{Display, Formatter};

/// Analysis operation whose configurable resource budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitToDetectorErrorModel,
    CircuitPass,
    CircuitFlatten,
    DetectingRegions,
    DetectorErrorModelFlatten,
    FeedbackInlining,
    FlowGeneration,
    LogicalErrorSearch,
    MissingDetectorDiscovery,
    PauliConjugation,
    SatMaterialization,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitToDetectorErrorModel => "circuit-to-detector-error-model",
            Self::CircuitPass => "circuit-pass",
            Self::CircuitFlatten => "circuit-flatten",
            Self::DetectingRegions => "detecting-regions",
            Self::DetectorErrorModelFlatten => "detector-error-model-flatten",
            Self::FeedbackInlining => "feedback-inlining",
            Self::FlowGeneration => "flow-generation",
            Self::LogicalErrorSearch => "logical-error-search",
            Self::MissingDetectorDiscovery => "missing-detector-discovery",
            Self::PauliConjugation => "pauli-conjugation",
            Self::SatMaterialization => "sat-materialization",
        }
    }
}

/// Analysis resource dimension whose configurable budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    RepeatNesting,
    RepresentedItems,
    TraversalWork,
    LiveStateUnits,
    ExpandedOperations,
    RepeatCount,
    RepeatIterations,
    MaterializedUnits,
    MaterializedBytes,
    ProjectedPayloadBytes,
    TargetOccurrences,
    ArgumentValues,
    ErrorMechanisms,
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
    OutputRecords,
    OutputBytes,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepeatNesting => "repeat-nesting",
            Self::RepresentedItems => "represented-items",
            Self::TraversalWork => "traversal-work",
            Self::LiveStateUnits => "live-state-units",
            Self::ExpandedOperations => "expanded-operations",
            Self::RepeatCount => "repeat-count",
            Self::RepeatIterations => "repeat-iterations",
            Self::MaterializedUnits => "materialized-units",
            Self::MaterializedBytes => "materialized-bytes",
            Self::ProjectedPayloadBytes => "projected-payload-bytes",
            Self::TargetOccurrences => "target-occurrences",
            Self::ArgumentValues => "argument-values",
            Self::ErrorMechanisms => "error-mechanisms",
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
            Self::OutputRecords => "output-records",
            Self::OutputBytes => "output-bytes",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SatMaterializationResource {
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
    FixedOperation {
        operation: ResourceOperation,
        resource: ResourceKind,
    },
    CircuitToDetectorErrorModel {
        resource: ResourceKind,
    },
    CircuitPass {
        stage: CircuitPassStage,
        resource: ResourceKind,
    },
    DetectingRegions {
        resource: ResourceKind,
    },
    CircuitFlattenRepeatNesting,
    CircuitFlattenExpandedOperations,
    CircuitFlattenTargetOccurrences,
    CircuitFlattenArgumentValues,
    CircuitFlattenMaterializedBytes,
    CircuitFlattenMaterializedUnits,
    DemFlattenRepeatCount,
    DemFlattenExpandedInstructions,
    DemFlattenRepeatIterations,
    DemFlattenTargetOccurrences,
    DemFlattenArgumentValues,
    DemFlattenMaterializedBytes,
    DemFlattenMaterializedUnits,
    SatMaterialization {
        resource: SatMaterializationResource,
    },
    SatTraversalRepeatIterations {
        context: &'static str,
    },
    LogicalErrorSearch {
        context: &'static str,
        resource: LogicalErrorSearchResource,
    },
    MissingDetectorDiscovery {
        resource: ResourceKind,
    },
    LogicalErrorTraversalRepeatIterations {
        context: &'static str,
    },
}

/// Framework phase that rejected circuit-pass resources.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CircuitPassStage {
    /// The source circuit was rejected before pass dispatch.
    Input,
    /// The declared output projection was rejected before output-producing lowering.
    OutputProjection,
    /// The actual returned circuit was rejected after lowering and before release.
    Output,
}

impl CircuitPassStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::OutputProjection => "projected-output",
            Self::Output => "output",
        }
    }
}

/// Typed resource-admission failure produced by pure analysis operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    cause: ResourceLimitCause,
    actual: u64,
    limit: u64,
}

impl ResourceLimitError {
    pub(crate) const fn fixed_operation(
        operation: ResourceOperation,
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::FixedOperation {
                operation,
                resource,
            },
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_to_detector_error_model(
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitToDetectorErrorModel { resource },
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_pass(
        stage: CircuitPassStage,
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitPass { stage, resource },
            actual,
            limit,
        }
    }

    pub(crate) const fn detecting_regions(resource: ResourceKind, actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DetectingRegions { resource },
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_expanded_operations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenExpandedOperations,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenRepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenMaterializedUnits,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_target_occurrences(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenTargetOccurrences,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_argument_values(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenArgumentValues,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitFlattenMaterializedBytes,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_repeat_count(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenRepeatCount,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_expanded_instructions(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenExpandedInstructions,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_repeat_iterations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenRepeatIterations,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_target_occurrences(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenTargetOccurrences,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_argument_values(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenArgumentValues,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_materialized_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenMaterializedBytes,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::DemFlattenMaterializedUnits,
            actual,
            limit,
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
        }
    }

    pub(crate) const fn sat_traversal_repeat_iterations(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::SatTraversalRepeatIterations { context },
            actual,
            limit,
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
        }
    }

    pub(crate) const fn missing_detector_discovery(
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::MissingDetectorDiscovery { resource },
            actual,
            limit,
        }
    }

    pub(crate) const fn logical_error_traversal_repeat_iterations(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::LogicalErrorTraversalRepeatIterations { context },
            actual,
            limit,
        }
    }

    pub const fn code(self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn operation(self) -> ResourceOperation {
        match self.cause {
            ResourceLimitCause::FixedOperation { operation, .. } => operation,
            ResourceLimitCause::CircuitToDetectorErrorModel { .. } => {
                ResourceOperation::CircuitToDetectorErrorModel
            }
            ResourceLimitCause::CircuitPass { .. } => ResourceOperation::CircuitPass,
            ResourceLimitCause::DetectingRegions { .. } => ResourceOperation::DetectingRegions,
            ResourceLimitCause::CircuitFlattenRepeatNesting
            | ResourceLimitCause::CircuitFlattenExpandedOperations
            | ResourceLimitCause::CircuitFlattenTargetOccurrences
            | ResourceLimitCause::CircuitFlattenArgumentValues
            | ResourceLimitCause::CircuitFlattenMaterializedBytes
            | ResourceLimitCause::CircuitFlattenMaterializedUnits => {
                ResourceOperation::CircuitFlatten
            }
            ResourceLimitCause::DemFlattenRepeatCount
            | ResourceLimitCause::DemFlattenExpandedInstructions
            | ResourceLimitCause::DemFlattenRepeatIterations
            | ResourceLimitCause::DemFlattenTargetOccurrences
            | ResourceLimitCause::DemFlattenArgumentValues
            | ResourceLimitCause::DemFlattenMaterializedBytes
            | ResourceLimitCause::DemFlattenMaterializedUnits => {
                ResourceOperation::DetectorErrorModelFlatten
            }
            ResourceLimitCause::SatMaterialization { .. }
            | ResourceLimitCause::SatTraversalRepeatIterations { .. } => {
                ResourceOperation::SatMaterialization
            }
            ResourceLimitCause::LogicalErrorSearch { .. }
            | ResourceLimitCause::LogicalErrorTraversalRepeatIterations { .. } => {
                ResourceOperation::LogicalErrorSearch
            }
            ResourceLimitCause::MissingDetectorDiscovery { .. } => {
                ResourceOperation::MissingDetectorDiscovery
            }
        }
    }

    pub const fn resource(self) -> ResourceKind {
        match self.cause {
            ResourceLimitCause::FixedOperation { resource, .. } => resource,
            ResourceLimitCause::CircuitToDetectorErrorModel { resource } => resource,
            ResourceLimitCause::CircuitPass { resource, .. } => resource,
            ResourceLimitCause::DetectingRegions { resource } => resource,
            ResourceLimitCause::CircuitFlattenRepeatNesting => ResourceKind::RepeatNesting,
            ResourceLimitCause::CircuitFlattenExpandedOperations => {
                ResourceKind::ExpandedOperations
            }
            ResourceLimitCause::CircuitFlattenTargetOccurrences => ResourceKind::TargetOccurrences,
            ResourceLimitCause::CircuitFlattenArgumentValues => ResourceKind::ArgumentValues,
            ResourceLimitCause::CircuitFlattenMaterializedBytes => ResourceKind::MaterializedBytes,
            ResourceLimitCause::CircuitFlattenMaterializedUnits => ResourceKind::MaterializedUnits,
            ResourceLimitCause::DemFlattenRepeatCount => ResourceKind::RepeatCount,
            ResourceLimitCause::DemFlattenExpandedInstructions => ResourceKind::ExpandedOperations,
            ResourceLimitCause::DemFlattenRepeatIterations => ResourceKind::RepeatIterations,
            ResourceLimitCause::DemFlattenTargetOccurrences => ResourceKind::TargetOccurrences,
            ResourceLimitCause::DemFlattenArgumentValues => ResourceKind::ArgumentValues,
            ResourceLimitCause::DemFlattenMaterializedBytes => ResourceKind::MaterializedBytes,
            ResourceLimitCause::DemFlattenMaterializedUnits => ResourceKind::MaterializedUnits,
            ResourceLimitCause::SatMaterialization { resource } => match resource {
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
            ResourceLimitCause::SatTraversalRepeatIterations { .. } => {
                ResourceKind::RepeatIterations
            }
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
            ResourceLimitCause::LogicalErrorTraversalRepeatIterations { .. } => {
                ResourceKind::RepeatIterations
            }
            ResourceLimitCause::MissingDetectorDiscovery { resource } => resource,
        }
    }

    pub const fn actual(self) -> u64 {
        self.actual
    }

    pub const fn limit(self) -> u64 {
        self.limit
    }

    pub(crate) const fn circuit_pass_stage(self) -> Option<CircuitPassStage> {
        match self.cause {
            ResourceLimitCause::CircuitPass { stage, .. } => Some(stage),
            _ => None,
        }
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.cause {
            ResourceLimitCause::FixedOperation {
                operation,
                resource,
            } => write!(
                formatter,
                "{} {} value {} exceeds current limit {}",
                operation.as_str(),
                resource.as_str(),
                self.actual,
                self.limit
            ),
            ResourceLimitCause::CircuitToDetectorErrorModel { resource } => write!(
                formatter,
                "circuit-to-detector-error-model {} value {} exceeds current limit {}",
                resource.as_str(),
                self.actual,
                self.limit
            ),
            ResourceLimitCause::CircuitPass { stage, resource } => write!(
                formatter,
                "circuit pass {} {} value {} exceeds current limit {}",
                stage.as_str(),
                resource.as_str(),
                self.actual,
                self.limit
            ),
            ResourceLimitCause::DetectingRegions { resource } => write!(
                formatter,
                "detecting-regions {} value {} exceeds current limit {}",
                resource.as_str(),
                self.actual,
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
            ResourceLimitCause::DemFlattenRepeatCount => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports repeat counts up to {}, got {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DemFlattenExpandedInstructions => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} expanded instructions, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DemFlattenRepeatIterations => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} expanded repeat iterations, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DemFlattenTargetOccurrences => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} target occurrences, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DemFlattenArgumentValues => write!(
                formatter,
                "invalid detector error model: DEM flattened currently supports at most {} argument values, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::DemFlattenMaterializedBytes => write!(
                formatter,
                "invalid detector error model: DEM flattened would require at least {} materialized bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::DemFlattenMaterializedUnits => write!(
                formatter,
                "invalid detector error model: DEM flattened instruction vector would require {} materialized units; platform limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::SatMaterialization { resource } => match resource {
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
            ResourceLimitCause::SatTraversalRepeatIterations { context } => write!(
                formatter,
                "invalid detector error model: DEM {context} traversal currently supports at most {} expanded repeat iterations, got at least {}",
                self.limit, self.actual
            ),
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
            ResourceLimitCause::LogicalErrorTraversalRepeatIterations { context } => write!(
                formatter,
                "invalid detector error model: DEM {context} traversal currently supports at most {} expanded repeat iterations, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery {
                resource: ResourceKind::ExpandedOperations,
            } => write!(
                formatter,
                "missing-detector analysis currently supports at most {} expanded instructions, got at least {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery {
                resource: ResourceKind::RepeatNesting,
            } => write!(
                formatter,
                "missing-detector repeat nesting exceeds current limit {}, got {}",
                self.limit, self.actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery { resource } => write!(
                formatter,
                "missing-detector discovery {} value {} exceeds current limit {}",
                resource.as_str(),
                self.actual,
                self.limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
