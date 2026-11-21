//! Typed payloads of the semantic [`super::AnalysisError`] variants.

use std::borrow::Cow;
use std::fmt::{Display, Formatter};

use stab_algebra::Flow;
use stab_model::{
    CircuitTick, DemDetectorId, DemTarget, GateKind, ObservableId, Probability, QubitId,
};

/// Detectors or observables whose value a collapse in the circuit randomizes.
///
/// Error analysis reports these with Stim's wording; the other users of the reverse frame tracker
/// (detecting regions, feedback inlining) report them as a plain list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NonDeterministicTargets {
    targets: Vec<DemTarget>,
    report: CollapseReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CollapseReport {
    /// Stim's `check_for_gauge` text; allowing gauge detectors suppresses the detector line.
    ErrorAnalysis {
        reports_detectors: bool,
    },
    Tracking,
}

impl NonDeterministicTargets {
    pub(crate) fn error_analysis(targets: Vec<DemTarget>, reports_detectors: bool) -> Self {
        Self {
            targets,
            report: CollapseReport::ErrorAnalysis { reports_detectors },
        }
    }

    pub(crate) fn tracking(targets: Vec<DemTarget>) -> Self {
        Self {
            targets,
            report: CollapseReport::Tracking,
        }
    }

    /// The anticommuting detectors and observables, in ascending target order.
    pub fn targets(&self) -> &[DemTarget] {
        &self.targets
    }

    pub fn has_observables(&self) -> bool {
        self.targets.iter().any(|target| target.is_observable())
    }

    pub fn has_detectors(&self) -> bool {
        self.targets.iter().any(|target| target.is_detector())
    }
}

impl Display for NonDeterministicTargets {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid detector error model: ")?;
        match self.report {
            CollapseReport::ErrorAnalysis { reports_detectors } => {
                if self.has_observables() {
                    formatter.write_str("The circuit contains non-deterministic observables.\n")?;
                }
                if reports_detectors && self.has_detectors() {
                    formatter.write_str("The circuit contains non-deterministic detectors.\n")?;
                }
                formatter
                    .write_str("The collapse anti-commuted with these detectors/observables:")?;
            }
            CollapseReport::Tracking => {
                formatter.write_str("collapse anti-commuted with tracked targets:")?;
            }
        }
        for target in &self.targets {
            write!(formatter, "\n    {target}")?;
        }
        Ok(())
    }
}

/// Why an operation that needs a unitary circuit refused an instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NonUnitaryReason {
    Measurement,
    Reset,
    /// A noise channel with a non-zero probability.
    Noise,
    /// The instruction has no strict inverse: it measures, resets, adds noise, or annotates.
    NoStrictInverse,
}

impl NonUnitaryReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measurement => "measurement",
            Self::Reset => "reset",
            Self::Noise => "noise",
            Self::NoStrictInverse => "no-strict-inverse",
        }
    }
}

/// How an instruction left a conjugated Pauli observable undefined.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauliCollapse {
    /// The observable has support on a qubit a reset replaces.
    TouchesReset {
        qubit: QubitId,
    },
    AnticommutesWithReset {
        qubit: QubitId,
    },
    AnticommutesWithMeasurement {
        qubit: QubitId,
    },
    AnticommutesWithProductMeasurement,
    /// A classically controlled Pauli on this qubit anticommutes with the observable.
    ClassicallyControlled {
        qubit: QubitId,
    },
}

impl PauliCollapse {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TouchesReset { .. } => "touches-reset",
            Self::AnticommutesWithReset { .. } => "anticommutes-with-reset",
            Self::AnticommutesWithMeasurement { .. } => "anticommutes-with-measurement",
            Self::AnticommutesWithProductMeasurement => "anticommutes-with-product-measurement",
            Self::ClassicallyControlled { .. } => "classically-controlled",
        }
    }

    pub const fn qubit(self) -> Option<QubitId> {
        match self {
            Self::TouchesReset { qubit }
            | Self::AnticommutesWithReset { qubit }
            | Self::AnticommutesWithMeasurement { qubit }
            | Self::ClassicallyControlled { qubit } => Some(qubit),
            Self::AnticommutesWithProductMeasurement => None,
        }
    }
}

/// A noise probability above the `approximate_disjoint_errors` threshold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThresholdExcess {
    probability: Probability,
    threshold: Probability,
}

// A `Probability` is never NaN, so its `PartialEq` is an equivalence relation.
impl Eq for ThresholdExcess {}

impl ThresholdExcess {
    pub(crate) const fn new(probability: Probability, threshold: Probability) -> Self {
        Self {
            probability,
            threshold,
        }
    }

    pub const fn probability(self) -> Probability {
        self.probability
    }

    pub const fn threshold(self) -> Probability {
        self.threshold
    }
}

/// Noise that error analysis cannot turn into independent error mechanisms.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnanalyzableNoise {
    /// A `DEPOLARIZE1` probability above 3/4.
    OverMixingDepolarize1,
    /// A `DEPOLARIZE2` probability above 15/16.
    OverMixingDepolarize2,
    /// An `ELSE_CORRELATED_ERROR` not preceded by `CORRELATED_ERROR` or `ELSE_CORRELATED_ERROR`.
    UnprecededElseCorrelatedError,
}

impl UnanalyzableNoise {
    pub const fn gate(self) -> GateKind {
        match self {
            Self::OverMixingDepolarize1 => GateKind::DEPOLARIZE1,
            Self::OverMixingDepolarize2 => GateKind::DEPOLARIZE2,
            Self::UnprecededElseCorrelatedError => GateKind::ELSE_CORRELATED_ERROR,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OverMixingDepolarize1 | Self::OverMixingDepolarize2 => "over-mixing",
            Self::UnprecededElseCorrelatedError => "unpreceded-else-correlated-error",
        }
    }
}

impl Display for UnanalyzableNoise {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::OverMixingDepolarize1 => {
                "cannot analyze over-mixing DEPOLARIZE1 probability above 3/4"
            }
            Self::OverMixingDepolarize2 => {
                "cannot analyze over-mixing DEPOLARIZE2 probability above 15/16"
            }
            Self::UnprecededElseCorrelatedError => {
                "ELSE_CORRELATED_ERROR wasn't preceded by ELSE_CORRELATED_ERROR or CORRELATED_ERROR (E)"
            }
        })
    }
}

/// A caller request an analysis cannot serve for this input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AnalysisRequestError {
    DetectorOutOfRange {
        detector: DemDetectorId,
        detector_count: u64,
    },
    ObservableOutOfRange {
        observable: ObservableId,
        observable_count: u64,
    },
    /// A detecting-region filter named a target that is neither a detector nor an observable.
    NotDetectorOrObservable {
        target: DemTarget,
    },
    TickOutOfRange {
        tick: CircuitTick,
        tick_count: u64,
    },
    ZeroSatQuantization,
    /// A flow-measurement query whose input and output are both the identity.
    EmptyFlowQuery,
    /// A flow the input circuit does not satisfy, given to a reversal that requires it.
    UnsatisfiedFlow {
        index: usize,
        flow: Box<Flow>,
    },
    /// A flow time reversal cannot carry through this circuit: one the circuit does not satisfy,
    /// one whose terms time reversal does not define, or a detecting region that reaches the
    /// start of the circuit without a flow to absorb it.
    IrreversibleFlow {
        detail: Cow<'static, str>,
    },
    /// A Pauli string conjugated through a circuit with an instruction acting on a qubit beyond
    /// the string's width.
    PauliNarrowerThanCircuit {
        gate: GateKind,
        qubit: QubitId,
        width: usize,
    },
}

impl AnalysisRequestError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::DetectorOutOfRange { .. } => "detector-out-of-range",
            Self::ObservableOutOfRange { .. } => "observable-out-of-range",
            Self::NotDetectorOrObservable { .. } => "not-detector-or-observable",
            Self::TickOutOfRange { .. } => "tick-out-of-range",
            Self::ZeroSatQuantization => "zero-sat-quantization",
            Self::EmptyFlowQuery => "empty-flow-query",
            Self::UnsatisfiedFlow { .. } => "unsatisfied-flow",
            Self::IrreversibleFlow { .. } => "irreversible-flow",
            Self::PauliNarrowerThanCircuit { .. } => "pauli-narrower-than-circuit",
        }
    }
}

impl Display for AnalysisRequestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DetectorOutOfRange {
                detector,
                detector_count,
            } => write!(
                formatter,
                "invalid detector error model: requested detector D{} but circuit only has {detector_count} detector(s)",
                detector.get()
            ),
            Self::ObservableOutOfRange {
                observable,
                observable_count,
            } => write!(
                formatter,
                "invalid detector error model: requested observable L{} but circuit only has {observable_count} observable(s)",
                observable.get()
            ),
            Self::NotDetectorOrObservable { target } => write!(
                formatter,
                "invalid detector error model: detecting-region target filters only supports detector and logical-observable targets, got {target}"
            ),
            Self::TickOutOfRange { tick, tick_count } => write!(
                formatter,
                "invalid detector error model: requested tick {} but circuit only has {tick_count} tick layer(s)",
                tick.get()
            ),
            Self::ZeroSatQuantization => formatter.write_str(
                "invalid detector error model: weighted SAT quantization must be at least 1",
            ),
            Self::EmptyFlowQuery => formatter.write_str(
                "cannot convert circuit to tableau: solve_for_flow_measurements only supports flows with non-empty Pauli input or output",
            ),
            Self::UnsatisfiedFlow { index, flow } => write!(
                formatter,
                "cannot convert circuit to tableau: time_reversed_for_flows unitary fast path requires input circuit to satisfy flow {index}: {flow}"
            ),
            Self::IrreversibleFlow { detail } => {
                write!(formatter, "cannot convert circuit to tableau: {detail}")
            }
            Self::PauliNarrowerThanCircuit { gate, qubit, width } => write!(
                formatter,
                "cannot convert circuit to tableau: {} targets qubit {} outside the {width}-qubit Pauli string",
                gate.canonical_name(),
                qubit.get()
            ),
        }
    }
}

/// A code-generation parameter outside the generator's supported range.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GeneratorParameter {
    /// A code distance outside `2..=2047`.
    CodeDistance,
    /// A round count of zero.
    RoundCount,
    /// An even color-code distance.
    ColorCodeDistance,
    /// A color-code round count below two.
    ColorCodeRoundCount,
}

impl GeneratorParameter {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodeDistance => "code-distance",
            Self::RoundCount => "round-count",
            Self::ColorCodeDistance => "color-code-distance",
            Self::ColorCodeRoundCount => "color-code-round-count",
        }
    }

    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::CodeDistance => "code distance",
            Self::RoundCount => "round count",
            Self::ColorCodeDistance => "color code distance",
            Self::ColorCodeRoundCount => "color code round count",
        }
    }
}
