//! Nightly external-consumer fixture for the portable Stab facade.

use std::error::Error;

use stab_core::{BitVec, CliffordString, SingleQubitClifford};

pub fn exercise_portable_facade() -> Result<(usize, usize), Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x5555_aaaa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaaaa_5555; 5]);
    left.xor_assign(&right.as_bitslice())?;

    let mut clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 257))?;
    let phase =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, 257))?;
    clifford.right_multiply_in_place(&phase)?;

    Ok((left.len(), clifford.len()))
}
