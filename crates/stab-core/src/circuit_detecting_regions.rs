use std::collections::BTreeMap;

use stab_model::CircuitTick;

use crate::{Circuit, CircuitResult, DemDetectorId, DemTarget, FlexPauliString};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectingRegionOptions {
    pub detectors: Vec<DemDetectorId>,
    pub ticks: Vec<u64>,
    pub ignore_anticommutation_errors: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectingRegionTargetOptions {
    pub targets: Vec<DemTarget>,
    pub ticks: Vec<u64>,
    pub ignore_anticommutation_errors: bool,
}

pub type DetectingRegionMap = BTreeMap<DemDetectorId, BTreeMap<u64, FlexPauliString>>;
pub type DetectingRegionTargetMap = BTreeMap<DemTarget, BTreeMap<u64, FlexPauliString>>;

pub fn circuit_detecting_regions(
    circuit: &Circuit,
    options: DetectingRegionOptions,
) -> CircuitResult<DetectingRegionMap> {
    let regions = stab_analysis::circuit_detecting_regions(
        circuit,
        stab_analysis::DetectingRegionOptions {
            detectors: options.detectors,
            ticks: options.ticks.into_iter().map(CircuitTick::new).collect(),
            ignore_anticommutation_errors: options.ignore_anticommutation_errors,
        },
    )?;
    Ok(regions
        .into_iter()
        .map(|(detector, ticks)| {
            (
                detector,
                ticks
                    .into_iter()
                    .map(|(tick, region)| (tick.get(), region))
                    .collect(),
            )
        })
        .collect())
}

pub fn circuit_detecting_regions_for_targets(
    circuit: &Circuit,
    options: DetectingRegionTargetOptions,
) -> CircuitResult<DetectingRegionTargetMap> {
    let regions = stab_analysis::circuit_detecting_regions_for_targets(
        circuit,
        stab_analysis::DetectingRegionTargetOptions {
            targets: options.targets,
            ticks: options.ticks.into_iter().map(CircuitTick::new).collect(),
            ignore_anticommutation_errors: options.ignore_anticommutation_errors,
        },
    )?;
    Ok(regions
        .into_iter()
        .map(|(target, ticks)| {
            (
                target,
                ticks
                    .into_iter()
                    .map(|(tick, region)| (tick.get(), region))
                    .collect(),
            )
        })
        .collect())
}

pub fn all_detecting_region_targets(circuit: &Circuit) -> CircuitResult<Vec<DemTarget>> {
    stab_analysis::all_detecting_region_targets(circuit).map_err(Into::into)
}

pub fn all_detecting_region_ticks(circuit: &Circuit) -> CircuitResult<Vec<u64>> {
    Ok(stab_analysis::all_detecting_region_ticks(circuit)?
        .into_iter()
        .map(CircuitTick::get)
        .collect())
}
