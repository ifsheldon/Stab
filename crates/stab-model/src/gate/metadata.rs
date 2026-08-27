use super::{ArgRule, Gate, GateCategory, GateFlags, TargetRule};

/// Public argument validation shape for a Stim gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateArgumentRule {
    /// The gate takes exactly this many parenthesized arguments.
    Exact(usize),
    /// The gate accepts any finite coordinate-like argument list.
    Any,
    /// The gate accepts zero or one probability argument.
    OptionalProbability,
    /// The gate takes exactly this many disjoint probability arguments.
    ProbabilityList(usize),
    /// The gate accepts any number of disjoint probability arguments.
    AnyProbabilityList,
    /// The gate takes exactly one unsigned integer argument.
    UnsignedInteger,
}

/// Public target validation shape for a Stim gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateTargetRule {
    None,
    AnySingleQubit,
    MeasurementQubits,
    MeasurementPads,
    PlainPairs,
    ClassicalControlPairs,
    MeasurementPairs,
    RecOnly,
    RecOrPauli,
    QubitCoords,
    PauliProducts,
    PauliList,
}

/// How a circuit instruction's flat target list is grouped by this gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateTargetGroupKind {
    None,
    Singles,
    Pairs,
    PauliProducts,
    AllTargets,
}

impl Gate {
    /// Returns all accepted names for this gate, in Stim v1.16.0 alias order.
    #[inline]
    pub fn aliases(self) -> &'static [&'static str] {
        if self.info.aliases.is_empty() {
            std::slice::from_ref(&self.info.name)
        } else {
            self.info.aliases
        }
    }

    #[inline]
    pub fn argument_rule(self) -> GateArgumentRule {
        self.info.arg_rule.into()
    }

    #[inline]
    pub fn target_rule(self) -> GateTargetRule {
        self.info.target_rule.into()
    }

    #[inline]
    pub fn target_group_kind(self) -> GateTargetGroupKind {
        self.info.target_rule.target_group_kind()
    }

    /// Returns true when Stim has a unitary/tableau inverse for this gate.
    #[inline]
    pub fn is_unitary(self) -> bool {
        // The Pauli-product category splits into the measuring MPP and the unitary
        // phasing gates SPP/SPP_DAG, so result production decides that category.
        matches!(
            self.info.category,
            GateCategory::Controlled
                | GateCategory::HadamardLike
                | GateCategory::Pauli
                | GateCategory::Period3
                | GateCategory::Period4
                | GateCategory::ParityPhasing
                | GateCategory::Swap
        ) || (matches!(self.info.category, GateCategory::PauliProduct)
            && !self.produces_measurements())
    }

    /// Returns true for reset or measure-reset gates, mirroring Stim's `GATE_IS_RESET`.
    #[inline]
    pub fn is_reset(self) -> bool {
        self.info.flags.contains(GateFlags::IS_RESET)
    }

    /// Returns Stim v1.16.0's `GateData.is_noisy_gate` flag.
    ///
    /// Noise-channel categories are noisy, and so is every result-producing gate because it
    /// accepts a flip-probability argument. This intentionally excludes `MPAD`, which can take
    /// a probability argument but is not flagged as noisy by Stim.
    #[inline]
    pub fn is_noisy(self) -> bool {
        matches!(
            self.info.category,
            GateCategory::Noise | GateCategory::HeraldedNoise | GateCategory::PairMeasurement
        ) || (self.produces_measurements() && !self.targets_are_pad_values())
    }

    /// Returns true when the gate appends results to the measurement record, mirroring
    /// Stim's `GATE_PRODUCES_RESULTS`.
    #[inline]
    pub fn produces_measurements(self) -> bool {
        self.info.flags.contains(GateFlags::PRODUCES_RESULTS)
    }

    /// Returns true when the gate's results are heralds reported by a noise channel rather
    /// than qubit measurements.
    #[inline]
    pub fn produces_heralded_results(self) -> bool {
        matches!(self.info.category, GateCategory::HeraldedNoise)
    }

    /// Returns true when this gate's targets play a metadata-only pad role instead of
    /// naming qubits.
    ///
    /// `MPAD` pads reserve measurement records, and their 0/1 targets are values, not qubits.
    /// Stim excludes them from simulation statistics, although its target-based
    /// `Circuit::count_qubits` compatibility helper still includes their numeric values.
    #[inline]
    pub fn targets_are_pad_values(self) -> bool {
        matches!(self.info.target_rule, TargetRule::MeasurementPads)
    }

    #[inline]
    pub fn is_single_qubit_gate(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::AnySingleQubit | TargetRule::MeasurementQubits
        )
    }

    #[inline]
    pub fn is_two_qubit_gate(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::PlainPairs
                | TargetRule::ClassicalControlPairs
                | TargetRule::MeasurementPairs
        )
    }

    #[inline]
    pub fn takes_measurement_record_targets(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::ClassicalControlPairs | TargetRule::RecOnly | TargetRule::RecOrPauli
        )
    }

    #[inline]
    pub fn takes_pauli_targets(self) -> bool {
        matches!(
            self.info.target_rule,
            TargetRule::RecOrPauli | TargetRule::PauliProducts | TargetRule::PauliList
        )
    }

    #[inline]
    pub fn is_symmetric_gate(self) -> bool {
        if matches!(
            self.info.target_rule,
            TargetRule::AnySingleQubit | TargetRule::MeasurementQubits
        ) {
            return true;
        }
        self.info.flags.contains(GateFlags::SYMMETRIC_PAIR)
    }

    /// Returns the true unitary inverse, or `None` for non-unitary gates.
    #[inline]
    pub fn inverse(self) -> Option<Self> {
        self.is_unitary()
            .then(|| Self::from_name(self.info.inverse_name).ok())
            .flatten()
    }

    /// Returns Stim's best candidate inverse, including non-unitary generalized inverses.
    #[inline]
    pub fn generalized_inverse(self) -> crate::ModelResult<Self> {
        self.best_candidate_inverse()
    }

    #[inline]
    pub fn can_fuse(self) -> bool {
        self.info.can_fuse
    }
}

impl From<ArgRule> for GateArgumentRule {
    fn from(value: ArgRule) -> Self {
        match value {
            ArgRule::Exact(count) => Self::Exact(count),
            ArgRule::Any => Self::Any,
            ArgRule::ZeroOrOneProbability => Self::OptionalProbability,
            ArgRule::ProbabilityList(count) => Self::ProbabilityList(count),
            ArgRule::AnyProbabilityList => Self::AnyProbabilityList,
            ArgRule::UnsignedInteger => Self::UnsignedInteger,
        }
    }
}

impl From<TargetRule> for GateTargetRule {
    fn from(value: TargetRule) -> Self {
        match value {
            TargetRule::None => Self::None,
            TargetRule::AnySingleQubit => Self::AnySingleQubit,
            TargetRule::MeasurementQubits => Self::MeasurementQubits,
            TargetRule::MeasurementPads => Self::MeasurementPads,
            TargetRule::PlainPairs => Self::PlainPairs,
            TargetRule::ClassicalControlPairs => Self::ClassicalControlPairs,
            TargetRule::MeasurementPairs => Self::MeasurementPairs,
            TargetRule::RecOnly => Self::RecOnly,
            TargetRule::RecOrPauli => Self::RecOrPauli,
            TargetRule::QubitCoords => Self::QubitCoords,
            TargetRule::PauliProducts => Self::PauliProducts,
            TargetRule::PauliList => Self::PauliList,
        }
    }
}
