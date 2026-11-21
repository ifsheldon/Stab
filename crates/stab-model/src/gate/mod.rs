mod decomposition;
mod flows;
mod kind;
mod metadata;
mod table;
mod unitary;

pub use decomposition::GateDecomposition;
pub(crate) use decomposition::gate_decomposition_text;
pub(crate) use flows::gate_flow_descriptors;
pub use kind::GateKind;
pub use metadata::{GateArgumentRule, GateTargetGroupKind, GateTargetRule};
use table::GATES;
pub use unitary::GateUnitaryRows;
pub(crate) use unitary::gate_unitary_rows;

use crate::{ModelResult, Pauli, Probability, Target, TargetKind, ValidationError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateCategory {
    Annotation,
    ControlFlow,
    Collapsing,
    Controlled,
    HadamardLike,
    Noise,
    HeraldedNoise,
    Pauli,
    Period3,
    Period4,
    ParityPhasing,
    PauliProduct,
    Swap,
    PairMeasurement,
}

/// A parsed Stim gate: a handle to its row in the one Stim v1.16.0 gate table.
///
/// [`Gate::kind`] returns the closed [`GateKind`] identity for exhaustive per-gate matches, and
/// the classification accessors read the same table row.
#[derive(Clone, Copy, Debug)]
pub struct Gate {
    info: &'static GateInfo,
}

/// Gates are equal exactly when they are the same table row, which their [`GateKind`] names, so
/// equality compares that identity instead of every field of the row.
impl PartialEq for Gate {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.kind() == other.kind()
    }
}

impl Eq for Gate {}

impl Gate {
    const fn from_info(info: &'static GateInfo) -> Self {
        Self { info }
    }

    /// Iterates over the canonical gates defined by Stim v1.16.0, in [`GateKind`] order.
    #[inline]
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        GateKind::ALL.into_iter().map(GateKind::gate)
    }

    #[inline]
    pub fn from_name(name: &str) -> ModelResult<Self> {
        Self::lookup_name(name).ok_or_else(|| ValidationError::UnknownGate(name.to_string()).into())
    }

    #[inline]
    pub(crate) fn lookup_name(name: &str) -> Option<Self> {
        gate_kind_from_name(name).map(GateKind::gate)
    }

    #[inline]
    pub(crate) fn from_simple_plain_name(name: &str) -> Option<Self> {
        gate_kind_from_simple_plain_name(name).map(GateKind::gate)
    }

    pub(crate) const DETECTOR: Self = GateKind::DETECTOR.gate();
    pub(crate) const TICK: Self = GateKind::TICK.gate();
    pub(crate) const SHIFT_COORDS: Self = GateKind::SHIFT_COORDS.gate();
    pub(crate) const H: Self = GateKind::H.gate();
    pub(crate) const S: Self = GateKind::S.gate();
    pub(crate) const M: Self = GateKind::M.gate();
    pub(crate) const CX: Self = GateKind::CX.gate();

    /// Returns this gate's closed identity.
    #[inline]
    pub fn kind(self) -> GateKind {
        self.info.kind
    }

    #[inline]
    pub fn canonical_name(self) -> &'static str {
        self.info.name
    }

    #[inline]
    pub fn category(self) -> GateCategory {
        self.info.category
    }

    /// Validates arguments and targets against this gate's closed syntax descriptor.
    ///
    /// # Errors
    ///
    /// Fails when the gate is block-only control flow, or when the arguments or targets break
    /// the gate's argument or target rule.
    pub fn validate(self, args: &[f64], targets: &[Target]) -> ModelResult<()> {
        // Block-only control-flow gates such as REPEAT exist in the gate table for name
        // resolution but must never become ordinary instructions; pinned Stim rejects a
        // braceless REPEAT at parse time and blocks never validate as instructions.
        if self.category() == GateCategory::ControlFlow {
            return Err(crate::ModelError::invalid_domain_value(
                "instruction gate",
                self.info.name,
            ));
        }
        self.info.arg_rule.validate(self.info.name, args)?;
        self.info.target_rule.validate(self.info.name, targets)
    }

    /// Validates only targets against this gate's closed syntax descriptor.
    ///
    /// # Errors
    ///
    /// Fails when the targets break the gate's target rule.
    pub fn validate_targets(self, targets: &[Target]) -> ModelResult<()> {
        self.info.target_rule.validate(self.info.name, targets)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct GateInfo {
    kind: GateKind,
    name: &'static str,
    /// Stim's best candidate inverse, which for non-unitary gates is a generalized inverse.
    inverse: GateKind,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    can_fuse: bool,
    flags: GateFlags,
    /// All accepted names in Stim v1.16.0 alias order; empty means the canonical name only.
    aliases: &'static [&'static str],
    /// The Pauli a controlled-Pauli gate applies to its target when the control is set, for
    /// the gates that accept measurement-record and sweep-bit controls.
    controlled_pauli: Option<Pauli>,
    /// The basis of a single-qubit measurement, reset, or measure-reset gate.
    collapse_basis: Option<Pauli>,
}

impl GateInfo {
    const fn with_flags(mut self, flags: GateFlags) -> Self {
        self.flags = flags;
        self
    }

    const fn with_aliases(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }

    const fn with_collapse_basis(mut self, basis: Pauli) -> Self {
        self.collapse_basis = Some(basis);
        self
    }

    const fn with_controlled_pauli(mut self, pauli: Pauli) -> Self {
        self.controlled_pauli = Some(pauli);
        self
    }
}

/// Stim-v1.16.0-style per-gate classification flags stored in the gate table.
///
/// Only classifications that cannot be derived from the gate's category, argument rule, or
/// target rule are stored here; everything else stays derived so each fact has one owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GateFlags(u8);

impl GateFlags {
    const NONE: Self = Self(0);
    /// Mirrors Stim's `GATE_PRODUCES_RESULTS`: the gate appends measurement records.
    const PRODUCES_RESULTS: Self = Self(1 << 0);
    /// Mirrors Stim's `GATE_IS_RESET`: reset or measure-reset gates.
    const IS_RESET: Self = Self(1 << 1);
    /// Exchange-symmetric two-qubit gate (Stim derives this set in `Gate::is_symmetric`).
    const SYMMETRIC_PAIR: Self = Self(1 << 2);

    const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ArgRule {
    /// No arguments; reported publicly as [`GateArgumentRule::Exact`]`(0)`.
    None,
    Any,
    ZeroOrOneProbability,
    /// Exactly one probability; reported publicly as [`GateArgumentRule::ProbabilityList`]`(1)`.
    Probability,
    /// Exactly this many (at least two) disjoint probabilities.
    ProbabilityList(usize),
    AnyProbabilityList,
    UnsignedInteger,
}

impl ArgRule {
    fn validate(self, gate: &'static str, args: &[f64]) -> ModelResult<()> {
        match self {
            Self::None if !args.is_empty() => Err(ValidationError::InvalidArgumentCount {
                gate,
                expected: "0",
                actual: args.len(),
            }
            .into()),
            Self::None => Ok(()),
            Self::Any => {
                for arg in args {
                    validate_finite_arg(gate, *arg)?;
                }
                Ok(())
            }
            Self::ZeroOrOneProbability => {
                if args.len() > 1 {
                    return Err(ValidationError::InvalidArgumentCount {
                        gate,
                        expected: "0 or 1",
                        actual: args.len(),
                    }
                    .into());
                }
                if let Some(arg) = args.first().copied() {
                    Probability::try_new(arg).map_err(|_| {
                        crate::ModelError::from(ValidationError::InvalidArgument {
                            gate,
                            argument: arg.to_string(),
                        })
                    })?;
                }
                Ok(())
            }
            Self::Probability => validate_probability_count(gate, args, 1),
            Self::ProbabilityList(expected) => validate_probability_count(gate, args, expected),
            Self::AnyProbabilityList => validate_probability_list(gate, args),
            Self::UnsignedInteger => {
                if args.len() != 1 {
                    return Err(ValidationError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    }
                    .into());
                }
                let Some(arg) = args.first().copied() else {
                    return Err(ValidationError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    }
                    .into());
                };
                // The only unsigned-integer argument is an observable id, so admit exactly the
                // ids a detector error model can hold.
                if crate::ObservableId::from_gate_arg(arg).is_none() {
                    return Err(ValidationError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    }
                    .into());
                }
                Ok(())
            }
        }
    }
}

fn validate_probability_count(
    gate: &'static str,
    args: &[f64],
    expected: usize,
) -> ModelResult<()> {
    if args.len() != expected {
        return Err(ValidationError::InvalidArgumentCount {
            gate,
            expected: "probability list",
            actual: args.len(),
        }
        .into());
    }
    validate_probability_list(gate, args)
}

fn validate_probability_list(gate: &'static str, args: &[f64]) -> ModelResult<()> {
    let mut total = 0.0;
    for arg in args {
        Probability::try_new(*arg).map_err(|_| {
            crate::ModelError::from(ValidationError::InvalidArgument {
                gate,
                argument: arg.to_string(),
            })
        })?;
        total += *arg;
    }
    if total > 1.0000001 {
        return Err(ValidationError::InvalidArgument {
            gate,
            argument: format!("sum {total}"),
        }
        .into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TargetRule {
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

impl TargetRule {
    fn validate(self, gate: &'static str, targets: &[Target]) -> ModelResult<()> {
        match self {
            Self::None => {
                if targets.is_empty() {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidTargetCount {
                        gate,
                        count: targets.len(),
                    }
                    .into())
                }
            }
            Self::AnySingleQubit => validate_targets(gate, targets, is_plain_qubit_target),
            Self::MeasurementQubits => validate_targets(gate, targets, Target::is_qubit_target),
            Self::MeasurementPads => validate_targets(gate, targets, is_measurement_pad_target),
            Self::PlainPairs => validate_pair_targets(gate, targets, is_plain_qubit_target),
            Self::ClassicalControlPairs => {
                validate_pair_targets(gate, targets, is_plain_qubit_or_classical_target)
            }
            Self::MeasurementPairs => validate_pair_targets(gate, targets, Target::is_qubit_target),
            Self::RecOnly => validate_targets(gate, targets, Target::is_measurement_record_target),
            Self::RecOrPauli => validate_targets(gate, targets, |target| {
                target.is_measurement_record_target() || target.is_pauli_target()
            }),
            Self::QubitCoords => validate_targets(gate, targets, is_plain_qubit_target),
            Self::PauliProducts => {
                validate_targets(gate, targets, |target| {
                    target.is_pauli_target() || target.is_combiner()
                })?;
                validate_combiners(gate, targets)
            }
            Self::PauliList => validate_targets(gate, targets, |target| {
                // Pinned Stim's correlated-error handlers consult only the
                // Pauli X/Z bits, so combiner targets and inversion bits are
                // accepted as ignored decoration (frame_simulator.inl:767-775).
                matches!(
                    target.kind(),
                    TargetKind::Pauli { .. } | TargetKind::Combiner
                )
            }),
        }
    }

    fn target_group_kind(self) -> GateTargetGroupKind {
        match self {
            Self::None => GateTargetGroupKind::None,
            Self::AnySingleQubit
            | Self::MeasurementQubits
            | Self::MeasurementPads
            | Self::RecOnly
            | Self::RecOrPauli
            | Self::QubitCoords => GateTargetGroupKind::Singles,
            Self::PlainPairs | Self::ClassicalControlPairs | Self::MeasurementPairs => {
                GateTargetGroupKind::Pairs
            }
            Self::PauliProducts => GateTargetGroupKind::PauliProducts,
            Self::PauliList => GateTargetGroupKind::AllTargets,
        }
    }
}

fn is_plain_qubit_target(target: &Target) -> bool {
    matches!(
        target.kind(),
        TargetKind::Qubit {
            inverted: false,
            ..
        }
    )
}

fn is_plain_qubit_or_classical_target(target: &Target) -> bool {
    is_plain_qubit_target(target) || target.is_classical_bit_target()
}

fn is_measurement_pad_target(target: &Target) -> bool {
    matches!(target.kind(), TargetKind::Qubit { id, inverted: false } if id.get() <= 1)
}

fn validate_pair_targets(
    gate: &'static str,
    targets: &[Target],
    predicate: impl Fn(&Target) -> bool,
) -> ModelResult<()> {
    if !targets.len().is_multiple_of(2) {
        return Err(ValidationError::InvalidTargetCount {
            gate,
            count: targets.len(),
        }
        .into());
    }
    validate_targets(gate, targets, predicate)?;
    for [left, right] in targets.as_chunks::<2>().0 {
        if left == right {
            return Err(ValidationError::InvalidTarget {
                gate,
                target: left.to_string(),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_targets(
    gate: &'static str,
    targets: &[Target],
    predicate: impl Fn(&Target) -> bool,
) -> ModelResult<()> {
    for target in targets {
        if !predicate(target) {
            return Err(ValidationError::InvalidTarget {
                gate,
                target: target.to_string(),
            }
            .into());
        }
    }
    Ok(())
}

fn validate_combiners(gate: &'static str, targets: &[Target]) -> ModelResult<()> {
    let mut previous_was_combiner = true;
    for target in targets {
        if target.is_combiner() {
            if previous_was_combiner {
                return Err(ValidationError::InvalidTarget {
                    gate,
                    target: target.to_string(),
                }
                .into());
            }
            previous_was_combiner = true;
        } else {
            previous_was_combiner = false;
        }
    }
    if previous_was_combiner && !targets.is_empty() {
        return Err(ValidationError::InvalidTarget {
            gate,
            target: "*".to_string(),
        }
        .into());
    }
    Ok(())
}

fn validate_finite_arg(gate: &'static str, arg: f64) -> ModelResult<()> {
    if arg.is_finite() {
        Ok(())
    } else {
        Err(ValidationError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        }
        .into())
    }
}

#[inline]
fn gate_kind_from_name(name: &str) -> Option<GateKind> {
    if let Some(kind) = gate_kind_from_uppercase_name(name) {
        return Some(kind);
    }
    if !name.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return None;
    }
    let uppercase = name.to_ascii_uppercase();
    gate_kind_from_uppercase_name(&uppercase)
}

#[inline]
fn gate_kind_from_simple_plain_name(name: &str) -> Option<GateKind> {
    Some(match name {
        "M" | "MZ" => GateKind::M,
        "CX" | "CNOT" => GateKind::CX,
        "H" => GateKind::H,
        "S" | "SQRT_Z" => GateKind::S,
        _ if name.eq_ignore_ascii_case("M") || name.eq_ignore_ascii_case("MZ") => GateKind::M,
        _ if name.eq_ignore_ascii_case("CX") || name.eq_ignore_ascii_case("CNOT") => GateKind::CX,
        _ if name.eq_ignore_ascii_case("H") => GateKind::H,
        _ if name.eq_ignore_ascii_case("S") || name.eq_ignore_ascii_case("SQRT_Z") => GateKind::S,
        _ => return None,
    })
}

#[inline]
fn gate_kind_from_uppercase_name(name: &str) -> Option<GateKind> {
    Some(match name {
        "DETECTOR" => GateKind::DETECTOR,
        "OBSERVABLE_INCLUDE" => GateKind::OBSERVABLE_INCLUDE,
        "TICK" => GateKind::TICK,
        "QUBIT_COORDS" => GateKind::QUBIT_COORDS,
        "SHIFT_COORDS" => GateKind::SHIFT_COORDS,
        "REPEAT" => GateKind::REPEAT,
        "MPAD" => GateKind::MPAD,
        "MX" => GateKind::MX,
        "MY" => GateKind::MY,
        "M" | "MZ" => GateKind::M,
        "MRX" => GateKind::MRX,
        "MRY" => GateKind::MRY,
        "MR" | "MRZ" => GateKind::MR,
        "RX" => GateKind::RX,
        "RY" => GateKind::RY,
        "R" | "RZ" => GateKind::R,
        "XCX" => GateKind::XCX,
        "XCY" => GateKind::XCY,
        "XCZ" => GateKind::XCZ,
        "YCX" => GateKind::YCX,
        "YCY" => GateKind::YCY,
        "YCZ" => GateKind::YCZ,
        "CX" | "CNOT" | "ZCX" => GateKind::CX,
        "CY" | "ZCY" => GateKind::CY,
        "CZ" | "ZCZ" => GateKind::CZ,
        "H" | "H_XZ" => GateKind::H,
        "H_XY" => GateKind::H_XY,
        "H_YZ" => GateKind::H_YZ,
        "H_NXY" => GateKind::H_NXY,
        "H_NXZ" => GateKind::H_NXZ,
        "H_NYZ" => GateKind::H_NYZ,
        "DEPOLARIZE1" => GateKind::DEPOLARIZE1,
        "DEPOLARIZE2" => GateKind::DEPOLARIZE2,
        "X_ERROR" => GateKind::X_ERROR,
        "Y_ERROR" => GateKind::Y_ERROR,
        "Z_ERROR" => GateKind::Z_ERROR,
        "I_ERROR" => GateKind::I_ERROR,
        "II_ERROR" => GateKind::II_ERROR,
        "PAULI_CHANNEL_1" => GateKind::PAULI_CHANNEL_1,
        "PAULI_CHANNEL_2" => GateKind::PAULI_CHANNEL_2,
        "E" | "CORRELATED_ERROR" => GateKind::E,
        "ELSE_CORRELATED_ERROR" => GateKind::ELSE_CORRELATED_ERROR,
        "HERALDED_ERASE" => GateKind::HERALDED_ERASE,
        "HERALDED_PAULI_CHANNEL_1" => GateKind::HERALDED_PAULI_CHANNEL_1,
        "I" => GateKind::I,
        "X" => GateKind::X,
        "Y" => GateKind::Y,
        "Z" => GateKind::Z,
        "C_XYZ" => GateKind::C_XYZ,
        "C_ZYX" => GateKind::C_ZYX,
        "C_NXYZ" => GateKind::C_NXYZ,
        "C_XNYZ" => GateKind::C_XNYZ,
        "C_XYNZ" => GateKind::C_XYNZ,
        "C_NZYX" => GateKind::C_NZYX,
        "C_ZNYX" => GateKind::C_ZNYX,
        "C_ZYNX" => GateKind::C_ZYNX,
        "SQRT_X" => GateKind::SQRT_X,
        "SQRT_X_DAG" => GateKind::SQRT_X_DAG,
        "SQRT_Y" => GateKind::SQRT_Y,
        "SQRT_Y_DAG" => GateKind::SQRT_Y_DAG,
        "S" | "SQRT_Z" => GateKind::S,
        "S_DAG" | "SQRT_Z_DAG" => GateKind::S_DAG,
        "II" => GateKind::II,
        "SQRT_XX" => GateKind::SQRT_XX,
        "SQRT_XX_DAG" => GateKind::SQRT_XX_DAG,
        "SQRT_YY" => GateKind::SQRT_YY,
        "SQRT_YY_DAG" => GateKind::SQRT_YY_DAG,
        "SQRT_ZZ" => GateKind::SQRT_ZZ,
        "SQRT_ZZ_DAG" => GateKind::SQRT_ZZ_DAG,
        "MPP" => GateKind::MPP,
        "SPP" => GateKind::SPP,
        "SPP_DAG" => GateKind::SPP_DAG,
        "SWAP" => GateKind::SWAP,
        "ISWAP" => GateKind::ISWAP,
        "CXSWAP" => GateKind::CXSWAP,
        "SWAPCX" => GateKind::SWAPCX,
        "CZSWAP" | "SWAPCZ" => GateKind::CZSWAP,
        "ISWAP_DAG" => GateKind::ISWAP_DAG,
        "MXX" => GateKind::MXX,
        "MYY" => GateKind::MYY,
        "MZZ" => GateKind::MZZ,
        _ => return None,
    })
}
