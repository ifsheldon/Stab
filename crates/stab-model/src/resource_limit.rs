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
    RetainedBytes,
    RepeatNesting,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceBytes => "source-bytes",
            Self::RetainedBytes => "retained-bytes",
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
        actual: u64,
        limit: u64,
        span: ByteSpan,
    ) -> Self {
        Self {
            dialect,
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
        let raise = "raise the memory budget to admit it";
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
        }
    }
}

impl std::error::Error for ResourceLimitError {}
