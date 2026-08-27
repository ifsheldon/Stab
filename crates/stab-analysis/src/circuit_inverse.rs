use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString, StabilizerResource, Tableau};
use stab_model::{Circuit, CircuitInstruction, CircuitItem, Target};

use crate::{
    AnalysisError, AnalysisResult, circuit_flow::check_unsigned_flows_with_sparse_tracker,
};

mod reverse_flow;

const MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_TIME_REVERSE_TABLEAU_QUBITS: usize = StabilizerResource::TableauQubits.limit();

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InverseQecOptions {
    /// Preserve measurements instead of turning eligible measurements into resets.
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

/// Returns the strict inverse of a unitary circuit and its invertible annotations.
///
/// Leading `QUBIT_COORDS` instructions are preserved, `TICK` is self-inverse,
/// `SHIFT_COORDS` arguments are negated, and repeat blocks are inverted recursively.
/// Measurements, resets, noise, detectors, observables, and non-leading qubit
/// coordinates return an error instead of being skipped or approximated.
pub fn circuit_inverse_unitary(circuit: &Circuit) -> AnalysisResult<Circuit> {
    let leading_coordinates = circuit
        .items()
        .iter()
        .take_while(|item| {
            matches!(
                item,
                CircuitItem::Instruction(instruction)
                    if instruction.gate().canonical_name() == "QUBIT_COORDS"
            )
        })
        .count();
    let mut result = Circuit::new();
    for item in circuit.items().iter().take(leading_coordinates) {
        let CircuitItem::Instruction(instruction) = item else {
            return Err(AnalysisError::invalid_tableau_conversion(
                "leading-coordinate prefix unexpectedly contained a repeat block",
            ));
        };
        result.append_instruction(inverse_coordinate_instruction(instruction)?);
    }
    for item in circuit.items().iter().skip(leading_coordinates).rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                let inverse = inverse_public_instruction(instruction)?;
                result.append_instruction(inverse);
            }
            CircuitItem::RepeatBlock(repeat) => {
                let inverse_body = circuit_inverse_unitary(repeat.body())?;
                result.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    inverse_body,
                    repeat.tag_bytes(),
                ));
            }
        }
    }
    Ok(result)
}

fn circuit_inverse_unitary_strict(circuit: &Circuit) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    for item in circuit.items().iter().rev() {
        match item {
            CircuitItem::Instruction(instruction) => {
                result.append_instruction(inverse_unitary_instruction(instruction)?);
            }
            CircuitItem::RepeatBlock(repeat) => {
                let inverse_body = circuit_inverse_unitary_strict(repeat.body())?;
                result.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    inverse_body,
                    repeat.tag_bytes(),
                ));
            }
        }
    }
    Ok(result)
}

/// Returns the tracker-driven QEC inverse of a circuit.
pub fn circuit_inverse_qec(circuit: &Circuit) -> AnalysisResult<Circuit> {
    circuit_inverse_qec_with_options(circuit, InverseQecOptions::default())
}

/// Returns the tracker-driven QEC inverse with explicit measurement handling.
pub fn circuit_inverse_qec_with_options(
    circuit: &Circuit,
    options: InverseQecOptions,
) -> AnalysisResult<Circuit> {
    let reverse_options = TimeReversedForFlowsOptions {
        dont_turn_measurements_into_resets: options.keep_measurements,
    };
    let (inverse, _) = circuit_time_reversed_for_flows_with_options(circuit, &[], reverse_options)?;
    Ok(inverse)
}

/// Returns the supported tracker-driven time reversal for flows.
///
/// The implementation validates each input flow, reverses supported Clifford,
/// measurement, reset, measure-reset, pair-measurement, MPP, MPAD, detector,
/// observable, coordinate, heralded-record, and ordinary-noise gate families
/// through shared reverse transitions, and validates the returned flows. Pure
/// unitary repeats stay folded; measurement-rich repeats use bounded expansion
/// capped at one million instructions. Supplied Pauli signs are accepted but
/// ignored in the returned flows, and measurement-record feedback is rejected,
/// matching pinned Stim.
pub fn circuit_time_reversed_for_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> AnalysisResult<(Circuit, Vec<Flow>)> {
    circuit_time_reversed_for_flows_with_options(
        circuit,
        flows,
        TimeReversedForFlowsOptions::default(),
    )
}

/// Returns tracker-driven time reversal for flows with explicit options.
pub fn circuit_time_reversed_for_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
) -> AnalysisResult<(Circuit, Vec<Flow>)> {
    if reverse_flow::requires_general_reversal(circuit, flows) {
        return reverse_flow::reverse_flows(circuit, flows, options);
    }
    for (index, flow) in flows.iter().enumerate() {
        reject_non_unitary_flow_terms(index, flow)?;
    }
    let inverse = circuit_inverse_unitary_strict(circuit).map_err(|error| {
        AnalysisError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary fast path requires a unitary circuit: {error}"
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
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows unitary fast path requires input circuit to satisfy flow {index}: {flow}"
            )));
        }
    }
    let reversed_flows = flows
        .iter()
        .map(reversed_pauli_only_flow)
        .collect::<Vec<_>>();
    let reversed_checks = check_unsigned_flows_with_sparse_tracker(&inverse, &reversed_flows)
        .map_err(|error| {
            AnalysisError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows could not validate reversed unitary flows: {error}"
            ))
        })?;
    for (index, (flow, satisfied)) in reversed_flows.iter().zip(reversed_checks).enumerate() {
        if !satisfied {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "time_reversed_for_flows produced an invalid reversed unitary flow {index}: {flow}"
            )));
        }
    }
    Ok((inverse, reversed_flows))
}

fn inverse_public_instruction(
    instruction: &CircuitInstruction,
) -> AnalysisResult<CircuitInstruction> {
    if instruction.gate().is_unitary() {
        return inverse_unitary_instruction(instruction);
    }
    let gate = instruction.gate();
    let args = match gate.canonical_name() {
        "TICK" => instruction.args().to_vec(),
        "SHIFT_COORDS" => instruction.args().iter().map(|arg| -*arg).collect(),
        "QUBIT_COORDS" => {
            return Err(AnalysisError::invalid_tableau_conversion(
                "inverting QUBIT_COORDS is supported only at the start of a circuit or repeat block",
            ));
        }
        _ => {
            return Err(AnalysisError::invalid_tableau_conversion(format!(
                "operation {} has no strict circuit inverse",
                gate.canonical_name()
            )));
        }
    };
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        gate,
        args,
        reversed_target_groups(instruction),
        instruction.tag_bytes(),
    )?)
}

fn inverse_coordinate_instruction(
    instruction: &CircuitInstruction,
) -> AnalysisResult<CircuitInstruction> {
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        instruction.args().to_vec(),
        reversed_target_groups(instruction),
        instruction.tag_bytes(),
    )?)
}

fn inverse_unitary_instruction(
    instruction: &CircuitInstruction,
) -> AnalysisResult<CircuitInstruction> {
    let gate = instruction.gate();
    if !gate.is_unitary() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "operation {} is not unitary",
            gate.canonical_name()
        )));
    }
    let inverse_gate = gate.best_candidate_inverse()?;
    let targets = reversed_target_groups(instruction);
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        inverse_gate,
        instruction.args().to_vec(),
        targets,
        instruction.tag_bytes(),
    )?)
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
    Flow::from_paulis(
        flow.output().with_sign(PauliSign::Plus),
        flow.input().with_sign(PauliSign::Plus),
    )
}

fn reject_non_unitary_flow_terms(index: usize, flow: &Flow) -> AnalysisResult<()> {
    if flow.measurements().next().is_some() || flow.observables().next().is_some() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary fast path does not support measurement-record or observable terms in flow {index}: {flow}"
        )));
    }
    Ok(())
}

enum FlowValidation {
    Tableau(Tableau),
    SparseFolded,
}

impl FlowValidation {
    fn for_circuit(circuit: &Circuit) -> AnalysisResult<Self> {
        let within_tableau_budget = stab_model::advanced::circuit_simulated_qubit_count(circuit)
            <= MAX_TIME_REVERSE_TABLEAU_QUBITS
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
            return Ok(Self::Tableau(crate::circuit_to_tableau(
                circuit, false, false, false,
            )?));
        }
        if sparse_tracker_can_validate_without_unbounded_unroll(circuit) {
            return Ok(Self::SparseFolded);
        }
        Err(AnalysisError::invalid_tableau_conversion(format!(
            "time_reversed_for_flows unitary fast path requires at most {MAX_TIME_REVERSE_TABLEAU_EXPANDED_INSTRUCTIONS} expanded instructions and {MAX_TIME_REVERSE_TABLEAU_QUBITS} tableau qubits unless the circuit is supported by folded sparse validation"
        )))
    }

    fn check_all(&self, circuit: &Circuit, flows: &[Flow]) -> AnalysisResult<Vec<bool>> {
        match self {
            Self::Tableau(tableau) => flows
                .iter()
                .map(|flow| unitary_flow_is_satisfied_by_tableau(tableau, flow))
                .collect(),
            Self::SparseFolded => check_unsigned_flows_with_sparse_tracker(circuit, flows)
                .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string())),
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
    crate::single_qubit_clifford_for_gate(instruction.gate()).is_ok()
        || matches!(instruction.gate().canonical_name(), "CX" | "CY" | "CZ")
}

fn unitary_flow_is_satisfied_by_tableau(tableau: &Tableau, flow: &Flow) -> AnalysisResult<bool> {
    let prefix_input = pauli_prefix(flow.input(), tableau.len());
    let actual_prefix = tableau
        .apply(&prefix_input)
        .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))?;
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
    stab_algebra::advanced::pauli_from_bases_unchecked(PauliSign::Plus, bases)
}
