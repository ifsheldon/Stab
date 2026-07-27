mod clifford;
mod scalar;

pub(crate) use clifford::{
    CliffordNonIdentityCounts, CliffordPlanes, CliffordPlanesMut, clifford_right_multiply_words,
};
pub use stab_bits::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PauliWordProduct {
    pub(crate) count_bit_1: u64,
    pub(crate) count_bit_2: u64,
    pub(crate) has_terms: bool,
}

pub(crate) fn pauli_right_multiply_words(
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> PauliWordProduct {
    let (count_bit_1, count_bit_2, has_terms) =
        scalar::pauli_right_multiply_words(left_x, left_z, right_x, right_z);
    PauliWordProduct {
        count_bit_1,
        count_bit_2,
        has_terms,
    }
}
