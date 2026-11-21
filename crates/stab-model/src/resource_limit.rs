use std::fmt::{Display, Formatter};

use crate::{
    ByteSpan, DemDetectorId, DiagnosticSeverity, MemoryBudget, MemoryBudgetExceeded, ModelDialect,
    ModelResult,
};

/// Model operation whose memory budget or fixed limit was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitParse,
    DetectorErrorModelParse,
    /// [`crate::Circuit::detector_coordinates_with_budget`] materializing every detector.
    CircuitDetectorCoordinates,
    /// [`crate::DetectorErrorModel::detector_coordinates_with_budget`] materializing every
    /// detector.
    DetectorErrorModelDetectorCoordinates,
    /// [`crate::DetectorErrorModel::detector_coordinates_for`] looking up selected detectors.
    DetectorErrorModelSelectedCoordinates,
    /// A folded traversal of an already parsed detector error model on behalf of a consumer.
    DetectorErrorModelTraversal,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitParse => "circuit-parse",
            Self::DetectorErrorModelParse => "detector-error-model-parse",
            Self::CircuitDetectorCoordinates => "circuit-detector-coordinates",
            Self::DetectorErrorModelDetectorCoordinates => {
                "detector-error-model-detector-coordinates"
            }
            Self::DetectorErrorModelSelectedCoordinates => {
                "detector-error-model-selected-coordinates"
            }
            Self::DetectorErrorModelTraversal => "detector-error-model-traversal",
        }
    }
}

/// Model resource dimension whose memory budget or fixed limit was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    SourceBytes,
    RetainedBytes,
    RepeatNesting,
    /// Bytes a query would allocate for its result from an already parsed model.
    MaterializedBytes,
    /// Repeat iterations a traversal would expand in total.
    RepeatIterations,
    /// Overlapping repeat iterations a selected-coordinate query retains as candidates.
    RepeatCandidates,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceBytes => "source-bytes",
            Self::RetainedBytes => "retained-bytes",
            Self::RepeatNesting => "repeat-nesting",
            Self::MaterializedBytes => "materialized-bytes",
            Self::RepeatIterations => "repeat-iterations",
            Self::RepeatCandidates => "repeat-candidates",
        }
    }
}

/// Conservative retained bytes of one all-detector coordinate map entry, excluding its coordinate
/// values: a selected-detector set node plus a result-map node holding a vector header.
const COORDINATE_MAP_ENTRY_BYTES: u128 = 96;

/// Admits a map holding the coordinates of `detector_count` detectors, none with more than
/// `max_coordinates` values, against `budget` before anything is allocated.
pub(crate) fn admit_detector_coordinate_map(
    dialect: ModelDialect,
    detector_count: u64,
    max_coordinates: usize,
    budget: MemoryBudget,
) -> ModelResult<()> {
    let entry_bytes = COORDINATE_MAP_ENTRY_BYTES
        .saturating_add((max_coordinates as u128).saturating_mul(size_of::<f64>() as u128));
    let required = u128::from(detector_count).saturating_mul(entry_bytes);
    budget.admit(required).map_err(|exceeded| {
        let operation = match dialect {
            ModelDialect::StimCircuit => ResourceOperation::CircuitDetectorCoordinates,
            ModelDialect::DetectorErrorModel => {
                ResourceOperation::DetectorErrorModelDetectorCoordinates
            }
        };
        ResourceLimitError {
            dialect,
            location: None,
            refusal: Refusal::MemoryBudget {
                operation,
                resource: ResourceKind::MaterializedBytes,
                exceeded,
            },
        }
        .into()
    })
}

/// Where in the parsed input a parse refusal happened.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ParseLocation {
    source_line: Option<usize>,
    span: ByteSpan,
}

/// A fixed limit of the model layer, which no memory budget raises.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum FixedLimit {
    /// The parser's configured repeat nesting.
    ParseRepeatNesting,
    /// The repeat depth a traversal consumer accepts.
    TraversalRepeatNesting { context: &'static str },
    /// The expanded repeat iterations a traversal consumer accepts.
    TraversalRepeatExpansion { context: &'static str },
    /// The overlapping repeat candidates a selected-coordinate query retains.
    SelectedCoordinateCandidates { detector: DemDetectorId },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Refusal {
    MemoryBudget {
        operation: ResourceOperation,
        resource: ResourceKind,
        exceeded: MemoryBudgetExceeded,
    },
    Fixed {
        limit_kind: FixedLimit,
        actual: u64,
        limit: u64,
    },
}

/// Typed model resource admission failure: a memory budget or a fixed limit refused the work.
///
/// A parse failure locates the refused input with [`Self::source_line`] and [`Self::span`]; a
/// query or traversal over an already parsed model has neither. Only a memory-budget refusal
/// answers [`Self::memory_budget_exceeded`]; a fixed limit is not raised by a larger budget.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    dialect: ModelDialect,
    location: Option<ParseLocation>,
    refusal: Refusal,
}

impl ResourceLimitError {
    const fn parse_operation(dialect: ModelDialect) -> ResourceOperation {
        match dialect {
            ModelDialect::StimCircuit => ResourceOperation::CircuitParse,
            ModelDialect::DetectorErrorModel => ResourceOperation::DetectorErrorModelParse,
        }
    }

    /// The parse memory budget refused `resource` at `span`.
    pub(crate) const fn parse_memory_budget(
        dialect: ModelDialect,
        resource: ResourceKind,
        source_line: Option<usize>,
        exceeded: MemoryBudgetExceeded,
        span: ByteSpan,
    ) -> Self {
        Self {
            dialect,
            location: Some(ParseLocation { source_line, span }),
            refusal: Refusal::MemoryBudget {
                operation: Self::parse_operation(dialect),
                resource,
                exceeded,
            },
        }
    }

    /// The parser's repeat nesting limit refused a block at `span`.
    pub(crate) const fn parse_repeat_nesting(
        dialect: ModelDialect,
        source_line: usize,
        actual: u64,
        limit: u64,
        span: ByteSpan,
    ) -> Self {
        Self {
            dialect,
            location: Some(ParseLocation {
                source_line: Some(source_line),
                span,
            }),
            refusal: Refusal::Fixed {
                limit_kind: FixedLimit::ParseRepeatNesting,
                actual,
                limit,
            },
        }
    }

    const fn dem_fixed(limit_kind: FixedLimit, actual: u64, limit: u64) -> Self {
        Self {
            dialect: ModelDialect::DetectorErrorModel,
            location: None,
            refusal: Refusal::Fixed {
                limit_kind,
                actual,
                limit,
            },
        }
    }

    /// A traversal consumer's fixed repeat depth refused a model nested `actual` levels deep.
    pub(crate) const fn dem_traversal_repeat_nesting(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::dem_fixed(
            FixedLimit::TraversalRepeatNesting { context },
            actual,
            limit,
        )
    }

    /// A traversal consumer's fixed repeat-expansion limit refused at least `actual` iterations.
    pub(crate) const fn dem_traversal_repeat_expansion(
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self {
        Self::dem_fixed(
            FixedLimit::TraversalRepeatExpansion { context },
            actual,
            limit,
        )
    }

    /// The selected-coordinate query retained its `limit` repeat candidates before it found
    /// `detector`.
    pub(crate) const fn dem_selected_coordinate_candidates(
        detector: DemDetectorId,
        limit: u64,
    ) -> Self {
        Self::dem_fixed(
            FixedLimit::SelectedCoordinateCandidates { detector },
            limit,
            limit,
        )
    }

    pub const fn code(&self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        DiagnosticSeverity::Error
    }

    pub const fn dialect(&self) -> ModelDialect {
        self.dialect
    }

    pub const fn operation(&self) -> ResourceOperation {
        match self.refusal {
            Refusal::MemoryBudget { operation, .. } => operation,
            Refusal::Fixed { limit_kind, .. } => match limit_kind {
                FixedLimit::ParseRepeatNesting => Self::parse_operation(self.dialect),
                FixedLimit::TraversalRepeatNesting { .. }
                | FixedLimit::TraversalRepeatExpansion { .. } => {
                    ResourceOperation::DetectorErrorModelTraversal
                }
                FixedLimit::SelectedCoordinateCandidates { .. } => {
                    ResourceOperation::DetectorErrorModelSelectedCoordinates
                }
            },
        }
    }

    pub const fn resource(&self) -> ResourceKind {
        match self.refusal {
            Refusal::MemoryBudget { resource, .. } => resource,
            Refusal::Fixed { limit_kind, .. } => match limit_kind {
                FixedLimit::ParseRepeatNesting | FixedLimit::TraversalRepeatNesting { .. } => {
                    ResourceKind::RepeatNesting
                }
                FixedLimit::TraversalRepeatExpansion { .. } => ResourceKind::RepeatIterations,
                FixedLimit::SelectedCoordinateCandidates { .. } => ResourceKind::RepeatCandidates,
            },
        }
    }

    /// The memory-budget refusal, when a caller-selected budget rather than a fixed limit
    /// refused the work.
    pub const fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match &self.refusal {
            Refusal::MemoryBudget { exceeded, .. } => Some(exceeded),
            Refusal::Fixed { .. } => None,
        }
    }

    /// The source line of a parse refusal that names one.
    pub const fn source_line(&self) -> Option<usize> {
        match self.location {
            Some(location) => location.source_line,
            None => None,
        }
    }

    /// The counted amount, or the required bytes of a memory-budget refusal (its lower bound
    /// when the requirement does not fit `u64`).
    pub const fn actual(&self) -> u64 {
        match self.refusal {
            Refusal::MemoryBudget { exceeded, .. } => exceeded.required().value(),
            Refusal::Fixed { actual, .. } => actual,
        }
    }

    /// The fixed limit, or the bytes of the refusing memory budget.
    pub const fn limit(&self) -> u64 {
        match self.refusal {
            Refusal::MemoryBudget { exceeded, .. } => exceeded.budget().bytes(),
            Refusal::Fixed { limit, .. } => limit,
        }
    }

    /// The refused input region of a parse refusal; `None` for a query or traversal over an
    /// already parsed model.
    pub const fn span(&self) -> Option<ByteSpan> {
        match self.location {
            Some(location) => Some(location.span),
            None => None,
        }
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let line = self.source_line().unwrap_or(0);
        let (actual, limit) = (self.actual(), self.limit());
        let raise = "raise the memory budget to admit it";
        let coordinates =
            "raise the memory budget or use detector_coordinates_for for selected detectors";
        if let Refusal::Fixed { limit_kind, .. } = self.refusal {
            return match (self.dialect, limit_kind) {
                (ModelDialect::StimCircuit, FixedLimit::ParseRepeatNesting) => write!(
                    formatter,
                    "failed to parse line {line}: repeat nesting exceeds current limit {limit}"
                ),
                (ModelDialect::DetectorErrorModel, FixedLimit::ParseRepeatNesting) => write!(
                    formatter,
                    "invalid detector error model: DEM repeat nesting exceeds current limit {limit}"
                ),
                (_, FixedLimit::TraversalRepeatNesting { context }) => write!(
                    formatter,
                    "invalid detector error model: DEM {context} repeat nesting exceeds current limit {limit}, got {actual}"
                ),
                (_, FixedLimit::TraversalRepeatExpansion { context }) => write!(
                    formatter,
                    "invalid detector error model: DEM {context} traversal currently supports at most {limit} expanded repeat iterations, got at least {actual}"
                ),
                (_, FixedLimit::SelectedCoordinateCandidates { detector }) => write!(
                    formatter,
                    "invalid detector error model: DEM detector_coordinates_for retains at most {limit} overlapping repeat candidates within the default memory budget before finding detector {}",
                    detector.get()
                ),
            };
        }
        match (self.dialect, self.resource()) {
            (ModelDialect::StimCircuit, ResourceKind::SourceBytes) => write!(
                formatter,
                "circuit input has {actual} bytes, exceeding the {limit}-byte memory budget; {raise}"
            ),
            (ModelDialect::StimCircuit, ResourceKind::MaterializedBytes) => write!(
                formatter,
                "circuit detector_coordinates needs at least {actual} bytes, exceeding the {limit}-byte memory budget; {coordinates}"
            ),
            (ModelDialect::StimCircuit, _) => write!(
                formatter,
                "failed to parse line {line}: circuit input and its parsed model need at least {actual} bytes, exceeding the {limit}-byte memory budget; {raise}"
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::SourceBytes) => write!(
                formatter,
                "invalid detector error model: DEM input has {actual} bytes, exceeding the {limit}-byte memory budget; {raise}"
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::MaterializedBytes) => write!(
                formatter,
                "DEM detector_coordinates needs at least {actual} bytes, exceeding the {limit}-byte memory budget; {coordinates}"
            ),
            (ModelDialect::DetectorErrorModel, _) => write!(
                formatter,
                "invalid detector error model: DEM input and its parsed model need at least {actual} bytes, exceeding the {limit}-byte memory budget; {raise}"
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
