use std::fmt::{Display, Formatter};

use super::{StabilizerError, StabilizerResult};

// Retains the one-million-qubit Pauli benchmark and the 65,536-qubit sparse regression.
const MAX_PAULI_QUBITS: usize = 1_048_576;
// Keeps Clifford storage in the same linear-materialization class as Pauli storage.
const MAX_CLIFFORD_QUBITS: usize = 1_048_576;
// Preserves the pinned 500-qubit regression while bounding quadratic dense Tableau storage.
const MAX_TABLEAU_QUBITS: usize = 512;
// Bounds repeated Tableau composition in the current random-construction algorithm.
const MAX_RANDOM_TABLEAU_QUBITS: usize = 64;
// Bounds the current dense Gaussian-elimination and destabilizer synthesis state.
const MAX_STABILIZER_SOLVE_QUBITS: usize = 512;
// Bounds cubic unitarity checks and the later matrix conjugation work.
const MAX_UNITARY_MATRIX_DIMENSION: usize = 64;

/// A deterministic materialization or work limit enforced by stabilizer-algebra APIs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StabilizerResource {
    /// Qubits stored by an owned Pauli string or its iterator state.
    PauliQubits,
    /// Entries stored by an owned Clifford string.
    CliffordQubits,
    /// Qubits stored by a dense Tableau.
    TableauQubits,
    /// Qubits accepted by the current random-Tableau construction algorithm.
    RandomTableauQubits,
    /// Qubits accepted by the current stabilizer-to-Tableau solver.
    StabilizerSolveQubits,
    /// Rows and columns accepted by unitary-matrix conversion.
    UnitaryMatrixDimension,
}

impl StabilizerResource {
    /// Returns the largest accepted value for this resource category.
    pub const fn limit(self) -> usize {
        match self {
            Self::PauliQubits => MAX_PAULI_QUBITS,
            Self::CliffordQubits => MAX_CLIFFORD_QUBITS,
            Self::TableauQubits => MAX_TABLEAU_QUBITS,
            Self::RandomTableauQubits => MAX_RANDOM_TABLEAU_QUBITS,
            Self::StabilizerSolveQubits => MAX_STABILIZER_SOLVE_QUBITS,
            Self::UnitaryMatrixDimension => MAX_UNITARY_MATRIX_DIMENSION,
        }
    }

    pub(crate) fn ensure(self, requested: usize) -> StabilizerResult<()> {
        let limit = self.limit();
        if requested <= limit {
            Ok(())
        } else {
            Err(StabilizerError::ResourceLimitExceeded {
                resource: self,
                requested,
                limit,
            })
        }
    }
}

impl Display for StabilizerResource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::PauliQubits => "Pauli qubits",
            Self::CliffordQubits => "Clifford qubits",
            Self::TableauQubits => "Tableau qubits",
            Self::RandomTableauQubits => "random Tableau qubits",
            Self::StabilizerSolveQubits => "stabilizer-solve qubits",
            Self::UnitaryMatrixDimension => "unitary matrix dimension",
        })
    }
}
