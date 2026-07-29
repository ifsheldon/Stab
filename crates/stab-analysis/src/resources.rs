use std::fmt::{Display, Formatter};

/// Analysis operation whose configurable resource budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceOperation {
    CircuitFlatten,
    DetectorErrorModelFlatten,
}

impl ResourceOperation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CircuitFlatten => "circuit-flatten",
            Self::DetectorErrorModelFlatten => "detector-error-model-flatten",
        }
    }
}

/// Analysis resource dimension whose configurable budget was exceeded.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceKind {
    RepeatNesting,
    ExpandedOperations,
    RepeatCount,
    RepeatIterations,
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
            Self::RepeatCount => "repeat-count",
            Self::RepeatIterations => "repeat-iterations",
            Self::MaterializedUnits => "materialized-units",
            Self::MaterializedBytes => "materialized-bytes",
            Self::TargetOccurrences => "target-occurrences",
            Self::ArgumentValues => "argument-values",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum ResourceLimitCause {
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

    pub const fn code(self) -> &'static str {
        "resource-limit-exceeded"
    }

    pub const fn operation(self) -> ResourceOperation {
        match self.cause {
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
        }
    }

    pub const fn resource(self) -> ResourceKind {
        match self.cause {
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
        }
    }
}

impl std::error::Error for ResourceLimitError {}
