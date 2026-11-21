use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::{FlexPauliString, PauliBasis};
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, CircuitTick, DemDetectorId, DemTarget,
    DemTargetKind, FoldedCircuitSummary, FoldedCount, FoldedWalkError, GateKind, MemoryBudget,
    ParseLimits, QubitId, RepeatNestingLimit, Target, TargetKind,
};

use crate::controlled_pauli::{ControlSlot, controlled_pauli_form};
use crate::{
    AnalysisError, AnalysisOperation, AnalysisRequestError, AnalysisResult, ResourceKind,
    ResourceLimitError,
    sparse_rev_frame_tracker::{AnticommutationPolicy, SparseReverseFrameTracker},
};

/// Conservative retained bytes of one live tracker-state unit (one sparse map entry).
const DETECTING_REGION_LIVE_STATE_UNIT_BYTES: u64 = 64;
const DETECTING_REGION_OUTPUT_ENTRY_OVERHEAD_BYTES: u64 = 64;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectingRegionOptions {
    pub detectors: Vec<DemDetectorId>,
    pub ticks: Vec<CircuitTick>,
    pub ignore_anticommutation_errors: bool,
    /// Budget for the retained tracker state, recurrence-probe clones, and output regions.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per undone instruction and repeat iteration.
    pub cancellation: Cancellation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DetectingRegionTargetOptions {
    pub targets: Vec<DemTarget>,
    pub ticks: Vec<CircuitTick>,
    pub ignore_anticommutation_errors: bool,
    /// Budget for the retained tracker state, recurrence-probe clones, and output regions.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per undone instruction and repeat iteration.
    pub cancellation: Cancellation,
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
                .map(DemTarget::from_detector)
                .collect(),
            ticks: options.ticks,
            ignore_anticommutation_errors: options.ignore_anticommutation_errors,
            memory_budget: options.memory_budget,
            cancellation: options.cancellation,
        },
    )?;
    Ok(target_regions
        .into_iter()
        .filter_map(|(target, regions)| target.detector().map(|detector| (detector, regions)))
        .collect())
}

pub fn circuit_detecting_regions_for_targets(
    circuit: &Circuit,
    options: DetectingRegionTargetOptions,
) -> AnalysisResult<DetectingRegionTargetMap> {
    let mut budget = DetectingRegionBudget::for_request(
        options.memory_budget,
        options.targets.len(),
        options.ticks.len(),
    )?;
    validate_supported_subset(circuit)?;
    let anticommutation_policy = if options.ignore_anticommutation_errors {
        AnticommutationPolicy::Record
    } else {
        AnticommutationPolicy::Fail
    };
    let targets = options.targets.into_iter().collect::<BTreeSet<_>>();
    let ticks = options.ticks.into_iter().collect::<BTreeSet<_>>();
    // One folded walk supplies every whole-circuit count the extraction needs.
    let summary = folded_summary(circuit, &options.cancellation)?;
    let detector_count = exact_count(summary.detectors(), "detector")?;
    let observable_count = circuit.count_observables()?;
    let tick_count = exact_count(summary.ticks(), "tick")?;
    let measurement_count = usize::try_from(exact_count(summary.measurements(), "measurement")?)
        .map_err(|_| {
            AnalysisError::count_overflow(
                AnalysisOperation::DetectingRegions,
                "detecting-region measurement count does not fit in memory on this platform",
            )
        })?;
    validate_targets(&targets, detector_count, observable_count)?;
    validate_ticks(&ticks, tick_count)?;

    let represented_qubits = represented_qubit_ids(circuit, &mut budget)?;
    let qubit_count = circuit.simulated_qubit_count();
    let mut regions = DetectingRegionTargetMap::new();
    let mut tracker = SparseReverseFrameTracker::new(
        qubit_count,
        measurement_count,
        detector_count,
        anticommutation_policy,
    );
    tracker.set_cancellation(options.cancellation);
    tracker.set_memory_budget(
        AnalysisOperation::DetectingRegions,
        ResourceKind::LiveStateUnits,
        options.memory_budget,
    );
    tracker.set_base_bytes(budget.held_bytes());
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

/// Lists every detector and observable target, admitted against [`MemoryBudget::DEFAULT`].
///
/// See [`all_detecting_region_targets_with_budget`].
pub fn all_detecting_region_targets(circuit: &Circuit) -> AnalysisResult<Vec<DemTarget>> {
    all_detecting_region_targets_with_budget(circuit, MemoryBudget::DEFAULT)
}

/// Lists every detector and observable target after admitting the dense vector against `budget`.
pub fn all_detecting_region_targets_with_budget(
    circuit: &Circuit,
    budget: MemoryBudget,
) -> AnalysisResult<Vec<DemTarget>> {
    let detector_count = circuit.count_detectors()?;
    let observable_count = circuit.count_observables()?;
    let target_capacity = dense_target_helper_capacity(detector_count, observable_count, budget)?;
    let mut targets = Vec::new();
    targets.try_reserve_exact(target_capacity).map_err(|_| {
        detecting_region_resource_error(ResourceKind::MaterializedUnits, target_capacity as u64, 0)
    })?;
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
    budget: MemoryBudget,
) -> AnalysisResult<usize> {
    let target_count = detector_count.saturating_add(observable_count);
    admit_helper_bytes(budget, target_count, size_of::<DemTarget>())?;
    usize::try_from(target_count).map_err(|_| {
        detecting_region_resource_error(
            ResourceKind::MaterializedUnits,
            target_count,
            usize::MAX as u64,
        )
    })
}

/// Lists every tick of `circuit`, admitted against [`MemoryBudget::DEFAULT`].
///
/// See [`all_detecting_region_ticks_with_budget`].
pub fn all_detecting_region_ticks(circuit: &Circuit) -> AnalysisResult<Vec<CircuitTick>> {
    all_detecting_region_ticks_with_budget(circuit, MemoryBudget::DEFAULT)
}

/// Lists every tick of `circuit` after admitting the dense vector against `budget`.
pub fn all_detecting_region_ticks_with_budget(
    circuit: &Circuit,
    budget: MemoryBudget,
) -> AnalysisResult<Vec<CircuitTick>> {
    let tick_count = circuit.count_ticks()?;
    let capacity = admit_helper_bytes(budget, tick_count, size_of::<CircuitTick>())?;
    let mut ticks = Vec::new();
    ticks.try_reserve_exact(capacity).map_err(|_| {
        detecting_region_resource_error(ResourceKind::MaterializedUnits, tick_count, 0)
    })?;
    ticks.extend((0..tick_count).map(CircuitTick::new));
    Ok(ticks)
}

/// Admits a dense helper vector of `count` elements against the budget before allocating it.
fn admit_helper_bytes(
    budget: MemoryBudget,
    count: u64,
    element_bytes: usize,
) -> AnalysisResult<usize> {
    let required = count.saturating_mul(element_bytes as u64);
    admit_region_bytes(budget, ResourceKind::MaterializedBytes, required)?;
    usize::try_from(count).map_err(|_| {
        detecting_region_resource_error(ResourceKind::MaterializedUnits, count, usize::MAX as u64)
    })
}

struct SnapshotContext<'a> {
    targets: &'a BTreeSet<DemTarget>,
    ticks: &'a BTreeSet<CircuitTick>,
    represented_qubits: &'a BTreeSet<QubitId>,
    qubit_count: usize,
}

enum SnapshotFrame<'a> {
    Items(std::slice::Iter<'a, CircuitItem>),
    Repeat {
        body: &'a Circuit,
        ticks_per_iteration: u64,
        repeat_start: u64,
        remaining: u64,
        pending_iteration_start: Option<u64>,
    },
}

fn undo_circuit_with_snapshots(
    circuit: &Circuit,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    let mut stack = vec![SnapshotFrame::Items(circuit.items().iter())];
    while let Some(frame) = stack.last_mut() {
        match frame {
            SnapshotFrame::Items(items) => {
                let Some(item) = items.next_back() else {
                    stack.pop();
                    continue;
                };
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
                        if let Some(next_frame) = prepare_repeat_with_snapshots(
                            repeat.body(),
                            repeat.repeat_count().get(),
                            tracker,
                            context,
                            current_tick,
                        )? {
                            stack.push(next_frame);
                        }
                    }
                }
            }
            SnapshotFrame::Repeat {
                body,
                ticks_per_iteration,
                repeat_start,
                remaining,
                pending_iteration_start,
            } => {
                if let Some(iteration_start) = pending_iteration_start.take() {
                    if *current_tick != iteration_start {
                        return Err(AnalysisError::internal(
                            AnalysisOperation::DetectingRegions,
                            "detecting-region repeat body tick count changed during traversal",
                        ));
                    }
                    *remaining -= 1;
                }
                if *remaining == 0 {
                    stack.pop();
                    continue;
                }
                tracker.poll_cancellation()?;
                let iteration_start =
                    current_tick
                        .checked_sub(*ticks_per_iteration)
                        .ok_or_else(|| {
                            AnalysisError::internal(
                                AnalysisOperation::DetectingRegions,
                                "detecting-region repeat iteration tick span underflowed",
                            )
                        })?;
                let selected_in_iteration = context
                    .ticks
                    .range(CircuitTick::new(iteration_start)..CircuitTick::new(*current_tick))
                    .next()
                    .is_some();
                if selected_in_iteration {
                    *pending_iteration_start = Some(iteration_start);
                    let body = *body;
                    stack.push(SnapshotFrame::Items(body.items().iter()));
                    continue;
                }
                let Some(previous_selected_tick) = context
                    .ticks
                    .range(CircuitTick::new(*repeat_start)..CircuitTick::new(*current_tick))
                    .next_back()
                    .map(|tick| tick.get())
                else {
                    tracker.undo_repeated_circuit(body, *remaining)?;
                    *current_tick = *repeat_start;
                    stack.pop();
                    continue;
                };
                let skipped = skipped_repeat_iterations(
                    previous_selected_tick,
                    *repeat_start,
                    *ticks_per_iteration,
                    *remaining,
                )?;
                tracker.undo_repeated_circuit(body, skipped)?;
                let skipped_ticks = ticks_per_iteration.checked_mul(skipped).ok_or_else(|| {
                    AnalysisError::count_overflow(
                        AnalysisOperation::DetectingRegions,
                        "detecting-region skipped tick count overflowed",
                    )
                })?;
                *current_tick = current_tick.checked_sub(skipped_ticks).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::DetectingRegions,
                        "detecting-region skipped tick span underflowed",
                    )
                })?;
                *remaining = remaining.checked_sub(skipped).ok_or_else(|| {
                    AnalysisError::internal(
                        AnalysisOperation::DetectingRegions,
                        "detecting-region skipped repeat count underflowed",
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn prepare_repeat_with_snapshots<'a>(
    body: &'a Circuit,
    repetitions: u64,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
) -> AnalysisResult<Option<SnapshotFrame<'a>>> {
    if repetitions == 0 {
        return Ok(None);
    }
    let ticks_per_iteration = body.count_ticks()?;
    if ticks_per_iteration == 0 {
        tracker.undo_repeated_circuit(body, repetitions)?;
        return Ok(None);
    }
    let repeated_ticks = ticks_per_iteration
        .checked_mul(repetitions)
        .ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::DetectingRegions,
                "detecting-region repeat tick count overflowed",
            )
        })?;
    let repeat_start = current_tick.checked_sub(repeated_ticks).ok_or_else(|| {
        AnalysisError::internal(
            AnalysisOperation::DetectingRegions,
            "detecting-region repeat tick span underflowed",
        )
    })?;
    let repeat_start_tick = CircuitTick::new(repeat_start);
    let repeat_end_tick = CircuitTick::new(*current_tick);
    if context
        .ticks
        .range(repeat_start_tick..repeat_end_tick)
        .next()
        .is_none()
    {
        tracker.undo_repeated_circuit(body, repetitions)?;
        *current_tick = repeat_start;
        return Ok(None);
    }
    Ok(Some(SnapshotFrame::Repeat {
        body,
        ticks_per_iteration,
        repeat_start,
        remaining: repetitions,
        pending_iteration_start: None,
    }))
}

fn skipped_repeat_iterations(
    previous_selected_tick: u64,
    repeat_start: u64,
    ticks_per_iteration: u64,
    remaining: u64,
) -> AnalysisResult<u64> {
    let selected_iteration = previous_selected_tick
        .checked_sub(repeat_start)
        .map(|offset| offset / ticks_per_iteration)
        .ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::DetectingRegions,
                "detecting-region selected tick preceded its repeat span",
            )
        })?;
    let skipped = remaining
        .checked_sub(selected_iteration.checked_add(1).ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::DetectingRegions,
                "detecting-region selected repeat iteration overflowed",
            )
        })?)
        .ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::DetectingRegions,
                "detecting-region selected repeat iteration exceeded its remaining span",
            )
        })?;
    if skipped == 0 {
        return Err(AnalysisError::internal(
            AnalysisOperation::DetectingRegions,
            "detecting-region repeat skip made no progress",
        ));
    }
    Ok(skipped)
}

fn undo_instruction_with_snapshots(
    instruction: &CircuitInstruction,
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    current_tick: &mut u64,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    if instruction.gate().kind() == GateKind::TICK {
        *current_tick = current_tick.checked_sub(1).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::DetectingRegions,
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
    tracker: &mut SparseReverseFrameTracker,
    context: &SnapshotContext<'_>,
    regions: &mut DetectingRegionTargetMap,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<()> {
    let mut active_targets = BTreeSet::new();
    for qubit in context.represented_qubits {
        let qubit_targets = tracker.pauli_targets_at(*qubit)?;
        active_targets.extend(qubit_targets.intersection(context.targets).copied());
    }
    let tracker_live_bytes = tracker.live_bytes();
    for target in active_targets {
        let output_bytes =
            budget.admit_output_candidate(context.qubit_count, tracker_live_bytes)?;
        let region = tracker.region_for_target(target, budget.memory)?;
        budget.commit_output_region(output_bytes, tracker_live_bytes)?;
        tracker.set_base_bytes(budget.held_bytes());
        regions.entry(target).or_default().insert(tick, region);
    }
    Ok(())
}

fn validate_supported_subset(circuit: &Circuit) -> AnalysisResult<()> {
    let mut stack = vec![(circuit.items().iter(), 0_usize)];
    while let Some((items, depth)) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
        match item {
            CircuitItem::Instruction(instruction) => {
                validate_supported_instruction(instruction)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let next_depth = depth.saturating_add(1);
                if next_depth > RepeatNestingLimit::HARD_MAX {
                    return Err(detecting_region_resource_error(
                        ResourceKind::RepeatNesting,
                        next_depth as u64,
                        RepeatNestingLimit::HARD_MAX as u64,
                    ));
                }
                stack.push((repeat.body().items().iter(), next_depth));
            }
        }
    }
    Ok(())
}

fn represented_qubit_ids(
    circuit: &Circuit,
    budget: &mut DetectingRegionBudget,
) -> AnalysisResult<BTreeSet<QubitId>> {
    let mut qubits = BTreeSet::new();
    let mut stack = vec![circuit.items().iter()];
    while let Some(items) = stack.last_mut() {
        let Some(item) = items.next() else {
            stack.pop();
            continue;
        };
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
            CircuitItem::RepeatBlock(repeat) => {
                stack.push(repeat.body().items().iter());
            }
        }
    }
    Ok(qubits)
}

fn validate_supported_instruction(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    match instruction.gate().kind() {
        GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG => validate_single_plain_qubit_targets(instruction),
        GateKind::CX | GateKind::CY | GateKind::CZ | GateKind::XCZ | GateKind::YCZ => {
            validate_controlled_pauli_targets(instruction)
        }
        GateKind::XCX
        | GateKind::XCY
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => validate_plain_qubit_pair_targets(instruction),
        GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR => Ok(()),
        GateKind::R | GateKind::RX | GateKind::RY => {
            validate_single_plain_qubit_targets(instruction)
        }
        GateKind::M
        | GateKind::MX
        | GateKind::MY
        | GateKind::MR
        | GateKind::MRX
        | GateKind::MRY => validate_single_measurement_qubit_targets(instruction),
        GateKind::MXX | GateKind::MYY | GateKind::MZZ => {
            validate_measurement_qubit_pair_targets(instruction)
        }
        GateKind::MPP | GateKind::SPP | GateKind::SPP_DAG => {
            validate_pauli_product_targets(instruction)
        }
        GateKind::MPAD => validate_measurement_pad_targets(instruction),
        GateKind::HERALDED_ERASE | GateKind::HERALDED_PAULI_CHANNEL_1 => {
            validate_single_plain_qubit_targets(instruction)
        }
        GateKind::TICK => validate_target_count(instruction, 0),
        GateKind::DETECTOR => validate_detector_targets(instruction),
        GateKind::OBSERVABLE_INCLUDE => validate_observable_include_targets(instruction),
        GateKind::QUBIT_COORDS | GateKind::SHIFT_COORDS => Ok(()),
        GateKind::REPEAT => Err(AnalysisError::unsupported(
            AnalysisOperation::DetectingRegions,
            instruction.gate().kind(),
            format!(
                "simple detecting-region extraction does not support gate {}",
                instruction.gate().canonical_name()
            ),
        )),
    }
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
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                instruction.gate().kind(),
                format!(
                    "simple detecting-region extraction only supports {} with qubit target pairs",
                    instruction.gate().canonical_name()
                ),
            ));
        };
        validate_qubit_target(instruction, left, false)?;
        validate_qubit_target(instruction, right, false)?;
    }
    Ok(())
}

fn validate_controlled_pauli_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                instruction.gate().kind(),
                format!(
                    "simple detecting-region extraction only supports {} with paired targets",
                    instruction.gate().canonical_name()
                ),
            ));
        };
        let gate_name = instruction.gate().canonical_name();
        let slot = controlled_pauli_form(instruction.gate()).map(|form| form.slot);
        if slot == Some(ControlSlot::Either)
            && left.is_classical_bit_target()
            && right.is_classical_bit_target()
        {
            continue;
        }
        let left_is_sweep = left.is_sweep_bit_target();
        let right_is_sweep = right.is_sweep_bit_target();
        if left_is_sweep || right_is_sweep {
            if left_is_sweep && right_is_sweep && slot == Some(ControlSlot::Either) {
                continue;
            }
            if left_is_sweep ^ right_is_sweep {
                validate_control_position(
                    instruction,
                    slot,
                    left_is_sweep,
                    "sweep-controlled targets",
                )?;
                let qubit_target = if left_is_sweep { right } else { left };
                validate_qubit_target(instruction, qubit_target, false)?;
                continue;
            }
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                instruction.gate().kind(),
                format!(
                    "simple detecting-region extraction only supports {} sweep-controlled groups with exactly one sweep bit and one plain qubit target",
                    gate_name
                ),
            ));
        }
        let left_is_record = left.is_measurement_record_target();
        let right_is_record = right.is_measurement_record_target();
        match (left_is_record, right_is_record) {
            (false, false) => {
                validate_qubit_target(instruction, left, false)?;
                validate_qubit_target(instruction, right, false)?;
            }
            (true, false) | (false, true) => {
                validate_control_position(
                    instruction,
                    slot,
                    left_is_record,
                    "measurement-record feedback",
                )?;
                let feedback_target = if left_is_record { right } else { left };
                validate_qubit_target(instruction, feedback_target, false)?;
            }
            (true, true) => {
                return Err(AnalysisError::unsupported(
                    AnalysisOperation::DetectingRegions,
                    instruction.gate().kind(),
                    format!(
                        "simple detecting-region extraction only supports {} measurement-record feedback with exactly one plain qubit target",
                        gate_name
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Rejects a classical control outside the gate's control slot; `slot` is `None` for a gate
/// that accepts no classical control.
fn validate_control_position(
    instruction: &CircuitInstruction,
    slot: Option<ControlSlot>,
    control_is_first: bool,
    control_kind: &str,
) -> AnalysisResult<()> {
    if slot.is_some_and(|slot| slot.accepts(control_is_first)) {
        Ok(())
    } else {
        Err(AnalysisError::unsupported(
            AnalysisOperation::DetectingRegions,
            instruction.gate().kind(),
            format!(
                "simple detecting-region extraction does not support {} {control_kind} in this target position",
                instruction.gate().canonical_name()
            ),
        ))
    }
}

fn validate_measurement_qubit_pair_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for group in instruction.target_groups() {
        let [left, right] = group else {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                instruction.gate().kind(),
                format!(
                    "simple detecting-region extraction only supports {} with qubit target pairs",
                    instruction.gate().canonical_name()
                ),
            ));
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
                return Err(AnalysisError::unsupported(
                    AnalysisOperation::DetectingRegions,
                    instruction.gate().kind(),
                    format!(
                        "simple detecting-region extraction only supports {} with Pauli-product targets, got {target}",
                        instruction.gate().canonical_name()
                    ),
                ));
            }
            has_pauli_target = true;
        }
        if !has_pauli_target {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                instruction.gate().kind(),
                format!(
                    "simple detecting-region extraction only supports {} with non-empty Pauli-product targets",
                    instruction.gate().canonical_name()
                ),
            ));
        }
    }
    Ok(())
}

fn validate_measurement_pad_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for target in instruction.targets() {
        let TargetKind::Qubit {
            id,
            inverted: false,
        } = target.kind()
        else {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                GateKind::MPAD,
                format!(
                    "simple detecting-region extraction only supports MPAD constant targets 0 or 1, got {target}"
                ),
            ));
        };
        if id.get() <= 1 {
            continue;
        }
        return Err(AnalysisError::unsupported(
            AnalysisOperation::DetectingRegions,
            GateKind::MPAD,
            format!(
                "simple detecting-region extraction only supports MPAD constant targets 0 or 1, got {target}"
            ),
        ));
    }
    Ok(())
}

fn validate_qubit_target(
    instruction: &CircuitInstruction,
    target: &Target,
    allow_inverted: bool,
) -> AnalysisResult<()> {
    if let TargetKind::Qubit { inverted, .. } = target.kind()
        && (allow_inverted || !inverted)
    {
        return Ok(());
    }
    let shape = if allow_inverted {
        "qubit targets"
    } else {
        "plain qubit targets"
    };
    Err(AnalysisError::unsupported(
        AnalysisOperation::DetectingRegions,
        instruction.gate().kind(),
        format!(
            "simple detecting-region extraction only supports {} with {shape}, got {target}",
            instruction.gate().canonical_name()
        ),
    ))
}

fn validate_target_count(instruction: &CircuitInstruction, expected: usize) -> AnalysisResult<()> {
    if instruction.targets().len() == expected {
        return Ok(());
    }
    Err(AnalysisError::unsupported(
        AnalysisOperation::DetectingRegions,
        instruction.gate().kind(),
        format!(
            "simple detecting-region extraction expected {} to have {expected} target(s), got {}",
            instruction.gate().canonical_name(),
            instruction.targets().len()
        ),
    ))
}

fn validate_detector_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    for target in instruction.targets() {
        if !target.is_measurement_record_target() {
            return Err(AnalysisError::unsupported(
                AnalysisOperation::DetectingRegions,
                None,
                format!(
                    "simple detecting-region extraction only supports DETECTOR measurement-record targets, got {target}"
                ),
            ));
        }
    }
    Ok(())
}

fn validate_observable_include_targets(instruction: &CircuitInstruction) -> AnalysisResult<()> {
    instruction.observable_id_argument()?.ok_or_else(|| {
        AnalysisError::unsupported(AnalysisOperation::DetectingRegions, GateKind::OBSERVABLE_INCLUDE,
            "simple detecting-region extraction requires OBSERVABLE_INCLUDE to have an observable id",
        )
    })?;
    for target in instruction.targets() {
        if target.is_measurement_record_target() || target.pauli_type().is_some() {
            continue;
        }
        return Err(AnalysisError::unsupported(
            AnalysisOperation::DetectingRegions,
            GateKind::OBSERVABLE_INCLUDE,
            format!(
                "simple detecting-region extraction only supports OBSERVABLE_INCLUDE measurement-record or Pauli targets, got {target}"
            ),
        ));
    }
    Ok(())
}

/// Summarizes a circuit whose nesting `validate_supported_subset` already admitted.
fn folded_summary(
    circuit: &Circuit,
    cancellation: &Cancellation,
) -> AnalysisResult<FoldedCircuitSummary> {
    FoldedCircuitSummary::of(circuit, ParseLimits::DEFAULT_REPEAT_NESTING, cancellation).map_err(
        |error| match error {
            FoldedWalkError::RepeatNesting { depth, limit } => detecting_region_resource_error(
                ResourceKind::RepeatNesting,
                u64::try_from(depth).unwrap_or(u64::MAX),
                limit as u64,
            ),
            FoldedWalkError::Cancelled(cancelled) => cancelled.into(),
        },
    )
}

fn exact_count(count: FoldedCount, label: &str) -> AnalysisResult<u64> {
    count.exact().ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::DetectingRegions,
            format!("detecting-region circuit {label} count overflowed"),
        )
    })
}

fn validate_targets(
    targets: &BTreeSet<DemTarget>,
    detector_count: u64,
    observable_count: u64,
) -> AnalysisResult<()> {
    for target in targets {
        match target.kind() {
            DemTargetKind::RelativeDetector(detector) => {
                if detector.get() >= detector_count {
                    return Err(AnalysisRequestError::DetectorOutOfRange {
                        detector,
                        detector_count,
                    }
                    .into());
                }
            }
            DemTargetKind::LogicalObservable(observable) => {
                if u64::from(observable.get()) >= observable_count {
                    return Err(AnalysisRequestError::ObservableOutOfRange {
                        observable,
                        observable_count,
                    }
                    .into());
                }
            }
            DemTargetKind::Separator => {
                return Err(
                    AnalysisRequestError::NotDetectorOrObservable { target: *target }.into(),
                );
            }
        }
    }
    Ok(())
}

fn validate_ticks(ticks: &BTreeSet<CircuitTick>, tick_count: u64) -> AnalysisResult<()> {
    for tick in ticks {
        if tick.get() >= tick_count {
            return Err(AnalysisRequestError::TickOutOfRange {
                tick: *tick,
                tick_count,
            }
            .into());
        }
    }
    Ok(())
}

struct DetectingRegionBudget {
    memory: MemoryBudget,
    live_state_units: u64,
    output_bytes: u64,
}

impl DetectingRegionBudget {
    fn for_request(
        memory: MemoryBudget,
        target_count: usize,
        tick_count: usize,
    ) -> AnalysisResult<Self> {
        let request_units = usize_to_u64(target_count, "requested target count")?
            .saturating_add(usize_to_u64(tick_count, "requested tick count")?);
        let mut result = Self {
            memory,
            live_state_units: 0,
            output_bytes: 0,
        };
        result.reserve_live_state(request_units)?;
        Ok(result)
    }

    fn reserve_live_state(&mut self, count: u64) -> AnalysisResult<()> {
        let next = self.live_state_units.saturating_add(count);
        admit_live_state_units(self.memory, next)?;
        self.live_state_units = next;
        Ok(())
    }

    fn held_bytes(&self) -> u64 {
        self.live_state_units
            .saturating_mul(DETECTING_REGION_LIVE_STATE_UNIT_BYTES)
            .saturating_add(self.output_bytes)
    }

    fn admit_output_candidate(
        &self,
        qubit_count: usize,
        tracker_live_bytes: u64,
    ) -> AnalysisResult<u64> {
        let candidate_bytes = output_region_bytes(qubit_count)?;
        self.next_output_bytes(candidate_bytes, tracker_live_bytes)?;
        Ok(candidate_bytes)
    }

    fn commit_output_region(&mut self, bytes: u64, tracker_live_bytes: u64) -> AnalysisResult<()> {
        self.output_bytes = self.next_output_bytes(bytes, tracker_live_bytes)?;
        Ok(())
    }

    /// Request state, live tracker state, and the output map share the operation's one budget.
    fn next_output_bytes(&self, bytes: u64, tracker_live_bytes: u64) -> AnalysisResult<u64> {
        let next_output_bytes = self.output_bytes.saturating_add(bytes);
        let live_bytes = self
            .live_state_units
            .saturating_mul(DETECTING_REGION_LIVE_STATE_UNIT_BYTES)
            .saturating_add(tracker_live_bytes);
        admit_region_bytes(
            self.memory,
            ResourceKind::OutputBytes,
            next_output_bytes.saturating_add(live_bytes),
        )?;
        Ok(next_output_bytes)
    }
}

fn admit_live_state_units(budget: MemoryBudget, units: u64) -> AnalysisResult<()> {
    admit_region_bytes(
        budget,
        ResourceKind::LiveStateUnits,
        units.saturating_mul(DETECTING_REGION_LIVE_STATE_UNIT_BYTES),
    )
}

fn admit_region_bytes(
    budget: MemoryBudget,
    resource: ResourceKind,
    required: u64,
) -> AnalysisResult<()> {
    if budget.admits(u128::from(required)) {
        return Ok(());
    }
    Err(ResourceLimitError::memory_budget(
        AnalysisOperation::DetectingRegions,
        resource,
        required,
        budget,
    )
    .into())
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
