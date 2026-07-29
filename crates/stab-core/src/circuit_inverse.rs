use crate::{Circuit, CircuitResult, Flow};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InverseQecOptions {
    /// Preserve selected measurement records instead of turning them into resets.
    pub keep_measurements: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeReversedForFlowsOptions {
    /// Keep measurements as measurements instead of converting eligible ones to resets.
    pub dont_turn_measurements_into_resets: bool,
}

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
    stab_analysis::circuit_inverse_qec_with_options(
        circuit,
        stab_analysis::InverseQecOptions {
            keep_measurements: options.keep_measurements,
        },
    )
    .map_err(Into::into)
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
    stab_analysis::circuit_time_reversed_for_flows_with_options(
        circuit,
        flows,
        stab_analysis::TimeReversedForFlowsOptions {
            dont_turn_measurements_into_resets: options.dont_turn_measurements_into_resets,
        },
    )
    .map_err(Into::into)
}
