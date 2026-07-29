use crate::{Circuit, CircuitResult};

pub fn circuit_with_inlined_feedback(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::circuit_with_inlined_feedback(circuit).map_err(Into::into)
}
