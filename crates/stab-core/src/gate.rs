mod decomposition;
mod flows;
mod metadata;
mod semantic_contract;
mod unitary;

pub use decomposition::GateDecomposition;
pub use metadata::{GateArgumentRule, GateTargetGroupKind, GateTargetRule};
#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub use semantic_contract::{GateContractStatisticalBucket, GateContractStatisticalPlan};
pub use unitary::GateUnitaryMatrix;

use crate::{CircuitError, CircuitResult, Probability, Target};
use semantic_contract::{
    GateSemanticFamily, gate, gate_with_inverse, not_fusable_gate, semantic_gate,
    semantic_gate_with_inverse, semantic_not_fusable_gate,
};

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
    pub fn from_name(name: &str) -> CircuitResult<Self> {
        gate_info_from_name(name)
            .map(|info| Self { info })
            .ok_or_else(|| CircuitError::UnknownGate(name.to_string()))
    }

    #[inline]
    pub(crate) fn from_simple_plain_name(name: &str) -> Option<Self> {
        gate_info_from_simple_plain_name(name).map(|info| Self { info })
    }

    #[inline]
    #[allow(
        clippy::indexing_slicing,
        reason = "constant gate-table indexes are guarded by canonical-name round-trip tests"
    )]
    pub(crate) fn plain_h() -> Self {
        Self { info: &GATES[25] }
    }

    #[inline]
    #[allow(
        clippy::indexing_slicing,
        reason = "constant gate-table indexes are guarded by canonical-name round-trip tests"
    )]
    pub(crate) fn plain_m() -> Self {
        Self { info: &GATES[9] }
    }

    #[inline]
    #[allow(
        clippy::indexing_slicing,
        reason = "constant gate-table indexes are guarded by canonical-name round-trip tests"
    )]
    pub(crate) fn plain_cx() -> Self {
        Self { info: &GATES[22] }
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

    pub fn best_candidate_inverse(self) -> CircuitResult<Self> {
        Self::from_name(self.info.inverse_name)
    }

    pub(crate) fn validate(self, args: &[f64], targets: &[Target]) -> CircuitResult<()> {
        self.info.arg_rule.validate(self.info.name, args)?;
        self.info.target_rule.validate(self.info.name, targets)
    }

    pub(crate) fn arg_rule(self) -> ArgRule {
        self.info.arg_rule
    }

    pub(crate) fn targets_are_measurement_pads(self) -> bool {
        self.info.target_rule == TargetRule::MeasurementPads
    }
}

#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub fn __gate_contract_family_names() -> &'static [&'static str] {
    semantic_contract::gate_contract_family_names()
}

#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub fn __gate_contract_surface_names() -> &'static [&'static str] {
    semantic_contract::gate_contract_surface_names()
}

#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub fn __gate_contract_statistical_plans() -> &'static [GateContractStatisticalPlan] {
    semantic_contract::gate_contract_statistical_plans()
}

#[cfg(feature = "ops-contracts")]
#[doc(hidden)]
pub fn __gate_contract_statistical_rejection_boundaries(
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
) -> (Option<u64>, Option<u64>) {
    semantic_contract::gate_contract_statistical_rejection_boundaries(
        shots,
        expected_probability,
        allowed_delta,
    )
}

#[derive(Debug, Eq, PartialEq)]
struct GateInfo {
    name: &'static str,
    inverse_name: &'static str,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
    semantic_family: GateSemanticFamily,
    can_fuse: bool,
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
    fn validate(self, gate: &'static str, args: &[f64]) -> CircuitResult<()> {
        match self {
            Self::Exact(expected) if args.len() != expected => {
                Err(CircuitError::InvalidArgumentCount {
                    gate,
                    expected: match expected {
                        0 => "0",
                        1 => "1",
                        2 => "2",
                        _ => "fixed",
                    },
                    actual: args.len(),
                })
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
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "0 or 1",
                        actual: args.len(),
                    });
                }
                if let Some(arg) = args.first().copied() {
                    Probability::try_new(arg).map_err(|_| CircuitError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    })?;
                }
                Ok(())
            }
            Self::ProbabilityList(expected) => {
                if args.len() != expected {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "probability list",
                        actual: args.len(),
                    });
                }
                validate_probability_list(gate, args)
            }
            Self::AnyProbabilityList => validate_probability_list(gate, args),
            Self::UnsignedInteger => {
                if args.len() != 1 {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    });
                }
                let Some(arg) = args.first().copied() else {
                    return Err(CircuitError::InvalidArgumentCount {
                        gate,
                        expected: "1",
                        actual: args.len(),
                    });
                };
                if !arg.is_finite() || arg < 0.0 || arg.fract() != 0.0 {
                    return Err(CircuitError::InvalidArgument {
                        gate,
                        argument: arg.to_string(),
                    });
                }
                Ok(())
            }
        }
    }
}

fn validate_probability_list(gate: &'static str, args: &[f64]) -> CircuitResult<()> {
    let mut total = 0.0;
    for arg in args {
        Probability::try_new(*arg).map_err(|_| CircuitError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        })?;
        total += *arg;
    }
    if total > 1.0000001 {
        return Err(CircuitError::InvalidArgument {
            gate,
            argument: format!("sum {total}"),
        });
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
    fn validate(self, gate: &'static str, targets: &[Target]) -> CircuitResult<()> {
        match self {
            Self::None => {
                if targets.is_empty() {
                    Ok(())
                } else {
                    Err(CircuitError::InvalidTargetCount {
                        gate,
                        count: targets.len(),
                    })
                }
            }
            Self::AnySingleQubit => validate_targets(gate, targets, is_plain_qubit_target),
            Self::MeasurementQubits => validate_targets(gate, targets, Target::is_qubit_like),
            Self::MeasurementPads => validate_targets(gate, targets, is_measurement_pad_target),
            Self::PlainPairs => validate_pair_targets(gate, targets, is_plain_qubit_target),
            Self::ClassicalControlPairs => {
                validate_pair_targets(gate, targets, is_plain_qubit_or_classical_target)
            }
            Self::MeasurementPairs => validate_pair_targets(gate, targets, Target::is_qubit_target),
            Self::RecOnly => validate_targets(gate, targets, Target::is_measurement_record),
            Self::RecOrPauli => validate_targets(gate, targets, |target| {
                target.is_measurement_record_target() || target.is_pauli_target()
            }),
            Self::QubitCoords => validate_targets(gate, targets, is_plain_qubit_target),
            Self::PauliProducts => {
                validate_targets(gate, targets, Target::is_pauli_product_part)?;
                validate_combiners(gate, targets)
            }
            Self::PauliList => validate_targets(gate, targets, |target| {
                matches!(target, Target::Pauli { .. })
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
) -> CircuitResult<()> {
    if !targets.len().is_multiple_of(2) {
        return Err(CircuitError::InvalidTargetCount {
            gate,
            count: targets.len(),
        });
    }
    validate_targets(gate, targets, predicate)?;
    for pair in targets.chunks_exact(2) {
        if let [left, right] = pair
            && left == right
        {
            return Err(CircuitError::InvalidTarget {
                gate,
                target: left.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_targets(
    gate: &'static str,
    targets: &[Target],
    predicate: impl Fn(&Target) -> bool,
) -> CircuitResult<()> {
    for target in targets {
        if !predicate(target) {
            return Err(CircuitError::InvalidTarget {
                gate,
                target: target.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_combiners(gate: &'static str, targets: &[Target]) -> CircuitResult<()> {
    let mut previous_was_combiner = true;
    for target in targets {
        if target.is_combiner() {
            if previous_was_combiner {
                return Err(CircuitError::InvalidTarget {
                    gate,
                    target: target.to_string(),
                });
            }
            previous_was_combiner = true;
        } else {
            previous_was_combiner = false;
        }
    }
    if previous_was_combiner && !targets.is_empty() {
        return Err(CircuitError::InvalidTarget {
            gate,
            target: "*".to_string(),
        });
    }
    Ok(())
}

fn validate_finite_arg(gate: &'static str, arg: f64) -> CircuitResult<()> {
    if arg.is_finite() {
        Ok(())
    } else {
        Err(CircuitError::InvalidArgument {
            gate,
            argument: arg.to_string(),
        })
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
        _ if name.eq_ignore_ascii_case("M") || name.eq_ignore_ascii_case("MZ") => &GATES[9],
        _ if name.eq_ignore_ascii_case("CX") || name.eq_ignore_ascii_case("CNOT") => &GATES[22],
        _ if name.eq_ignore_ascii_case("H") => &GATES[25],
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

const GATES: &[GateInfo] = &[
    not_fusable_gate(
        "DETECTOR",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::RecOnly,
    ),
    not_fusable_gate(
        "OBSERVABLE_INCLUDE",
        GateCategory::Annotation,
        ArgRule::UnsignedInteger,
        TargetRule::RecOrPauli,
    ),
    not_fusable_gate(
        "TICK",
        GateCategory::Annotation,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    not_fusable_gate(
        "QUBIT_COORDS",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::QubitCoords,
    ),
    not_fusable_gate(
        "SHIFT_COORDS",
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::None,
    ),
    not_fusable_gate(
        "REPEAT",
        GateCategory::ControlFlow,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    semantic_gate(
        "MPAD",
        GateCategory::Annotation,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPads,
        GateSemanticFamily::MeasurementPad,
    ),
    semantic_gate(
        "MX",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::Measurement,
    ),
    semantic_gate(
        "MY",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::Measurement,
    ),
    semantic_gate(
        "M",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::Measurement,
    ),
    semantic_gate(
        "MRX",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::MeasureReset,
    ),
    semantic_gate(
        "MRY",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::MeasureReset,
    ),
    semantic_gate(
        "MR",
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
        GateSemanticFamily::MeasureReset,
    ),
    semantic_gate_with_inverse(
        "RX",
        "MX",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::Reset,
    ),
    semantic_gate_with_inverse(
        "RY",
        "MY",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::Reset,
    ),
    semantic_gate_with_inverse(
        "R",
        "M",
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::Reset,
    ),
    gate(
        "XCX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        "XCY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    semantic_gate(
        "XCZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
        GateSemanticFamily::ReverseClassicalControl,
    ),
    gate(
        "YCX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        "YCY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    semantic_gate(
        "YCZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
        GateSemanticFamily::ReverseClassicalControl,
    ),
    semantic_gate(
        "CX",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
        GateSemanticFamily::ForwardClassicalControl,
    ),
    semantic_gate(
        "CY",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
        GateSemanticFamily::ForwardClassicalControl,
    ),
    semantic_gate(
        "CZ",
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
        GateSemanticFamily::SymmetricClassicalControl,
    ),
    gate(
        "H",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_XY",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_YZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NXY",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NXZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "H_NYZ",
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    semantic_gate(
        "DEPOLARIZE1",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::Depolarization,
    ),
    semantic_gate(
        "DEPOLARIZE2",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PlainPairs,
        GateSemanticFamily::Depolarization,
    ),
    semantic_gate(
        "X_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::PauliNoise,
    ),
    semantic_gate(
        "Y_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::PauliNoise,
    ),
    semantic_gate(
        "Z_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::PauliNoise,
    ),
    semantic_gate(
        "I_ERROR",
        GateCategory::Noise,
        ArgRule::AnyProbabilityList,
        TargetRule::AnySingleQubit,
        GateSemanticFamily::IdentityNoise,
    ),
    semantic_gate(
        "II_ERROR",
        GateCategory::Noise,
        ArgRule::AnyProbabilityList,
        TargetRule::PlainPairs,
        GateSemanticFamily::IdentityNoise,
    ),
    semantic_gate(
        "PAULI_CHANNEL_1",
        GateCategory::Noise,
        ArgRule::ProbabilityList(3),
        TargetRule::AnySingleQubit,
        GateSemanticFamily::PauliChannel,
    ),
    semantic_gate(
        "PAULI_CHANNEL_2",
        GateCategory::Noise,
        ArgRule::ProbabilityList(15),
        TargetRule::PlainPairs,
        GateSemanticFamily::PauliChannel,
    ),
    semantic_not_fusable_gate(
        "E",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
        GateSemanticFamily::CorrelatedError,
    ),
    semantic_not_fusable_gate(
        "ELSE_CORRELATED_ERROR",
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
        GateSemanticFamily::CorrelatedError,
    ),
    gate(
        "HERALDED_ERASE",
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "HERALDED_PAULI_CHANNEL_1",
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(4),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "I",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "X",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Y",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "Z",
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XYZ",
        "C_ZYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZYX",
        "C_XYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_NXYZ",
        "C_ZYNX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XNYZ",
        "C_ZNYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_XYNZ",
        "C_NZYX",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_NZYX",
        "C_XYNZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZNYX",
        "C_XNYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "C_ZYNX",
        "C_NXYZ",
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_X",
        "SQRT_X_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_X_DAG",
        "SQRT_X",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_Y",
        "SQRT_Y_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "SQRT_Y_DAG",
        "SQRT_Y",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "S",
        "S_DAG",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        "S_DAG",
        "S",
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        "II",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_XX",
        "SQRT_XX_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_XX_DAG",
        "SQRT_XX",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_YY",
        "SQRT_YY_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_YY_DAG",
        "SQRT_YY",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SQRT_ZZ_DAG",
        "SQRT_ZZ",
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    semantic_gate(
        "MPP",
        GateCategory::PauliProduct,
        ArgRule::ZeroOrOneProbability,
        TargetRule::PauliProducts,
        GateSemanticFamily::PauliProductMeasurement,
    ),
    semantic_gate_with_inverse(
        "SPP",
        "SPP_DAG",
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
        GateSemanticFamily::PauliProductPhase,
    ),
    semantic_gate_with_inverse(
        "SPP_DAG",
        "SPP",
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
        GateSemanticFamily::PauliProductPhase,
    ),
    gate(
        "SWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "ISWAP",
        "ISWAP_DAG",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "CXSWAP",
        "SWAPCX",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "SWAPCX",
        "CXSWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        "CZSWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        "ISWAP_DAG",
        "ISWAP",
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        "MXX",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    ),
    gate(
        "MYY",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    ),
    gate(
        "MZZ",
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    ),
];
