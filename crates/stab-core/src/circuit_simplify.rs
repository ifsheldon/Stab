use std::collections::{BTreeSet, VecDeque};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate, GateCategory,
    RepeatBlock, SingleQubitClifford, StabilizerError, Tableau, Target,
};

/// Rewrites supported Clifford operations into the current base-gate subset.
///
/// M6 covers single-qubit Clifford gates and selected two-qubit Clifford gates used
/// by the tableau milestone. Gates outside that subset are preserved verbatim.
pub fn simplified_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    let mut result = Circuit::new();
    append_simplified_circuit(circuit, &mut result)?;
    Ok(result)
}

fn append_simplified_circuit(circuit: &Circuit, result: &mut Circuit) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                append_simplified_instruction(instruction, result)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                result.append_repeat_block(RepeatBlock::new(
                    repeat.repeat_count(),
                    simplified_circuit(repeat.body())?,
                    repeat.tag().map(ToOwned::to_owned),
                ));
            }
        }
    }
    Ok(())
}

fn append_simplified_instruction(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
) -> CircuitResult<()> {
    if instruction.args().is_empty()
        && let Some(sequence) = single_qubit_base_decomposition(instruction.gate())?
    {
        for target in instruction.targets() {
            append_single_target_sequence(result, &sequence, target.clone(), instruction.tag())?;
        }
        return Ok(());
    }

    if instruction.args().is_empty()
        && let Some(sequence) = two_qubit_base_decomposition(instruction.gate())?
    {
        for group in instruction.target_groups() {
            if group.iter().all(Target::is_qubit_target) {
                append_two_target_sequence(result, &sequence, group, instruction.tag())?;
            } else {
                append_instruction_group(result, instruction, group)?;
            }
        }
        return Ok(());
    }

    result.append_instruction(instruction.clone());
    Ok(())
}

fn append_single_target_sequence(
    result: &mut Circuit,
    sequence: &[Gate],
    target: Target,
    tag: Option<&str>,
) -> CircuitResult<()> {
    for gate in sequence {
        result.append_instruction(CircuitInstruction::new(
            *gate,
            Vec::new(),
            vec![target.clone()],
            tag.map(ToOwned::to_owned),
        )?);
    }
    Ok(())
}

fn append_instruction_group(
    result: &mut Circuit,
    instruction: &CircuitInstruction,
    targets: &[Target],
) -> CircuitResult<()> {
    result.append_instruction(CircuitInstruction::new(
        instruction.gate(),
        instruction.args().to_vec(),
        targets.to_vec(),
        instruction.tag().map(ToOwned::to_owned),
    )?);
    Ok(())
}

fn append_two_target_sequence(
    result: &mut Circuit,
    sequence: &[BaseTwoQubitStep],
    targets: &[Target],
    tag: Option<&str>,
) -> CircuitResult<()> {
    let left = targets
        .first()
        .cloned()
        .ok_or_else(|| invalid_simplification("missing first two-qubit target"))?;
    let right = targets
        .get(1)
        .cloned()
        .ok_or_else(|| invalid_simplification("missing second two-qubit target"))?;
    for step in sequence {
        match step {
            BaseTwoQubitStep::Right(gate) => append_single_target_sequence(
                result,
                std::slice::from_ref(gate),
                right.clone(),
                tag,
            )?,
            BaseTwoQubitStep::Pair(gate, order) => {
                let pair = match order {
                    PairOrder::LeftRight => vec![left.clone(), right.clone()],
                    PairOrder::RightLeft => vec![right.clone(), left.clone()],
                };
                result.append_instruction(CircuitInstruction::new(
                    *gate,
                    Vec::new(),
                    pair,
                    tag.map(ToOwned::to_owned),
                )?);
            }
        }
    }
    Ok(())
}

fn single_qubit_base_decomposition(gate: Gate) -> CircuitResult<Option<Vec<Gate>>> {
    if !matches!(
        gate.category(),
        GateCategory::HadamardLike
            | GateCategory::Pauli
            | GateCategory::Period3
            | GateCategory::Period4
    ) {
        return Ok(None);
    }
    let clifford = SingleQubitClifford::from_gate(gate).map_err(stabilizer_to_simplify_error)?;
    shortest_single_qubit_base_sequence(clifford).map(Some)
}

fn two_qubit_base_decomposition(gate: Gate) -> CircuitResult<Option<Vec<BaseTwoQubitStep>>> {
    let h = Gate::from_name("H")?;
    let s = Gate::from_name("S")?;
    let cx = Gate::from_name("CX")?;
    Ok(match gate.canonical_name() {
        "CX" => Some(vec![BaseTwoQubitStep::Pair(cx, PairOrder::LeftRight)]),
        "CZ" => Some(vec![
            BaseTwoQubitStep::Right(h),
            BaseTwoQubitStep::Pair(cx, PairOrder::LeftRight),
            BaseTwoQubitStep::Right(h),
        ]),
        "CY" => Some(vec![
            BaseTwoQubitStep::Right(s),
            BaseTwoQubitStep::Right(s),
            BaseTwoQubitStep::Right(s),
            BaseTwoQubitStep::Pair(cx, PairOrder::LeftRight),
            BaseTwoQubitStep::Right(s),
        ]),
        "SWAP" => Some(vec![
            BaseTwoQubitStep::Pair(cx, PairOrder::LeftRight),
            BaseTwoQubitStep::Pair(cx, PairOrder::RightLeft),
            BaseTwoQubitStep::Pair(cx, PairOrder::LeftRight),
        ]),
        _ => None,
    })
}

fn shortest_single_qubit_base_sequence(clifford: SingleQubitClifford) -> CircuitResult<Vec<Gate>> {
    let target = clifford.tableau();
    let h = (
        Gate::from_name("H")?,
        SingleQubitClifford::from_gate(Gate::from_name("H")?)
            .map_err(stabilizer_to_simplify_error)?
            .tableau(),
    );
    let s = (
        Gate::from_name("S")?,
        SingleQubitClifford::from_gate(Gate::from_name("S")?)
            .map_err(stabilizer_to_simplify_error)?
            .tableau(),
    );
    let mut queue = VecDeque::from([(Tableau::identity(1), Vec::<Gate>::new())]);
    let mut seen = BTreeSet::new();
    while let Some((tableau, sequence)) = queue.pop_front() {
        if tableau == target {
            return Ok(sequence);
        }
        if !seen.insert(tableau.to_string()) {
            continue;
        }
        for (gate, gate_tableau) in [&h, &s] {
            let mut next_sequence = sequence.clone();
            next_sequence.push(*gate);
            let next_tableau = tableau
                .then(gate_tableau)
                .map_err(stabilizer_to_simplify_error)?;
            queue.push_back((next_tableau, next_sequence));
        }
    }
    Err(invalid_simplification(format!(
        "no H/S decomposition for {}",
        clifford.canonical_name()
    )))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BaseTwoQubitStep {
    Right(Gate),
    Pair(Gate, PairOrder),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PairOrder {
    LeftRight,
    RightLeft,
}

fn stabilizer_to_simplify_error(error: StabilizerError) -> CircuitError {
    invalid_simplification(error.to_string())
}

fn invalid_simplification(message: impl Into<String>) -> CircuitError {
    CircuitError::invalid_circuit_simplification(message)
}
