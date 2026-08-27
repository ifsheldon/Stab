use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::{FlexPauliString, PauliBasis, StabilizerResource};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, CircuitTick, DemDetectorId, DemTarget, GateCategory,
    QubitId, Target,
};

use crate::{
    AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError,
    sparse_rev_frame_tracker::{ReverseTrackerWorkBudget, SparseReverseFrameTracker},
};

const MAX_DETECTING_REGION_REPRESENTED_WORK: u64 = 1_000_000;
const MAX_DETECTING_REGION_TRAVERSAL_WORK: u64 = 1_000_000;
const MAX_DETECTING_REGION_LIVE_STATE_UNITS: u64 = 1_000_000;
const MAX_DETECTING_REGION_OUTPUT_REGIONS: u64 = 1_000_000;
const MAX_DETECTING_REGION_OUTPUT_BYTES: u64 = 256 * 1024 * 1024;
// This keeps every recursive compact traversal inside ordinary worker and test-thread stacks.
const MAX_DETECTING_REGION_REPEAT_NESTING: usize = 128;
const MAX_DETECTING_REGION_HELPER_TARGETS: u64 = MAX_DETECTING_REGION_REPRESENTED_WORK;
const MAX_DETECTING_REGION_HELPER_TICKS: u64 = 1_000_000;
const DETECTING_REGION_OUTPUT_ENTRY_OVERHEAD_BYTES: u64 = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectingRegionOptions {
    pub detectors: Vec<DemDetectorId>,
    pub ticks: Vec<CircuitTick>,
    pub ignore_anticommutation_errors: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectingRegionTargetOptions {
    pub targets: Vec<DemTarget>,
    pub ticks: Vec<CircuitTick>,
    pub ignore_anticommutation_errors: bool,
}

pub type DetectingRegionMap = BTreeMap<DemDetectorId, BTreeMap<CircuitTick, FlexPauliString>>;
pub type DetectingRegionTargetMap = BTreeMap<DemTarget, BTreeMap<CircuitTick, FlexPauliString>>;

pub fn circuit_detecting_regions(
    circuit: &Circuit,
    options: DetectingRegionOptions,
) -> AnalysisResult<DetectingRegionMap> {
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
) -> AnalysisResult<DetectingRegionTargetMap> {
    let mut budget =
        DetectingRegionBudget::for_request(options.targets.len(), options.ticks.len())?;
    validate_supported_subset(circuit, &mut budget)?;
    let fail_on_anticommute = !options.ignore_anticommutation_errors;
    let targets = options.targets.into_iter().collect::<BTreeSet<_>>();
    let ticks = options.ticks.into_iter().collect::<BTreeSet<_>>();
    let detector_count = circuit.count_detectors()?;
    let observable_count = circuit.count_observables()?;
    let tick_count = circuit.count_ticks()?;
    validate_targets(&targets, detector_count, observable_count)?;
    validate_ticks(&ticks, tick_count)?;

    let represented_qubits = represented_qubit_ids(circuit, &mut budget)?;
    let qubit_count = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    let mut regions = DetectingRegionTargetMap::new();
    let mut tracker = SparseReverseFrameTracker::new(
        qubit_count,
        detecting_region_measurement_count(circuit)?,
        detector_count,
        fail_on_anticommute,
    );
    let mut current_tick = tick_count;
    let snapshot_context = SnapshotContext {
        targets: &targets,
        ticks: &ticks,
        represented_qubits: &represented_qubits,
        qubit_count,
    };
    undo_circuit_with_snapshots(
        circuit,
        &mut tracker,
        &snapshot_context,
        &mut current_tick,
        &mut regions,
        &mut budget,
    )?;
    tracker.undo_implicit_rz_at_start_of_circuit()?;
    Ok(regions)
}

pub fn all_detecting_region_targets(circuit: &Circuit) -> AnalysisResult<Vec<DemTarget>> {
    let detector_count = circuit.count_detectors()?;
    let observable_count = circuit.count_observables()?;
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
) -> AnalysisResult<usize> {
    let target_count = detector_count.saturating_add(observable_count);
    if target_count > MAX_DETECTING_REGION_HELPER_TARGETS {
        return Err(detecting_region_resource_error(
            ResourceKind::MaterializedUnits,
            target_count,
            MAX_DETECTING_REGION_HELPER_TARGETS,
        ));
    }
    usize::try_from(target_count).map_err(|_| {
        detecting_region_resource_error(
            ResourceKind::MaterializedUnits,
            target_count,
            usize::MAX as u64,
        )
    })
}

pub fn all_detecting_region_ticks(circuit: &Circuit) -> AnalysisResult<Vec<CircuitTick>> {
    let tick_count = circuit.count_ticks()?;
    if tick_count > MAX_DETECTING_REGION_HELPER_TICKS {
        return Err(detecting_region_resource_error(
            ResourceKind::MaterializedUnits,
            tick_count,
            MAX_DETECTING_REGION_HELPER_TICKS,
        ));
    }
    Ok((0..tick_count).map(CircuitTick::new).collect())
}

struct SnapshotContext<'a> {
    targets: &'a BTreeSet<DemTarget>,
    ticks: &'a BTreeSet<CircuitTick>,
    represented_qubits: &'a BTreeSet<QubitId>,
    qubit_count: usize,
}

fn undo_circuit_with_snapshots(
    circuit: &Circuit,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    for item in circuit.items().iter().rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                undo_instruction_with_snapshots(
                    instruction,
                    tracker,
                    context,
                    current_tick,
                    regions,
                    budget,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                undo_repeat_with_snapshots(
                    repeat.body(),
                    repeat.repeat_count().get(),
                    tracker,
                    context,
                    current_tick,
                    regions,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn undo_repeat_with_snapshots(
    body: &Circuit,
    repetitions: u64,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    if repetitions == 0 {
        return Ok(());
    }
    let ticks_per_iteration = body.count_ticks()?;
    if ticks_per_iteration == 0 {
        return tracker.undo_repeated_circuit_with_budget(body, repetitions, budget);
    }
    let repeated_ticks = ticks_per_iteration
        .checked_mul(repetitions)
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detecting-region repeat tick count overflowed",
            )
        })?;
    let repeat_start = current_tick.checked_sub(repeated_ticks).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model("detecting-region repeat tick span underflowed")
    })?;
    let repeat_start_tick = CircuitTick::new(repeat_start);
    let repeat_end_tick = CircuitTick::new(*current_tick);
    if context
        .ticks
        .range(repeat_start_tick..repeat_end_tick)
        .next()
        .is_none()
    {
        tracker.undo_repeated_circuit_with_budget(body, repetitions, budget)?;
        *current_tick = repeat_start;
        return Ok(());
    }

    let mut remaining = repetitions;
    while remaining > 0 {
        let iteration_start = current_tick
            .checked_sub(ticks_per_iteration)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "detecting-region repeat iteration tick span underflowed",
                )
            })?;
        let selected_in_iteration = context
            .ticks
            .range(CircuitTick::new(iteration_start)..CircuitTick::new(*current_tick))
            .next()
            .is_some();
        if selected_in_iteration {
            undo_circuit_with_snapshots(body, tracker, context, current_tick, regions, budget)?;
            if *current_tick != iteration_start {
                return Err(AnalysisError::invalid_detector_error_model(
                    "detecting-region repeat body tick count changed during traversal",
                ));
            }
            remaining -= 1;
            continue;
        }

        let Some(previous_selected_tick) = context
            .ticks
            .range(CircuitTick::new(repeat_start)..CircuitTick::new(*current_tick))
            .next_back()
            .map(|tick| tick.get())
        else {
            tracker.undo_repeated_circuit_with_budget(body, remaining, budget)?;
            *current_tick = repeat_start;
            return Ok(());
        };
        let selected_iteration = previous_selected_tick
            .checked_sub(repeat_start)
            .map(|offset| offset / ticks_per_iteration)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "detecting-region selected tick preceded its repeat span",
                )
            })?;
        let skipped = remaining
            .checked_sub(selected_iteration.checked_add(1).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "detecting-region selected repeat iteration overflowed",
                )
            })?)
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "detecting-region selected repeat iteration exceeded its remaining span",
                )
            })?;
        if skipped == 0 {
            return Err(AnalysisError::invalid_detector_error_model(
                "detecting-region repeat skip made no progress",
            ));
        }
        tracker.undo_repeated_circuit_with_budget(body, skipped, budget)?;
        let skipped_ticks = ticks_per_iteration.checked_mul(skipped).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detecting-region skipped tick count overflowed",
            )
        })?;
        *current_tick = current_tick.checked_sub(skipped_ticks).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detecting-region skipped tick span underflowed",
            )
        })?;
        remaining = remaining.checked_sub(skipped).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detecting-region skipped repeat count underflowed",
            )
        })?;
    }
    Ok(())
}

fn undo_instruction_with_snapshots(
    instruction: &CircuitInstruction,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    budget.admit_tracker_instruction(instruction)?;
    if instruction.gate().canonical_name() == "TICK" {
        *current_tick = current_tick.checked_sub(1).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "tick count underflowed while extracting detecting regions",
            )
        })?;
        let tick = CircuitTick::new(*current_tick);
        if context.ticks.contains(&tick) {
            snapshot_regions(tick, tracker, context, regions, budget)?;
        }
    }
    tracker.undo_instruction(instruction)
}

fn snapshot_regions(
    tick: CircuitTick,
    tracker: &SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    budget.consume_traversal_work(usize_to_u64(
        context.represented_qubits.len(),
        "snapshot qubit count",
    )?)?;
    let mut active_targets = BTreeSet::new();
    for qubit in context.represented_qubits {
        let qubit_targets = tracker.pauli_targets_at(*qubit)?;
        active_targets.extend(qubit_targets.intersection(context.targets).copied());
    }
    budget.consume_traversal_work(usize_to_u64(
        active_targets.len(),
        "snapshot active target count",
    )?)?;
    for target in active_targets {
        let output_bytes = budget.admit_output_candidate(context.qubit_count)?;
        let region = tracker.region_for_target(target)?;
        budget.commit_output_region(output_bytes)?;
        regions.entry(target).or_default().insert(tick, region);
    }
    Ok(())
}

fn validate_supported_subset(
    circuit: &Circuit,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    validate_supported_subset_inner(circuit, 0, budget)
}

fn represented_qubit_ids(
    circuit: &Circuit,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<BTreeSet<QubitId>> {
    fn collect(
        circuit: &Circuit,
        qubits: &mut BTreeSet<QubitId>,
        budget: &mut DetectingRegionBudget,
    ) -> AnalysisResult<()> {
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    if instruction.gate().targets_are_pad_values() {
                        continue;
                    }
                    for qubit in instruction.targets().iter().filter_map(Target::qubit_id) {
                        if !qubits.contains(&qubit) {
                            budget.reserve_live_state(1)?;
                            qubits.insert(qubit);
                        }
                    }
                }
                CircuitItem::RepeatBlock(repeat) => collect(repeat.body(), qubits, budget)?,
            }
        }
        Ok(())
    }

    let mut qubits = BTreeSet::new();
    collect(circuit, &mut qubits, budget)?;
    Ok(qubits)
}

fn validate_supported_subset_inner(
    circuit: &Circuit,
    depth: usize,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    if depth > MAX_DETECTING_REGION_REPEAT_NESTING {
        return Err(detecting_region_resource_error(
            ResourceKind::RepeatNesting,
            depth as u64,
            MAX_DETECTING_REGION_REPEAT_NESTING as u64,
        ));
    }
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                validate_supported_instruction(instruction)?;
                budget.add_represented_work(instruction_represented_work(instruction)?)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let tag_bytes = repeat.tag_bytes().map_or(0, <[u8]>::len);
                budget.add_represented_work(
                    usize_to_u64(tag_bytes, "repeat tag byte count")?
                        .checked_add(1)
                        .ok_or_else(|| {
                            AnalysisError::invalid_detector_error_model(
                                "detecting-region represented repeat work overflowed",
                            )
                        })?,
                )?;
                validate_supported_subset_inner(repeat.body(), depth.saturating_add(1), budget)?;
            }
        }
    }
    Ok(())
}

fn instruction_represented_work(instruction: &CircuitInstruction) -> AnalysisResult<u64> {
    let target_count = usize_to_u64(instruction.targets().len(), "instruction target count")?;
    let argument_count = usize_to_u64(instruction.args().len(), "instruction argument count")?;
    let tag_bytes = usize_to_u64(
        instruction.tag_bytes().map_or(0, <[u8]>::len),
        "instruction tag byte count",
    )?;
    1_u64
        .checked_add(target_count)
        .and_then(|work| work.checked_add(argument_count))
        .and_then(|work| work.checked_add(tag_bytes))
        .ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "detecting-region represented instruction work overflowed",
            )
        })
}

fn validate_supported_instruction(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    if crate::single_qubit_clifford_for_gate(instruction.gate()).is_ok() {
        return validate_single_plain_qubit_targets(instruction);
    }
    if is_feedback_capable_controlled_pauli(instruction.gate().canonical_name()) {
        return validate_controlled_pauli_targets(instruction);
    }
    if instruction.gate().is_two_qubit_gate() && crate::gate_has_tableau(instruction.gate()) {
        return validate_plain_qubit_pair_targets(instruction);
    }
    if instruction.gate().category() == GateCategory::Noise
        && !is_heralded_record_noise(instruction)
    {
        return Ok(());
    }
    match instruction.gate().canonical_name() {
        "R" | "RX" | "RY" => validate_single_plain_qubit_targets(instruction),
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => {
            validate_single_measurement_qubit_targets(instruction)
        }
        "MXX" | "MYY" | "MZZ" => validate_measurement_qubit_pair_targets(instruction),
        "MPP" | "SPP" | "SPP_DAG" => validate_pauli_product_targets(instruction),
        "MPAD" => validate_measurement_pad_targets(instruction),
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
            validate_single_plain_qubit_targets(instruction)
        }
        "TICK" => validate_target_count(instruction, 0),
        "DETECTOR" => validate_detector_targets(instruction),
        "OBSERVABLE_INCLUDE" => validate_observable_include_targets(instruction),
        "QUBIT_COORDS" | "SHIFT_COORDS" => Ok(()),
        name => Err(AnalysisError::invalid_detector_error_model(format!(
            "simple detecting-region extraction does not support gate {name}"
        ))),
    }
}

fn is_heralded_record_noise(instruction: &CircuitInstruction) -> bool {
    matches!(
        instruction.gate().canonical_name(),
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn is_feedback_capable_controlled_pauli(gate_name: &str) -> bool {
    matches!(gate_name, "CX" | "CY" | "CZ" | "XCZ" | "YCZ")
}

fn validate_single_plain_qubit_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for target in instruction.targets() {
        validate_qubit_target(instruction, target, false)?;
    }
    Ok(())
}

fn validate_single_measurement_qubit_targets(
    instruction: &CircuitInstruction,
) -> AnalysisResult<()> {
    for target in instruction.targets() {
        validate_qubit_target(instruction, target, true)?;
    }
    Ok(())
}

fn validate_plain_qubit_pair_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} with qubit target pairs",
                instruction.gate().canonical_name()
            )));
        };
        validate_qubit_target(instruction, left, false)?;
        validate_qubit_target(instruction, right, false)?;
    }
    Ok(())
}

fn validate_controlled_pauli_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} with paired targets",
                instruction.gate().canonical_name()
            )));
        };
        let gate_name = instruction.gate().canonical_name();
        if is_cz_classical_bit_noop(gate_name, left, right) {
            continue;
        }
        let left_is_sweep = left.is_sweep_bit_target();
        let right_is_sweep = right.is_sweep_bit_target();
        if left_is_sweep || right_is_sweep {
            if left_is_sweep && right_is_sweep && gate_name == "CZ" {
                continue;
            }
            if left_is_sweep ^ right_is_sweep {
                validate_sweep_position(gate_name, left_is_sweep)?;
                let qubit_target = if left_is_sweep { right } else { left };
                validate_qubit_target(instruction, qubit_target, false)?;
                continue;
            }
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} sweep-controlled groups with exactly one sweep bit and one plain qubit target",
                gate_name
            )));
        }
        let left_is_record = left.is_measurement_record_target();
        let right_is_record = right.is_measurement_record_target();
        match (left_is_record, right_is_record) {
            (false, false) => {
                validate_qubit_target(instruction, left, false)?;
                validate_qubit_target(instruction, right, false)?;
            }
            (true, false) | (false, true) => {
                validate_feedback_position(gate_name, left_is_record)?;
                let feedback_target = if left_is_record { right } else { left };
                validate_qubit_target(instruction, feedback_target, false)?;
            }
            (true, true) => {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "simple detecting-region extraction only supports {} measurement-record feedback with exactly one plain qubit target",
                    gate_name
                )));
            }
        }
    }
    Ok(())
}

fn is_cz_classical_bit_noop(gate_name: &str, left: &Target, right: &Target) -> bool {
    gate_name == "CZ" && left.is_classical_bit_target() && right.is_classical_bit_target()
}

fn validate_feedback_position(gate_name: &str, record_is_first: bool) -> AnalysisResult<()> {
    let valid = match gate_name {
        "CX" | "CY" => record_is_first,
        "XCZ" | "YCZ" => !record_is_first,
        "CZ" => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AnalysisError::invalid_detector_error_model(format!(
            "simple detecting-region extraction does not support {gate_name} measurement-record feedback in this target position"
        )))
    }
}

fn validate_sweep_position(gate_name: &str, sweep_is_first: bool) -> AnalysisResult<()> {
    let valid = match gate_name {
        "CX" | "CY" => sweep_is_first,
        "XCZ" | "YCZ" => !sweep_is_first,
        "CZ" => true,
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(AnalysisError::invalid_detector_error_model(format!(
            "simple detecting-region extraction does not support {gate_name} sweep-controlled targets in this target position"
        )))
    }
}

fn validate_measurement_qubit_pair_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} with qubit target pairs",
                instruction.gate().canonical_name()
            )));
        };
        validate_qubit_target(instruction, left, true)?;
        validate_qubit_target(instruction, right, true)?;
    }
    Ok(())
}

fn validate_pauli_product_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let mut has_pauli_target = false;
        for target in group {
            if target.is_combiner() {
                continue;
            }
            if target.pauli_type().is_none() {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "simple detecting-region extraction only supports {} with Pauli-product targets, got {target}",
                    instruction.gate().canonical_name()
                )));
            }
            has_pauli_target = true;
        }
        if !has_pauli_target {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports {} with non-empty Pauli-product targets",
                instruction.gate().canonical_name()
            )));
        }
    }
    Ok(())
}

fn validate_measurement_pad_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for target in instruction.targets() {
        let Target::Qubit {
            id,
            inverted: false,
        } = target
        else {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports MPAD constant targets 0 or 1, got {target}"
            )));
        };
        if id.get() <= 1 {
            continue;
        }
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "simple detecting-region extraction only supports MPAD constant targets 0 or 1, got {target}"
        )));
    }
    Ok(())
}

fn validate_qubit_target(
    instruction: &CircuitInstruction,
    target: &Target,
    allow_inverted: bool,
) -> AnalysisResult<()> {
    if let Target::Qubit { inverted, .. } = target
        && (allow_inverted || !inverted)
    {
        return Ok(());
    }
    let shape = if allow_inverted {
        "qubit targets"
    } else {
        "plain qubit targets"
    };
    Err(AnalysisError::invalid_detector_error_model(format!(
        "simple detecting-region extraction only supports {} with {shape}, got {target}",
        instruction.gate().canonical_name()
    )))
}

fn validate_target_count(instruction: &CircuitInstruction, expected: usize) -> AnalysisResult<()> {
    if instruction.targets().len() == expected {
        return Ok(());
    }
    Err(AnalysisError::invalid_detector_error_model(format!(
        "simple detecting-region extraction expected {} to have {expected} target(s), got {}",
        instruction.gate().canonical_name(),
        instruction.targets().len()
    )))
}

fn validate_detector_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for target in instruction.targets() {
        if !target.is_measurement_record_target() {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "simple detecting-region extraction only supports DETECTOR measurement-record targets, got {target}"
            )));
        }
    }
    Ok(())
}

fn validate_observable_include_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    instruction.observable_id_argument()?.ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(
            "simple detecting-region extraction requires OBSERVABLE_INCLUDE to have an observable id",
        )
    })?;
    for target in instruction.targets() {
        if target.is_measurement_record_target() || target.pauli_type().is_some() {
            continue;
        }
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "simple detecting-region extraction only supports OBSERVABLE_INCLUDE measurement-record or Pauli targets, got {target}"
        )));
    }
    Ok(())
}

fn detecting_region_measurement_count(circuit: &Circuit) -> AnalysisResult<usize> {
    usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "detecting-region measurement count does not fit in memory on this platform",
        )
    })
}

fn validate_targets(
    targets: &BTreeSet<DemTarget>,
    detector_count: u64,
    observable_count: u64,
) -> AnalysisResult<()> {
    for target in targets {
        match target {
            DemTarget::RelativeDetector(detector) => {
                if detector.get() >= detector_count {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "requested detector D{} but circuit only has {detector_count} detector(s)",
                        detector.get()
                    )));
                }
            }
            DemTarget::LogicalObservable(observable) => {
                if observable.get() >= observable_count {
                    return Err(AnalysisError::invalid_detector_error_model(format!(
                        "requested observable L{} but circuit only has {observable_count} observable(s)",
                        observable.get()
                    )));
                }
            }
            DemTarget::Separator | DemTarget::Numeric(_) => {
                return Err(AnalysisError::invalid_detector_error_model(format!(
                    "detecting-region target filters only supports detector and logical-observable targets, got {target}",
                )));
            }
        }
    }
    Ok(())
}

fn validate_ticks(ticks: &BTreeSet<CircuitTick>, tick_count: u64) -> AnalysisResult<()> {
    for tick in ticks {
        if tick.get() >= tick_count {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "requested tick {} but circuit only has {tick_count} tick layer(s)",
                tick.get()
            )));
        }
    }
    Ok(())
}

struct DetectingRegionBudget {
    represented_work: u64,
    traversal_work: u64,
    live_state_units: u64,
    tracker_state_units: u64,
    tracked_target_upper_bound: u64,
    output_regions: u64,
    output_bytes: u64,
}

impl DetectingRegionBudget {
    fn for_request(target_count: usize, tick_count: usize) -> AnalysisResult<Self> {
        let request_units = usize_to_u64(target_count, "requested target count")?
            .saturating_add(usize_to_u64(tick_count, "requested tick count")?);
        let mut result = Self {
            represented_work: 0,
            traversal_work: 0,
            live_state_units: 0,
            tracker_state_units: 0,
            tracked_target_upper_bound: 0,
            output_regions: 0,
            output_bytes: 0,
        };
        result.reserve_live_state(request_units)?;
        Ok(result)
    }

    fn add_represented_work(&mut self, count: u64) -> AnalysisResult<()> {
        let next = self.represented_work.saturating_add(count);
        ensure_resource_limit(
            ResourceKind::RepresentedItems,
            next,
            MAX_DETECTING_REGION_REPRESENTED_WORK,
        )?;
        self.represented_work = next;
        Ok(())
    }

    fn consume_traversal_work(&mut self, count: u64) -> AnalysisResult<()> {
        let next = self.traversal_work.saturating_add(count);
        ensure_resource_limit(
            ResourceKind::TraversalWork,
            next,
            MAX_DETECTING_REGION_TRAVERSAL_WORK,
        )?;
        self.traversal_work = next;
        Ok(())
    }

    fn reserve_live_state(&mut self, count: u64) -> AnalysisResult<()> {
        let next = self.live_state_units.saturating_add(count);
        ensure_resource_limit(
            ResourceKind::LiveStateUnits,
            next,
            MAX_DETECTING_REGION_LIVE_STATE_UNITS,
        )?;
        self.live_state_units = next;
        Ok(())
    }

    fn admit_transient_live_state(&self, count: u64) -> AnalysisResult<()> {
        ensure_resource_limit(
            ResourceKind::LiveStateUnits,
            self.live_state_units.saturating_add(count),
            MAX_DETECTING_REGION_LIVE_STATE_UNITS,
        )
    }

    fn reserve_tracker_state(&mut self, count: u64) -> AnalysisResult<()> {
        let next_tracker = self.tracker_state_units.saturating_add(count);
        self.reserve_live_state(count)?;
        self.tracker_state_units = next_tracker;
        Ok(())
    }

    fn admit_tracker_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<()> {
        self.consume_traversal_work(1)?;
        let gate_name = instruction.gate().canonical_name();
        let target_slots = usize_to_u64(instruction.targets().len(), "traversed target count")?;
        let state_growth = match gate_name {
            "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => 0,
            "DETECTOR" | "OBSERVABLE_INCLUDE" => {
                self.tracked_target_upper_bound = self.tracked_target_upper_bound.saturating_add(1);
                target_slots.max(1)
            }
            _ if instruction.gate().category() == GateCategory::Noise
                && !is_heralded_record_noise(instruction) =>
            {
                0
            }
            _ => self.tracked_target_upper_bound.saturating_mul(target_slots),
        };
        self.reserve_tracker_state(state_growth)
    }

    fn admit_recurrence_probe(&mut self) -> AnalysisResult<()> {
        let cloned_tracker_state = self.tracker_state_units.saturating_mul(2);
        self.admit_transient_live_state(cloned_tracker_state)
    }

    fn admit_output_candidate(&self, qubit_count: usize) -> AnalysisResult<u64> {
        let pauli_limit = StabilizerResource::PauliQubits.limit();
        if qubit_count > pauli_limit {
            return Err(detecting_region_resource_error(
                ResourceKind::MaterializedUnits,
                qubit_count as u64,
                pauli_limit as u64,
            ));
        }
        let candidate_bytes = output_region_bytes(qubit_count)?;
        self.next_output_totals(candidate_bytes)?;
        Ok(candidate_bytes)
    }

    fn commit_output_region(&mut self, bytes: u64) -> AnalysisResult<()> {
        let (next_region_count, next_output_bytes) = self.next_output_totals(bytes)?;
        self.output_regions = next_region_count;
        self.output_bytes = next_output_bytes;
        Ok(())
    }

    fn next_output_totals(&self, bytes: u64) -> AnalysisResult<(u64, u64)> {
        let next_region_count = self.output_regions.saturating_add(1);
        ensure_resource_limit(
            ResourceKind::OutputRecords,
            next_region_count,
            MAX_DETECTING_REGION_OUTPUT_REGIONS,
        )?;
        let next_output_bytes = self.output_bytes.saturating_add(bytes);
        ensure_resource_limit(
            ResourceKind::OutputBytes,
            next_output_bytes,
            MAX_DETECTING_REGION_OUTPUT_BYTES,
        )?;
        Ok((next_region_count, next_output_bytes))
    }
}

impl ReverseTrackerWorkBudget for DetectingRegionBudget {
    fn admit_probe_iteration(&mut self) -> AnalysisResult<()> {
        self.consume_traversal_work(1)
    }

    fn admit_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<()> {
        self.admit_tracker_instruction(instruction)
    }

    fn admit_recurrence_search(&mut self) -> AnalysisResult<()> {
        self.admit_recurrence_probe()
    }
}

fn output_region_bytes(qubit_count: usize) -> AnalysisResult<u64> {
    let qubits = usize_to_u64(qubit_count, "output qubit count")?;
    let dense_temporary =
        qubits.saturating_mul(usize_to_u64(size_of::<PauliBasis>(), "Pauli basis size")?);
    let words = qubits.saturating_add(63) / 64;
    let packed_storage = words.saturating_mul(16);
    Ok(dense_temporary
        .saturating_add(packed_storage)
        .saturating_add(DETECTING_REGION_OUTPUT_ENTRY_OVERHEAD_BYTES))
}

fn ensure_resource_limit(resource: ResourceKind, actual: u64, limit: u64) -> AnalysisResult<()> {
    if actual <= limit {
        return Ok(());
    }
    Err(detecting_region_resource_error(resource, actual, limit))
}

fn detecting_region_resource_error(
    resource: ResourceKind,
    actual: u64,
    limit: u64,
) -> AnalysisError {
    ResourceLimitError::detecting_regions(resource, actual, limit).into()
}

fn usize_to_u64(value: usize, label: &str) -> AnalysisResult<u64> {
    u64::try_from(value).map_err(|_| {
        let _ = label;
        detecting_region_resource_error(ResourceKind::LiveStateUnits, u64::MAX, u64::MAX - 1)
    })
}

#[cfg(test)]
mod tests;
