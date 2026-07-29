use crate::{Circuit, CircuitInstruction, CircuitResult};

pub fn simplified_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::simplified_circuit(circuit).map_err(Into::into)
}

pub fn decomposed_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::decomposed_circuit(circuit).map_err(Into::into)
}

pub(crate) fn decomposed_single_instruction(
    instruction: &CircuitInstruction,
) -> CircuitResult<Circuit> {
    stab_analysis::advanced::decomposed_single_instruction(instruction).map_err(Into::into)
}
