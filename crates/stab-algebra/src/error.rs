use std::fmt::{Display, Formatter};

use thiserror::Error;

use super::{PauliBasis, PauliPhase, StabilizerResource};
use stab_bits::BitError;
use stab_foundation::MemoryBudgetExceeded;

/// The leading characters of rejected input text, kept so an error can show where the text
/// started without copying all of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextExcerpt {
    text: String,
    truncated: bool,
}

impl TextExcerpt {
    /// Characters kept from the start of the rejected text.
    pub const MAX_CHARS: usize = 64;

    pub(crate) fn new(text: &str) -> Self {
        match text.char_indices().nth(Self::MAX_CHARS) {
            Some((end, _)) => Self {
                text: text.get(..end).unwrap_or_default().to_owned(),
                truncated: true,
            },
            None => Self {
                text: text.to_owned(),
                truncated: false,
            },
        }
    }

    /// The kept leading characters.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Whether the rejected text was longer than [`Self::MAX_CHARS`] characters.
    pub fn is_truncated(&self) -> bool {
        self.truncated
    }
}

impl Display for TextExcerpt {
    /// Writes the kept text quoted, followed by `...` when it was truncated.
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.text)?;
        if self.truncated {
            f.write_str("...")?;
        }
        Ok(())
    }
}

pub type StabilizerResult<T> = Result<T, StabilizerError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StabilizerError {
    #[error(transparent)]
    Bit(#[from] BitError),

    #[error("Pauli string length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

    /// Packed planes that an algebra value keeps at one word count disagree; this is an internal
    /// invariant violation, not a property of the input.
    #[error(
        "internal error: packed planes disagree in word count: expected {expected_words}, got {actual_words}"
    )]
    PlaneWordCountMismatch {
        expected_words: usize,
        actual_words: usize,
    },

    /// The memory budget refused the storage `resource` needed, before any of it was allocated.
    #[error(
        "{resource} needs {} bytes, exceeding the {}-byte memory budget",
        exceeded.required(),
        exceeded.budget().bytes()
    )]
    MemoryBudgetExceeded {
        resource: StabilizerResource,
        exceeded: MemoryBudgetExceeded,
    },

    #[error("allocating {bytes} bytes for {resource} failed")]
    AllocationFailed {
        resource: StabilizerResource,
        bytes: usize,
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

    #[error("invalid sparse Pauli string shorthand {excerpt}")]
    InvalidSparsePauliString { excerpt: TextExcerpt },

    #[error("Pauli product has imaginary phase {phase}")]
    ImaginaryProduct { phase: PauliPhase },

    #[error("gate {gate} is not a single-qubit Clifford gate")]
    InvalidSingleQubitCliffordGate { gate: String },

    #[error("Clifford index {index} is outside length {len}")]
    CliffordIndexOutOfRange { index: usize, len: usize },

    #[error("Clifford string non-identity metadata is inconsistent")]
    InconsistentCliffordStringMetadata,

    #[error("Tableau index {index} is outside length {len}")]
    TableauIndexOutOfRange { index: usize, len: usize },

    #[error(
        "Tableau has {x_generators} conjugated X generator(s) but {z_generators} conjugated Z generator(s)"
    )]
    TableauGeneratorCountMismatch {
        x_generators: usize,
        z_generators: usize,
    },

    #[error(
        "conjugated {basis} generator {index} has width {width}, expected Tableau width {expected}"
    )]
    TableauGeneratorWidthMismatch {
        basis: PauliBasis,
        index: usize,
        width: usize,
        expected: usize,
    },

    #[error("conjugated X and Z generators at index {index} commute")]
    ConjugatedGeneratorPairCommutes { index: usize },

    #[error(
        "conjugated {left_basis} generator {left_index} anticommutes with conjugated {right_basis} generator {right_index}"
    )]
    ConjugatedGeneratorsAnticommute {
        left_basis: PauliBasis,
        left_index: usize,
        right_basis: PauliBasis,
        right_index: usize,
    },

    #[error(
        "{tableau_qubits}-qubit Tableau requires {tableau_qubits} target(s), got {target_count}"
    )]
    TableauTargetCountMismatch {
        tableau_qubits: usize,
        target_count: usize,
    },

    #[error("Tableau target {target} is outside {num_qubits} destination qubit(s)")]
    TableauTargetOutOfRange { target: usize, num_qubits: usize },

    #[error("duplicate Tableau target {target}")]
    DuplicateTableauTarget { target: usize },

    #[error("commuting Pauli string iteration requires 1..64 qubits but got {num_qubits}")]
    InvalidCommutingPauliIteratorQubitCount { num_qubits: usize },

    #[error("Tableau iteration requires fewer than 64 qubits but got {num_qubits}")]
    InvalidTableauIteratorQubitCount { num_qubits: usize },

    #[error("invalid stabilizer flow text {excerpt}")]
    InvalidFlowText { excerpt: TextExcerpt },

    #[error("anti-Hermitian stabilizer flows are not allowed")]
    AntiHermitianFlow,

    /// The two flows' input and output products have an imaginary relative phase.
    #[error("stabilizer flow product anticommutes")]
    InvalidFlowProduct,

    #[error("Tableau is not a Pauli product")]
    NotPauliProduct,

    #[error("failed to derive inverse Tableau row")]
    InvalidTableauInverse,

    /// The stabilizers at input indices `stabilizer` and `conflict < stabilizer` anticommute.
    #[error(
        "stabilizer at index {stabilizer} anticommutes with the earlier stabilizer at index {conflict}"
    )]
    AntiCommutingStabilizer { stabilizer: usize, conflict: usize },

    /// The stabilizer at input index `stabilizer` is generated by the earlier ones.
    #[error("redundant stabilizer at index {stabilizer} is not allowed")]
    RedundantStabilizer { stabilizer: usize },

    /// The earlier stabilizers generate the negation of the stabilizer at input index
    /// `stabilizer`.
    #[error("stabilizer at index {stabilizer} has an inconsistent sign")]
    InconsistentStabilizer { stabilizer: usize },

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

impl StabilizerError {
    /// The memory-budget refusal this error reports, if it is one.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::MemoryBudgetExceeded { exceeded, .. } => Some(exceeded),
            _ => None,
        }
    }
}
