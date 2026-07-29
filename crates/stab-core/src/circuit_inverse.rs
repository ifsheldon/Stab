use crate::{Circuit, CircuitResult, Flow};

pub use stab_analysis::{InverseQecOptions, TimeReversedForFlowsOptions};

pub fn circuit_inverse_unitary(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::circuit_inverse_unitary(circuit).map_err(Into::into)
}

pub fn circuit_inverse_qec(circuit: &Circuit) -> CircuitResult<Circuit> {
    stab_analysis::circuit_inverse_qec(circuit).map_err(Into::into)
}

pub fn circuit_inverse_qec_with_options(
    circuit: &Circuit,
    options: InverseQecOptions,
) -> CircuitResult<Circuit> {
    stab_analysis::circuit_inverse_qec_with_options(circuit, options).map_err(Into::into)
}

pub fn circuit_time_reversed_for_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    stab_analysis::circuit_time_reversed_for_flows(circuit, flows).map_err(Into::into)
}

pub fn circuit_time_reversed_for_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    stab_analysis::circuit_time_reversed_for_flows_with_options(circuit, flows, options)
        .map_err(Into::into)
}
