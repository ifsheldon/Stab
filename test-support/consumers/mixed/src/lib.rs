//! Mixed direct-component and facade consumer used to check feature unification.

use std::error::Error;

use stab_algebra::{CliffordString, SingleQubitClifford};
use stab_bits::BitVec;

pub fn exercise_unified_portable_dependencies() -> Result<(usize, usize), Box<dyn Error>> {
    let mut direct_bits = BitVec::from_words_truncated(256, vec![1; 4]);
    let right = BitVec::from_words_truncated(256, vec![2; 4]);
    direct_bits.xor_assign(&right.as_bitslice())?;

    let direct_clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 256))?;
    let facade_clifford = stab_core::CliffordString::from_gates(std::iter::repeat_n(
        stab_core::SingleQubitClifford::S,
        256,
    ))?;

    let circuit = stab_core::Circuit::from_stim_str("M 0\n")?;
    let direct = stab_analysis::circuit_with_inlined_feedback(&circuit)?;
    let facade = stab_core::analysis::circuit_with_inlined_feedback(&circuit)?;
    if direct != facade {
        return Err(std::io::Error::other("facade and owner transform outputs differ").into());
    }

    Ok((direct_bits.len(), direct_clifford.len() + facade_clifford.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_facade_dependencies_share_portable_features() {
        assert_eq!(exercise_unified_portable_dependencies().unwrap(), (256, 512));
    }
}
