use super::{CompiledSampler, ReferenceSampleTree};
use crate::{Circuit, CircuitResult};

/// Computes Stim's deterministic reference sample for a circuit.
pub fn circuit_reference_sample(circuit: &Circuit) -> CircuitResult<Vec<bool>> {
    CompiledSampler::compile_allowing_sweep(circuit)?.reference_sample()
}

/// Computes a reference-sample tree for a circuit.
pub fn circuit_reference_sample_tree(circuit: &Circuit) -> CircuitResult<ReferenceSampleTree> {
    ReferenceSampleTree::from_circuit_reference_sample(circuit).map_err(Into::into)
}
