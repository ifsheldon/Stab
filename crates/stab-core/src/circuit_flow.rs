use crate::{Circuit, CircuitResult, Flow};

pub use stab_analysis::{
    UnsignedStabilizerFlowCheck, UnsignedStabilizerFlowFailure,
    check_if_circuit_has_unsigned_stabilizer_flows,
    check_unsigned_stabilizer_flows_with_diagnostics, circuit_has_all_unsigned_stabilizer_flows,
    circuit_has_unsigned_stabilizer_flow,
};

pub(crate) use stab_analysis::advanced::flow_record_index;

pub fn circuit_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    stab_analysis::circuit_flow_generators(circuit).map_err(Into::into)
}

pub fn solve_for_flow_measurements(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Vec<Option<Vec<i32>>>> {
    stab_analysis::solve_for_flow_measurements(circuit, flows).map_err(Into::into)
}
