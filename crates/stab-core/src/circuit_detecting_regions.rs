use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemDetectorId,
    DemTarget, FlexPauliString, PauliBasis, Target, detection::measurement_record_count,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectingRegionOptions {
    pub detectors: Vec<DemDetectorId>,
    pub ticks: Vec<u64>,
    pub ignore_anticommutation_errors: bool,
}

pub type DetectingRegionMap = BTreeMap<DemDetectorId, BTreeMap<u64, FlexPauliString>>;

pub fn circuit_detecting_regions(
    circuit: &Circuit,
    options: DetectingRegionOptions,
) -> CircuitResult<DetectingRegionMap> {
    if options.ignore_anticommutation_errors {
        return Err(CircuitError::invalid_detector_error_model(
            "detecting regions with ignored anticommutation errors are not implemented",
        ));
    }

    let detectors = options.detectors.into_iter().collect::<BTreeSet<_>>();
    let ticks = options.ticks.into_iter().collect::<BTreeSet<_>>();
    validate_supported_subset(circuit)?;
    let detector_count = detector_count(circuit)?;
    let tick_count = tick_count(circuit)?;
    validate_detector_ids(&detectors, detector_count)?;
    validate_ticks(&ticks, tick_count)?;

    let mut regions = detectors
        .iter()
        .copied()
        .map(|detector| (detector, BTreeMap::new()))
        .collect::<DetectingRegionMap>();
    if detectors.is_empty() || ticks.is_empty() {
        return Ok(regions);
    }

    let mut tracker = SparseReverseFrameTracker::new(
        circuit.count_qubits(),
        measurement_record_count(circuit)?,
        detector_count,
        true,
    );
    let mut current_tick = tick_count;
    undo_circuit_with_snapshots(
        circuit,
        &mut tracker,
        &detectors,
        &ticks,
        &mut current_tick,
        &mut regions,
    )?;
    tracker.undo_implicit_rz_at_start_of_circuit()?;
    Ok(regions)
}

fn undo_circuit_with_snapshots(
    circuit: &Circuit,
    tracker: &mut SparseReverseFrameTracker,
    detectors: &BTreeSet<DemDetectorId>,
    ticks: &BTreeSet<u64>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionMap,
) -> CircuitResult<()> {
    for item in circuit.items().iter().rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                undo_instruction_with_snapshots(
                    instruction,
                    tracker,
                    detectors,
                    ticks,
                    current_tick,
                    regions,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                return Err(unsupported_repeat_block_error(repeat.repeat_count().get()));
            }
        }
    }
    Ok(())
}

fn undo_instruction_with_snapshots(
    instruction: &CircuitInstruction,
    tracker: &mut SparseReverseFrameTracker,
    detectors: &BTreeSet<DemDetectorId>,
    ticks: &BTreeSet<u64>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionMap,
) -> CircuitResult<()> {
    if instruction.gate().canonical_name() == "TICK" {
        *current_tick = current_tick.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "tick count underflowed while extracting detecting regions",
            )
        })?;
        if ticks.contains(current_tick) {
            snapshot_regions(*current_tick, tracker, detectors, regions)?;
        }
    }
    tracker.undo_instruction(instruction)
}

fn snapshot_regions(
    tick: u64,
    tracker: &SparseReverseFrameTracker,
    detectors: &BTreeSet<DemDetectorId>,
    regions: &mut DetectingRegionMap,
) -> CircuitResult<()> {
    for detector in detectors {
        let target = DemTarget::RelativeDetector(*detector);
        let region = tracker.region_for_target(target)?;
        if is_identity_region(&region) {
            continue;
        }
        let Some(detector_regions) = regions.get_mut(detector) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "detector D{} was not initialized in detecting-region output",
                detector.get()
            )));
        };
        detector_regions.insert(tick, region);
    }
    Ok(())
}

fn is_identity_region(region: &FlexPauliString) -> bool {
    (0..region.len()).all(|index| region.get(index).unwrap_or(PauliBasis::I) == PauliBasis::I)
}

fn validate_supported_subset(circuit: &Circuit) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                validate_supported_instruction(instruction)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                return Err(unsupported_repeat_block_error(repeat.repeat_count().get()));
            }
        }
    }
    Ok(())
}

fn validate_supported_instruction(instruction: &CircuitInstruction) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        "H" => validate_single_plain_qubit_targets(instruction),
        "CX" | "MXX" => validate_plain_qubit_pair_targets(instruction),
        "TICK" => validate_target_count(instruction, 0),
        "DETECTOR" => validate_detector_targets(instruction),
        name => Err(CircuitError::invalid_detector_error_model(format!(
            "simple detecting-region extraction does not support gate {name}"
        ))),
    }
}

fn unsupported_repeat_block_error(repeat_count: u64) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!(
        "simple detecting-region extraction does not support repeat blocks with count {repeat_count}"
    ))
}

fn validate_single_plain_qubit_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target in instruction.targets() {
        validate_plain_qubit_target(instruction, target)?;
    }
    Ok(())
}

fn validate_plain_qubit_pair_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} with qubit target pairs",
                instruction.gate().canonical_name()
            )));
        };
        validate_plain_qubit_target(instruction, left)?;
        validate_plain_qubit_target(instruction, right)?;
    }
    Ok(())
}

fn validate_plain_qubit_target(
    instruction: &CircuitInstruction,
    target: &Target,
) -> CircuitResult<()> {
    match target {
        Target::Qubit {
            inverted: false, ..
        } => Ok(()),
        _ => Err(CircuitError::invalid_detector_error_model(format!(
            "simple detecting-region extraction only supports {} with plain qubit targets, got {target}",
            instruction.gate().canonical_name()
        ))),
    }
}

fn validate_target_count(instruction: &CircuitInstruction, expected: usize) -> CircuitResult<()> {
    if instruction.targets().len() == expected {
        return Ok(());
    }
    Err(CircuitError::invalid_detector_error_model(format!(
        "simple detecting-region extraction expected {} to have {expected} target(s), got {}",
        instruction.gate().canonical_name(),
        instruction.targets().len()
    )))
}

fn validate_detector_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    for target in instruction.targets() {
        if !target.is_measurement_record_target() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports DETECTOR measurement-record targets, got {target}"
            )));
        }
    }
    Ok(())
}

fn detector_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut count = 0u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "DETECTOR" {
                    count = count.checked_add(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("detector count overflowed")
                    })?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                return Err(unsupported_repeat_block_error(repeat.repeat_count().get()));
            }
        }
    }
    Ok(count)
}

fn tick_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut count = 0u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "TICK" {
                    count = count.checked_add(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("tick count overflowed")
                    })?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                return Err(unsupported_repeat_block_error(repeat.repeat_count().get()));
            }
        }
    }
    Ok(count)
}

fn validate_detector_ids(
    detectors: &BTreeSet<DemDetectorId>,
    detector_count: u64,
) -> CircuitResult<()> {
    for detector in detectors {
        if detector.get() >= detector_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "requested detector D{} but circuit only has {detector_count} detector(s)",
                detector.get()
            )));
        }
    }
    Ok(())
}

fn validate_ticks(ticks: &BTreeSet<u64>, tick_count: u64) -> CircuitResult<()> {
    for tick in ticks {
        if *tick >= tick_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "requested tick {tick} but circuit only has {tick_count} tick layer(s)"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "parity tests use fixed detector and tick ids for compact expected maps"
    )]

    use super::*;

    fn detector(id: u64) -> DemDetectorId {
        DemDetectorId::try_new(id).unwrap()
    }

    fn regions(text: &str, detectors: Vec<DemDetectorId>, ticks: Vec<u64>) -> DetectingRegionMap {
        let circuit = Circuit::from_stim_str(text).unwrap();
        circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors,
                ticks,
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn detecting_regions_simple_h_cx_mxx() {
        let actual = regions(
            "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
            vec![detector(0)],
            vec![0, 1],
        );

        assert_eq!(actual[&detector(0)][&0].to_string(), "+X_");
        assert_eq!(actual[&detector(0)][&1].to_string(), "+XX");
    }

    #[test]
    fn detecting_regions_deduplicates_requested_ids() {
        let actual = regions(
            "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
            vec![detector(0), detector(0)],
            vec![1, 0, 1],
        );

        assert_eq!(actual.len(), 1);
        assert_eq!(actual[&detector(0)].len(), 2);
    }

    #[test]
    fn detecting_regions_rejects_unknown_detector() {
        let circuit = Circuit::from_stim_str("MXX 0 1\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(1)],
                ticks: vec![],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("requested detector D1"));
    }

    #[test]
    fn detecting_regions_rejects_out_of_range_tick() {
        let circuit = Circuit::from_stim_str("TICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![1],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("requested tick 1"));
    }

    #[test]
    fn detecting_regions_rejects_ignored_anticommutation_mode() {
        let circuit = Circuit::from_stim_str("TICK\nM 0\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: true,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("ignored anticommutation"));
    }

    #[test]
    fn detecting_regions_rejects_false_mode_anticommutation() {
        let circuit = Circuit::from_stim_str(
            "MXX 0 1\n\
             DETECTOR rec[-1]\n\
             TICK\n\
             H 0\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
        )
        .unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("anti-commuted"));
    }

    #[test]
    fn detecting_regions_rejects_implicit_start_state_anticommutation() {
        let circuit = Circuit::from_stim_str("TICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("anti-commuted"));
    }

    #[test]
    fn detecting_regions_omits_identity_snapshots() {
        let actual = regions(
            "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n\
             TICK\n",
            vec![detector(0)],
            vec![2],
        );

        assert!(actual[&detector(0)].is_empty());
    }

    #[test]
    fn detecting_regions_rejects_unsupported_gate() {
        let circuit = Circuit::from_stim_str("R 0\nTICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("does not support gate R"));
    }

    #[test]
    fn detecting_regions_rejects_feedback_controlled_cx() {
        let circuit =
            Circuit::from_stim_str("MXX 0 1\nCX rec[-1] 2\nTICK\nMXX 2 3\nDETECTOR rec[-1]\n")
                .unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("plain qubit targets"));
    }

    #[test]
    fn detecting_regions_rejects_sweep_controlled_cx() {
        let circuit =
            Circuit::from_stim_str("CX sweep[0] 2\nTICK\nMXX 2 3\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("plain qubit targets"));
    }

    #[test]
    fn detecting_regions_rejects_repeat_blocks() {
        let circuit =
            Circuit::from_stim_str("REPEAT 2 {\n    H 0\n    TICK\n}\nMXX 0 1\nDETECTOR rec[-1]\n")
                .unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("does not support repeat blocks"));
    }
}
