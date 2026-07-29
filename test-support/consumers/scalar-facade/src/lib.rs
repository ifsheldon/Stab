//! Nightly facade fixture with portable SIMD explicitly disabled.

use std::error::Error;

use stab_core::BitVec;

pub fn exercise_scalar_facade() -> Result<usize, Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaa55; 5]);
    left.xor_assign(&right.as_bitslice())?;
    Ok(left.len())
}
