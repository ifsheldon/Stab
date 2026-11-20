use std::fmt::{Display, Formatter};

use crate::{ByteSpan, DiagnosticSeverity, MemoryBudget, ModelDialect, ModelResult};

/// Model operation whose caller-selected budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitParse,
    DetectorErrorModelParse,
    /// [`crate::Circuit::detector_coordinates_with_memory_budget`] materializing every detector.
    CircuitDetectorCoordinates,
    /// [`crate::DetectorErrorModel::detector_coordinates_with_memory_budget`] materializing every
    /// detector.
    DetectorErrorModelDetectorCoordinates,
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
        }
    }
}

/// Model resource dimension whose caller-selected budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    SourceBytes,
    RetainedBytes,
    RepeatNesting,
    /// Bytes a query would allocate for its result from an already parsed model.
    MaterializedBytes,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceBytes => "source-bytes",
            Self::RetainedBytes => "retained-bytes",
            Self::RepeatNesting => "repeat-nesting",
            Self::MaterializedBytes => "materialized-bytes",
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
    if budget.admits(required) {
        return Ok(());
    }
    let operation = match dialect {
        ModelDialect::StimCircuit => ResourceOperation::CircuitDetectorCoordinates,
        ModelDialect::DetectorErrorModel => {
            ResourceOperation::DetectorErrorModelDetectorCoordinates
        }
    };
    Err(ResourceLimitError {
        dialect,
        operation,
        resource: ResourceKind::MaterializedBytes,
        source_line: None,
        // The report saturates; the refusal above compared the exact requirement.
        actual: u64::try_from(required).unwrap_or(u64::MAX),
        limit: budget.bytes(),
        span: ByteSpan::from_valid_range(0, 0),
    }
    .into())
}

/// Typed model resource admission failure.
///
/// A parse failure locates the refused input with [`Self::source_line`] and [`Self::span`]; a
/// query over an already parsed model has no source line and an empty span at byte zero.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    dialect: ModelDialect,
    operation: ResourceOperation,
    resource: ResourceKind,
    source_line: Option<usize>,
    actual: u64,
    limit: u64,
    span: ByteSpan,
}

impl ResourceLimitError {
    pub(crate) const fn new(
        dialect: ModelDialect,
        resource: ResourceKind,
        source_line: Option<usize>,
        actual: u64,
        limit: u64,
        span: ByteSpan,
    ) -> Self {
        Self {
            dialect,
            operation: match dialect {
                ModelDialect::StimCircuit => ResourceOperation::CircuitParse,
                ModelDialect::DetectorErrorModel => ResourceOperation::DetectorErrorModelParse,
            },
            resource,
            source_line,
            actual,
            limit,
            span,
        }
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
        self.operation
    }

    pub const fn resource(&self) -> ResourceKind {
        self.resource
    }

    pub const fn source_line(&self) -> Option<usize> {
        self.source_line
    }

    pub const fn actual(&self) -> u64 {
        self.actual
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }

    pub const fn span(&self) -> ByteSpan {
        self.span
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let raise = "raise the memory budget to admit it";
        let coordinates =
            "raise the memory budget or use detector_coordinates_for for selected detectors";
        match (self.dialect, self.resource) {
            (ModelDialect::StimCircuit, ResourceKind::SourceBytes) => write!(
                formatter,
                "circuit input has {} bytes, exceeding the {}-byte memory budget; {raise}",
                self.actual, self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::RetainedBytes) => write!(
                formatter,
                "failed to parse line {}: circuit input and its parsed model need at least {} bytes, exceeding the {}-byte memory budget; {raise}",
                self.source_line.unwrap_or(0),
                self.actual,
                self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::RepeatNesting) => write!(
                formatter,
                "failed to parse line {}: repeat nesting exceeds current limit {}",
                self.source_line.unwrap_or(0),
                self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::SourceBytes) => write!(
                formatter,
                "invalid detector error model: DEM input has {} bytes, exceeding the {}-byte memory budget; {raise}",
                self.actual, self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::RetainedBytes) => write!(
                formatter,
                "invalid detector error model: DEM input and its parsed model need at least {} bytes, exceeding the {}-byte memory budget; {raise}",
                self.actual, self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::RepeatNesting) => write!(
                formatter,
                "invalid detector error model: DEM repeat nesting exceeds current limit {}",
                self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::MaterializedBytes) => write!(
                formatter,
                "circuit detector_coordinates needs at least {} bytes, exceeding the {}-byte memory budget; {coordinates}",
                self.actual, self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::MaterializedBytes) => write!(
                formatter,
                "DEM detector_coordinates needs at least {} bytes, exceeding the {}-byte memory budget; {coordinates}",
                self.actual, self.limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
