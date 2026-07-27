use std::fmt::{Display, Formatter};

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
        Self {
            input_bytes: Estimate::Exact(input.len()),
            input_items: Estimate::Exact(input.lines().count()),
            ..Self::default()
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
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitParse => "circuit-parse",
            Self::DetectorErrorModelParse => "detector-error-model-parse",
        }
    }
}

/// Resource dimension whose configurable budget was exceeded.
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ResourceLimitCause {
    CircuitSourceLines,
    CircuitRepeatNesting { source_line: usize },
    DetectorErrorModelSourceLines,
    DetectorErrorModelRepeatNesting,
}

/// Typed resource-admission failure with compatibility-preserving human output.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    cause: ResourceLimitCause,
    actual: usize,
    limit: usize,
}

impl ResourceLimitError {
    pub(crate) const fn circuit_source_lines(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitSourceLines,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_repeat_nesting(
        source_line: usize,
        actual: usize,
        limit: usize,
    ) -> Self {
        Self {
            cause: ResourceLimitCause::CircuitRepeatNesting { source_line },
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_source_lines(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelSourceLines,
            actual,
            limit,
        }
    }

    pub(crate) const fn dem_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::DetectorErrorModelRepeatNesting,
            actual,
            limit,
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
        }
    }

    pub const fn resource(&self) -> ResourceKind {
        match self.cause {
            ResourceLimitCause::CircuitSourceLines
            | ResourceLimitCause::DetectorErrorModelSourceLines => ResourceKind::SourceLines,
            ResourceLimitCause::CircuitRepeatNesting { .. }
            | ResourceLimitCause::DetectorErrorModelRepeatNesting => ResourceKind::RepeatNesting,
        }
    }

    pub const fn actual(&self) -> usize {
        self.actual
    }

    pub const fn limit(&self) -> usize {
        self.limit
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
        }
    }
}

impl std::error::Error for ResourceLimitError {}
