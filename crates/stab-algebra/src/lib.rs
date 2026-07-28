//! Stable Pauli, Clifford, tableau, and stabilizer-flow algebra.

mod clifford;
mod conversions;
mod error;
mod flow;
mod iter;
mod kernels;
mod limits;
mod pauli;
mod tableau;
mod unitary;

pub use clifford::{CliffordString, SingleQubitClifford};
pub use conversions::stabilizers_to_tableau;
pub use error::{StabilizerError, StabilizerResult};
pub use flow::{Flow, FlowMeasurementIndex};
pub use iter::{CommutingPauliStringIterator, PauliStringIterator, TableauIterator};
pub use limits::StabilizerResource;
pub use pauli::{FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString};
pub use tableau::Tableau;
pub use unitary::unitary_to_tableau;

/// Low-level constructors for algorithms that have already performed resource and shape admission.
pub mod advanced {
    use super::{PauliBasis, PauliSign, PauliString, Tableau};

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

    /// Creates an identity tableau after the caller has admitted `num_qubits`.
    pub fn tableau_identity_unchecked(num_qubits: usize) -> Tableau {
        Tableau::identity_unchecked(num_qubits)
    }

    /// Creates a tableau from already validated output columns.
    pub fn tableau_from_output_columns_unchecked(
        xs: Vec<PauliString>,
        zs: Vec<PauliString>,
    ) -> Tableau {
        Tableau::from_output_columns_unchecked(xs, zs)
    }
}
