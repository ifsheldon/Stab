use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemDetectorId,
    DemTarget, FlexPauliString, PauliBasis, Target, detection::measurement_record_count,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

const MAX_DETECTING_REGION_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_DETECTING_REGION_REPEAT_ITERATIONS: u64 = 1_000_000;
const MAX_DETECTING_REGION_HELPER_TARGETS: u64 = MAX_DETECTING_REGION_EXPANDED_INSTRUCTIONS;
const MAX_DETECTING_REGION_OBSERVABLE_TARGETS: u64 = u32::MAX as u64 + 1;

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
    let target_regions = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: options
                .detectors
                .into_iter()
                .map(DemTarget::RelativeDetector)
                .collect(),
            ticks: options.ticks,
            ignore_anticommutation_errors: options.ignore_anticommutation_errors,
        },
    )?;
    Ok(target_regions
        .into_iter()
        .filter_map(|(target, regions)| match target {
            DemTarget::RelativeDetector(detector) => Some((detector, regions)),
            DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => None,
        })
        .collect())
}

pub fn circuit_detecting_regions_for_targets(
    circuit: &Circuit,
    options: DetectingRegionTargetOptions,
) -> CircuitResult<DetectingRegionTargetMap> {
    if options.ignore_anticommutation_errors {
        return Err(CircuitError::invalid_detector_error_model(
            "detecting regions with ignored anticommutation errors are not implemented",
        ));
    }

    let targets = options.targets.into_iter().collect::<BTreeSet<_>>();
    let ticks = options.ticks.into_iter().collect::<BTreeSet<_>>();
    validate_supported_subset(circuit)?;
    let detector_count = detector_count(circuit)?;
    let observable_count = observable_count(circuit)?;
    let tick_count = tick_count(circuit)?;
    validate_targets(&targets, detector_count, observable_count)?;
    validate_ticks(&ticks, tick_count)?;

    let mut regions = targets
        .iter()
        .copied()
        .map(|target| (target, BTreeMap::new()))
        .collect::<DetectingRegionTargetMap>();
    if targets.is_empty() || ticks.is_empty() {
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
        &targets,
        &ticks,
        &mut current_tick,
        &mut regions,
    )?;
    tracker.undo_implicit_rz_at_start_of_circuit()?;
    Ok(regions)
}

pub fn all_detecting_region_targets(circuit: &Circuit) -> CircuitResult<Vec<DemTarget>> {
    let detector_count = detector_count(circuit)?;
    let observable_count = observable_count(circuit)?;
    let target_capacity = dense_target_helper_capacity(detector_count, observable_count)?;
    let mut targets = Vec::with_capacity(target_capacity);
    for detector in 0..detector_count {
        targets.push(DemTarget::relative_detector(detector)?);
    }
    for observable in 0..observable_count {
        targets.push(DemTarget::logical_observable(observable)?);
    }
    Ok(targets)
}

fn dense_target_helper_capacity(
    detector_count: u64,
    observable_count: u64,
) -> CircuitResult<usize> {
    if observable_count > MAX_DETECTING_REGION_OBSERVABLE_TARGETS {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "detecting-region all-target helper cannot materialize {observable_count} observable target(s); logical-observable targets are limited to {MAX_DETECTING_REGION_OBSERVABLE_TARGETS}"
        )));
    }
    let target_count = detector_count
        .checked_add(observable_count)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "detecting-region all-target helper target count overflowed",
            )
        })?;
    if target_count > MAX_DETECTING_REGION_HELPER_TARGETS {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "detecting-region all-target helper currently supports at most {MAX_DETECTING_REGION_HELPER_TARGETS} materialized target(s), got {target_count}"
        )));
    }
    usize::try_from(target_count).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!(
            "detecting-region all-target helper target count {target_count} does not fit in memory on this platform"
        ))
    })
}

pub fn all_detecting_region_ticks(circuit: &Circuit) -> CircuitResult<Vec<u64>> {
    let tick_count = tick_count(circuit)?;
    if tick_count > MAX_DETECTING_REGION_REPEAT_ITERATIONS {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "detecting-region all-tick helper currently supports at most {MAX_DETECTING_REGION_REPEAT_ITERATIONS} ticks, got {tick_count}"
        )));
    }
    Ok((0..tick_count).collect())
}

fn undo_circuit_with_snapshots(
    circuit: &Circuit,
    tracker: &mut SparseReverseFrameTracker,
    targets: &BTreeSet<DemTarget>,
    ticks: &BTreeSet<u64>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
) -> CircuitResult<()> {
    for item in circuit.items().iter().rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                undo_instruction_with_snapshots(
                    instruction,
                    tracker,
                    targets,
                    ticks,
                    current_tick,
                    regions,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                for _ in 0..repeat.repeat_count().get() {
                    undo_circuit_with_snapshots(
                        repeat.body(),
                        tracker,
                        targets,
                        ticks,
                        current_tick,
                        regions,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn undo_instruction_with_snapshots(
    instruction: &CircuitInstruction,
    tracker: &mut SparseReverseFrameTracker,
    targets: &BTreeSet<DemTarget>,
    ticks: &BTreeSet<u64>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
) -> CircuitResult<()> {
    if instruction.gate().canonical_name() == "TICK" {
        *current_tick = current_tick.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "tick count underflowed while extracting detecting regions",
            )
        })?;
        if ticks.contains(current_tick) {
            snapshot_regions(*current_tick, tracker, targets, regions)?;
        }
    }
    tracker.undo_instruction(instruction)
}

fn snapshot_regions(
    tick: u64,
    tracker: &SparseReverseFrameTracker,
    targets: &BTreeSet<DemTarget>,
    regions: &mut DetectingRegionTargetMap,
) -> CircuitResult<()> {
    for target in targets {
        let region = tracker.region_for_target(*target)?;
        if is_identity_region(&region) {
            continue;
        }
        let Some(target_regions) = regions.get_mut(target) else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "target {target} was not initialized in detecting-region output",
            )));
        };
        target_regions.insert(tick, region);
    }
    Ok(())
}

fn is_identity_region(region: &FlexPauliString) -> bool {
    (0..region.len()).all(|index| region.get(index).unwrap_or(PauliBasis::I) == PauliBasis::I)
}

fn validate_supported_subset(circuit: &Circuit) -> CircuitResult<()> {
    let mut budget = DetectingRegionBudget::default();
    validate_supported_subset_inner(circuit, 1, &mut budget)
}

fn validate_supported_subset_inner(
    circuit: &Circuit,
    multiplier: u64,
    budget: &mut DetectingRegionBudget,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                budget.add_expanded_instructions(multiplier)?;
                validate_supported_instruction(instruction)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "detecting-region repeat expansion count overflowed",
                        )
                    })?;
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_supported_subset_inner(repeat.body(), repeated_multiplier, budget)?;
            }
        }
    }
    Ok(())
}

fn validate_supported_instruction(instruction: &CircuitInstruction) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        "H" | "R" | "RX" | "RY" | "M" | "MX" | "MY" => {
            validate_single_plain_qubit_targets(instruction)
        }
        "CX" | "MXX" | "MYY" | "MZZ" => validate_plain_qubit_pair_targets(instruction),
        "TICK" => validate_target_count(instruction, 0),
        "DETECTOR" => validate_detector_targets(instruction),
        "OBSERVABLE_INCLUDE" => validate_observable_include_targets(instruction),
        name => Err(CircuitError::invalid_detector_error_model(format!(
            "simple detecting-region extraction does not support gate {name}"
        ))),
    }
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

fn validate_observable_include_targets(instruction: &CircuitInstruction) -> CircuitResult<()> {
    instruction.observable_id_argument()?.ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "simple detecting-region extraction requires OBSERVABLE_INCLUDE to have an observable id",
        )
    })?;
    for target in instruction.targets() {
        if target.is_measurement_record_target() || target.pauli_type().is_some() {
            continue;
        }
        return Err(CircuitError::invalid_detector_error_model(format!(
            "simple detecting-region extraction only supports OBSERVABLE_INCLUDE measurement-record or Pauli targets, got {target}"
        )));
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
                let body_count = detector_count(repeat.body())?;
                let repeated = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(
                            "repeat detector count overflowed",
                        )
                    })?;
                count = count.checked_add(repeated).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("detector count overflowed")
                })?;
            }
        }
    }
    Ok(count)
}

fn observable_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut max_observable = None;
    visit_observables(circuit, &mut max_observable)?;
    Ok(max_observable.map_or(0, |id| id.saturating_add(1)))
}

fn visit_observables(circuit: &Circuit, max_observable: &mut Option<u64>) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if let Some(observable) = instruction.observable_id_argument()? {
                    *max_observable = Some(
                        max_observable
                            .map_or(observable.get(), |current| current.max(observable.get())),
                    );
                }
            }
            CircuitItem::RepeatBlock(repeat) => visit_observables(repeat.body(), max_observable)?,
        }
    }
    Ok(())
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
                let body_count = tick_count(repeat.body())?;
                let repeated = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("repeat tick count overflowed")
                    })?;
                count = count.checked_add(repeated).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("tick count overflowed")
                })?;
            }
        }
    }
    Ok(count)
}

fn validate_targets(
    targets: &BTreeSet<DemTarget>,
    detector_count: u64,
    observable_count: u64,
) -> CircuitResult<()> {
    for target in targets {
        match target {
            DemTarget::RelativeDetector(detector) => {
                if detector.get() >= detector_count {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "requested detector D{} but circuit only has {detector_count} detector(s)",
                        detector.get()
                    )));
                }
            }
            DemTarget::LogicalObservable(observable) => {
                if observable.get() >= observable_count {
                    return Err(CircuitError::invalid_detector_error_model(format!(
                        "requested observable L{} but circuit only has {observable_count} observable(s)",
                        observable.get()
                    )));
                }
            }
            DemTarget::Separator | DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "detecting-region target filters only supports detector and logical-observable targets, got {target}",
                )));
            }
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

#[derive(Default)]
struct DetectingRegionBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl DetectingRegionBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
                        "detecting-region expanded instruction count overflowed",
                    )
                })?;
        if self.expanded_instructions > MAX_DETECTING_REGION_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "detecting-region extraction currently supports at most {MAX_DETECTING_REGION_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "detecting-region repeat iteration count overflowed",
            )
        })?;
        if self.repeat_iterations > MAX_DETECTING_REGION_REPEAT_ITERATIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "detecting-region extraction currently supports at most {MAX_DETECTING_REGION_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
    }
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
    fn detecting_regions_target_api_matches_mx_python_example() {
        let circuit = Circuit::from_stim_str(
            "R 0\n\
             TICK\n\
             H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MX 0 1\n\
             DETECTOR rec[-1] rec[-2]\n",
        )
        .unwrap();
        let actual = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: all_detecting_region_targets(&circuit).unwrap(),
                ticks: all_detecting_region_ticks(&circuit).unwrap(),
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap();
        let detector = DemTarget::relative_detector(0).unwrap();

        assert_eq!(actual[&detector][&0].to_string(), "+Z_");
        assert_eq!(actual[&detector][&1].to_string(), "+X_");
        assert_eq!(actual[&detector][&2].to_string(), "+XX");
    }

    #[test]
    fn detecting_regions_target_api_supports_mzz_example() {
        let circuit = Circuit::from_stim_str(
            "TICK\n\
             MZZ 0 1 1 2\n\
             TICK\n\
             M 2\n\
             DETECTOR rec[-1]\n",
        )
        .unwrap();
        let actual = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: vec![DemTarget::relative_detector(0).unwrap()],
                ticks: all_detecting_region_ticks(&circuit).unwrap(),
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap();
        let detector = DemTarget::relative_detector(0).unwrap();

        assert_eq!(actual[&detector][&0].to_string(), "+__Z");
        assert_eq!(actual[&detector][&1].to_string(), "+__Z");
    }

    #[test]
    fn detecting_regions_target_api_supports_logical_observable_targets() {
        let circuit = Circuit::from_stim_str(
            "TICK\n\
             M 0\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             TICK\n\
             H 1\n\
             OBSERVABLE_INCLUDE(1) X1\n",
        )
        .unwrap();
        let actual = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: vec![
                    DemTarget::logical_observable(0).unwrap(),
                    DemTarget::logical_observable(1).unwrap(),
                    DemTarget::logical_observable(1).unwrap(),
                ],
                ticks: vec![0, 1],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap();

        assert_eq!(
            actual[&DemTarget::logical_observable(0).unwrap()][&0].to_string(),
            "+Z_"
        );
        assert_eq!(
            actual[&DemTarget::logical_observable(1).unwrap()][&1].to_string(),
            "+_Z"
        );
        assert_eq!(actual.len(), 2);
    }

    #[test]
    fn detecting_regions_target_api_rejects_invalid_targets() {
        let circuit = Circuit::from_stim_str("TICK\nM 0\nDETECTOR rec[-1]\n").unwrap();
        for (target, message) in [
            (
                DemTarget::relative_detector(1).unwrap(),
                "requested detector D1",
            ),
            (
                DemTarget::logical_observable(0).unwrap(),
                "requested observable L0",
            ),
            (DemTarget::separator(), "only supports detector"),
            (DemTarget::numeric(5), "only supports detector"),
        ] {
            let error = circuit_detecting_regions_for_targets(
                &circuit,
                DetectingRegionTargetOptions {
                    targets: vec![target],
                    ticks: vec![0],
                    ignore_anticommutation_errors: false,
                },
            )
            .unwrap_err();
            assert!(error.to_string().contains(message), "{target}: {error}");
        }
    }

    #[test]
    fn detecting_regions_target_api_rejects_dense_helper_expansion() {
        let high_observable =
            Circuit::from_stim_str("OBSERVABLE_INCLUDE(4294967296) Z0\n").unwrap();
        let error = all_detecting_region_targets(&high_observable).unwrap_err();
        assert!(error.to_string().contains("observable target"));

        let many_detectors =
            Circuit::from_stim_str("M 0\nREPEAT 1000001 {\n    DETECTOR rec[-1]\n}\n").unwrap();
        let error = all_detecting_region_targets(&many_detectors).unwrap_err();
        assert!(error.to_string().contains("materialized target"));
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
    fn detecting_regions_repeat_supports_bounded_ticks() {
        let actual = regions(
            "H 0\n\
             REPEAT 2 {\n\
                 TICK\n\
             }\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
            vec![detector(0)],
            vec![0, 1, 2],
        );

        assert_eq!(actual[&detector(0)][&0].to_string(), "+X_");
        assert_eq!(actual[&detector(0)][&1].to_string(), "+X_");
        assert_eq!(actual[&detector(0)][&2].to_string(), "+XX");
    }

    #[test]
    fn detecting_regions_rejects_unsupported_gate() {
        let circuit = Circuit::from_stim_str("X 0\nTICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("does not support gate X"));
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
    fn detecting_regions_repeat_rejects_excessive_expansion() {
        let circuit =
            Circuit::from_stim_str("REPEAT 1000001 {\n    TICK\n}\nMXX 0 1\nDETECTOR rec[-1]\n")
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

        assert!(error.to_string().contains("expanded repeat iterations"));
    }
}
