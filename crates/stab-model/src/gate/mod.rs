mod decomposition;
mod flows;
mod metadata;
mod table;
mod unitary;

pub use decomposition::GateDecomposition;
pub(crate) use decomposition::gate_decomposition_text;
pub(crate) use flows::gate_flow_descriptors;
pub use metadata::{GateArgumentRule, GateTargetGroupKind, GateTargetRule};
use table::GATES;
pub use unitary::GateUnitaryRows;
pub(crate) use unitary::gate_unitary_rows;

use crate::{ModelResult, Probability, Target, ValidationError};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gate {
    info: &'static GateInfo,
}

impl Gate {
    /// Iterates over the canonical gates defined by Stim v1.16.0.
    #[inline]
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        GATES.iter().map(|info| Self { info })
    }

    #[inline]
    pub fn from_name(name: &str) -> ModelResult<Self> {
        Self::lookup_name(name).ok_or_else(|| ValidationError::UnknownGate(name.to_string()).into())
    }

    #[inline]
    pub(crate) fn lookup_name(name: &str) -> Option<Self> {
        gate_info_from_name(name).map(|info| Self { info })
    }

    #[inline]
    pub(crate) fn from_simple_plain_name(name: &str) -> Option<Self> {
        gate_info_from_simple_plain_name(name).map(|info| Self { info })
    }

    /// Canonical `DETECTOR` gate, resolved from the gate table at compile time.
    pub(crate) const DETECTOR: Self = Self::table_constant("DETECTOR");
    /// Canonical `OBSERVABLE_INCLUDE` gate, resolved from the gate table at compile time.
    pub(crate) const OBSERVABLE_INCLUDE: Self = Self::table_constant("OBSERVABLE_INCLUDE");
    /// Canonical `TICK` gate, resolved from the gate table at compile time.
    pub(crate) const TICK: Self = Self::table_constant("TICK");
    /// Canonical `QUBIT_COORDS` gate, resolved from the gate table at compile time.
    pub(crate) const QUBIT_COORDS: Self = Self::table_constant("QUBIT_COORDS");
    /// Canonical `SHIFT_COORDS` gate, resolved from the gate table at compile time.
    pub(crate) const SHIFT_COORDS: Self = Self::table_constant("SHIFT_COORDS");
    /// Canonical `H` gate, resolved from the gate table at compile time.
    pub(crate) const H: Self = Self::table_constant("H");
    /// Canonical `S` gate, resolved from the gate table at compile time.
    pub(crate) const S: Self = Self::table_constant("S");
    /// Canonical `M` gate, resolved from the gate table at compile time.
    pub(crate) const M: Self = Self::table_constant("M");
    /// Canonical `CX` gate, resolved from the gate table at compile time.
    pub(crate) const CX: Self = Self::table_constant("CX");

    /// Looks a canonical gate up by name at compile time.
    ///
    /// The scan runs during constant evaluation, so a name that is not in the canonical
    /// gate table fails the build instead of producing a runtime error.
    #[allow(
        clippy::indexing_slicing,
        clippy::panic,
        reason = "compile-time table scan: a missing name or out-of-range index fails the build during const evaluation and cannot panic at runtime"
    )]
    const fn table_constant(name: &str) -> Self {
        let mut index = 0;
        while index < GATES.len() {
            if const_name_eq(GATES[index].name, name) {
                return Self {
                    info: &GATES[index],
                };
            }
            index += 1;
        }
        panic!("gate constant does not name a canonical gate-table entry");
    }

    #[inline]
    pub fn canonical_name(self) -> &'static str {
        self.info.name
    }

    #[doc(hidden)]
    #[inline(always)]
    pub fn stim_name_hash(name: &str) -> usize {
        gate_name_hash(name)
    }

    #[inline]
    pub fn category(self) -> GateCategory {
        self.info.category
    }

    #[inline]
    pub fn best_candidate_inverse(self) -> ModelResult<Self> {
        Self::from_name(self.info.inverse_name)
    }

    pub(crate) fn validate(self, args: &[f64], targets: &[Target]) -> ModelResult<()> {
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

    pub(crate) fn validate_targets(self, targets: &[Target]) -> ModelResult<()> {
        self.info.target_rule.validate(self.info.name, targets)
    }
}

/// Compares two gate names byte for byte during constant evaluation.
#[allow(
    clippy::indexing_slicing,
    reason = "indexes are bounded by the compared lengths and run during const evaluation"
)]
const fn const_name_eq(left: &str, right: &str) -> bool {
    let (left, right) = (left.as_bytes(), right.as_bytes());
    if left.len() != right.len() {
        return false;
    }
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return false;
        }
        index += 1;
    }
    true
}

#[derive(Debug, Eq, PartialEq)]
struct GateInfo {
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    can_fuse: bool,
    flags: GateFlags,
    /// All accepted names in Stim v1.16.0 alias order; empty means the canonical name only.
    aliases: &'static [&'static str],
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
    Exact(usize),
    Any,
    ZeroOrOneProbability,
    ProbabilityList(usize),
    AnyProbabilityList,
    UnsignedInteger,
}

impl ArgRule {
    fn validate(self, gate: &'static str, args: &[f64]) -> ModelResult<()> {
        match self {
            Self::Exact(expected) if args.len() != expected => {
                Err(ValidationError::InvalidArgumentCount {
                    gate,
                    expected: match expected {
                        0 => "0",
                        1 => "1",
                        2 => "2",
                        _ => "fixed",
                    },
                    actual: args.len(),
                }
                .into())
            }
            Self::Exact(_) => Ok(()),
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
            Self::ProbabilityList(expected) => {
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
                if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 {
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
                matches!(target, Target::Pauli { .. } | Target::Combiner)
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
        target,
        Target::Qubit {
            inverted: false,
            ..
        }
    )
}

fn is_plain_qubit_or_classical_target(target: &Target) -> bool {
    is_plain_qubit_target(target) || target.is_classical_bit_target()
}

fn is_measurement_pad_target(target: &Target) -> bool {
    matches!(target, Target::Qubit { id, inverted: false } if id.get() <= 1)
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
    for pair in targets.chunks_exact(2) {
        if let [left, right] = pair
            && left == right
        {
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
fn gate_info_from_name(name: &str) -> Option<&'static GateInfo> {
    if let Some(info) = gate_info_from_uppercase_name(name) {
        return Some(info);
    }
    if !name.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return None;
    }
    let uppercase = name.to_ascii_uppercase();
    gate_info_from_uppercase_name(&uppercase)
}

#[inline(always)]
#[allow(
    clippy::indexing_slicing,
    reason = "Stim v1.16.0 hash indexes are guarded by explicit byte-length checks"
)]
fn gate_name_hash(text: &str) -> usize {
    // Matches Stim v1.16.0's gate_name_to_hash for benchmark parity.
    let bytes = text.as_bytes();
    let mut result = bytes.len();
    if !bytes.is_empty() {
        result ^= usize::from(bytes[0] | 0x20) * 2126;
        result = result.wrapping_add(usize::from(bytes[bytes.len() - 1] | 0x20) * 9883);
    }
    if bytes.len() > 2 {
        result ^= usize::from(bytes[1] | 0x20) * 8039;
        result = result.wrapping_add(usize::from(bytes[2] | 0x20) * 9042);
    }
    if bytes.len() > 4 {
        result ^= usize::from(bytes[3] | 0x20) * 4916;
        result = result.wrapping_add(usize::from(bytes[4] | 0x20) * 4048);
    }
    if bytes.len() > 5 {
        result ^= usize::from(bytes[5] | 0x20) * 7081;
    }
    result & 0x1ff
}

#[inline]
#[allow(
    clippy::indexing_slicing,
    reason = "constant gate-table indexes are guarded by canonical-name round-trip tests"
)]
fn gate_info_from_simple_plain_name(name: &str) -> Option<&'static GateInfo> {
    Some(match name {
        "M" | "MZ" => &GATES[9],
        "CX" | "CNOT" => &GATES[22],
        "H" => &GATES[25],
        "S" | "SQRT_Z" => &GATES[60],
        _ if name.eq_ignore_ascii_case("M") || name.eq_ignore_ascii_case("MZ") => &GATES[9],
        _ if name.eq_ignore_ascii_case("CX") || name.eq_ignore_ascii_case("CNOT") => &GATES[22],
        _ if name.eq_ignore_ascii_case("H") => &GATES[25],
        _ if name.eq_ignore_ascii_case("S") || name.eq_ignore_ascii_case("SQRT_Z") => &GATES[60],
        _ => return None,
    })
}

#[inline]
#[allow(
    clippy::indexing_slicing,
    reason = "constant gate-table indexes are guarded by canonical-name round-trip tests"
)]
fn gate_info_from_uppercase_name(name: &str) -> Option<&'static GateInfo> {
    Some(match name {
        "DETECTOR" => &GATES[0],
        "OBSERVABLE_INCLUDE" => &GATES[1],
        "TICK" => &GATES[2],
        "QUBIT_COORDS" => &GATES[3],
        "SHIFT_COORDS" => &GATES[4],
        "REPEAT" => &GATES[5],
        "MPAD" => &GATES[6],
        "MX" => &GATES[7],
        "MY" => &GATES[8],
        "M" | "MZ" => &GATES[9],
        "MRX" => &GATES[10],
        "MRY" => &GATES[11],
        "MR" | "MRZ" => &GATES[12],
        "RX" => &GATES[13],
        "RY" => &GATES[14],
        "R" | "RZ" => &GATES[15],
        "XCX" => &GATES[16],
        "XCY" => &GATES[17],
        "XCZ" => &GATES[18],
        "YCX" => &GATES[19],
        "YCY" => &GATES[20],
        "YCZ" => &GATES[21],
        "CX" | "CNOT" | "ZCX" => &GATES[22],
        "CY" | "ZCY" => &GATES[23],
        "CZ" | "ZCZ" => &GATES[24],
        "H" | "H_XZ" => &GATES[25],
        "H_XY" => &GATES[26],
        "H_YZ" => &GATES[27],
        "H_NXY" => &GATES[28],
        "H_NXZ" => &GATES[29],
        "H_NYZ" => &GATES[30],
        "DEPOLARIZE1" => &GATES[31],
        "DEPOLARIZE2" => &GATES[32],
        "X_ERROR" => &GATES[33],
        "Y_ERROR" => &GATES[34],
        "Z_ERROR" => &GATES[35],
        "I_ERROR" => &GATES[36],
        "II_ERROR" => &GATES[37],
        "PAULI_CHANNEL_1" => &GATES[38],
        "PAULI_CHANNEL_2" => &GATES[39],
        "E" | "CORRELATED_ERROR" => &GATES[40],
        "ELSE_CORRELATED_ERROR" => &GATES[41],
        "HERALDED_ERASE" => &GATES[42],
        "HERALDED_PAULI_CHANNEL_1" => &GATES[43],
        "I" => &GATES[44],
        "X" => &GATES[45],
        "Y" => &GATES[46],
        "Z" => &GATES[47],
        "C_XYZ" => &GATES[48],
        "C_ZYX" => &GATES[49],
        "C_NXYZ" => &GATES[50],
        "C_XNYZ" => &GATES[51],
        "C_XYNZ" => &GATES[52],
        "C_NZYX" => &GATES[53],
        "C_ZNYX" => &GATES[54],
        "C_ZYNX" => &GATES[55],
        "SQRT_X" => &GATES[56],
        "SQRT_X_DAG" => &GATES[57],
        "SQRT_Y" => &GATES[58],
        "SQRT_Y_DAG" => &GATES[59],
        "S" | "SQRT_Z" => &GATES[60],
        "S_DAG" | "SQRT_Z_DAG" => &GATES[61],
        "II" => &GATES[62],
        "SQRT_XX" => &GATES[63],
        "SQRT_XX_DAG" => &GATES[64],
        "SQRT_YY" => &GATES[65],
        "SQRT_YY_DAG" => &GATES[66],
        "SQRT_ZZ" => &GATES[67],
        "SQRT_ZZ_DAG" => &GATES[68],
        "MPP" => &GATES[69],
        "SPP" => &GATES[70],
        "SPP_DAG" => &GATES[71],
        "SWAP" => &GATES[72],
        "ISWAP" => &GATES[73],
        "CXSWAP" => &GATES[74],
        "SWAPCX" => &GATES[75],
        "CZSWAP" | "SWAPCZ" => &GATES[76],
        "ISWAP_DAG" => &GATES[77],
        "MXX" => &GATES[78],
        "MYY" => &GATES[79],
        "MZZ" => &GATES[80],
        _ => return None,
    })
}
