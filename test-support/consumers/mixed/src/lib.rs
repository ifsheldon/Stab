//! Mixed direct-component and facade consumer used to check feature unification.

use std::error::Error;

use stab_algebra::{CliffordString, SingleQubitClifford};
use stab_bits::BitVec;

pub fn exercise_unified_portable_dependencies() -> Result<(usize, usize), Box<dyn Error>> {
    let mut direct_bits = BitVec::from_words_truncated(256, vec![1; 4]);
    let facade_bits = stab_core::advanced::storage::BitVec::from_words_truncated(256, vec![2; 4]);
    direct_bits.xor_assign(&facade_bits.as_bitslice())?;

    let direct_clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 256))?;
    let facade_clifford = stab_core::CliffordString::from_gates(std::iter::repeat_n(
        stab_core::SingleQubitClifford::S,
        256,
    ))?;

    Ok((direct_bits.len(), direct_clifford.len() + facade_clifford.len()))
}
