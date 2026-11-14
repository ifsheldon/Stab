//! The closed identity of every canonical Stim v1.16.0 gate.

use super::{GATES, Gate};

/// Closed identity of a canonical Stim v1.16.0 gate.
///
/// Every variant spells its gate's canonical Stim name, and aliases such as `CNOT` or `MZ`
/// resolve to their canonical variant when a circuit is parsed, so a [`Gate`] has exactly one
/// kind. The enum is deliberately exhaustive: the dialect is closed (ADR 0001), and code that
/// selects behavior per gate matches on it without a wildcard so that a new gate fails to
/// compile instead of silently taking a default path. Per-gate facts live in the one gate table
/// and are read through [`Gate`].
#[allow(
    non_camel_case_types,
    clippy::upper_case_acronyms,
    reason = "variants spell Stim's canonical gate names so per-gate matches read like circuits"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GateKind {
    DETECTOR,
    OBSERVABLE_INCLUDE,
    TICK,
    QUBIT_COORDS,
    SHIFT_COORDS,
    REPEAT,
    MPAD,
    MX,
    MY,
    M,
    MRX,
    MRY,
    MR,
    RX,
    RY,
    R,
    XCX,
    XCY,
    XCZ,
    YCX,
    YCY,
    YCZ,
    CX,
    CY,
    CZ,
    H,
    H_XY,
    H_YZ,
    H_NXY,
    H_NXZ,
    H_NYZ,
    DEPOLARIZE1,
    DEPOLARIZE2,
    X_ERROR,
    Y_ERROR,
    Z_ERROR,
    I_ERROR,
    II_ERROR,
    PAULI_CHANNEL_1,
    PAULI_CHANNEL_2,
    E,
    ELSE_CORRELATED_ERROR,
    HERALDED_ERASE,
    HERALDED_PAULI_CHANNEL_1,
    I,
    X,
    Y,
    Z,
    C_XYZ,
    C_ZYX,
    C_NXYZ,
    C_XNYZ,
    C_XYNZ,
    C_NZYX,
    C_ZNYX,
    C_ZYNX,
    SQRT_X,
    SQRT_X_DAG,
    SQRT_Y,
    SQRT_Y_DAG,
    S,
    S_DAG,
    II,
    SQRT_XX,
    SQRT_XX_DAG,
    SQRT_YY,
    SQRT_YY_DAG,
    SQRT_ZZ,
    SQRT_ZZ_DAG,
    MPP,
    SPP,
    SPP_DAG,
    SWAP,
    ISWAP,
    CXSWAP,
    SWAPCX,
    CZSWAP,
    ISWAP_DAG,
    MXX,
    MYY,
    MZZ,
}

impl GateKind {
    /// The number of canonical Stim v1.16.0 gates.
    pub const COUNT: usize = 81;

    /// Every canonical gate kind, in gate-table order.
    pub const ALL: [Self; Self::COUNT] = [
        Self::DETECTOR,
        Self::OBSERVABLE_INCLUDE,
        Self::TICK,
        Self::QUBIT_COORDS,
        Self::SHIFT_COORDS,
        Self::REPEAT,
        Self::MPAD,
        Self::MX,
        Self::MY,
        Self::M,
        Self::MRX,
        Self::MRY,
        Self::MR,
        Self::RX,
        Self::RY,
        Self::R,
        Self::XCX,
        Self::XCY,
        Self::XCZ,
        Self::YCX,
        Self::YCY,
        Self::YCZ,
        Self::CX,
        Self::CY,
        Self::CZ,
        Self::H,
        Self::H_XY,
        Self::H_YZ,
        Self::H_NXY,
        Self::H_NXZ,
        Self::H_NYZ,
        Self::DEPOLARIZE1,
        Self::DEPOLARIZE2,
        Self::X_ERROR,
        Self::Y_ERROR,
        Self::Z_ERROR,
        Self::I_ERROR,
        Self::II_ERROR,
        Self::PAULI_CHANNEL_1,
        Self::PAULI_CHANNEL_2,
        Self::E,
        Self::ELSE_CORRELATED_ERROR,
        Self::HERALDED_ERASE,
        Self::HERALDED_PAULI_CHANNEL_1,
        Self::I,
        Self::X,
        Self::Y,
        Self::Z,
        Self::C_XYZ,
        Self::C_ZYX,
        Self::C_NXYZ,
        Self::C_XNYZ,
        Self::C_XYNZ,
        Self::C_NZYX,
        Self::C_ZNYX,
        Self::C_ZYNX,
        Self::SQRT_X,
        Self::SQRT_X_DAG,
        Self::SQRT_Y,
        Self::SQRT_Y_DAG,
        Self::S,
        Self::S_DAG,
        Self::II,
        Self::SQRT_XX,
        Self::SQRT_XX_DAG,
        Self::SQRT_YY,
        Self::SQRT_YY_DAG,
        Self::SQRT_ZZ,
        Self::SQRT_ZZ_DAG,
        Self::MPP,
        Self::SPP,
        Self::SPP_DAG,
        Self::SWAP,
        Self::ISWAP,
        Self::CXSWAP,
        Self::SWAPCX,
        Self::CZSWAP,
        Self::ISWAP_DAG,
        Self::MXX,
        Self::MYY,
        Self::MZZ,
    ];

    /// Returns the gate-table entry for this kind.
    #[inline]
    #[allow(
        clippy::indexing_slicing,
        reason = "the table has one row per variant in declaration order, which a constant assertion beside the table proves at compile time"
    )]
    pub const fn gate(self) -> Gate {
        Gate::from_info(&GATES[self as usize])
    }

    /// Returns the canonical Stim name, which is also this variant's spelling.
    #[inline]
    pub fn canonical_name(self) -> &'static str {
        self.gate().canonical_name()
    }
}

impl From<GateKind> for Gate {
    #[inline]
    fn from(kind: GateKind) -> Self {
        kind.gate()
    }
}
