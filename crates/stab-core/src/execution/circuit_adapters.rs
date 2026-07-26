use super::{CompiledSampler, reference_sample_tree::ReferenceSampleTree};
use crate::{Circuit, CircuitResult};

/// Computes Stim's deterministic reference sample for a circuit.
pub fn circuit_reference_sample(circuit: &Circuit) -> CircuitResult<Vec<bool>> {
    Ok(CompiledSampler::compile_allowing_sweep(circuit)?.reference_sample())
}

/// Computes a reference-sample tree for a circuit.
pub fn circuit_reference_sample_tree(circuit: &Circuit) -> CircuitResult<ReferenceSampleTree> {
    ReferenceSampleTree::from_circuit_reference_sample(circuit)
}

// Temporary pre-0.2 method adapters. The free execution functions own sampler-backed behavior.
impl Circuit {
    /// Computes Stim's deterministic reference sample for this circuit.
    ///
    /// This compatibility method compiles the circuit with sweep-bit support and returns the
    /// deterministic reference branch used by sampling and detection conversion.
    pub fn reference_sample(&self) -> CircuitResult<Vec<bool>> {
        circuit_reference_sample(self)
    }

    /// Computes a compact reference-sample tree for this circuit.
    ///
    /// The tree preserves folded repeat structure and supports indexed lookup without
    /// materializing one boolean per repeated measurement.
    pub fn reference_sample_tree(&self) -> CircuitResult<ReferenceSampleTree> {
        circuit_reference_sample_tree(self)
    }

    /// Counts deterministic measurement results under known-zero or unknown-input assumptions.
    ///
    /// This is a simulator-backed semantic query. Unsupported execution behavior returns a
    /// circuit error instead of contributing an approximate count.
    pub fn count_determined_measurements(&self, unknown_input: bool) -> CircuitResult<u64> {
        super::count_determined_measurements(self, unknown_input)
    }
}
