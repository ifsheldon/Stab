//! Stable external-consumer fixture for the extracted component stack.

use std::error::Error;

use stab_algebra::{CliffordString, SingleQubitClifford};
use stab_analysis::circuit_without_tags;
use stab_bits::BitVec;
use stab_engine::SamplingCompiler;
use stab_model::Circuit;
use stab_records::PackedShotBatch;

pub fn exercise_stable_components() -> Result<(usize, usize), Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaa55; 5]);
    left.xor_assign(&right.as_bitslice())?;

    let mut clifford =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, 257))?;
    let phase =
        CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::S, 257))?;
    clifford.right_multiply_in_place(&phase)?;

    let circuit = Circuit::from_stim_str("M 0\n")?;
    let stripped = circuit_without_tags(&circuit);
    let _plan = SamplingCompiler::new().compile(&stripped)?;
    let records = PackedShotBatch::zeros(2, 1)?;

    Ok((left.len(), records.shot_count()))
}
