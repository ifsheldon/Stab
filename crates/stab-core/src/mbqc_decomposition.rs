use crate::{Circuit, CircuitResult, Gate};

/// Returns an MBQC decomposition for the supported M6 gate subset.
///
/// The subset covers selected measurement, reset, Pauli, phase, identity, and `CX`
/// decompositions. Unsupported annotation, noise, heralded, and unimplemented gates
/// return `Ok(None)` instead of a partial circuit.
pub fn mbqc_decomposition(gate: Gate) -> CircuitResult<Option<Circuit>> {
    stab_analysis::mbqc_decomposition(gate).map_err(Into::into)
}
