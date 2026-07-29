use crate::{Circuit, CircuitResult};

pub use stab_analysis::MissingDetectorOptions;

pub fn missing_detectors(
    circuit: &Circuit,
    options: MissingDetectorOptions,
) -> CircuitResult<Circuit> {
    stab_analysis::missing_detectors(circuit, options).map_err(Into::into)
}
