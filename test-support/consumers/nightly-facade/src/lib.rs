//! Nightly external-consumer fixture for the portable Stab facade.

use std::error::Error;

use stab_core::{CliffordString, SingleQubitClifford};

pub fn exercise_portable_facade() -> Result<usize, Box<dyn Error>> {
    let mut clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 257))?;
    let phase =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, 257))?;
    clifford.right_multiply_in_place(&phase)?;
    Ok(clifford.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_facade_executes_clifford_work() {
        assert_eq!(exercise_portable_facade().unwrap(), 257);
    }
}
