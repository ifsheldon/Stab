use std::fmt::{Display, Formatter};

use crate::{ByteSpan, DiagnosticSeverity, ModelDialect};

/// Model operation whose caller-selected parse budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitParse,
    DetectorErrorModelParse,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitParse => "circuit-parse",
            Self::DetectorErrorModelParse => "detector-error-model-parse",
        }
    }
}

/// Parse resource dimension whose caller-selected budget was exceeded.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    SourceBytes,
    SourceLines,
    RepresentedInstructions,
    RepresentedTargets,
    RepeatNesting,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceBytes => "source-bytes",
            Self::SourceLines => "source-lines",
            Self::RepresentedInstructions => "represented-instructions",
            Self::RepresentedTargets => "represented-targets",
            Self::RepeatNesting => "repeat-nesting",
        }
    }
}

/// Typed model-parse resource admission failure.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    dialect: ModelDialect,
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
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> Self {
        Self {
            dialect,
            resource,
            source_line,
            actual: actual as u64,
            limit: limit as u64,
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
        match self.dialect {
            ModelDialect::StimCircuit => ResourceOperation::CircuitParse,
            ModelDialect::DetectorErrorModel => ResourceOperation::DetectorErrorModelParse,
        }
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
        match (self.dialect, self.resource) {
            (ModelDialect::StimCircuit, ResourceKind::SourceBytes) => write!(
                formatter,
                "circuit input has {} bytes, exceeding current limit {}",
                self.actual, self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::SourceLines) => write!(
                formatter,
                "failed to parse line {}: circuit input has more than {} lines",
                self.source_line.unwrap_or_default(),
                self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::RepresentedInstructions) => write!(
                formatter,
                "failed to parse line {}: represented instruction count exceeds current limit {}",
                self.source_line.unwrap_or(0),
                self.limit
            ),
            (ModelDialect::StimCircuit, ResourceKind::RepresentedTargets) => write!(
                formatter,
                "failed to parse line {}: represented target count exceeds current limit {}",
                self.source_line.unwrap_or(0),
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
                "invalid detector error model: DEM input has {} bytes, exceeding current limit {}",
                self.actual, self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::SourceLines) => write!(
                formatter,
                "invalid detector error model: DEM input has more than {} lines",
                self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::RepresentedInstructions) => write!(
                formatter,
                "invalid detector error model: represented instruction count exceeds current limit {}",
                self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::RepresentedTargets) => write!(
                formatter,
                "invalid detector error model: represented target count exceeds current limit {}",
                self.limit
            ),
            (ModelDialect::DetectorErrorModel, ResourceKind::RepeatNesting) => write!(
                formatter,
                "invalid detector error model: DEM repeat nesting exceeds current limit {}",
                self.limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
