//! Typed, allocation-free views of a [`CircuitInstruction`]'s arguments and targets.
//!
//! Every instruction is validated against its gate's argument and target rules when it is
//! built, so each view names exactly the shape that rule admits and decodes the stored values
//! without re-checking them. The decoders are total on every stored value, and debug builds
//! re-validate the instruction when a view is taken, so a broken invariant is caught in tests and
//! never becomes a panic in release builds.

use std::iter::FusedIterator;
use std::slice;

use crate::controlled_pauli::ControlledPauliTargetPair;
use crate::gate::{ArgRule, GateTargetGroupKind};
use crate::target::{PauliTarget, QubitTarget};
use crate::{
    Gate, GateTargetRule, MeasureRecordOffset, ObservableId, Probability, QubitId, Target,
};

use super::CircuitInstruction;

/// The arguments of a [`CircuitInstruction`], shaped by its gate's argument rule.
///
/// Returned by [`CircuitInstruction::gate_arguments`].
#[derive(Clone, Debug, PartialEq)]
pub enum GateArguments<'a> {
    /// The gate takes no arguments ([`crate::GateArgumentRule::Exact`]`(0)`).
    None,
    /// Finite coordinate-like values ([`crate::GateArgumentRule::Any`]): `DETECTOR`,
    /// `QUBIT_COORDS`, and `SHIFT_COORDS`.
    Coordinates(&'a [f64]),
    /// An optional flip or error probability ([`crate::GateArgumentRule::OptionalProbability`]):
    /// measurement gates and `MPAD`.
    OptionalProbability(Option<Probability>),
    /// Exactly one probability ([`crate::GateArgumentRule::ProbabilityList`]`(1)`).
    Probability(Probability),
    /// Disjoint probabilities whose count is the rule's
    /// ([`crate::GateArgumentRule::ProbabilityList`]`(n)` for `n >= 2`, or
    /// [`crate::GateArgumentRule::AnyProbabilityList`]).
    Probabilities(ProbabilityArgs<'a>),
    /// An observable id ([`crate::GateArgumentRule::UnsignedInteger`]): `OBSERVABLE_INCLUDE`.
    Observable(ObservableId),
}

/// The disjoint probability arguments of a channel such as `PAULI_CHANNEL_1` or `I_ERROR`.
///
/// Iterates the probabilities in argument order without allocating.
#[derive(Clone, Debug)]
pub struct ProbabilityArgs<'a> {
    values: slice::Iter<'a, f64>,
}

impl<'a> ProbabilityArgs<'a> {
    /// Returns the probability at `index` among those not yet iterated.
    #[inline]
    pub fn get(&self, index: usize) -> Option<Probability> {
        self.values
            .as_slice()
            .get(index)
            .copied()
            .map(Probability::from_validated)
    }

    /// Returns the probabilities as an array when there are exactly `N` of them, which the
    /// gate's fixed-count rule (`ProbabilityList(N)`) guarantees.
    #[inline]
    pub fn to_array<const N: usize>(&self) -> Option<[Probability; N]> {
        let values: &[f64; N] = self.values.as_slice().try_into().ok()?;
        Some(values.map(Probability::from_validated))
    }

    /// Returns the raw argument values not yet iterated.
    #[inline]
    pub fn as_f64_slice(&self) -> &'a [f64] {
        self.values.as_slice()
    }
}

/// Two views are equal when they hold the same remaining probabilities.
impl PartialEq for ProbabilityArgs<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_f64_slice() == other.as_f64_slice()
    }
}

impl Iterator for ProbabilityArgs<'_> {
    type Item = Probability;

    #[inline]
    fn next(&mut self) -> Option<Probability> {
        self.values.next().copied().map(Probability::from_validated)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.values.size_hint()
    }
}

impl DoubleEndedIterator for ProbabilityArgs<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Probability> {
        self.values
            .next_back()
            .copied()
            .map(Probability::from_validated)
    }
}

impl ExactSizeIterator for ProbabilityArgs<'_> {}
impl FusedIterator for ProbabilityArgs<'_> {}

/// The targets of a [`CircuitInstruction`], shaped by its gate's target rule.
///
/// Returned by [`CircuitInstruction::gate_targets`]; each variant corresponds to one
/// [`GateTargetRule`], as [`GateTargets::rule`] reports.
#[derive(Clone, Debug)]
pub enum GateTargets<'a> {
    /// No targets ([`GateTargetRule::None`]).
    None,
    /// Plain qubits ([`GateTargetRule::AnySingleQubit`]): single-qubit unitaries, resets, and
    /// single-qubit noise.
    Qubits(QubitIds<'a>),
    /// Qubits that may carry a result inversion ([`GateTargetRule::MeasurementQubits`]).
    MeasuredQubits(QubitTargets<'a>),
    /// `MPAD` pad values ([`GateTargetRule::MeasurementPads`]).
    MeasurementPads(PadValues<'a>),
    /// Distinct plain qubit pairs ([`GateTargetRule::PlainPairs`]).
    QubitPairs(QubitPairs<'a>),
    /// Controlled-Pauli pairs that may hold classical controls
    /// ([`GateTargetRule::ClassicalControlPairs`]).
    ControlledPairs(ControlledPairs<'a>),
    /// Distinct qubit pairs whose results may be inverted ([`GateTargetRule::MeasurementPairs`]).
    MeasuredQubitPairs(QubitTargetPairs<'a>),
    /// Measurement records ([`GateTargetRule::RecOnly`]): `DETECTOR`.
    Records(RecordTargets<'a>),
    /// Measurement records and Pauli targets ([`GateTargetRule::RecOrPauli`]):
    /// `OBSERVABLE_INCLUDE`.
    ObservableTargets(ObservableTargets<'a>),
    /// Plain qubits whose coordinates are declared ([`GateTargetRule::QubitCoords`]).
    QubitCoords(QubitIds<'a>),
    /// `*`-joined Pauli products ([`GateTargetRule::PauliProducts`]): `MPP`, `SPP`, `SPP_DAG`.
    PauliProducts(PauliProducts<'a>),
    /// One Pauli product whose `*` combiners are ignored decoration ([`GateTargetRule::PauliList`]):
    /// `E` and `ELSE_CORRELATED_ERROR`.
    PauliTerms(PauliTerms<'a>),
}

impl GateTargets<'_> {
    /// Returns the target rule this view's shape comes from.
    pub const fn rule(&self) -> GateTargetRule {
        match self {
            Self::None => GateTargetRule::None,
            Self::Qubits(_) => GateTargetRule::AnySingleQubit,
            Self::MeasuredQubits(_) => GateTargetRule::MeasurementQubits,
            Self::MeasurementPads(_) => GateTargetRule::MeasurementPads,
            Self::QubitPairs(_) => GateTargetRule::PlainPairs,
            Self::ControlledPairs(_) => GateTargetRule::ClassicalControlPairs,
            Self::MeasuredQubitPairs(_) => GateTargetRule::MeasurementPairs,
            Self::Records(_) => GateTargetRule::RecOnly,
            Self::ObservableTargets(_) => GateTargetRule::RecOrPauli,
            Self::QubitCoords(_) => GateTargetRule::QubitCoords,
            Self::PauliProducts(_) => GateTargetRule::PauliProducts,
            Self::PauliTerms(_) => GateTargetRule::PauliList,
        }
    }
}

/// One `OBSERVABLE_INCLUDE` target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservableTarget {
    MeasurementRecord(MeasureRecordOffset),
    Pauli(PauliTarget),
}

/// Declares a view over single targets that yields one decoded item per target.
macro_rules! single_target_view {
    ($(#[$doc:meta])* $name:ident => $item:ty, $decode:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug)]
        pub struct $name<'a> {
            targets: slice::Iter<'a, Target>,
        }

        impl<'a> $name<'a> {
            #[inline]
            fn new(targets: &'a [Target]) -> Self {
                Self {
                    targets: targets.iter(),
                }
            }

            /// Returns the item at `index` among those not yet iterated.
            #[inline]
            pub fn get(&self, index: usize) -> Option<$item> {
                self.targets.as_slice().get(index).copied().map($decode)
            }

            /// Returns the raw targets not yet iterated.
            #[inline]
            pub fn as_targets(&self) -> &'a [Target] {
                self.targets.as_slice()
            }
        }

        impl Iterator for $name<'_> {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<$item> {
                self.targets.next().copied().map($decode)
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.targets.size_hint()
            }
        }

        impl DoubleEndedIterator for $name<'_> {
            #[inline]
            fn next_back(&mut self) -> Option<$item> {
                self.targets.next_back().copied().map($decode)
            }
        }

        impl ExactSizeIterator for $name<'_> {}
        impl FusedIterator for $name<'_> {}
    };
}

single_target_view!(
    /// Plain qubit targets, yielded as [`QubitId`]s.
    QubitIds => QubitId, Target::decode_qubit_id
);
single_target_view!(
    /// Measured qubit targets, yielded with their inversion flag.
    QubitTargets => QubitTarget, Target::decode_qubit_target
);
single_target_view!(
    /// `MPAD` pad values, yielded as the recorded bit (`false` for `0`, `true` for `1`).
    PadValues => bool, |target: Target| target.decode_qubit_id().get() != 0
);
single_target_view!(
    /// Measurement-record targets, yielded as their lookback offsets.
    RecordTargets => MeasureRecordOffset, Target::decode_record
);
single_target_view!(
    /// `OBSERVABLE_INCLUDE` targets: measurement records and Pauli targets.
    ObservableTargets => ObservableTarget, decode_observable_target
);

#[inline(always)]
fn decode_observable_target(target: Target) -> ObservableTarget {
    if target.is_measurement_record_target() {
        ObservableTarget::MeasurementRecord(target.decode_record())
    } else {
        ObservableTarget::Pauli(target.decode_pauli())
    }
}

/// Declares a view over target pairs that yields one decoded item per pair.
macro_rules! pair_target_view {
    ($(#[$doc:meta])* $name:ident => $item:ty, $decode:expr) => {
        $(#[$doc])*
        #[derive(Clone, Debug)]
        pub struct $name<'a> {
            pairs: slice::Iter<'a, [Target; 2]>,
        }

        impl<'a> $name<'a> {
            #[inline]
            fn new(targets: &'a [Target]) -> Self {
                let (pairs, remainder) = targets.as_chunks::<2>();
                debug_assert!(remainder.is_empty(), "pair targets have an odd count");
                Self {
                    pairs: pairs.iter(),
                }
            }

            /// Returns the pair at `index` among those not yet iterated.
            #[inline]
            pub fn get(&self, index: usize) -> Option<$item> {
                self.pairs.as_slice().get(index).map($decode)
            }

            /// Returns the raw target pairs not yet iterated.
            #[inline]
            pub fn as_target_pairs(&self) -> &'a [[Target; 2]] {
                self.pairs.as_slice()
            }
        }

        impl Iterator for $name<'_> {
            type Item = $item;

            #[inline]
            fn next(&mut self) -> Option<$item> {
                self.pairs.next().map($decode)
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.pairs.size_hint()
            }
        }

        impl DoubleEndedIterator for $name<'_> {
            #[inline]
            fn next_back(&mut self) -> Option<$item> {
                self.pairs.next_back().map($decode)
            }
        }

        impl ExactSizeIterator for $name<'_> {}
        impl FusedIterator for $name<'_> {}
    };
}

pair_target_view!(
    /// Distinct plain qubit pairs, yielded as `[first, second]`.
    QubitPairs => [QubitId; 2],
    |[first, second]: &[Target; 2]| [first.decode_qubit_id(), second.decode_qubit_id()]
);
pair_target_view!(
    /// Distinct measured qubit pairs, yielded with their inversion flags.
    QubitTargetPairs => [QubitTarget; 2],
    |[first, second]: &[Target; 2]| [first.decode_qubit_target(), second.decode_qubit_target()]
);

/// The target pairs of a classically controllable controlled-Pauli gate, yielded classified by
/// which slot holds a classical control.
#[derive(Clone, Debug)]
pub struct ControlledPairs<'a> {
    gate: Gate,
    pairs: slice::Iter<'a, [Target; 2]>,
}

impl<'a> ControlledPairs<'a> {
    #[inline]
    fn new(gate: Gate, targets: &'a [Target]) -> Self {
        let (pairs, remainder) = targets.as_chunks::<2>();
        debug_assert!(remainder.is_empty(), "pair targets have an odd count");
        Self {
            gate,
            pairs: pairs.iter(),
        }
    }

    /// Returns the classified pair at `index` among those not yet iterated.
    #[inline]
    pub fn get(&self, index: usize) -> Option<ControlledPauliTargetPair> {
        self.pairs
            .as_slice()
            .get(index)
            .map(|pair| ControlledPauliTargetPair::classify(self.gate, pair))
    }

    /// Returns the raw target pairs not yet iterated.
    #[inline]
    pub fn as_target_pairs(&self) -> &'a [[Target; 2]] {
        self.pairs.as_slice()
    }
}

impl Iterator for ControlledPairs<'_> {
    type Item = ControlledPauliTargetPair;

    #[inline]
    fn next(&mut self) -> Option<ControlledPauliTargetPair> {
        let gate = self.gate;
        self.pairs
            .next()
            .map(|pair| ControlledPauliTargetPair::classify(gate, pair))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.pairs.size_hint()
    }
}

impl DoubleEndedIterator for ControlledPairs<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<ControlledPauliTargetPair> {
        let gate = self.gate;
        self.pairs
            .next_back()
            .map(|pair| ControlledPauliTargetPair::classify(gate, pair))
    }
}

impl ExactSizeIterator for ControlledPairs<'_> {}
impl FusedIterator for ControlledPairs<'_> {}

/// Splits Pauli-product target lists into products, from either end.
///
/// A product is one Pauli target followed by any number of combiner-Pauli pairs; validation
/// rejects leading, trailing, and doubled combiners, so both walks see the same products.
#[derive(Clone, Debug)]
struct ProductSplitter<'a> {
    targets: &'a [Target],
    remaining: usize,
}

impl<'a> ProductSplitter<'a> {
    fn new(targets: &'a [Target]) -> Self {
        Self {
            targets,
            remaining: pauli_product_count(targets),
        }
    }

    #[inline]
    fn next_front(&mut self) -> Option<&'a [Target]> {
        if self.targets.is_empty() {
            return None;
        }
        let mut end = 1;
        while self.targets.get(end).is_some_and(Target::is_combiner) {
            end = end.saturating_add(2).min(self.targets.len());
        }
        let (product, rest) = self.targets.split_at(end);
        self.targets = rest;
        self.remaining = self.remaining.saturating_sub(1);
        Some(product)
    }

    #[inline]
    fn next_back(&mut self) -> Option<&'a [Target]> {
        if self.targets.is_empty() {
            return None;
        }
        let mut start = self.targets.len() - 1;
        while start > 0 && self.targets.get(start - 1).is_some_and(Target::is_combiner) {
            start = start.saturating_sub(2);
        }
        let (rest, product) = self.targets.split_at(start);
        self.targets = rest;
        self.remaining = self.remaining.saturating_sub(1);
        Some(product)
    }
}

/// Counts the products of a Pauli-product target list.
fn pauli_product_count(targets: &[Target]) -> usize {
    let mut count = 0;
    let mut previous_was_combiner = false;
    for target in targets {
        if target.is_combiner() {
            previous_was_combiner = true;
        } else {
            if !previous_was_combiner {
                count += 1;
            }
            previous_was_combiner = false;
        }
    }
    count
}

/// The `*`-joined Pauli products of an `MPP`, `SPP`, or `SPP_DAG` instruction.
#[derive(Clone, Debug)]
pub struct PauliProducts<'a> {
    products: ProductSplitter<'a>,
}

impl<'a> PauliProducts<'a> {
    /// Returns the raw targets of the products not yet iterated.
    #[inline]
    pub fn as_targets(&self) -> &'a [Target] {
        self.products.targets
    }
}

impl<'a> Iterator for PauliProducts<'a> {
    type Item = PauliProduct<'a>;

    #[inline]
    fn next(&mut self) -> Option<PauliProduct<'a>> {
        self.products.next_front().map(PauliProduct::new)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.products.remaining, Some(self.products.remaining))
    }
}

impl DoubleEndedIterator for PauliProducts<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.products.next_back().map(PauliProduct::new)
    }
}

impl ExactSizeIterator for PauliProducts<'_> {}
impl FusedIterator for PauliProducts<'_> {}

/// One Pauli product such as `X0*!Y1*Z2`, yielding its Pauli targets without the combiners.
#[derive(Clone, Debug)]
pub struct PauliProduct<'a> {
    targets: &'a [Target],
    paulis: std::iter::StepBy<slice::Iter<'a, Target>>,
}

impl<'a> PauliProduct<'a> {
    #[inline]
    fn new(targets: &'a [Target]) -> Self {
        Self {
            targets,
            paulis: targets.iter().step_by(2),
        }
    }

    /// Returns the product's raw targets, combiners included.
    #[inline]
    pub fn as_targets(&self) -> &'a [Target] {
        self.targets
    }
}

impl Iterator for PauliProduct<'_> {
    type Item = PauliTarget;

    #[inline]
    fn next(&mut self) -> Option<PauliTarget> {
        self.paulis.next().copied().map(Target::decode_pauli)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.paulis.size_hint()
    }
}

impl DoubleEndedIterator for PauliProduct<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<PauliTarget> {
        self.paulis.next_back().copied().map(Target::decode_pauli)
    }
}

impl ExactSizeIterator for PauliProduct<'_> {}
impl FusedIterator for PauliProduct<'_> {}

/// The Pauli targets of an `E` or `ELSE_CORRELATED_ERROR` instruction.
///
/// Pinned Stim's correlated-error handlers consult only the Pauli X/Z bits, so `*` combiners
/// anywhere in the list are skipped (frame_simulator.inl:767-775).
#[derive(Clone, Debug)]
pub struct PauliTerms<'a> {
    targets: slice::Iter<'a, Target>,
    remaining: usize,
}

impl<'a> PauliTerms<'a> {
    fn new(targets: &'a [Target]) -> Self {
        Self {
            targets: targets.iter(),
            remaining: targets
                .iter()
                .filter(|target| !target.is_combiner())
                .count(),
        }
    }

    /// Returns the raw targets not yet iterated, combiners included.
    #[inline]
    pub fn as_targets(&self) -> &'a [Target] {
        self.targets.as_slice()
    }
}

impl Iterator for PauliTerms<'_> {
    type Item = PauliTarget;

    #[inline]
    fn next(&mut self) -> Option<PauliTarget> {
        let target = self.targets.find(|target| !target.is_combiner())?;
        self.remaining = self.remaining.saturating_sub(1);
        Some(target.decode_pauli())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl DoubleEndedIterator for PauliTerms<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<PauliTarget> {
        let target = self.targets.rfind(|target| !target.is_combiner())?;
        self.remaining = self.remaining.saturating_sub(1);
        Some(target.decode_pauli())
    }
}

impl ExactSizeIterator for PauliTerms<'_> {}
impl FusedIterator for PauliTerms<'_> {}

/// The raw target groups of an instruction, as its gate's [`GateTargetGroupKind`] splits them.
///
/// Returned by [`CircuitInstruction::target_groups`].
#[derive(Clone, Debug)]
pub struct TargetGroups<'a> {
    shape: GroupShape<'a>,
}

#[derive(Clone, Debug)]
enum GroupShape<'a> {
    /// Groups of one or two targets.
    Fixed(slice::Chunks<'a, Target>),
    Products(ProductSplitter<'a>),
    /// The whole non-empty target list as one group, or nothing once taken.
    Whole(Option<&'a [Target]>),
}

impl<'a> Iterator for TargetGroups<'a> {
    type Item = &'a [Target];

    #[inline]
    fn next(&mut self) -> Option<&'a [Target]> {
        match &mut self.shape {
            GroupShape::Fixed(chunks) => chunks.next(),
            GroupShape::Products(products) => products.next_front(),
            GroupShape::Whole(whole) => whole.take(),
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.shape {
            GroupShape::Fixed(chunks) => chunks.size_hint(),
            GroupShape::Products(products) => (products.remaining, Some(products.remaining)),
            GroupShape::Whole(whole) => {
                let count = usize::from(whole.is_some());
                (count, Some(count))
            }
        }
    }
}

impl DoubleEndedIterator for TargetGroups<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match &mut self.shape {
            GroupShape::Fixed(chunks) => chunks.next_back(),
            GroupShape::Products(products) => products.next_back(),
            GroupShape::Whole(whole) => whole.take(),
        }
    }
}

impl ExactSizeIterator for TargetGroups<'_> {}
impl FusedIterator for TargetGroups<'_> {}

impl CircuitInstruction {
    /// Returns this instruction's arguments, shaped by its gate's argument rule.
    ///
    /// Every constructor validated the arguments against that rule, so the single-argument
    /// shapes always find their argument; the empty-list fallback below only keeps the decoder
    /// total, and debug builds re-validate the instruction instead.
    #[inline]
    pub fn gate_arguments(&self) -> GateArguments<'_> {
        debug_assert!(self.gate.validate(&self.args, &self.targets).is_ok());
        let args = &*self.args;
        match self.gate.arg_rule() {
            ArgRule::None => GateArguments::None,
            ArgRule::Any => GateArguments::Coordinates(args),
            ArgRule::ZeroOrOneProbability => GateArguments::OptionalProbability(
                args.first().copied().map(Probability::from_validated),
            ),
            ArgRule::Probability => GateArguments::Probability(Probability::from_validated(
                args.first().copied().unwrap_or_default(),
            )),
            ArgRule::ProbabilityList(_) | ArgRule::AnyProbabilityList => {
                GateArguments::Probabilities(ProbabilityArgs {
                    values: args.iter(),
                })
            }
            ArgRule::UnsignedInteger => GateArguments::Observable(
                ObservableId::from_validated_gate_arg(args.first().copied().unwrap_or_default()),
            ),
        }
    }

    /// Returns this instruction's targets, shaped by its gate's target rule.
    #[inline]
    pub fn gate_targets(&self) -> GateTargets<'_> {
        debug_assert!(self.gate.validate_targets(&self.targets).is_ok());
        let targets = self.targets.as_slice();
        match self.gate.target_rule() {
            GateTargetRule::None => GateTargets::None,
            GateTargetRule::AnySingleQubit => GateTargets::Qubits(QubitIds::new(targets)),
            GateTargetRule::MeasurementQubits => {
                GateTargets::MeasuredQubits(QubitTargets::new(targets))
            }
            GateTargetRule::MeasurementPads => {
                GateTargets::MeasurementPads(PadValues::new(targets))
            }
            GateTargetRule::PlainPairs => GateTargets::QubitPairs(QubitPairs::new(targets)),
            GateTargetRule::ClassicalControlPairs => {
                GateTargets::ControlledPairs(ControlledPairs::new(self.gate, targets))
            }
            GateTargetRule::MeasurementPairs => {
                GateTargets::MeasuredQubitPairs(QubitTargetPairs::new(targets))
            }
            GateTargetRule::RecOnly => GateTargets::Records(RecordTargets::new(targets)),
            GateTargetRule::RecOrPauli => {
                GateTargets::ObservableTargets(ObservableTargets::new(targets))
            }
            GateTargetRule::QubitCoords => GateTargets::QubitCoords(QubitIds::new(targets)),
            GateTargetRule::PauliProducts => GateTargets::PauliProducts(PauliProducts {
                products: ProductSplitter::new(targets),
            }),
            GateTargetRule::PauliList => GateTargets::PauliTerms(PauliTerms::new(targets)),
        }
    }

    /// Returns this instruction's raw target groups, as its gate's [`GateTargetGroupKind`]
    /// splits them, without allocating.
    #[inline]
    pub fn target_groups(&self) -> TargetGroups<'_> {
        let targets = self.targets.as_slice();
        let shape = match self.gate.target_group_kind() {
            GateTargetGroupKind::None => GroupShape::Whole(None),
            GateTargetGroupKind::Singles => GroupShape::Fixed(targets.chunks(1)),
            GateTargetGroupKind::Pairs => GroupShape::Fixed(targets.chunks(2)),
            GateTargetGroupKind::PauliProducts => {
                GroupShape::Products(ProductSplitter::new(targets))
            }
            GateTargetGroupKind::AllTargets => {
                GroupShape::Whole((!targets.is_empty()).then_some(targets))
            }
        };
        TargetGroups { shape }
    }

    /// Returns this instruction's probability when its gate takes exactly one
    /// ([`GateArguments::Probability`]) or an optional one that is present
    /// ([`GateArguments::OptionalProbability`]).
    #[inline]
    pub fn probability(&self) -> Option<Probability> {
        match self.gate_arguments() {
            GateArguments::Probability(probability) => Some(probability),
            GateArguments::OptionalProbability(probability) => probability,
            GateArguments::None
            | GateArguments::Coordinates(_)
            | GateArguments::Probabilities(_)
            | GateArguments::Observable(_) => None,
        }
    }
}
