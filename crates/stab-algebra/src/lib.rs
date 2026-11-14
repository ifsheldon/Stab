//! Stable Pauli, Clifford, tableau, and stabilizer-flow algebra.

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

pub use budget::{AlgebraMemoryBudget, StabilizerResource};
pub use clifford::{CliffordString, SingleQubitClifford};
pub use conversions::{stabilizers_to_tableau, stabilizers_to_tableau_with_budget};
pub use error::{StabilizerError, StabilizerResult};
pub use flow::{Flow, FlowMeasurementIndex};
pub use iter::{CommutingPauliStringIterator, PauliStringIterator, TableauIterator};
pub use pauli::{FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString};
pub use tableau::Tableau;
pub use unitary::unitary_to_tableau;

/// Low-level constructors for algorithms that have already performed resource and shape admission.
pub mod advanced {
    use super::{PauliBasis, PauliSign, PauliString};

    /// Creates an identity Pauli string after the caller has admitted `num_qubits`.
    pub fn pauli_identity_unchecked(num_qubits: usize) -> PauliString {
        PauliString::identity_unchecked(num_qubits)
    }

    /// Creates a Pauli string after the caller has admitted the supplied basis count.
    pub fn pauli_from_bases_unchecked(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> PauliString {
        PauliString::from_bases_unchecked(sign, bases)
    }
}
