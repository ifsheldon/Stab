use thiserror::Error;

use super::PauliPhase;
use crate::BitError;

pub type StabilizerResult<T> = Result<T, StabilizerError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StabilizerError {
    #[error(transparent)]
    Bit(#[from] BitError),

    #[error("Pauli string length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

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

    #[error("Tableau is not a Pauli product")]
    NotPauliProduct,

    #[error("failed to derive inverse Tableau row")]
    InvalidTableauInverse,
}
