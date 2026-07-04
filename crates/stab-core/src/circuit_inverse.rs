use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Flow, GateCategory,
    PauliBasis, PauliSign, PauliString, RepeatBlock, Tableau, Target,
    circuit_flow::check_unsigned_flow_with_sparse_tracker,
};

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
/// In M6 this delegates to `circuit_inverse_unitary`. Stim's QEC-specific inverse
/// rewrites for measurements, resets, detectors, noise, and feedback are deferred.
pub fn circuit_inverse_qec(circuit: &Circuit) -> CircuitResult<Circuit> {
    circuit_inverse_unitary(circuit)
}

/// Returns the currently supported unitary time-reversal subset for flows.
///
/// This additive API validates that every provided unsigned flow is satisfied by
/// the original unitary circuit, returns the QEC inverse subset, and swaps each
/// flow's input and output Pauli terms. Measurement-record and observable flow
/// terms remain outside this scoped implementation because Stim's richer QEC
/// measurement, detector, feedback, and noise rewrites are still deferred.
pub fn circuit_time_reversed_for_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<(Circuit, Vec<Flow>)> {
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
    let reversed_flows = flows
        .iter()
        .map(|flow| {
            Flow::new(
                flow.output().with_sign(PauliSign::Plus),
                flow.input().with_sign(PauliSign::Plus),
                [],
                [],
            )
        })
        .collect();
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
            sparse_tracker_supports_top_level_unitary(instruction.gate().canonical_name())
        }
        CircuitItem::RepeatBlock(repeat) => {
            sparse_tracker_supports_folded_unitary_repeat(repeat.body())
        }
    })
}

fn sparse_tracker_supports_top_level_unitary(gate_name: &str) -> bool {
    matches!(
        gate_name,
        "H" | "H_XY" | "S" | "S_DAG" | "C_XYZ" | "CX" | "CY" | "CZ"
    )
}

fn sparse_tracker_supports_folded_unitary_repeat(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => matches!(
            instruction.gate().canonical_name(),
            "H" | "H_XY" | "S" | "S_DAG" | "C_XYZ" | "CX" | "CZ"
        ),
        CircuitItem::RepeatBlock(repeat) => {
            sparse_tracker_supports_folded_unitary_repeat(repeat.body())
        }
    })
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
