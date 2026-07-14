use thiserror::Error;

use super::{PauliPhase, StabilizerResource};
use crate::BitError;

pub type StabilizerResult<T> = Result<T, StabilizerError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StabilizerError {
    #[error(transparent)]
    Bit(#[from] BitError),

    #[error("Pauli string length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

    #[error("{resource} request {requested} exceeds limit {limit}")]
    ResourceLimitExceeded {
        resource: StabilizerResource,
        requested: usize,
        limit: usize,
    },

    #[error(
        "{resource} size overflowed while repeating {item_count} item(s) {repetitions} time(s)"
    )]
    ResourceSizeOverflow {
        resource: StabilizerResource,
        item_count: usize,
        repetitions: usize,
    },

    #[error("unrecognized Pauli character {character:?} at offset {offset}")]
    InvalidPauliCharacter { character: char, offset: usize },

    #[error("invalid sparse Pauli string shorthand {text:?}")]
    InvalidSparsePauliString { text: String },

    #[error("Pauli product has imaginary phase {phase}")]
    ImaginaryProduct { phase: PauliPhase },

    #[error("gate {gate} is not a single-qubit Clifford gate")]
    InvalidSingleQubitCliffordGate { gate: String },

    #[error("Clifford index {index} is outside length {len}")]
    CliffordIndexOutOfRange { index: usize, len: usize },

    #[error("invalid single-qubit Clifford product")]
    InvalidSingleQubitCliffordProduct,

    #[error("Tableau index {index} is outside length {len}")]
    TableauIndexOutOfRange { index: usize, len: usize },

    #[error("duplicate Tableau target {target}")]
    DuplicateTableauTarget { target: usize },

    #[error("commuting Pauli string iteration requires 1..64 qubits but got {num_qubits}")]
    InvalidCommutingPauliIteratorQubitCount { num_qubits: usize },

    #[error("Tableau iteration requires fewer than 64 qubits but got {num_qubits}")]
    InvalidTableauIteratorQubitCount { num_qubits: usize },

    #[error("invalid stabilizer flow text {text:?}")]
    InvalidFlowText { text: String },

    #[error("anti-Hermitian stabilizer flows are not allowed")]
    AntiHermitianFlow,

    #[error("stabilizer flow product anticommutes: {left} with {right}")]
    InvalidFlowProduct { left: String, right: String },

    #[error("Tableau is not a Pauli product")]
    NotPauliProduct,

    #[error("failed to derive inverse Tableau row")]
    InvalidTableauInverse,

    #[error("stabilizer {stabilizer} anticommutes with earlier stabilizer {conflict}")]
    AntiCommutingStabilizer {
        stabilizer: String,
        conflict: String,
    },

    #[error("redundant stabilizer {stabilizer} is not allowed")]
    RedundantStabilizer { stabilizer: String },

    #[error("stabilizer {stabilizer} has an inconsistent sign")]
    InconsistentStabilizer { stabilizer: String },

    #[error("stabilizer set has {independent} independent generators but {num_qubits} qubits")]
    OverconstrainedStabilizers {
        independent: usize,
        num_qubits: usize,
    },

    #[error(
        "stabilizer set has {independent} independent generators but {num_qubits} qubits and underconstrained conversion is disabled"
    )]
    UnderconstrainedStabilizers {
        independent: usize,
        num_qubits: usize,
    },

    #[error("failed to synthesize a stabilizer Tableau")]
    InvalidStabilizerTableauSynthesis,

    #[error("unitary matrix height must be a non-zero power of 2, got {height}")]
    UnitaryMatrixHeightNotPowerOfTwo { height: usize },

    #[error("unitary matrix row {row} had width {width}, expected square width {height}")]
    UnitaryMatrixRowWidthMismatch {
        row: usize,
        width: usize,
        height: usize,
    },

    #[error("matrix is not unitary")]
    MatrixNotUnitary,

    #[error("unitary matrix is not a Clifford operation")]
    UnitaryMatrixNotClifford,
}
