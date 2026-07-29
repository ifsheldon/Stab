use crate::{Circuit, CircuitResult, Gate, Tableau};

pub fn circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
) -> CircuitResult<Tableau> {
    stab_analysis::circuit_to_tableau(circuit, ignore_noise, ignore_measurement, ignore_reset)
        .map_err(Into::into)
}

pub(crate) fn gate_tableau(gate_name: &str) -> CircuitResult<Tableau> {
    let gate = Gate::from_name(gate_name)?;
    stab_analysis::gate_tableau(gate).map_err(Into::into)
}
