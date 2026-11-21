//! Stable Pauli, Clifford, tableau, and stabilizer-flow algebra.

mod bit_runs;
mod budget;
mod clifford;
mod conversions;
mod error;
mod flow;
mod iter;
mod kernels;
mod pauli;
mod tableau;
mod unitary;

pub use budget::StabilizerResource;
pub use clifford::{CliffordString, SingleQubitClifford};
pub use conversions::StabilizerTableauOptions;
pub use error::{StabilizerError, StabilizerResult, TextExcerpt};
pub use flow::{Flow, FlowMeasurementIndex};
pub use iter::{
    CommutingPauliStringIterator, PauliIterationBases, PauliStringIterator, SignIteration,
    TableauIterator,
};
pub use pauli::{FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString};
pub use stab_foundation::{MemoryBudget, MemoryBudgetExceeded, ResourceAmount};
pub use tableau::{Tableau, TableauAppender};
pub use unitary::Endianness;

/// Low-level constructors for algorithms that have already performed resource and shape admission.
///
/// Both skip only the memory-budget admission that [`PauliString::identity_with_budget`] and
/// [`PauliString::from_bases_with_budget`] perform; they are safe and still bounds-check.
pub mod advanced {
    use super::{PauliBasis, PauliSign, PauliString};

    /// Creates an identity Pauli string without a budget admission, after the caller has
    /// admitted `num_qubits`.
    pub fn pauli_identity_unbudgeted(num_qubits: usize) -> PauliString {
        PauliString::identity_unbudgeted(num_qubits)
    }

    /// Creates a Pauli string without a budget admission, after the caller has admitted the
    /// supplied basis count.
    pub fn pauli_from_bases_unbudgeted(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> PauliString {
        PauliString::from_bases_unbudgeted(sign, bases)
    }
}
