use std::borrow::Cow;
use std::fmt::{Display, Formatter};

use stab_algebra::PauliPhase;
use stab_model::{DemTarget, GateKind, MemoryBudgetExceeded, ModelError};

use crate::ResourceLimitError;

mod details;
mod operation;

pub use details::{
    AnalysisRequestError, GeneratorParameter, NonDeterministicTargets, NonUnitaryReason,
    PauliCollapse, ThresholdExcess, UnanalyzableNoise,
};
pub use operation::AnalysisOperation;

/// Result type for pure Stab model analysis.
pub type AnalysisResult<T> = Result<T, AnalysisError>;

/// A typed failure while transforming or semantically analyzing a Stab model.
///
/// The variants say what happened, so a caller can tell a circuit the analysis cannot accept
/// apart from a request it cannot serve, a size it cannot represent, or a bug. `Display` renders
/// each failure with the wording Stab has always used, which follows Stim where Stim reports the
/// same failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisError {
    /// The model layer refused the input or the traversal.
    Model(ModelError),

    /// A caller-selected budget or a fixed safety limit refused the work before allocation.
    ResourceLimit(ResourceLimitError),

    /// The allocator refused storage the operation had already admitted.
    AllocationFailed {
        operation: AnalysisOperation,
        detail: Cow<'static, str>,
    },

    /// The operation's cancellation signal was observed.
    Cancelled(stab_model::Cancelled),

    /// A collapse in the circuit randomizes these detectors or observables.
    NonDeterministicTargets(NonDeterministicTargets),

    /// An operation that needs a unitary circuit met a non-unitary instruction.
    NonUnitaryOperation {
        operation: AnalysisOperation,
        gate: GateKind,
        reason: NonUnitaryReason,
    },

    /// A Pauli observable stops being well defined at this instruction.
    PauliNotPreserved {
        gate: GateKind,
        collapse: PauliCollapse,
    },

    /// A Pauli-product instruction whose product is anti-Hermitian.
    AntiHermitianPauliProduct {
        operation: AnalysisOperation,
        gate: GateKind,
        /// The product's imaginary phase, when the operation computed it.
        phase: Option<PauliPhase>,
    },

    /// The operation does not support this gate, target shape, or model structure.
    UnsupportedInput {
        operation: AnalysisOperation,
        gate: Option<GateKind>,
        detail: Cow<'static, str>,
    },

    /// Error analysis needs `approximate_disjoint_errors`, or a larger threshold, for this
    /// noise channel.
    NoiseRequiresApproximation {
        gate: GateKind,
        /// The probability above the configured threshold, when a threshold was configured.
        excess: Option<ThresholdExcess>,
    },

    /// Noise error analysis cannot express as independent error mechanisms.
    UnanalyzableNoise(UnanalyzableNoise),

    /// Error decomposition could not split this error into graphlike components.
    ErrorDecompositionFailed {
        /// The failing component's targets, without separators.
        component: Vec<DemTarget>,
        remnant_edges_blocked: bool,
    },

    /// A graphlike search met an error mechanism with more than two detection events.
    NonGraphlikeError,

    /// The caller's request does not fit the analyzed input.
    InvalidRequest(AnalysisRequestError),

    /// A code-generation parameter outside the generator's supported range.
    InvalidGeneratorParameter {
        parameter: GeneratorParameter,
        value: u64,
    },

    /// A count the input implies does not fit the integer type the operation computes it in.
    CountOverflow {
        operation: AnalysisOperation,
        detail: Cow<'static, str>,
    },

    /// Accumulated coordinate shifts left the finite `f64` range.
    NonFiniteCoordinate { operation: AnalysisOperation },

    /// A measurement-record target looks back before the first measurement.
    MeasurementRecordOutOfRange {
        operation: AnalysisOperation,
        detail: Cow<'static, str>,
    },

    /// An internal invariant failed; this is a bug in Stab.
    InternalInvariant {
        operation: AnalysisOperation,
        detail: Cow<'static, str>,
    },
}

impl Display for AnalysisError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(error) => error.fmt(formatter),
            Self::ResourceLimit(error) => error.fmt(formatter),
            Self::Cancelled(cancelled) => cancelled.fmt(formatter),
            Self::AllocationFailed { operation, detail }
            | Self::UnsupportedInput {
                operation, detail, ..
            }
            | Self::CountOverflow { operation, detail }
            | Self::MeasurementRecordOutOfRange { operation, detail }
            | Self::InternalInvariant { operation, detail } => {
                write!(formatter, "{}{detail}", operation.message_prefix())
            }
            Self::NonDeterministicTargets(targets) => targets.fmt(formatter),
            Self::NonUnitaryOperation {
                operation,
                gate,
                reason,
            } => {
                let gate = gate.canonical_name();
                formatter.write_str(operation.message_prefix())?;
                match reason {
                    NonUnitaryReason::Measurement => {
                        write!(formatter, "measurement operation {gate}")
                    }
                    NonUnitaryReason::Reset => write!(formatter, "reset operation {gate}"),
                    NonUnitaryReason::Noise => write!(formatter, "noisy operation {gate}"),
                    NonUnitaryReason::NoStrictInverse => {
                        write!(formatter, "operation {gate} has no strict circuit inverse")
                    }
                }
            }
            Self::PauliNotPreserved { gate, collapse } => {
                let gate = gate.canonical_name();
                formatter.write_str(AnalysisOperation::PauliConjugation.message_prefix())?;
                match collapse {
                    PauliCollapse::TouchesReset { qubit } => write!(
                        formatter,
                        "Pauli observable touches reset {gate} at qubit {}",
                        qubit.get()
                    ),
                    PauliCollapse::AnticommutesWithReset { qubit } => write!(
                        formatter,
                        "Pauli observable anticommutes with reset {gate} at qubit {}",
                        qubit.get()
                    ),
                    PauliCollapse::AnticommutesWithMeasurement { qubit } => write!(
                        formatter,
                        "Pauli observable anticommutes with measurement {gate} at qubit {}",
                        qubit.get()
                    ),
                    PauliCollapse::AnticommutesWithProductMeasurement => {
                        formatter.write_str("Pauli observable anticommutes with an MPP measurement")
                    }
                    PauliCollapse::ClassicallyControlled { qubit } => write!(
                        formatter,
                        "Pauli observable is affected by classically controlled {gate} on qubit {}",
                        qubit.get()
                    ),
                }
            }
            Self::AntiHermitianPauliProduct {
                operation,
                gate,
                phase,
            } => {
                let gate = gate.canonical_name();
                formatter.write_str(operation.message_prefix())?;
                match (operation, phase) {
                    (AnalysisOperation::CircuitSimplification, _) => formatter.write_str(
                        "Pauli product decomposition encountered an anti-Hermitian product",
                    ),
                    // These operations decompose the product through circuit simplification and
                    // have always reported its message after their own prefix.
                    (
                        AnalysisOperation::TableauConversion | AnalysisOperation::PauliConjugation,
                        _,
                    ) => formatter.write_str(
                        "cannot simplify circuit: Pauli product decomposition encountered an anti-Hermitian product",
                    ),
                    (AnalysisOperation::FlowGeneration, Some(phase)) => write!(
                        formatter,
                        "{gate} flow generator Pauli product is not Hermitian: Pauli product has imaginary phase {phase}"
                    ),
                    _ => write!(formatter, "{gate} Pauli product is anti-Hermitian"),
                }
            }
            Self::NoiseRequiresApproximation { gate, excess } => {
                formatter
                    .write_str(AnalysisOperation::CircuitToDetectorErrorModel.message_prefix())?;
                let name = gate.canonical_name();
                match excess {
                    None => write!(
                        formatter,
                        "{name} requires approximate_disjoint_errors during error analysis"
                    ),
                    Some(excess) if *gate == GateKind::ELSE_CORRELATED_ERROR => write!(
                        formatter,
                        "CORRELATED_ERROR/ELSE_CORRELATED_ERROR block has a component probability '{}' larger than the approximate_disjoint_errors threshold of '{}'.",
                        excess.probability().get(),
                        excess.threshold().get()
                    ),
                    Some(excess) => write!(
                        formatter,
                        "{name} has a probability argument ({}) larger than the approximate_disjoint_errors threshold ({})",
                        excess.probability().get(),
                        excess.threshold().get()
                    ),
                }
            }
            Self::UnanalyzableNoise(noise) => write!(
                formatter,
                "{}{noise}",
                AnalysisOperation::CircuitToDetectorErrorModel.message_prefix()
            ),
            Self::ErrorDecompositionFailed {
                component,
                remnant_edges_blocked,
            } => {
                formatter
                    .write_str(AnalysisOperation::CircuitToDetectorErrorModel.message_prefix())?;
                formatter.write_str(
                    "Failed to decompose errors into graphlike components with at most two symptoms.\nThe error component that failed to decompose is '",
                )?;
                for (index, target) in component.iter().enumerate() {
                    if index > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "{target}")?;
                }
                formatter.write_str("'.")?;
                if *remnant_edges_blocked {
                    formatter.write_str(
                        "\n\nNote: `block_decomposition_from_introducing_remnant_edges` is ON.\nTurning it off may prevent this error.",
                    )?;
                }
                Ok(())
            }
            Self::NonGraphlikeError => write!(
                formatter,
                "{}The detector error model contained a non-graphlike error mechanism.\nYou can ignore such errors using `ignore_ungraphlike_errors`.\nYou can use `decompose_errors` when converting a circuit into a model to ensure no such errors are present.",
                AnalysisOperation::LogicalErrorSearch.message_prefix()
            ),
            Self::InvalidRequest(request) => request.fmt(formatter),
            Self::InvalidGeneratorParameter { parameter, value } => {
                write!(
                    formatter,
                    "invalid {} value {value}",
                    parameter.description()
                )
            }
            Self::NonFiniteCoordinate { .. } => {
                formatter.write_str("invalid result format data: coordinate shift overflowed")
            }
        }
    }
}

impl std::error::Error for AnalysisError {}

impl From<ModelError> for AnalysisError {
    /// Keeps one representation of cancellation: a model traversal stopped by the operation's
    /// signal becomes [`AnalysisError::Cancelled`].
    fn from(error: ModelError) -> Self {
        match error {
            ModelError::Cancelled(cancelled) => Self::Cancelled(cancelled),
            error => Self::Model(error),
        }
    }
}

impl From<ResourceLimitError> for AnalysisError {
    fn from(error: ResourceLimitError) -> Self {
        Self::ResourceLimit(error)
    }
}

impl From<stab_model::Cancelled> for AnalysisError {
    fn from(cancelled: stab_model::Cancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

impl From<NonDeterministicTargets> for AnalysisError {
    fn from(targets: NonDeterministicTargets) -> Self {
        Self::NonDeterministicTargets(targets)
    }
}

impl From<AnalysisRequestError> for AnalysisError {
    fn from(request: AnalysisRequestError) -> Self {
        Self::InvalidRequest(request)
    }
}

impl From<UnanalyzableNoise> for AnalysisError {
    fn from(noise: UnanalyzableNoise) -> Self {
        Self::UnanalyzableNoise(noise)
    }
}

impl AnalysisError {
    /// An internal invariant of `operation` failed.
    pub(crate) fn internal(
        operation: AnalysisOperation,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::InternalInvariant {
            operation,
            detail: detail.into(),
        }
    }

    /// A count `operation` derives from its input does not fit its integer type.
    pub(crate) fn count_overflow(
        operation: AnalysisOperation,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::CountOverflow {
            operation,
            detail: detail.into(),
        }
    }

    /// The allocator refused storage `operation` had admitted.
    pub(crate) fn allocation_failed(
        operation: AnalysisOperation,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::AllocationFailed {
            operation,
            detail: detail.into(),
        }
    }

    /// A measurement-record target of the input looks back before the first measurement.
    pub(crate) fn record_out_of_range(
        operation: AnalysisOperation,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::MeasurementRecordOutOfRange {
            operation,
            detail: detail.into(),
        }
    }

    /// `operation` does not support what this instruction asks of it.
    pub(crate) fn unsupported(
        operation: AnalysisOperation,
        gate: impl Into<Option<GateKind>>,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::UnsupportedInput {
            operation,
            gate: gate.into(),
            detail: detail.into(),
        }
    }

    pub(crate) const fn non_unitary(
        operation: AnalysisOperation,
        gate: GateKind,
        reason: NonUnitaryReason,
    ) -> Self {
        Self::NonUnitaryOperation {
            operation,
            gate,
            reason,
        }
    }

    pub(crate) const fn pauli_not_preserved(gate: GateKind, collapse: PauliCollapse) -> Self {
        Self::PauliNotPreserved { gate, collapse }
    }

    pub(crate) const fn invalid_generator_parameter(
        parameter: GeneratorParameter,
        value: u64,
    ) -> Self {
        Self::InvalidGeneratorParameter { parameter, value }
    }

    /// Re-reports a Pauli-product decomposition failure from inside `operation`: an
    /// anti-Hermitian product stays one, and anything else is an internal invariant of
    /// `operation`, since decomposing a validated Pauli product cannot otherwise fail.
    pub(crate) fn into_nested_pauli_product_error(self, operation: AnalysisOperation) -> Self {
        match self {
            Self::AntiHermitianPauliProduct { gate, .. } => Self::AntiHermitianPauliProduct {
                operation,
                gate,
                phase: None,
            },
            error => Self::internal(operation, error.to_string()),
        }
    }

    pub const fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::ResourceLimit(error) => Some(error),
            _ => None,
        }
    }

    /// The memory-budget refusal this error reports, from the analysis itself or from the model
    /// layer; `None` for fixed safety limits and every other failure.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::ResourceLimit(error) => error.memory_budget_exceeded(),
            Self::Model(error) => error.memory_budget_exceeded(),
            _ => None,
        }
    }

    /// The analysis operation the failure belongs to, when the variant records one.
    pub const fn operation(&self) -> Option<AnalysisOperation> {
        match self {
            Self::ResourceLimit(error) => Some(error.operation()),
            Self::AllocationFailed { operation, .. }
            | Self::NonUnitaryOperation { operation, .. }
            | Self::AntiHermitianPauliProduct { operation, .. }
            | Self::UnsupportedInput { operation, .. }
            | Self::CountOverflow { operation, .. }
            | Self::MeasurementRecordOutOfRange { operation, .. }
            | Self::NonFiniteCoordinate { operation }
            | Self::InternalInvariant { operation, .. } => Some(*operation),
            Self::PauliNotPreserved { .. } => Some(AnalysisOperation::PauliConjugation),
            Self::NoiseRequiresApproximation { .. }
            | Self::UnanalyzableNoise(_)
            | Self::ErrorDecompositionFailed { .. } => {
                Some(AnalysisOperation::CircuitToDetectorErrorModel)
            }
            Self::NonGraphlikeError => Some(AnalysisOperation::LogicalErrorSearch),
            Self::InvalidGeneratorParameter { .. } => Some(AnalysisOperation::CircuitGeneration),
            Self::Model(_)
            | Self::Cancelled(_)
            | Self::NonDeterministicTargets(_)
            | Self::InvalidRequest(_) => None,
        }
    }
}
