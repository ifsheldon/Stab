use crate::{Circuit, CircuitResult, DemTarget};

pub use stab_analysis::{
    DetectingRegionMap, DetectingRegionOptions, DetectingRegionTargetMap,
    DetectingRegionTargetOptions,
};

pub fn circuit_detecting_regions(
    circuit: &Circuit,
    options: DetectingRegionOptions,
) -> CircuitResult<DetectingRegionMap> {
    stab_analysis::circuit_detecting_regions(circuit, options).map_err(Into::into)
}

pub fn circuit_detecting_regions_for_targets(
    circuit: &Circuit,
    options: DetectingRegionTargetOptions,
) -> CircuitResult<DetectingRegionTargetMap> {
    stab_analysis::circuit_detecting_regions_for_targets(circuit, options).map_err(Into::into)
}

pub fn all_detecting_region_targets(circuit: &Circuit) -> CircuitResult<Vec<DemTarget>> {
    stab_analysis::all_detecting_region_targets(circuit).map_err(Into::into)
}

pub fn all_detecting_region_ticks(circuit: &Circuit) -> CircuitResult<Vec<u64>> {
    stab_analysis::all_detecting_region_ticks(circuit).map_err(Into::into)
}
