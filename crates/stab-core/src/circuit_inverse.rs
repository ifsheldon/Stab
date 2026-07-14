use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Flow, GateCategory,
    PauliBasis, PauliSign, PauliString, RepeatBlock, SingleQubitClifford, StabilizerResource,
    Tableau, Target, circuit_flow::check_unsigned_flows_with_sparse_tracker,
};

mod qec;
mod reverse_flow;

const MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_TIME_REVERSE_TABLEAU_QUBITS: usize = StabilizerResource::TableauQubits.limit();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InverseQecOptions {
    /// Preserve selected measurement records instead of turning them into resets.
    ///
    /// The current Rust API implements this only for the exact one-qubit
    /// reset-measure-detector `r_m_det_keep_m` packet. Other selected QEC packets
    /// and broader reset-measure-detector variants reject this option instead of
    /// silently ignoring it.
    pub keep_measurements: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeReversedForFlowsOptions {
    /// Keep measurements as measurements instead of converting eligible ones to resets.
    ///
    /// This matches Stim v1.16.0's `dont_turn_measurements_into_resets` option
    /// across the supported tracker-driven measurement reversal surface.
    pub dont_turn_measurements_into_resets: bool,
}

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
/// selected MPP identity-parity detector-flow, selected MPAD record-tail,
/// selected noisy MZZ detector-flow, noisy measurement-only,
/// noisy measure-reset-only, exact noisy measure-reset detector-flow, selected
/// observable Pauli include, and measure-reset pass-through packets.
/// Broader QEC-specific inverse rewrites for measurements, resets, detectors,
/// observables, noise, and feedback remain active follow-up work.
pub fn circuit_inverse_qec(circuit: &Circuit) -> CircuitResult<Circuit> {
    circuit_inverse_qec_with_options(circuit, InverseQecOptions::default())
}

/// Returns the currently implemented QEC inverse subset with explicit options.
///
/// `keep_measurements` is currently implemented only for the exact one-qubit
/// reset-measure-detector packet matching Stim v1.16.0 `r_m_det_keep_m`.
pub fn circuit_inverse_qec_with_options(
    circuit: &Circuit,
    options: InverseQecOptions,
) -> CircuitResult<Circuit> {
    if options.keep_measurements {
        if let Some(inverse) = qec::selected_keep_measurements_qec_inverse(circuit)? {
            return Ok(inverse);
        }
        if qec::selected_qec_inverse(circuit)?.is_some() {
            return Err(CircuitError::invalid_tableau_conversion(
                "inverse_qec keep_measurements is currently supported only for the exact one-qubit reset-measure-detector subset",
            ));
        }
        return circuit_inverse_unitary(circuit);
    }
    if let Some(inverse) = qec::selected_qec_inverse(circuit)? {
        return Ok(inverse);
    }
    circuit_inverse_unitary(circuit)
}

/// Returns the supported tracker-driven time reversal for unsigned flows.
///
/// The implementation validates each input flow, reverses supported Clifford,
/// measurement, reset, measure-reset, pair-measurement, MPP, MPAD, detector,
/// observable, coordinate, and ordinary-noise gate families through shared
/// reverse transitions, and validates the returned flows. Pure unitary repeats
/// stay folded; measurement-rich repeats use bounded expansion capped at one
/// million instructions. Measurement-record feedback, heralded record reversal,
/// and duplicate collapse targets remain fail-closed.
pub fn circuit_time_reversed_for_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    circuit_time_reversed_for_flows_with_options(
        circuit,
        flows,
        TimeReversedForFlowsOptions::default(),
    )
}

/// Returns the currently supported time-reversal subset for flows with explicit options.
///
/// See [`TimeReversedForFlowsOptions`] for the currently selected option scope.
pub fn circuit_time_reversed_for_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
) -> CircuitResult<(Circuit, Vec<Flow>)> {
    if reverse_flow::requires_general_reversal(circuit, flows) {
        return reverse_flow::reverse_flows(circuit, flows, options);
    }
    for (index, flow) in flows.iter().enumerate() {
        reject_non_unitary_flow_terms(index, flow)?;
    }
    let inverse = circuit_inverse_unitary(circuit).map_err(|error| {
        CircuitError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary subset requires a unitary circuit: {error}"
        ))
    })?;
    if flows.is_empty() {
        return Ok((inverse, Vec::new()));
    }
    let validation = FlowValidation::for_circuit(circuit)?;
    for (index, (flow, satisfied)) in flows
        .iter()
        .zip(validation.check_all(circuit, flows)?)
        .enumerate()
    {
        if !satisfied {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows unitary subset requires input circuit to satisfy flow {index}: {flow}"
            )));
        }
    }
    let reversed_flows = flows
        .iter()
        .map(reversed_pauli_only_flow)
        .collect::<Vec<_>>();
    let reversed_checks = check_unsigned_flows_with_sparse_tracker(&inverse, &reversed_flows)
        .map_err(|error| {
            CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows could not validate reversed unitary flows: {error}"
            ))
        })?;
    for (index, (flow, satisfied)) in reversed_flows.iter().zip(reversed_checks).enumerate() {
        if !satisfied {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows produced an invalid reversed unitary flow {index}: {flow}"
            )));
        }
    }
    Ok((inverse, reversed_flows))
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

fn reset_inverse_gate_and_basis(name: &str) -> Option<(&'static str, PauliBasis)> {
    match name {
        "R" => Some(("M", PauliBasis::Z)),
        "RX" => Some(("MX", PauliBasis::X)),
        "RY" => Some(("MY", PauliBasis::Y)),
        _ => None,
    }
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

fn reversed_target_groups(instruction: &CircuitInstruction) -> Vec<Target> {
    if !instruction.gate().is_two_qubit_gate() {
        return instruction.targets().iter().rev().cloned().collect();
    }
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for group in instruction.target_groups().into_iter().rev() {
        targets.extend_from_slice(group);
    }
    targets
}

fn reversed_pauli_only_flow(flow: &Flow) -> Flow {
    Flow::new(
        flow.output().with_sign(PauliSign::Plus),
        flow.input().with_sign(PauliSign::Plus),
        [],
        [],
    )
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
        let within_tableau_budget = circuit.count_qubits() <= MAX_TIME_REVERSE_TABLEAU_QUBITS
            && expanded_instruction_count(circuit)
                .is_some_and(|count| count <= MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS);
        let has_repeat = circuit
            .items()
            .iter()
            .any(|item| matches!(item, CircuitItem::RepeatBlock(_)));
        if has_repeat && sparse_tracker_can_validate_without_unbounded_unroll(circuit) {
            return Ok(Self::SparseFolded);
        }
        if within_tableau_budget {
            return Ok(Self::Tableau(circuit.to_tableau(false, false, false)?));
        }
        if sparse_tracker_can_validate_without_unbounded_unroll(circuit) {
            return Ok(Self::SparseFolded);
        }
        Err(CircuitError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary subset requires at most {MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS} expanded instructions and {MAX_TIME_REVERSE_TABLEAU_QUBITS} tableau qubits unless the circuit is supported by folded sparse validation"
        )))
    }

    fn check_all(&self, circuit: &Circuit, flows: &[Flow]) -> CircuitResult<Vec<bool>> {
        match self {
            Self::Tableau(tableau) => flows
                .iter()
                .map(|flow| unitary_flow_is_satisfied_by_tableau(tableau, flow))
                .collect(),
            Self::SparseFolded => check_unsigned_flows_with_sparse_tracker(circuit, flows)
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
    PauliString::from_bases_unchecked(PauliSign::Plus, bases)
}
