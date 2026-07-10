#![allow(
    dead_code,
    reason = "PFM-B2 contract groundwork is consumed by generated semantic tests and staged surface migrations"
)]

use std::collections::BTreeMap;

use super::{ArgRule, Gate, GateCategory, GateInfo, TargetRule};
use crate::{Pauli, PauliBasis, PauliPhase, QubitId, Target};

macro_rules! define_gate_contract_enum {
    (
        $(#[$meta:meta])*
        $visibility:vis enum $name:ident {
            $($variant:ident => $wire_name:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        $visibility enum $name {
            $($variant),+
        }

        impl $name {
            pub(super) const ALL: [Self; [$(stringify!($variant)),+].len()] = [
                $(Self::$variant),+
            ];
            pub(super) const NAMES: [&'static str; [$(stringify!($variant)),+].len()] = [
                $($wire_name),+
            ];

            pub(super) const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire_name),+
                }
            }
        }
    };
}

define_gate_contract_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub(super) enum GateSemanticFamily {
        Annotation => "annotation",
        ControlFlow => "control-flow",
        MeasurementPad => "measurement-pad",
        Measurement => "measurement",
        MeasureReset => "measure-reset",
        Reset => "reset",
        FixedTableau => "fixed-tableau",
        ForwardClassicalControl => "forward-classical-control",
        SymmetricClassicalControl => "symmetric-classical-control",
        ReverseClassicalControl => "reverse-classical-control",
        Depolarization => "depolarization",
        PauliNoise => "pauli-noise",
        IdentityNoise => "identity-noise",
        PauliChannel => "pauli-channel",
        CorrelatedError => "correlated-error",
        HeraldedNoise => "heralded-noise",
        PauliProductMeasurement => "pauli-product-measurement",
        PauliProductPhase => "pauli-product-phase",
        PairMeasurement => "pair-measurement",
    }
}

define_gate_contract_enum! {
    #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
    pub(crate) enum GateSurface {
        Parser => "parser",
        MeasurementSampler => "measurement-sampler",
        ReferenceSampler => "reference-sampler",
        DetectionConverter => "detection-converter",
        DetectorFrame => "detector-frame",
        DetectionSampler => "detection-sampler",
        ErrorAnalyzer => "error-analyzer",
        FlowGenerator => "flow-generator",
    }
}

#[cfg(feature = "ops-contracts")]
pub(super) fn gate_contract_family_names() -> &'static [&'static str] {
    &GateSemanticFamily::NAMES
}

#[cfg(feature = "ops-contracts")]
pub(super) fn gate_contract_surface_names() -> &'static [&'static str] {
    &GateSurface::NAMES
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum GateSurfaceBehavior {
    Execute,
    SemanticNoop,
    Annotation,
    LowerThenExecute,
    UnsupportedShape,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum GateTargetPattern {
    NoTargets,
    EmptyTargetList,
    PlainQubit,
    MeasurementQubit,
    MeasurementPad,
    PlainQubitPair,
    MeasurementQubitPair,
    DetectorDeclaration,
    ObservableDeclaration,
    QubitCoordinates,
    HermitianPauliProduct,
    AntiHermitianPauliProduct,
    PauliList,
    QubitQubit,
    RecordQubit,
    SweepQubit,
    QubitRecord,
    QubitSweep,
    RecordRecord,
    RecordSweep,
    SweepRecord,
    SweepSweep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GateShapeExclusion {
    AntiHermitianPauliProduct,
    QuantumCannotControlClassical,
    ClassicalOnlyPairRequiresSymmetricCz,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GateSurfaceDecision {
    pub(crate) behavior: GateSurfaceBehavior,
    pub(crate) exclusion: Option<GateShapeExclusion>,
}

impl GateSurfaceDecision {
    const fn new(behavior: GateSurfaceBehavior) -> Self {
        Self {
            behavior,
            exclusion: None,
        }
    }

    const fn unsupported(exclusion: GateShapeExclusion) -> Self {
        Self {
            behavior: GateSurfaceBehavior::UnsupportedShape,
            exclusion: Some(exclusion),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GateSurfaceContract {
    gate: Gate,
}

impl GateSurfaceContract {
    pub(crate) fn gate(self) -> Gate {
        self.gate
    }

    pub(crate) fn target_patterns(self) -> &'static [GateTargetPattern] {
        target_patterns(self.gate.info.target_rule)
    }

    pub(crate) fn decision(
        self,
        surface: GateSurface,
        pattern: GateTargetPattern,
    ) -> Option<GateSurfaceDecision> {
        if !self.target_patterns().contains(&pattern) {
            return None;
        }
        decision_for(self.gate.info.semantic_family, surface, pattern)
    }

    pub(crate) fn classify_target_groups(
        self,
        targets: &[Target],
    ) -> Option<Vec<GateTargetPattern>> {
        self.gate
            .info
            .target_rule
            .validate(self.gate.info.name, targets)
            .ok()?;
        classify_target_groups(self.gate.info.target_rule, targets)
    }
}

impl Gate {
    pub(crate) fn surface_contract(self) -> GateSurfaceContract {
        GateSurfaceContract { gate: self }
    }
}

const NO_TARGETS: &[GateTargetPattern] = &[GateTargetPattern::NoTargets];
const PLAIN_QUBIT: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::PlainQubit,
];
const MEASUREMENT_QUBIT: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::MeasurementQubit,
];
const MEASUREMENT_PAD: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::MeasurementPad,
];
const PLAIN_QUBIT_PAIR: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::PlainQubitPair,
];
const MEASUREMENT_QUBIT_PAIR: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::MeasurementQubitPair,
];
const DETECTOR_DECLARATION: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::DetectorDeclaration,
];
const OBSERVABLE_DECLARATION: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::ObservableDeclaration,
];
const QUBIT_COORDINATES: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::QubitCoordinates,
];
const PAULI_PRODUCTS: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::HermitianPauliProduct,
    GateTargetPattern::AntiHermitianPauliProduct,
];
const PAULI_LIST: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::PauliList,
];
const CLASSICAL_CONTROL_PAIRS: &[GateTargetPattern] = &[
    GateTargetPattern::EmptyTargetList,
    GateTargetPattern::QubitQubit,
    GateTargetPattern::RecordQubit,
    GateTargetPattern::SweepQubit,
    GateTargetPattern::QubitRecord,
    GateTargetPattern::QubitSweep,
    GateTargetPattern::RecordRecord,
    GateTargetPattern::RecordSweep,
    GateTargetPattern::SweepRecord,
    GateTargetPattern::SweepSweep,
];

const fn target_patterns(rule: TargetRule) -> &'static [GateTargetPattern] {
    match rule {
        TargetRule::None => NO_TARGETS,
        TargetRule::AnySingleQubit => PLAIN_QUBIT,
        TargetRule::MeasurementQubits => MEASUREMENT_QUBIT,
        TargetRule::MeasurementPads => MEASUREMENT_PAD,
        TargetRule::PlainPairs => PLAIN_QUBIT_PAIR,
        TargetRule::ClassicalControlPairs => CLASSICAL_CONTROL_PAIRS,
        TargetRule::MeasurementPairs => MEASUREMENT_QUBIT_PAIR,
        TargetRule::RecOnly => DETECTOR_DECLARATION,
        TargetRule::RecOrPauli => OBSERVABLE_DECLARATION,
        TargetRule::QubitCoords => QUBIT_COORDINATES,
        TargetRule::PauliProducts => PAULI_PRODUCTS,
        TargetRule::PauliList => PAULI_LIST,
    }
}

const fn decision_for(
    family: GateSemanticFamily,
    surface: GateSurface,
    pattern: GateTargetPattern,
) -> Option<GateSurfaceDecision> {
    if matches!(surface, GateSurface::Parser) {
        return Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute));
    }
    if matches!(pattern, GateTargetPattern::EmptyTargetList) {
        return Some(match family {
            GateSemanticFamily::Annotation => {
                GateSurfaceDecision::new(GateSurfaceBehavior::Annotation)
            }
            GateSemanticFamily::ControlFlow => {
                GateSurfaceDecision::new(GateSurfaceBehavior::NotApplicable)
            }
            GateSemanticFamily::CorrelatedError => stochastic_noise_decision(surface),
            GateSemanticFamily::MeasurementPad
            | GateSemanticFamily::Measurement
            | GateSemanticFamily::MeasureReset
            | GateSemanticFamily::Reset
            | GateSemanticFamily::FixedTableau
            | GateSemanticFamily::ForwardClassicalControl
            | GateSemanticFamily::SymmetricClassicalControl
            | GateSemanticFamily::ReverseClassicalControl
            | GateSemanticFamily::Depolarization
            | GateSemanticFamily::PauliNoise
            | GateSemanticFamily::IdentityNoise
            | GateSemanticFamily::PauliChannel
            | GateSemanticFamily::HeraldedNoise
            | GateSemanticFamily::PauliProductMeasurement
            | GateSemanticFamily::PauliProductPhase
            | GateSemanticFamily::PairMeasurement => {
                GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop)
            }
        });
    }
    if matches!(pattern, GateTargetPattern::AntiHermitianPauliProduct) {
        return match family {
            GateSemanticFamily::PauliProductMeasurement | GateSemanticFamily::PauliProductPhase => {
                Some(GateSurfaceDecision::unsupported(
                    GateShapeExclusion::AntiHermitianPauliProduct,
                ))
            }
            _ => None,
        };
    }
    match family {
        GateSemanticFamily::Annotation => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::Annotation))
        }
        GateSemanticFamily::ControlFlow => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::NotApplicable))
        }
        GateSemanticFamily::PauliProductPhase => Some(GateSurfaceDecision::new(
            GateSurfaceBehavior::LowerThenExecute,
        )),
        GateSemanticFamily::IdentityNoise => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop))
        }
        GateSemanticFamily::Depolarization
        | GateSemanticFamily::PauliNoise
        | GateSemanticFamily::PauliChannel
        | GateSemanticFamily::CorrelatedError => Some(stochastic_noise_decision(surface)),
        GateSemanticFamily::ForwardClassicalControl
        | GateSemanticFamily::SymmetricClassicalControl
        | GateSemanticFamily::ReverseClassicalControl => {
            classical_control_decision(family, surface, pattern)
        }
        GateSemanticFamily::MeasurementPad
        | GateSemanticFamily::Measurement
        | GateSemanticFamily::MeasureReset
        | GateSemanticFamily::Reset
        | GateSemanticFamily::FixedTableau
        | GateSemanticFamily::HeraldedNoise
        | GateSemanticFamily::PauliProductMeasurement
        | GateSemanticFamily::PairMeasurement => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute))
        }
    }
}

const fn stochastic_noise_decision(surface: GateSurface) -> GateSurfaceDecision {
    match surface {
        GateSurface::Parser
        | GateSurface::MeasurementSampler
        | GateSurface::DetectorFrame
        | GateSurface::DetectionSampler
        | GateSurface::ErrorAnalyzer => GateSurfaceDecision::new(GateSurfaceBehavior::Execute),
        GateSurface::ReferenceSampler
        | GateSurface::DetectionConverter
        | GateSurface::FlowGenerator => GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop),
    }
}

const fn classical_control_decision(
    family: GateSemanticFamily,
    surface: GateSurface,
    pattern: GateTargetPattern,
) -> Option<GateSurfaceDecision> {
    if matches!(pattern, GateTargetPattern::QubitQubit) {
        return Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute));
    }
    if matches!(surface, GateSurface::FlowGenerator) {
        match pattern {
            GateTargetPattern::RecordQubit | GateTargetPattern::QubitRecord => {
                return Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute));
            }
            GateTargetPattern::SweepQubit | GateTargetPattern::QubitSweep => {
                return Some(GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop));
            }
            GateTargetPattern::RecordRecord
            | GateTargetPattern::RecordSweep
            | GateTargetPattern::SweepRecord
            | GateTargetPattern::SweepSweep => {
                return Some(GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop));
            }
            _ => {}
        }
    }
    match family {
        GateSemanticFamily::ForwardClassicalControl => {
            forward_classical_control_decision(surface, pattern)
        }
        GateSemanticFamily::SymmetricClassicalControl => {
            symmetric_classical_control_decision(surface, pattern)
        }
        GateSemanticFamily::ReverseClassicalControl => {
            reverse_classical_control_decision(surface, pattern)
        }
        GateSemanticFamily::Annotation
        | GateSemanticFamily::ControlFlow
        | GateSemanticFamily::MeasurementPad
        | GateSemanticFamily::Measurement
        | GateSemanticFamily::MeasureReset
        | GateSemanticFamily::Reset
        | GateSemanticFamily::FixedTableau
        | GateSemanticFamily::Depolarization
        | GateSemanticFamily::PauliNoise
        | GateSemanticFamily::IdentityNoise
        | GateSemanticFamily::PauliChannel
        | GateSemanticFamily::CorrelatedError
        | GateSemanticFamily::HeraldedNoise
        | GateSemanticFamily::PauliProductMeasurement
        | GateSemanticFamily::PauliProductPhase
        | GateSemanticFamily::PairMeasurement => None,
    }
}

const fn forward_classical_control_decision(
    surface: GateSurface,
    pattern: GateTargetPattern,
) -> Option<GateSurfaceDecision> {
    match pattern {
        GateTargetPattern::RecordQubit => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute))
        }
        GateTargetPattern::SweepQubit => Some(sweep_control_decision(surface)),
        GateTargetPattern::QubitRecord | GateTargetPattern::QubitSweep => Some(
            GateSurfaceDecision::unsupported(GateShapeExclusion::QuantumCannotControlClassical),
        ),
        GateTargetPattern::RecordRecord
        | GateTargetPattern::RecordSweep
        | GateTargetPattern::SweepRecord
        | GateTargetPattern::SweepSweep => Some(GateSurfaceDecision::unsupported(
            GateShapeExclusion::ClassicalOnlyPairRequiresSymmetricCz,
        )),
        GateTargetPattern::QubitQubit
        | GateTargetPattern::NoTargets
        | GateTargetPattern::EmptyTargetList
        | GateTargetPattern::PlainQubit
        | GateTargetPattern::MeasurementQubit
        | GateTargetPattern::MeasurementPad
        | GateTargetPattern::PlainQubitPair
        | GateTargetPattern::MeasurementQubitPair
        | GateTargetPattern::DetectorDeclaration
        | GateTargetPattern::ObservableDeclaration
        | GateTargetPattern::QubitCoordinates
        | GateTargetPattern::HermitianPauliProduct
        | GateTargetPattern::AntiHermitianPauliProduct
        | GateTargetPattern::PauliList => None,
    }
}

const fn symmetric_classical_control_decision(
    surface: GateSurface,
    pattern: GateTargetPattern,
) -> Option<GateSurfaceDecision> {
    match pattern {
        GateTargetPattern::RecordQubit | GateTargetPattern::QubitRecord => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute))
        }
        GateTargetPattern::SweepQubit | GateTargetPattern::QubitSweep => {
            Some(sweep_control_decision(surface))
        }
        GateTargetPattern::RecordRecord
        | GateTargetPattern::RecordSweep
        | GateTargetPattern::SweepRecord
        | GateTargetPattern::SweepSweep => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop))
        }
        GateTargetPattern::QubitQubit
        | GateTargetPattern::NoTargets
        | GateTargetPattern::EmptyTargetList
        | GateTargetPattern::PlainQubit
        | GateTargetPattern::MeasurementQubit
        | GateTargetPattern::MeasurementPad
        | GateTargetPattern::PlainQubitPair
        | GateTargetPattern::MeasurementQubitPair
        | GateTargetPattern::DetectorDeclaration
        | GateTargetPattern::ObservableDeclaration
        | GateTargetPattern::QubitCoordinates
        | GateTargetPattern::HermitianPauliProduct
        | GateTargetPattern::AntiHermitianPauliProduct
        | GateTargetPattern::PauliList => None,
    }
}

const fn reverse_classical_control_decision(
    surface: GateSurface,
    pattern: GateTargetPattern,
) -> Option<GateSurfaceDecision> {
    match pattern {
        GateTargetPattern::QubitRecord => {
            Some(GateSurfaceDecision::new(GateSurfaceBehavior::Execute))
        }
        GateTargetPattern::QubitSweep => Some(sweep_control_decision(surface)),
        GateTargetPattern::RecordQubit | GateTargetPattern::SweepQubit => Some(
            GateSurfaceDecision::unsupported(GateShapeExclusion::QuantumCannotControlClassical),
        ),
        GateTargetPattern::RecordRecord
        | GateTargetPattern::RecordSweep
        | GateTargetPattern::SweepRecord
        | GateTargetPattern::SweepSweep => Some(GateSurfaceDecision::unsupported(
            GateShapeExclusion::ClassicalOnlyPairRequiresSymmetricCz,
        )),
        GateTargetPattern::QubitQubit
        | GateTargetPattern::NoTargets
        | GateTargetPattern::EmptyTargetList
        | GateTargetPattern::PlainQubit
        | GateTargetPattern::MeasurementQubit
        | GateTargetPattern::MeasurementPad
        | GateTargetPattern::PlainQubitPair
        | GateTargetPattern::MeasurementQubitPair
        | GateTargetPattern::DetectorDeclaration
        | GateTargetPattern::ObservableDeclaration
        | GateTargetPattern::QubitCoordinates
        | GateTargetPattern::HermitianPauliProduct
        | GateTargetPattern::AntiHermitianPauliProduct
        | GateTargetPattern::PauliList => None,
    }
}

const fn sweep_control_decision(surface: GateSurface) -> GateSurfaceDecision {
    match surface {
        GateSurface::Parser | GateSurface::ReferenceSampler | GateSurface::DetectionConverter => {
            GateSurfaceDecision::new(GateSurfaceBehavior::Execute)
        }
        GateSurface::MeasurementSampler
        | GateSurface::DetectorFrame
        | GateSurface::DetectionSampler
        | GateSurface::ErrorAnalyzer
        | GateSurface::FlowGenerator => GateSurfaceDecision::new(GateSurfaceBehavior::SemanticNoop),
    }
}

fn classify_target_groups(rule: TargetRule, targets: &[Target]) -> Option<Vec<GateTargetPattern>> {
    if targets.is_empty() {
        return Some(vec![match rule {
            TargetRule::None => GateTargetPattern::NoTargets,
            TargetRule::AnySingleQubit
            | TargetRule::MeasurementQubits
            | TargetRule::MeasurementPads
            | TargetRule::PlainPairs
            | TargetRule::ClassicalControlPairs
            | TargetRule::MeasurementPairs
            | TargetRule::RecOnly
            | TargetRule::RecOrPauli
            | TargetRule::QubitCoords
            | TargetRule::PauliProducts
            | TargetRule::PauliList => GateTargetPattern::EmptyTargetList,
        }]);
    }
    Some(match rule {
        TargetRule::None => return None,
        TargetRule::AnySingleQubit => vec![GateTargetPattern::PlainQubit],
        TargetRule::MeasurementQubits => vec![GateTargetPattern::MeasurementQubit],
        TargetRule::MeasurementPads => vec![GateTargetPattern::MeasurementPad],
        TargetRule::PlainPairs => vec![GateTargetPattern::PlainQubitPair],
        TargetRule::ClassicalControlPairs => {
            let mut patterns = Vec::new();
            for pair in targets.chunks_exact(2) {
                if let [left, right] = pair {
                    push_unique(&mut patterns, classical_pair_pattern(left, right)?);
                }
            }
            patterns
        }
        TargetRule::MeasurementPairs => vec![GateTargetPattern::MeasurementQubitPair],
        TargetRule::RecOnly => vec![GateTargetPattern::DetectorDeclaration],
        TargetRule::RecOrPauli => vec![GateTargetPattern::ObservableDeclaration],
        TargetRule::QubitCoords => vec![GateTargetPattern::QubitCoordinates],
        TargetRule::PauliProducts => classify_pauli_products(targets)?,
        TargetRule::PauliList => vec![GateTargetPattern::PauliList],
    })
}

fn push_unique(patterns: &mut Vec<GateTargetPattern>, pattern: GateTargetPattern) {
    if !patterns.contains(&pattern) {
        patterns.push(pattern);
    }
}

fn classical_pair_pattern(left: &Target, right: &Target) -> Option<GateTargetPattern> {
    Some(match (target_role(left)?, target_role(right)?) {
        (ClassicalTargetRole::Qubit, ClassicalTargetRole::Qubit) => GateTargetPattern::QubitQubit,
        (ClassicalTargetRole::Record, ClassicalTargetRole::Qubit) => GateTargetPattern::RecordQubit,
        (ClassicalTargetRole::Sweep, ClassicalTargetRole::Qubit) => GateTargetPattern::SweepQubit,
        (ClassicalTargetRole::Qubit, ClassicalTargetRole::Record) => GateTargetPattern::QubitRecord,
        (ClassicalTargetRole::Qubit, ClassicalTargetRole::Sweep) => GateTargetPattern::QubitSweep,
        (ClassicalTargetRole::Record, ClassicalTargetRole::Record) => {
            GateTargetPattern::RecordRecord
        }
        (ClassicalTargetRole::Record, ClassicalTargetRole::Sweep) => GateTargetPattern::RecordSweep,
        (ClassicalTargetRole::Sweep, ClassicalTargetRole::Record) => GateTargetPattern::SweepRecord,
        (ClassicalTargetRole::Sweep, ClassicalTargetRole::Sweep) => GateTargetPattern::SweepSweep,
    })
}

#[derive(Clone, Copy)]
enum ClassicalTargetRole {
    Qubit,
    Record,
    Sweep,
}

fn target_role(target: &Target) -> Option<ClassicalTargetRole> {
    if target.is_measurement_record_target() {
        Some(ClassicalTargetRole::Record)
    } else if target.is_sweep_bit_target() {
        Some(ClassicalTargetRole::Sweep)
    } else if target.is_qubit_target() {
        Some(ClassicalTargetRole::Qubit)
    } else {
        None
    }
}

fn classify_pauli_products(targets: &[Target]) -> Option<Vec<GateTargetPattern>> {
    let mut patterns = Vec::new();
    let mut start = 0;
    while start < targets.len() {
        let mut end = start + 1;
        while end < targets.len() && targets.get(end).is_some_and(Target::is_combiner) {
            end = end.checked_add(2)?;
        }
        let group = targets.get(start..end)?;
        push_unique(&mut patterns, classify_pauli_product(group)?);
        start = end;
    }
    Some(patterns)
}

fn classify_pauli_product(group: &[Target]) -> Option<GateTargetPattern> {
    let mut terms = BTreeMap::<QubitId, PauliBasis>::new();
    let mut phase = PauliPhase::Plus;
    for target in group {
        if target.is_combiner() {
            continue;
        }
        let (Some(qubit), Some(pauli)) = (target.qubit_id(), target.pauli_type()) else {
            return None;
        };
        if target.is_inverted_result_target() {
            phase = phase.multiply(PauliPhase::Minus);
        }
        let basis = match pauli {
            Pauli::X => PauliBasis::X,
            Pauli::Y => PauliBasis::Y,
            Pauli::Z => PauliBasis::Z,
        };
        if let Some(previous) = terms.remove(&qubit) {
            let (product, product_phase) = previous.multiply(basis);
            phase = phase.multiply(product_phase);
            if product != PauliBasis::I {
                terms.insert(qubit, product);
            }
        } else {
            terms.insert(qubit, basis);
        }
    }
    Some(if phase.is_real() {
        GateTargetPattern::HermitianPauliProduct
    } else {
        GateTargetPattern::AntiHermitianPauliProduct
    })
}

pub(super) const fn gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    gate_with_inverse(name, name, category, arg_rule, target_rule)
}

pub(super) const fn gate_with_inverse(
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    semantic_gate_with_inverse(
        name,
        inverse_name,
        category,
        arg_rule,
        target_rule,
        default_semantic_family(category),
    )
}

pub(super) const fn semantic_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    semantic_gate_with_inverse(name, name, category, arg_rule, target_rule, semantic_family)
}

pub(super) const fn semantic_gate_with_inverse(
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name,
        category,
        arg_rule,
        target_rule,
        semantic_family,
        can_fuse: true,
    }
}

pub(super) const fn not_fusable_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    semantic_not_fusable_gate(
        name,
        category,
        arg_rule,
        target_rule,
        default_semantic_family(category),
    )
}

pub(super) const fn semantic_not_fusable_gate(
    name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
) -> GateInfo {
    GateInfo {
        name,
        inverse_name: name,
        category,
        arg_rule,
        target_rule,
        semantic_family,
        can_fuse: false,
    }
}

#[allow(
    clippy::panic,
    reason = "ambiguous categories must choose an explicit semantic family in the canonical gate table"
)]
const fn default_semantic_family(category: GateCategory) -> GateSemanticFamily {
    match category {
        GateCategory::Annotation => GateSemanticFamily::Annotation,
        GateCategory::ControlFlow => GateSemanticFamily::ControlFlow,
        GateCategory::Controlled
        | GateCategory::HadamardLike
        | GateCategory::Pauli
        | GateCategory::Period3
        | GateCategory::Period4
        | GateCategory::ParityPhasing
        | GateCategory::Swap => GateSemanticFamily::FixedTableau,
        GateCategory::HeraldedNoise => GateSemanticFamily::HeraldedNoise,
        GateCategory::PairMeasurement => GateSemanticFamily::PairMeasurement,
        GateCategory::Collapsing | GateCategory::Noise | GateCategory::PauliProduct => {
            panic!("ambiguous gate category requires an explicit semantic family")
        }
    }
}

#[cfg(test)]
mod tests;
