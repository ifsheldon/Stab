pub use stab_analysis::CircuitFlattenLimits;

use crate::{Circuit, CircuitInstruction, CircuitResult};

pub fn flattened_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::flattened_circuit(circuit).map_err(Into::into)
}

pub fn flattened_circuit_with_limits(
    circuit: &Circuit,
    limits: CircuitFlattenLimits,
) -> CircuitResult<Circuit> {
    stab_analysis::flattened_circuit_with_limits(circuit, limits).map_err(Into::into)
}

pub fn flattened_circuit_operations(circuit: &Circuit) -> CircuitResult<Vec<CircuitInstruction>> {
    stab_analysis::flattened_circuit_operations(circuit).map_err(Into::into)
}

pub fn flattened_circuit_operations_with_limits(
    circuit: &Circuit,
    limits: CircuitFlattenLimits,
) -> CircuitResult<Vec<CircuitInstruction>> {
    stab_analysis::flattened_circuit_operations_with_limits(circuit, limits).map_err(Into::into)
}

pub fn circuit_without_noise(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::circuit_without_noise(circuit).map_err(Into::into)
}
