use std::fmt::{Display, Formatter};

/// Analysis operation whose configurable resource budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitFlatten,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitFlatten => "circuit-flatten",
        }
    }
}

/// Analysis resource dimension whose configurable budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    RepeatNesting,
    ExpandedOperations,
    MaterializedUnits,
    MaterializedBytes,
    TargetOccurrences,
    ArgumentValues,
}

impl ResourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepeatNesting => "repeat-nesting",
            Self::ExpandedOperations => "expanded-operations",
            Self::MaterializedUnits => "materialized-units",
            Self::MaterializedBytes => "materialized-bytes",
            Self::TargetOccurrences => "target-occurrences",
            Self::ArgumentValues => "argument-values",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ResourceLimitCause {
    RepeatNesting,
    ExpandedOperations,
    TargetOccurrences,
    ArgumentValues,
    MaterializedBytes,
    MaterializedUnits,
}

/// Typed resource-admission failure produced by pure analysis operations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceLimitError {
    cause: ResourceLimitCause,
    actual: u64,
    limit: u64,
}

impl ResourceLimitError {
    pub(crate) const fn circuit_flatten_expanded_operations(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::ExpandedOperations,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_repeat_nesting(actual: usize, limit: usize) -> Self {
        Self {
            cause: ResourceLimitCause::RepeatNesting,
            actual: actual as u64,
            limit: limit as u64,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_units(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::MaterializedUnits,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_target_occurrences(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::TargetOccurrences,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_argument_values(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::ArgumentValues,
            actual,
            limit,
        }
    }

    pub(crate) const fn circuit_flatten_materialized_bytes(actual: u64, limit: u64) -> Self {
        Self {
            cause: ResourceLimitCause::MaterializedBytes,
            actual,
            limit,
        }
    }

    pub const fn code(self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn operation(self) -> ResourceOperation {
        ResourceOperation::CircuitFlatten
    }

    pub const fn resource(self) -> ResourceKind {
        match self.cause {
            ResourceLimitCause::RepeatNesting => ResourceKind::RepeatNesting,
            ResourceLimitCause::ExpandedOperations => ResourceKind::ExpandedOperations,
            ResourceLimitCause::TargetOccurrences => ResourceKind::TargetOccurrences,
            ResourceLimitCause::ArgumentValues => ResourceKind::ArgumentValues,
            ResourceLimitCause::MaterializedBytes => ResourceKind::MaterializedBytes,
            ResourceLimitCause::MaterializedUnits => ResourceKind::MaterializedUnits,
        }
    }

    pub const fn actual(self) -> u64 {
        self.actual
    }

    pub const fn limit(self) -> u64 {
        self.limit
    }
}

impl Display for ResourceLimitError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self.cause {
            ResourceLimitCause::RepeatNesting => write!(
                formatter,
                "invalid flattened circuit repeat nesting value {} exceeds fixed safety limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::ExpandedOperations => write!(
                formatter,
                "invalid flattened circuit operation count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::TargetOccurrences => write!(
                formatter,
                "invalid flattened circuit target count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::ArgumentValues => write!(
                formatter,
                "invalid flattened circuit argument count value {} exceeds current materialized limit {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::MaterializedBytes => write!(
                formatter,
                "invalid flattened circuit would require at least {} materialized bytes; current limit is {}",
                self.actual, self.limit
            ),
            ResourceLimitCause::MaterializedUnits => write!(
                formatter,
                "invalid flattened circuit instruction vector would require {} materialized units; platform limit is {}",
                self.actual, self.limit
            ),
        }
    }
}

impl std::error::Error for ResourceLimitError {}
