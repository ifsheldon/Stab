use crate::{Circuit, CircuitResult, Tableau};

pub fn circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
) -> CircuitResult<Tableau> {
    stab_analysis::circuit_to_tableau(circuit, ignore_noise, ignore_measurement, ignore_reset)
        .map_err(Into::into)
}
