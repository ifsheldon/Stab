use crate::{Circuit, CircuitResult};

pub fn simplified_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::simplified_circuit(circuit).map_err(Into::into)
}

pub fn decomposed_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::decomposed_circuit(circuit).map_err(Into::into)
}
