use std::fmt::{Display, Formatter};

use crate::{ByteSpan, DiagnosticSeverity};

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
    SourceLines,
    RepeatNesting,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SourceLines => "source-lines",
            Self::RepeatNesting => "repeat-nesting",
        }
    }
}

/// Typed context needed to preserve model-specific parse diagnostics.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceLimitContext {
    CircuitSourceLines,
    CircuitRepeatNesting { source_line: usize },
    DetectorErrorModelSourceLines,
    DetectorErrorModelRepeatNesting,
}

/// Typed model-parse resource admission failure.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    context: ResourceLimitContext,
    actual: u64,
    limit: u64,
    span: ByteSpan,
}

impl ResourceLimitError {
    pub(crate) const fn circuit_source_lines(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            context: ResourceLimitContext::CircuitSourceLines,
            actual: actual as u64,
            limit: limit as u64,
            span,
        }
    }

    pub(crate) const fn circuit_repeat_nesting(
        source_line: usize,
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> Self {
        Self {
            context: ResourceLimitContext::CircuitRepeatNesting { source_line },
            actual: actual as u64,
            limit: limit as u64,
            span,
        }
    }

    pub(crate) const fn dem_source_lines(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            context: ResourceLimitContext::DetectorErrorModelSourceLines,
            actual: actual as u64,
            limit: limit as u64,
            span,
        }
    }

    pub(crate) const fn dem_repeat_nesting(actual: usize, limit: usize, span: ByteSpan) -> Self {
        Self {
            context: ResourceLimitContext::DetectorErrorModelRepeatNesting,
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

    pub const fn operation(&self) -> ResourceOperation {
        match self.context {
            ResourceLimitContext::CircuitSourceLines
            | ResourceLimitContext::CircuitRepeatNesting { .. } => ResourceOperation::CircuitParse,
            ResourceLimitContext::DetectorErrorModelSourceLines
            | ResourceLimitContext::DetectorErrorModelRepeatNesting => {
                ResourceOperation::DetectorErrorModelParse
            }
        }
    }

    pub const fn resource(&self) -> ResourceKind {
        match self.context {
            ResourceLimitContext::CircuitSourceLines
            | ResourceLimitContext::DetectorErrorModelSourceLines => ResourceKind::SourceLines,
            ResourceLimitContext::CircuitRepeatNesting { .. }
            | ResourceLimitContext::DetectorErrorModelRepeatNesting => ResourceKind::RepeatNesting,
        }
    }

    pub const fn context(&self) -> ResourceLimitContext {
        self.context
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
        match self.context {
            ResourceLimitContext::CircuitSourceLines => write!(
                formatter,
                "failed to parse line {}: circuit input has more than {} lines",
                self.actual, self.limit
            ),
            ResourceLimitContext::CircuitRepeatNesting { source_line } => write!(
                formatter,
                "failed to parse line {source_line}: repeat nesting exceeds current limit {}",
                self.limit
            ),
            ResourceLimitContext::DetectorErrorModelSourceLines => write!(
                formatter,
                "invalid detector error model: DEM input has more than {} lines",
                self.limit
            ),
            ResourceLimitContext::DetectorErrorModelRepeatNesting => write!(
                formatter,
                "invalid detector error model: DEM repeat nesting exceeds current limit {}",
                self.limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
