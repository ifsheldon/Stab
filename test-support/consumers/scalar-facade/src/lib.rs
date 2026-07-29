//! Nightly facade fixture with portable SIMD explicitly disabled.

use std::error::Error;

pub use stab_core::experimental;

use stab_core::advanced::{
    backend::BackendPreference,
    compat::CompiledSampler,
    records::DetsLayout,
    storage::BitVec,
    traversal::CircuitFlattenedInstructionIter,
};
use stab_core::{Circuit, SamplingCompiler};

pub fn exercise_scalar_facade() -> Result<(usize, usize, usize), Box<dyn Error>> {
    let mut left = BitVec::from_words_truncated(257, vec![0x55aa; 5]);
    let right = BitVec::from_words_truncated(257, vec![0xaa55; 5]);
    left.xor_assign(&right.as_bitslice())?;

    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n")?;
    let _: CircuitFlattenedInstructionIter<'_> = circuit.iter_flattened_instructions();
    let layout = DetsLayout::try_new(1, 1, 0)?;
    let plan = SamplingCompiler::new()
        .backend(BackendPreference::Scalar)
        .compile(&circuit)?;
    let adapter = CompiledSampler::compile(&circuit)?;

    Ok((
        left.len(),
        layout.total_bits(),
        plan.measurement_width().get() + adapter.plan().measurement_width().get(),
    ))
}
