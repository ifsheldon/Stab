use std::fmt::{Display, Formatter};

use stab_model::{MemoryBudget, MemoryBudgetExceeded, ResourceAmount};

use crate::{AnalysisOperation, CodeTask};

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
        operation: AnalysisOperation,
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
    CircuitFlattenMaterializedUnits,
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

/// What a caller-selected memory budget refused.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum MemoryBudgetCause {
    Operation {
        operation: AnalysisOperation,
        resource: ResourceKind,
    },
    /// The projected payload bytes of a circuit pass.
    CircuitPass { stage: CircuitPassStage },
    GeneratedCircuit {
        task: CodeTask,
        physical_qubits: u64,
    },
}

/// A fixed safety limit with its counted amount, or a memory-budget refusal.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Refusal {
    Fixed {
        cause: ResourceLimitCause,
        actual: u64,
        limit: u64,
    },
    MemoryBudget {
        cause: MemoryBudgetCause,
        exceeded: MemoryBudgetExceeded,
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
    refusal: Refusal,
}

impl ResourceLimitError {
    pub(crate) const fn fixed_operation(
        operation: AnalysisOperation,
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::FixedOperation {
                operation,
                resource,
            },
            actual,
            limit,
        )
    }

    const fn fixed(cause: ResourceLimitCause, actual: u64, limit: u64) -> Self {
        Self {
            refusal: Refusal::Fixed {
                cause,
                actual,
                limit,
            },
        }
    }

    const fn refused_by_budget(cause: MemoryBudgetCause, exceeded: MemoryBudgetExceeded) -> Self {
        Self {
            refusal: Refusal::MemoryBudget { cause, exceeded },
        }
    }

    /// A conservative byte requirement of `required` bytes that exceeds the operation's
    /// caller-selected `budget`.
    pub(crate) const fn memory_budget(
        operation: AnalysisOperation,
        resource: ResourceKind,
        required: u64,
        budget: MemoryBudget,
    ) -> Self {
        Self::budget_refusal(
            operation,
            resource,
            MemoryBudgetExceeded::new(ResourceAmount::exact(required), budget),
        )
    }

    /// A refusal another component's memory-budget admission reported for this operation.
    pub(crate) const fn budget_refusal(
        operation: AnalysisOperation,
        resource: ResourceKind,
        exceeded: MemoryBudgetExceeded,
    ) -> Self {
        Self::refused_by_budget(
            MemoryBudgetCause::Operation {
                operation,
                resource,
            },
            exceeded,
        )
    }

    pub(crate) const fn circuit_to_detector_error_model(
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::CircuitToDetectorErrorModel { resource },
            actual,
            limit,
        )
    }

    /// A circuit-pass limit; the projected payload bytes are limited by the caller's memory
    /// budget, every other resource by a fixed limit.
    pub(crate) const fn circuit_pass(
        stage: CircuitPassStage,
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        match resource {
            ResourceKind::ProjectedPayloadBytes => Self::refused_by_budget(
                MemoryBudgetCause::CircuitPass { stage },
                MemoryBudgetExceeded::new(
                    ResourceAmount::exact(actual),
                    MemoryBudget::from_bytes(limit),
                ),
            ),
            resource => Self::fixed(
                ResourceLimitCause::CircuitPass { stage, resource },
                actual,
                limit,
            ),
        }
    }

    pub(crate) const fn detecting_regions(resource: ResourceKind, actual: u64, limit: u64) -> Self {
        Self::fixed(
            ResourceLimitCause::DetectingRegions { resource },
            actual,
            limit,
        )
    }

    pub(crate) const fn circuit_flatten_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self::fixed(
            ResourceLimitCause::CircuitFlattenRepeatNesting,
            actual as u64,
            limit as u64,
        )
    }

    pub(crate) const fn circuit_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self::fixed(
            ResourceLimitCause::CircuitFlattenMaterializedUnits,
            actual,
            limit,
        )
    }

    pub(crate) const fn dem_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self::fixed(
            ResourceLimitCause::DemFlattenMaterializedUnits,
            actual,
            limit,
        )
    }

    pub(crate) const fn sat_materialization(
        resource: SatMaterializationResource,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::SatMaterialization { resource },
            actual,
            limit,
        )
    }

    pub(crate) const fn sat_traversal_repeat_iterations(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::SatTraversalRepeatIterations { context },
            actual,
            limit,
        )
    }

    pub(crate) const fn logical_error_search(
        context: &'static str,
        resource: LogicalErrorSearchResource,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::LogicalErrorSearch { context, resource },
            actual,
            limit,
        )
    }

    pub(crate) const fn missing_detector_discovery(
        resource: ResourceKind,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::MissingDetectorDiscovery { resource },
            actual,
            limit,
        )
    }

    pub(crate) const fn logical_error_traversal_repeat_iterations(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::fixed(
            ResourceLimitCause::LogicalErrorTraversalRepeatIterations { context },
            actual,
            limit,
        )
    }

    /// A generated circuit whose projected peak memory exceeds the caller-selected budget.
    pub(crate) const fn generated_circuit(
        task: CodeTask,
        physical_qubits: u64,
        required_bytes: u64,
        budget: MemoryBudget,
    ) -> Self {
        Self::refused_by_budget(
            MemoryBudgetCause::GeneratedCircuit {
                task,
                physical_qubits,
            },
            MemoryBudgetExceeded::new(ResourceAmount::exact(required_bytes), budget),
        )
    }

    /// The caller-selected memory budget's refusal, when that budget rather than a fixed safety
    /// limit refused the work; raising the budget admits it.
    pub const fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match &self.refusal {
            Refusal::MemoryBudget { exceeded, .. } => Some(exceeded),
            Refusal::Fixed { .. } => None,
        }
    }

    pub const fn code(self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn operation(self) -> AnalysisOperation {
        let cause = match self.refusal {
            Refusal::MemoryBudget { cause, .. } => {
                return match cause {
                    MemoryBudgetCause::Operation { operation, .. } => operation,
                    MemoryBudgetCause::CircuitPass { .. } => AnalysisOperation::CircuitPass,
                    MemoryBudgetCause::GeneratedCircuit { .. } => {
                        AnalysisOperation::CircuitGeneration
                    }
                };
            }
            Refusal::Fixed { cause, .. } => cause,
        };
        match cause {
            ResourceLimitCause::FixedOperation { operation, .. } => operation,
            ResourceLimitCause::CircuitToDetectorErrorModel { .. } => {
                AnalysisOperation::CircuitToDetectorErrorModel
            }
            ResourceLimitCause::CircuitPass { .. } => AnalysisOperation::CircuitPass,
            ResourceLimitCause::DetectingRegions { .. } => AnalysisOperation::DetectingRegions,
            ResourceLimitCause::CircuitFlattenRepeatNesting
            | ResourceLimitCause::CircuitFlattenMaterializedUnits => {
                AnalysisOperation::CircuitFlatten
            }
            ResourceLimitCause::DemFlattenMaterializedUnits => {
                AnalysisOperation::DetectorErrorModelFlatten
            }
            ResourceLimitCause::SatMaterialization { .. }
            | ResourceLimitCause::SatTraversalRepeatIterations { .. } => {
                AnalysisOperation::SatMaterialization
            }
            ResourceLimitCause::LogicalErrorSearch { .. }
            | ResourceLimitCause::LogicalErrorTraversalRepeatIterations { .. } => {
                AnalysisOperation::LogicalErrorSearch
            }
            ResourceLimitCause::MissingDetectorDiscovery { .. } => {
                AnalysisOperation::MissingDetectorDiscovery
            }
        }
    }

    pub const fn resource(self) -> ResourceKind {
        let cause = match self.refusal {
            Refusal::MemoryBudget { cause, .. } => {
                return match cause {
                    MemoryBudgetCause::Operation { resource, .. } => resource,
                    MemoryBudgetCause::CircuitPass { .. } => ResourceKind::ProjectedPayloadBytes,
                    MemoryBudgetCause::GeneratedCircuit { .. } => ResourceKind::MaterializedBytes,
                };
            }
            Refusal::Fixed { cause, .. } => cause,
        };
        match cause {
            ResourceLimitCause::FixedOperation { resource, .. } => resource,
            ResourceLimitCause::CircuitToDetectorErrorModel { resource } => resource,
            ResourceLimitCause::CircuitPass { resource, .. } => resource,
            ResourceLimitCause::DetectingRegions { resource } => resource,
            ResourceLimitCause::CircuitFlattenRepeatNesting => ResourceKind::RepeatNesting,
            ResourceLimitCause::CircuitFlattenMaterializedUnits => ResourceKind::MaterializedUnits,
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

    /// The counted amount, or the required bytes of a memory-budget refusal (its lower bound
    /// when the requirement does not fit `u64`).
    pub const fn actual(self) -> u64 {
        match self.refusal {
            Refusal::Fixed { actual, .. } => actual,
            Refusal::MemoryBudget { exceeded, .. } => exceeded.required().value(),
        }
    }

    /// The fixed limit, or the bytes of the refusing memory budget.
    pub const fn limit(self) -> u64 {
        match self.refusal {
            Refusal::Fixed { limit, .. } => limit,
            Refusal::MemoryBudget { exceeded, .. } => exceeded.budget().bytes(),
        }
    }

    pub(crate) const fn circuit_pass_stage(self) -> Option<CircuitPassStage> {
        match self.refusal {
            Refusal::Fixed {
                cause: ResourceLimitCause::CircuitPass { stage, .. },
                ..
            }
            | Refusal::MemoryBudget {
                cause: MemoryBudgetCause::CircuitPass { stage },
                ..
            } => Some(stage),
            Refusal::Fixed { .. } | Refusal::MemoryBudget { .. } => None,
        }
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let (cause, actual, limit) = match self.refusal {
            Refusal::MemoryBudget { cause, exceeded } => {
                // The requirement is already worded as a minimum, so a lower bound prints alone.
                let required = exceeded.required().value();
                let budget = exceeded.budget().bytes();
                return match cause {
                    MemoryBudgetCause::Operation {
                        operation,
                        resource,
                    } => write!(
                        formatter,
                        "{} {} needs at least {required} bytes, exceeding the {budget}-byte memory budget; raise the memory budget to admit it",
                        operation.as_str(),
                        resource.as_str(),
                    ),
                    MemoryBudgetCause::CircuitPass { stage } => write!(
                        formatter,
                        "circuit pass {} needs at least {required} projected payload bytes, exceeding the {budget}-byte memory budget; raise the memory budget to admit it",
                        stage.as_str(),
                    ),
                    MemoryBudgetCause::GeneratedCircuit {
                        task,
                        physical_qubits,
                    } => write!(
                        formatter,
                        "invalid generated circuit size value {physical_qubits} physical qubits for {} need at least {required} bytes, exceeding the {budget}-byte memory budget; raise the memory budget to admit it",
                        crate::circuit_generation::budget_family_name(task),
                    ),
                };
            }
            Refusal::Fixed {
                cause,
                actual,
                limit,
            } => (cause, actual, limit),
        };
        match cause {
            ResourceLimitCause::FixedOperation {
                operation,
                resource,
            } => write!(
                formatter,
                "{} {} value {} exceeds current limit {}",
                operation.as_str(),
                resource.as_str(),
                actual,
                limit
            ),
            ResourceLimitCause::CircuitToDetectorErrorModel { resource } => write!(
                formatter,
                "circuit-to-detector-error-model {} value {} exceeds current limit {}",
                resource.as_str(),
                actual,
                limit
            ),
            ResourceLimitCause::CircuitPass { stage, resource } => write!(
                formatter,
                "circuit pass {} {} value {} exceeds current limit {}",
                stage.as_str(),
                resource.as_str(),
                actual,
                limit
            ),
            ResourceLimitCause::DetectingRegions { resource } => write!(
                formatter,
                "detecting-regions {} value {} exceeds current limit {}",
                resource.as_str(),
                actual,
                limit
            ),
            ResourceLimitCause::CircuitFlattenRepeatNesting => write!(
                formatter,
                "invalid flattened circuit repeat nesting value {} exceeds fixed safety limit {}",
                actual, limit
            ),
            ResourceLimitCause::CircuitFlattenMaterializedUnits => write!(
                formatter,
                "invalid flattened circuit instruction vector would require {} materialized units; platform limit is {}",
                actual, limit
            ),
            ResourceLimitCause::DemFlattenMaterializedUnits => write!(
                formatter,
                "invalid detector error model: DEM flattened instruction vector would require {} materialized units; platform limit is {}",
                actual, limit
            ),
            ResourceLimitCause::SatMaterialization { resource } => match resource {
                SatMaterializationResource::ExpandedInstructions => write!(
                    formatter,
                    "invalid detector error model: DEM SAT problem generation currently supports at most {} expanded instructions, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::ErrorMechanisms => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} error mechanisms, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::TargetOccurrences => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} target occurrences, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::Variables => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} variables, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::Clauses => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} clauses, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::ClauseLiterals => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} clause literals, got at least {}",
                    limit, actual
                ),
                SatMaterializationResource::OutputBytes => write!(
                    formatter,
                    "invalid detector error model: SAT problem generation currently supports at most {} WDIMACS output bytes, got at least {}",
                    limit, actual
                ),
            },
            ResourceLimitCause::SatTraversalRepeatIterations { context } => write!(
                formatter,
                "invalid detector error model: DEM {context} traversal currently supports at most {} expanded repeat iterations, got at least {}",
                limit, actual
            ),
            ResourceLimitCause::LogicalErrorSearch { context, resource } => match resource {
                LogicalErrorSearchResource::RepeatCount => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports repeat counts up to {}, got {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::ExpandedErrorMechanisms => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} expanded nonzero error mechanisms, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::ErrorTargetOccurrencesPerMechanism => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} target occurrences per nonzero error mechanism, got {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::TotalErrorTargetOccurrences => write!(
                    formatter,
                    "invalid detector error model: DEM {context} currently supports at most {} total target occurrences across expanded nonzero error mechanisms, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::EffectiveDetectorNodes => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} effective detector nodes, got {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::UniqueGraphEdges => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} unique graph edges, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::StoredGraphTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} stored graph payload terms, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::HyperedgeDegree => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports edges with at most {} detectors, got {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::HyperedgeIncidences => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} edge incidences, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::SearchStates => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} search states, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::SearchTransitions => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} search transitions, got at least {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::SearchStateTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} detector and observable terms per search state, got {}",
                    limit, actual
                ),
                LogicalErrorSearchResource::StoredSearchStateTerms => write!(
                    formatter,
                    "invalid detector error model: {context} currently supports at most {} stored detector and observable search-state terms, got at least {}",
                    limit, actual
                ),
            },
            ResourceLimitCause::LogicalErrorTraversalRepeatIterations { context } => write!(
                formatter,
                "invalid detector error model: DEM {context} traversal currently supports at most {} expanded repeat iterations, got at least {}",
                limit, actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery {
                resource: ResourceKind::ExpandedOperations,
            } => write!(
                formatter,
                "missing-detector analysis currently supports at most {} expanded instructions, got at least {}",
                limit, actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery {
                resource: ResourceKind::RepeatNesting,
            } => write!(
                formatter,
                "missing-detector repeat nesting exceeds current limit {}, got {}",
                limit, actual
            ),
            ResourceLimitCause::MissingDetectorDiscovery { resource } => write!(
                formatter,
                "missing-detector discovery {} value {} exceeds current limit {}",
                resource.as_str(),
                actual,
                limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
