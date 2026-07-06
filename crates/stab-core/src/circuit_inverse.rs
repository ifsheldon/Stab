use std::collections::HashSet;

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Flow, Gate,
    GateCategory, PauliBasis, PauliSign, PauliString, RepeatBlock, SingleQubitClifford, Tableau,
    Target, circuit_flow::check_unsigned_flow_with_sparse_tracker,
};

mod qec;

const MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;

/// Returns the inverse of a circuit made only from supported unitary Clifford gates.
///
/// Repeat blocks are inverted recursively. Non-unitary instructions return a circuit
/// error instead of being skipped or approximated.
pub fn circuit_inverse_unitary(circuit: &Circuit) -> CircuitResult<Circuit> {
    let mut result = Circuit::new();
    for item in circuit.items().iter().rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                let inverse = inverse_instruction(instruction)?;
                result.append_instruction(inverse);
            }
            CircuitItem::RepeatBlock(repeat) => {
                let inverse_body = circuit_inverse_unitary(repeat.body())?;
                result.append_repeat_block(RepeatBlock::new(
                    repeat.repeat_count(),
                    inverse_body,
                    repeat.tag().map(str::to_owned),
                ));
            }
        }
    }
    Ok(result)
}

/// Returns the currently implemented QEC inverse subset.
///
/// This includes the unitary inverse plus selected Stim-compatible
/// reset-measure-detector, selected two-to-one detector-flow, selected `m_det`,
/// selected MPP identity-parity detector-flow, noisy measurement-only, noisy
/// measure-reset-only, exact noisy measure-reset detector-flow, and
/// measure-reset pass-through packets. Broader QEC-specific inverse rewrites
/// for measurements, resets, detectors, noise, and feedback remain active
/// follow-up work.
pub fn circuit_inverse_qec(circuit: &Circuit) -> CircuitResult<Circuit> {
    if let Some(inverse) = qec::selected_qec_inverse(circuit)? {
        return Ok(inverse);
    }
    circuit_inverse_unitary(circuit)
}

/// Returns the currently supported time-reversal subset for flows.
///
/// This additive API validates that every provided unsigned flow is satisfied by
/// the original circuit, returns the selected QEC inverse subset, and swaps each
/// flow's input and output Pauli terms. The measurement-rich subset is limited
/// to one noiseless plain unique-target measurement group, selected plain
/// unique-target reset, or selected unique-target measure-reset
/// instruction; detectors, feedback, noise, repeats, and multi-instruction QEC
/// rewrites remain deferred.
pub fn circuit_time_reversed_for_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    if let Some(selected) = selected_measurement_rich_time_reversal(circuit)? {
        return time_reverse_flows_with_sparse_validation(circuit, selected, flows);
    }
    if is_single_unpromoted_measurement_rich_instruction(circuit) {
        return Err(measurement_rich_time_reversal_error());
    }
    if has_classical_flow_terms(flows) {
        return Err(measurement_rich_time_reversal_error());
    }
    for (index, flow) in flows.iter().enumerate() {
        reject_non_unitary_flow_terms(index, flow)?;
    }
    let inverse = circuit_inverse_qec(circuit).map_err(|error| {
        CircuitError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary subset requires a unitary circuit: {error}"
        ))
    })?;
    let validation = FlowValidation::for_circuit(circuit)?;
    for (index, flow) in flows.iter().enumerate() {
        if !validation.is_satisfied(circuit, flow)? {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows unitary subset requires input circuit to satisfy flow {index}: {flow}"
            )));
        }
    }
    let reversed_flows = flows.iter().map(reversed_pauli_only_flow).collect();
    Ok((inverse, reversed_flows))
}

struct SelectedMeasurementRichTimeReversal {
    inverse: Circuit,
    kind: MeasurementRichTimeReversalKind,
}

enum MeasurementRichTimeReversalKind {
    Measurement {
        reset_candidate: Option<MeasurementToResetCandidate>,
        record_count: usize,
    },
    Reset {
        targets: Vec<MeasureResetTarget>,
        basis: PauliBasis,
    },
    MeasureReset {
        targets: Vec<MeasureResetTarget>,
        basis: PauliBasis,
    },
}

struct MeasureResetTarget {
    qubit: usize,
    measurement: i32,
}

struct MeasurementToResetCandidate {
    qubit: usize,
    reset_gate: &'static str,
    target: Target,
    tag: Option<String>,
}

fn selected_measurement_rich_time_reversal(
    circuit: &Circuit,
) -> CircuitResult<Option<SelectedMeasurementRichTimeReversal>> {
    let [CircuitItem::Instruction(instruction)] = circuit.items() else {
        return Ok(None);
    };
    if !instruction.args().is_empty() {
        return Ok(None);
    }
    let groups = instruction.target_groups();
    let name = instruction.gate().canonical_name();
    if matches!(name, "M" | "MX" | "MY" | "MXX" | "MYY" | "MZZ")
        && measurement_groups_are_plain_unique(&groups)
    {
        let reset_candidate = selected_measurement_to_reset_candidate(instruction);
        return Ok(Some(SelectedMeasurementRichTimeReversal {
            inverse: reversed_single_instruction_circuit(instruction)?,
            kind: MeasurementRichTimeReversalKind::Measurement {
                reset_candidate,
                record_count: groups.len(),
            },
        }));
    }
    if let Some((measurement_gate, basis)) = reset_inverse_gate_and_basis(name) {
        let targets = plain_measure_reset_targets(&groups)?;
        if targets.is_empty() {
            return Ok(None);
        }
        return Ok(Some(SelectedMeasurementRichTimeReversal {
            inverse: reversed_single_instruction_circuit_with_gate(instruction, measurement_gate)?,
            kind: MeasurementRichTimeReversalKind::Reset { targets, basis },
        }));
    }
    let basis = match name {
        "MR" => PauliBasis::Z,
        "MRX" => PauliBasis::X,
        "MRY" => PauliBasis::Y,
        _ => return Ok(None),
    };
    let targets = measure_reset_targets(&groups)?;
    if targets.is_empty() {
        return Ok(None);
    }
    Ok(Some(SelectedMeasurementRichTimeReversal {
        inverse: reversed_single_instruction_circuit(instruction)?,
        kind: MeasurementRichTimeReversalKind::MeasureReset { targets, basis },
    }))
}

fn reset_inverse_gate_and_basis(name: &str) -> Option<(&'static str, PauliBasis)> {
    match name {
        "R" => Some(("M", PauliBasis::Z)),
        "RX" => Some(("MX", PauliBasis::X)),
        "RY" => Some(("MY", PauliBasis::Y)),
        _ => None,
    }
}

fn is_single_unpromoted_measurement_rich_instruction(circuit: &Circuit) -> bool {
    let [CircuitItem::Instruction(instruction)] = circuit.items() else {
        return false;
    };
    matches!(
        instruction.gate().canonical_name(),
        "M" | "MX" | "MY" | "MXX" | "MYY" | "MZZ" | "R" | "RX" | "RY" | "MR" | "MRX" | "MRY"
    )
}

fn measurement_groups_are_plain_unique(groups: &[&[Target]]) -> bool {
    if groups.is_empty() {
        return false;
    }
    let mut qubits = HashSet::with_capacity(groups.len());
    for group in groups {
        if group.is_empty() || !group.iter().all(is_plain_qubit_target) {
            return false;
        }
        for target in *group {
            let Some(qubit) = target.qubit_id().map(|id| id.get() as usize) else {
                return false;
            };
            if !qubits.insert(qubit) {
                return false;
            }
        }
    }
    true
}

fn plain_measure_reset_targets(groups: &[&[Target]]) -> CircuitResult<Vec<MeasureResetTarget>> {
    selected_measure_reset_targets(groups, false)
}

fn measure_reset_targets(groups: &[&[Target]]) -> CircuitResult<Vec<MeasureResetTarget>> {
    selected_measure_reset_targets(groups, true)
}

fn selected_measure_reset_targets(
    groups: &[&[Target]],
    allow_inverted: bool,
) -> CircuitResult<Vec<MeasureResetTarget>> {
    if groups.is_empty() {
        return Ok(Vec::new());
    }
    let mut targets = Vec::with_capacity(groups.len());
    let mut qubits = HashSet::with_capacity(groups.len());
    for (index, group) in groups.iter().enumerate() {
        let [target] = *group else {
            return Ok(Vec::new());
        };
        if !(target.is_qubit_target() && (allow_inverted || !target.is_inverted_result_target())) {
            return Ok(Vec::new());
        }
        let Some(qubit) = target.qubit_id().map(|id| id.get() as usize) else {
            return Ok(Vec::new());
        };
        if !qubits.insert(qubit) {
            return Ok(Vec::new());
        }
        let measurement = i32::try_from(index + 1).map_err(|_| {
            CircuitError::invalid_tableau_conversion(
                "time_reversed_for_flows measurement-rich subset requires selected measure-reset target count to fit i32",
            )
        })?;
        targets.push(MeasureResetTarget {
            qubit,
            measurement: -measurement,
        });
    }
    Ok(targets)
}

fn selected_measurement_to_reset_candidate(
    instruction: &CircuitInstruction,
) -> Option<MeasurementToResetCandidate> {
    let reset_gate = match instruction.gate().canonical_name() {
        "M" => "R",
        "MX" => "RX",
        "MY" => "RY",
        _ => return None,
    };
    let groups = instruction.target_groups();
    let [group] = groups.as_slice() else {
        return None;
    };
    let [target] = *group else {
        return None;
    };
    let qubit = target.qubit_id()?.get() as usize;
    Some(MeasurementToResetCandidate {
        qubit,
        reset_gate,
        target: target.clone(),
        tag: instruction.tag().map(str::to_owned),
    })
}

fn is_plain_qubit_target(target: &Target) -> bool {
    matches!(
        target,
        Target::Qubit {
            inverted: false,
            ..
        }
    )
}

fn time_reverse_flows_with_sparse_validation(
    circuit: &Circuit,
    selected: SelectedMeasurementRichTimeReversal,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    for (index, flow) in flows.iter().enumerate() {
        reject_unsupported_selected_reversal_terms(index, flow, &selected.kind)?;
        if !check_unsigned_flow_with_sparse_tracker(circuit, flow)
            .map_err(|error| CircuitError::invalid_tableau_conversion(error.to_string()))?
        {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows measurement-rich subset requires selected measurement-rich circuit to satisfy flow {index}: {flow}"
            )));
        }
    }
    reverse_measurement_rich_flows(selected, flows)
}

fn reject_unsupported_selected_reversal_terms(
    index: usize,
    flow: &Flow,
    kind: &MeasurementRichTimeReversalKind,
) -> CircuitResult<()> {
    match kind {
        MeasurementRichTimeReversalKind::Reset { .. }
        | MeasurementRichTimeReversalKind::MeasureReset { .. }
            if flow.observables().next().is_some() =>
        {
            Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows measurement-rich subset does not support observable terms in selected reset or measure-reset flow {index}: {flow}"
            )))
        }
        MeasurementRichTimeReversalKind::Reset { .. } if flow.measurements().next().is_some() => {
            Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows measurement-rich subset does not support measurement-record terms in selected reset-only flow {index}: {flow}"
            )))
        }
        _ => Ok(()),
    }
}

fn reversed_pauli_only_flow(flow: &Flow) -> Flow {
    Flow::new(
        flow.output().with_sign(PauliSign::Plus),
        flow.input().with_sign(PauliSign::Plus),
        [],
        [],
    )
}

fn reverse_measurement_rich_flows(
    selected: SelectedMeasurementRichTimeReversal,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    match &selected.kind {
        MeasurementRichTimeReversalKind::Measurement {
            reset_candidate: Some(reset_candidate),
            ..
        } if should_turn_measurement_into_reset(reset_candidate, flows) => Ok((
            measurement_to_reset_circuit(reset_candidate)?,
            flows.iter().map(reversed_pauli_only_flow).collect(),
        )),
        _ => Ok((
            selected.inverse,
            flows
                .iter()
                .map(|flow| reverse_measurement_rich_flow(flow, &selected.kind))
                .collect::<CircuitResult<Vec<_>>>()?,
        )),
    }
}

fn should_turn_measurement_into_reset(
    candidate: &MeasurementToResetCandidate,
    flows: &[Flow],
) -> bool {
    let has_record_dependence = flows.iter().any(flow_depends_on_selected_measurement);
    let no_future_dependence_on_measured_qubit = flows
        .iter()
        .all(|flow| !flow_has_future_dependence_on_qubit(flow, candidate.qubit));
    has_record_dependence && no_future_dependence_on_measured_qubit
}

fn flow_depends_on_selected_measurement(flow: &Flow) -> bool {
    flow.measurements()
        .any(|measurement| matches!(measurement, -1 | 0))
}

fn flow_has_future_dependence_on_qubit(flow: &Flow, qubit: usize) -> bool {
    !matches!(flow.output().get(qubit), None | Some(PauliBasis::I))
}

fn measurement_to_reset_circuit(candidate: &MeasurementToResetCandidate) -> CircuitResult<Circuit> {
    single_target_circuit(
        candidate.reset_gate,
        &candidate.target,
        candidate.tag.clone(),
    )
}

fn single_target_circuit(
    gate_name: &str,
    target: &Target,
    tag: Option<String>,
) -> CircuitResult<Circuit> {
    let mut circuit = Circuit::new();
    circuit.append_instruction(CircuitInstruction::new(
        Gate::from_name(gate_name)?,
        Vec::new(),
        vec![target.clone()],
        tag,
    )?);
    Ok(circuit)
}

fn reversed_single_instruction_circuit(instruction: &CircuitInstruction) -> CircuitResult<Circuit> {
    reversed_single_instruction_circuit_with_gate(instruction, instruction.gate().canonical_name())
}

fn reversed_single_instruction_circuit_with_gate(
    instruction: &CircuitInstruction,
    gate_name: &str,
) -> CircuitResult<Circuit> {
    let mut circuit = Circuit::new();
    circuit.append_instruction(CircuitInstruction::new(
        Gate::from_name(gate_name)?,
        instruction.args().to_vec(),
        reversed_target_groups(instruction),
        instruction.tag().map(str::to_owned),
    )?);
    Ok(circuit)
}

fn reverse_measurement_rich_flow(
    flow: &Flow,
    kind: &MeasurementRichTimeReversalKind,
) -> CircuitResult<Flow> {
    Ok(match kind {
        MeasurementRichTimeReversalKind::Measurement { record_count, .. } => Flow::new(
            flow.output().with_sign(PauliSign::Plus),
            flow.input().with_sign(PauliSign::Plus),
            reversed_measurement_order(flow.measurements(), *record_count)?,
            flow.observables(),
        ),
        MeasurementRichTimeReversalKind::Reset { targets, basis } => {
            let input = flow.output().with_sign(PauliSign::Plus);
            let output = flow.input().with_sign(PauliSign::Plus);
            let measurements = targets
                .iter()
                .filter(|target| output_depends_on_reset_basis(flow, target.qubit, *basis))
                .map(|target| target.measurement);
            Flow::new(input, output, measurements, [])
        }
        MeasurementRichTimeReversalKind::MeasureReset { targets, basis } => {
            let input = flow.output().with_sign(PauliSign::Plus);
            let output = flow.input().with_sign(PauliSign::Plus);
            let measurements = targets
                .iter()
                .filter(|target| output_depends_on_reset_basis(flow, target.qubit, *basis))
                .map(|target| target.measurement);
            Flow::new(input, output, measurements, [])
        }
    })
}

fn reversed_measurement_order(
    measurements: impl IntoIterator<Item = i32>,
    record_count: usize,
) -> CircuitResult<Vec<i32>> {
    let record_count = i32::try_from(record_count).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "time_reversed_for_flows measurement-rich subset requires selected measurement count to fit i32",
        )
    })?;
    measurements
        .into_iter()
        .map(|measurement| reversed_measurement_index(measurement, record_count))
        .collect()
}

fn reversed_measurement_index(measurement: i32, record_count: i32) -> CircuitResult<i32> {
    let old_index = if measurement < 0 {
        record_count.checked_add(measurement).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "time_reversed_for_flows measurement-rich subset encountered out-of-range measurement record",
            )
        })?
    } else {
        measurement
    };
    if !(0..record_count).contains(&old_index) {
        return Err(CircuitError::invalid_tableau_conversion(
            "time_reversed_for_flows measurement-rich subset encountered out-of-range measurement record",
        ));
    }
    Ok(-old_index - 1)
}

fn output_depends_on_reset_basis(flow: &Flow, qubit: usize, basis: PauliBasis) -> bool {
    flow.output()
        .get(qubit)
        .is_some_and(|actual| actual == basis)
}

fn has_classical_flow_terms(flows: &[Flow]) -> bool {
    flows
        .iter()
        .any(|flow| flow.measurements().next().is_some() || flow.observables().next().is_some())
}

fn measurement_rich_time_reversal_error() -> CircuitError {
    CircuitError::invalid_tableau_conversion(
        "time_reversed_for_flows measurement-rich subset supports only one noiseless plain unique-target measurement instruction group from M, MX, MY, MXX, MYY, or MZZ, one noiseless plain unique-target reset instruction from R, RX, or RY, or one noiseless unique-target measure-reset instruction from MR, MRX, or MRY; detectors, feedback, noise, repeats, and multi-instruction rewrites remain unsupported",
    )
}

fn inverse_instruction(instruction: &CircuitInstruction) -> CircuitResult<CircuitInstruction> {
    let gate = instruction.gate();
    if !is_unitary_category(gate.category()) {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "operation {} is not unitary",
            gate.canonical_name()
        )));
    }
    let inverse_gate = gate.best_candidate_inverse()?;
    let targets = reversed_target_groups(instruction);
    CircuitInstruction::new(
        inverse_gate,
        instruction.args().to_vec(),
        targets,
        instruction.tag().map(str::to_owned),
    )
}

fn is_unitary_category(category: GateCategory) -> bool {
    matches!(
        category,
        GateCategory::Controlled
            | GateCategory::HadamardLike
            | GateCategory::Pauli
            | GateCategory::Period3
            | GateCategory::Period4
            | GateCategory::ParityPhasing
            | GateCategory::Swap
    )
}

fn reversed_target_groups(instruction: &CircuitInstruction) -> Vec<Target> {
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for group in instruction.target_groups().into_iter().rev() {
        targets.extend_from_slice(group);
    }
    targets
}

fn reject_non_unitary_flow_terms(index: usize, flow: &Flow) -> CircuitResult<()> {
    if flow.measurements().next().is_some() || flow.observables().next().is_some() {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary subset does not support measurement-record or observable terms in flow {index}: {flow}"
        )));
    }
    Ok(())
}

enum FlowValidation {
    Tableau(Tableau),
    SparseFolded,
}

impl FlowValidation {
    fn for_circuit(circuit: &Circuit) -> CircuitResult<Self> {
        if expanded_instruction_count(circuit)
            .is_some_and(|count| count <= MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS)
        {
            return Ok(Self::Tableau(circuit.to_tableau(false, false, false)?));
        }
        if sparse_tracker_can_validate_without_unbounded_unroll(circuit) {
            return Ok(Self::SparseFolded);
        }
        Err(CircuitError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary subset requires at most {MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS} expanded instructions unless every repeated unitary body is supported by folded sparse validation"
        )))
    }

    fn is_satisfied(&self, circuit: &Circuit, flow: &Flow) -> CircuitResult<bool> {
        match self {
            Self::Tableau(tableau) => unitary_flow_is_satisfied_by_tableau(tableau, flow),
            Self::SparseFolded => check_unsigned_flow_with_sparse_tracker(circuit, flow)
                .map_err(|error| CircuitError::invalid_tableau_conversion(error.to_string())),
        }
    }
}

fn expanded_instruction_count(circuit: &Circuit) -> Option<u64> {
    circuit.items().iter().try_fold(0_u64, |count, item| {
        let item_count = match item {
            CircuitItem::Instruction(_) => 1,
            CircuitItem::RepeatBlock(repeat) => expanded_instruction_count(repeat.body())?
                .checked_mul(repeat.repeat_count().get())?,
        };
        count.checked_add(item_count)
    })
}

fn sparse_tracker_can_validate_without_unbounded_unroll(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            sparse_tracker_supports_folded_instruction(instruction)
        }
        CircuitItem::RepeatBlock(repeat) => {
            sparse_tracker_supports_folded_unitary_repeat(repeat.body())
        }
    })
}

fn sparse_tracker_supports_folded_unitary_repeat(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            sparse_tracker_supports_folded_instruction(instruction)
        }
        CircuitItem::RepeatBlock(repeat) => {
            sparse_tracker_supports_folded_unitary_repeat(repeat.body())
        }
    })
}

fn sparse_tracker_supports_folded_instruction(instruction: &CircuitInstruction) -> bool {
    SingleQubitClifford::from_gate(instruction.gate()).is_ok()
        || matches!(instruction.gate().canonical_name(), "CX" | "CY" | "CZ")
}

fn unitary_flow_is_satisfied_by_tableau(tableau: &Tableau, flow: &Flow) -> CircuitResult<bool> {
    let prefix_input = pauli_prefix(flow.input(), tableau.len());
    let actual_prefix = tableau
        .apply(&prefix_input)
        .map_err(|error| CircuitError::invalid_tableau_conversion(error.to_string()))?;
    let len = flow
        .input()
        .len()
        .max(flow.output().len())
        .max(tableau.len());
    Ok((0..len).all(|index| {
        let actual = if index < tableau.len() {
            actual_prefix.get(index).unwrap_or(PauliBasis::I)
        } else {
            flow.input().get(index).unwrap_or(PauliBasis::I)
        };
        actual == flow.output().get(index).unwrap_or(PauliBasis::I)
    }))
}

fn pauli_prefix(pauli: &PauliString, len: usize) -> PauliString {
    let bases = (0..len).map(|index| pauli.get(index).unwrap_or(PauliBasis::I));
    PauliString::from_bases(PauliSign::Plus, bases)
}
