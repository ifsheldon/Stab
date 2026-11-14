//! The static Stim v1.16.0 gate table: one row per [`GateKind`], in declaration order, carrying
//! the canonical name, category, argument and target rules, inverse, classification flags, and
//! accepted alias list. Classification accessors in the parent module derive every other view
//! from these rows, so per-gate facts have this one owner.

use super::{ArgRule, GateCategory, GateFlags, GateInfo, GateKind, TargetRule};
use crate::Pauli;

/// Spells a row's identity once: the kind variant and its canonical name are the same token.
macro_rules! id {
    ($kind:ident) => {
        (GateKind::$kind, stringify!($kind))
    };
}

const fn gate(
    id: (GateKind, &'static str),
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    gate_with_inverse(id, id.0, category, arg_rule, target_rule)
}

const fn gate_with_inverse(
    (kind, name): (GateKind, &'static str),
    inverse: GateKind,
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    GateInfo {
        kind,
        name,
        inverse,
        category,
        arg_rule,
        target_rule,
        can_fuse: true,
        flags: GateFlags::NONE,
        aliases: &[],
        controlled_pauli: None,
        collapse_basis: None,
    }
}

const fn not_fusable_gate(
    id: (GateKind, &'static str),
    category: GateCategory,
    arg_rule: ArgRule,
    target_rule: TargetRule,
) -> GateInfo {
    let mut info = gate(id, category, arg_rule, target_rule);
    info.can_fuse = false;
    info
}

/// Proves at compile time that row `i` describes `GateKind::ALL[i]`, which makes the
/// discriminant-indexed lookup in [`GateKind::gate`] total.
#[allow(
    clippy::indexing_slicing,
    reason = "indexes are bounded by the equal table and kind-list lengths during const evaluation"
)]
const _: () = {
    let mut index = 0;
    while index < GATES.len() {
        assert!(
            GATES[index].kind as usize == index && GateKind::ALL[index] as usize == index,
            "gate table rows must follow GateKind declaration order"
        );
        assert!(
            GATES[index].controlled_pauli.is_some()
                == matches!(GATES[index].target_rule, TargetRule::ClassicalControlPairs),
            "exactly the classically controllable gates name a controlled Pauli"
        );
        assert!(
            GATES[index].collapse_basis.is_some()
                == matches!(GATES[index].category, GateCategory::Collapsing),
            "exactly the single-qubit collapsing gates name a collapse basis"
        );
        index += 1;
    }
};

pub(super) const GATES: [GateInfo; GateKind::COUNT] = [
    not_fusable_gate(
        id!(DETECTOR),
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::RecOnly,
    ),
    not_fusable_gate(
        id!(OBSERVABLE_INCLUDE),
        GateCategory::Annotation,
        ArgRule::UnsignedInteger,
        TargetRule::RecOrPauli,
    ),
    not_fusable_gate(
        id!(TICK),
        GateCategory::Annotation,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    not_fusable_gate(
        id!(QUBIT_COORDS),
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::QubitCoords,
    ),
    not_fusable_gate(
        id!(SHIFT_COORDS),
        GateCategory::Annotation,
        ArgRule::Any,
        TargetRule::None,
    ),
    not_fusable_gate(
        id!(REPEAT),
        GateCategory::ControlFlow,
        ArgRule::Exact(0),
        TargetRule::None,
    ),
    gate(
        id!(MPAD),
        GateCategory::Annotation,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPads,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate(
        id!(MX),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::X)
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate(
        id!(MY),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::Y)
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate(
        id!(M),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::Z)
    .with_flags(GateFlags::PRODUCES_RESULTS)
    .with_aliases(&["M", "MZ"]),
    gate(
        id!(MRX),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::X)
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::IS_RESET)),
    gate(
        id!(MRY),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::Y)
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::IS_RESET)),
    gate(
        id!(MR),
        GateCategory::Collapsing,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementQubits,
    )
    .with_collapse_basis(Pauli::Z)
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::IS_RESET))
    .with_aliases(&["MR", "MRZ"]),
    gate_with_inverse(
        id!(RX),
        GateKind::MX,
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_collapse_basis(Pauli::X)
    .with_flags(GateFlags::IS_RESET),
    gate_with_inverse(
        id!(RY),
        GateKind::MY,
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_collapse_basis(Pauli::Y)
    .with_flags(GateFlags::IS_RESET),
    gate_with_inverse(
        id!(R),
        GateKind::M,
        GateCategory::Collapsing,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_collapse_basis(Pauli::Z)
    .with_flags(GateFlags::IS_RESET)
    .with_aliases(&["R", "RZ"]),
    gate(
        id!(XCX),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(XCY),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        id!(XCZ),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
    )
    .with_controlled_pauli(Pauli::X),
    gate(
        id!(YCX),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        id!(YCY),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(YCZ),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
    )
    .with_controlled_pauli(Pauli::Y),
    gate(
        id!(CX),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
    )
    .with_controlled_pauli(Pauli::X)
    .with_aliases(&["CNOT", "CX", "ZCX"]),
    gate(
        id!(CY),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
    )
    .with_controlled_pauli(Pauli::Y)
    .with_aliases(&["CY", "ZCY"]),
    gate(
        id!(CZ),
        GateCategory::Controlled,
        ArgRule::Exact(0),
        TargetRule::ClassicalControlPairs,
    )
    .with_controlled_pauli(Pauli::Z)
    .with_flags(GateFlags::SYMMETRIC_PAIR)
    .with_aliases(&["CZ", "ZCZ"]),
    gate(
        id!(H),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_aliases(&["H", "H_XZ"]),
    gate(
        id!(H_XY),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(H_YZ),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(H_NXY),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(H_NXZ),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(H_NYZ),
        GateCategory::HadamardLike,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(DEPOLARIZE1),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(DEPOLARIZE2),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(X_ERROR),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(Y_ERROR),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(Z_ERROR),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(I_ERROR),
        GateCategory::Noise,
        ArgRule::AnyProbabilityList,
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(II_ERROR),
        GateCategory::Noise,
        ArgRule::AnyProbabilityList,
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(PAULI_CHANNEL_1),
        GateCategory::Noise,
        ArgRule::ProbabilityList(3),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(PAULI_CHANNEL_2),
        GateCategory::Noise,
        ArgRule::ProbabilityList(15),
        TargetRule::PlainPairs,
    ),
    not_fusable_gate(
        id!(E),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
    )
    .with_aliases(&["CORRELATED_ERROR", "E"]),
    not_fusable_gate(
        id!(ELSE_CORRELATED_ERROR),
        GateCategory::Noise,
        ArgRule::ProbabilityList(1),
        TargetRule::PauliList,
    ),
    gate(
        id!(HERALDED_ERASE),
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(1),
        TargetRule::AnySingleQubit,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate(
        id!(HERALDED_PAULI_CHANNEL_1),
        GateCategory::HeraldedNoise,
        ArgRule::ProbabilityList(4),
        TargetRule::AnySingleQubit,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate(
        id!(I),
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(X),
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(Y),
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate(
        id!(Z),
        GateCategory::Pauli,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_XYZ),
        GateKind::C_ZYX,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_ZYX),
        GateKind::C_XYZ,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_NXYZ),
        GateKind::C_ZYNX,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_XNYZ),
        GateKind::C_ZNYX,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_XYNZ),
        GateKind::C_NZYX,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_NZYX),
        GateKind::C_XYNZ,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_ZNYX),
        GateKind::C_XNYZ,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(C_ZYNX),
        GateKind::C_NXYZ,
        GateCategory::Period3,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(SQRT_X),
        GateKind::SQRT_X_DAG,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(SQRT_X_DAG),
        GateKind::SQRT_X,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(SQRT_Y),
        GateKind::SQRT_Y_DAG,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(SQRT_Y_DAG),
        GateKind::SQRT_Y,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    ),
    gate_with_inverse(
        id!(S),
        GateKind::S_DAG,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_aliases(&["S", "SQRT_Z"]),
    gate_with_inverse(
        id!(S_DAG),
        GateKind::S,
        GateCategory::Period4,
        ArgRule::Exact(0),
        TargetRule::AnySingleQubit,
    )
    .with_aliases(&["S_DAG", "SQRT_Z_DAG"]),
    gate(
        id!(II),
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_XX),
        GateKind::SQRT_XX_DAG,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_XX_DAG),
        GateKind::SQRT_XX,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_YY),
        GateKind::SQRT_YY_DAG,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_YY_DAG),
        GateKind::SQRT_YY,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_ZZ),
        GateKind::SQRT_ZZ_DAG,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(SQRT_ZZ_DAG),
        GateKind::SQRT_ZZ,
        GateCategory::ParityPhasing,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(MPP),
        GateCategory::PauliProduct,
        ArgRule::ZeroOrOneProbability,
        TargetRule::PauliProducts,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS),
    gate_with_inverse(
        id!(SPP),
        GateKind::SPP_DAG,
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
    ),
    gate_with_inverse(
        id!(SPP_DAG),
        GateKind::SPP,
        GateCategory::PauliProduct,
        ArgRule::Exact(0),
        TargetRule::PauliProducts,
    ),
    gate(
        id!(SWAP),
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(ISWAP),
        GateKind::ISWAP_DAG,
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate_with_inverse(
        id!(CXSWAP),
        GateKind::SWAPCX,
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate_with_inverse(
        id!(SWAPCX),
        GateKind::CXSWAP,
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    ),
    gate(
        id!(CZSWAP),
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR)
    .with_aliases(&["CZSWAP", "SWAPCZ"]),
    gate_with_inverse(
        id!(ISWAP_DAG),
        GateKind::ISWAP,
        GateCategory::Swap,
        ArgRule::Exact(0),
        TargetRule::PlainPairs,
    )
    .with_flags(GateFlags::SYMMETRIC_PAIR),
    gate(
        id!(MXX),
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::SYMMETRIC_PAIR)),
    gate(
        id!(MYY),
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::SYMMETRIC_PAIR)),
    gate(
        id!(MZZ),
        GateCategory::PairMeasurement,
        ArgRule::ZeroOrOneProbability,
        TargetRule::MeasurementPairs,
    )
    .with_flags(GateFlags::PRODUCES_RESULTS.union(GateFlags::SYMMETRIC_PAIR)),
];
