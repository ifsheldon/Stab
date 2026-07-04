use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate, GateCategory,
    Pauli, QubitId, RepeatBlock, SingleQubitClifford, StabilizerError, Tableau, Target,
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

/// Rewrites supported operations into Stim's public `Circuit.decomposed()` base-gate set.
pub fn decomposed_circuit(circuit: &Circuit) -> CircuitResult<Circuit> {
    let mut result = Circuit::new();
    append_decomposed_circuit(circuit, &mut result)?;
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

fn append_decomposed_circuit(circuit: &Circuit, result: &mut Circuit) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                append_decomposed_instruction(instruction, result)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                result.append_repeat_block(RepeatBlock::new(
                    repeat.repeat_count(),
                    decomposed_circuit(repeat.body())?,
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

fn append_decomposed_instruction(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        "I" | "II" => return Ok(()),
        "MPP" => return append_decomposed_mpp(instruction, result),
        "SPP" => return append_decomposed_spp(instruction, result, false),
        "SPP_DAG" => return append_decomposed_spp(instruction, result, true),
        "MPAD" | "DETECTOR" | "OBSERVABLE_INCLUDE" | "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" => {
            result.append_instruction(instruction.clone());
            return Ok(());
        }
        _ if instruction.gate().is_noisy() && !instruction.gate().produces_measurements() => {
            result.append_instruction(instruction.clone());
            return Ok(());
        }
        _ if !instruction.gate().has_h_s_cx_m_r_decomposition() => {
            result.append_instruction(instruction.clone());
            return Ok(());
        }
        _ => {}
    }

    if instruction.gate().is_single_qubit_gate() {
        for segment in instruction.disjoint_target_segments() {
            for group in segment.target_groups() {
                append_template_decomposition(instruction, group, result)?;
            }
        }
        return Ok(());
    }

    if instruction.gate().is_two_qubit_gate() {
        for segment in instruction.disjoint_target_segments() {
            for group in segment.target_groups() {
                if !decomposed_pair_group_supported(instruction.gate(), group) {
                    return Err(invalid_simplification(format!(
                        "decomposition of {} with classical target group is not yet supported",
                        instruction.gate().canonical_name()
                    )));
                }
                append_template_decomposition(instruction, group, result)?;
            }
        }
        return Ok(());
    }

    result.append_instruction(instruction.clone());
    Ok(())
}

fn append_template_decomposition(
    instruction: &CircuitInstruction,
    actual_targets: &[Target],
    result: &mut Circuit,
) -> CircuitResult<()> {
    let template = instruction
        .gate()
        .h_s_cx_m_r_decomposition()
        .map_err(|error| invalid_simplification(error.to_string()))?
        .to_circuit()?;
    for item in template.items() {
        let CircuitItem::Instruction(template_instruction) = item else {
            return Err(invalid_simplification(format!(
                "{} decomposition metadata unexpectedly contained a repeat block",
                instruction.gate().canonical_name()
            )));
        };
        let targets = template_instruction
            .targets()
            .iter()
            .map(|target| {
                substitute_template_target(
                    target,
                    actual_targets,
                    template_instruction.gate().produces_measurements(),
                )
            })
            .collect::<CircuitResult<Vec<_>>>()?;
        append_gate_targets(
            result,
            template_instruction.gate(),
            targets,
            instruction.tag(),
        )?;
    }
    Ok(())
}

fn substitute_template_target(
    target: &Target,
    actual_targets: &[Target],
    preserves_inversion: bool,
) -> CircuitResult<Target> {
    match target {
        Target::Qubit { id, .. } => {
            let index = id.get() as usize;
            let actual = actual_targets.get(index).ok_or_else(|| {
                invalid_simplification(format!(
                    "decomposition template referenced missing target {index}"
                ))
            })?;
            Ok(match actual {
                Target::Qubit {
                    id,
                    inverted: actual_inverted,
                } => Target::qubit(*id, preserves_inversion && *actual_inverted),
                Target::MeasurementRecord { offset } => Target::measurement_record(*offset),
                Target::SweepBit { id } => Target::sweep_bit(*id),
                Target::Pauli { id, .. } => Target::qubit(*id, false),
                Target::Combiner => {
                    return Err(invalid_simplification(
                        "decomposition template cannot substitute a combiner target",
                    ));
                }
            })
        }
        Target::MeasurementRecord { offset } => Ok(Target::measurement_record(*offset)),
        Target::SweepBit { id } => Ok(Target::sweep_bit(*id)),
        Target::Pauli {
            pauli,
            id,
            inverted,
        } => Ok(Target::pauli(*pauli, *id, *inverted)),
        Target::Combiner => Ok(Target::combiner()),
    }
}

fn decomposed_pair_group_supported(gate: Gate, group: &[Target]) -> bool {
    group.iter().all(|target| target.qubit_id().is_some())
        || matches!(gate.canonical_name(), "CX" | "CY" | "CZ")
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

fn append_decomposed_mpp(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
) -> CircuitResult<()> {
    for group in instruction.target_groups() {
        let product = reduce_pauli_product(group)?;
        if product.terms.is_empty() {
            append_gate_targets(
                result,
                Gate::from_name("MPAD")?,
                vec![Target::qubit(
                    QubitId::new(u32::from(product.negative))?,
                    false,
                )],
                instruction.tag(),
            )?;
            continue;
        }
        append_product_basis_change(result, &product.terms, instruction.tag())?;
        append_product_cx_fanout(result, &product.terms, instruction.tag())?;
        let accumulator = product
            .terms
            .first()
            .ok_or_else(|| invalid_simplification("missing MPP accumulator"))?
            .qubit;
        append_gate_targets(
            result,
            Gate::from_name("M")?,
            vec![Target::qubit(accumulator, product.negative)],
            instruction.tag(),
        )?;
        append_product_cx_fanout(result, &product.terms, instruction.tag())?;
        append_product_basis_change_reversed(result, &product.terms, instruction.tag())?;
    }
    Ok(())
}

fn append_decomposed_spp(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
    dagger: bool,
) -> CircuitResult<()> {
    for group in instruction.target_groups() {
        let product = reduce_pauli_product(group)?;
        if product.terms.is_empty() {
            continue;
        }
        append_product_basis_change(result, &product.terms, instruction.tag())?;
        append_product_cx_fanout(result, &product.terms, instruction.tag())?;
        let phase_gate = if product.negative ^ dagger {
            Gate::from_name("S_DAG")?
        } else {
            Gate::from_name("S")?
        };
        append_single_target_sequence(
            result,
            &shortest_single_qubit_base_sequence(
                SingleQubitClifford::from_gate(phase_gate).map_err(stabilizer_to_simplify_error)?,
            )?,
            Target::qubit(
                product
                    .terms
                    .first()
                    .ok_or_else(|| invalid_simplification("missing SPP accumulator"))?
                    .qubit,
                false,
            ),
            instruction.tag(),
        )?;
        append_product_cx_fanout(result, &product.terms, instruction.tag())?;
        append_product_basis_change_reversed(result, &product.terms, instruction.tag())?;
    }
    Ok(())
}

fn append_product_basis_change(
    result: &mut Circuit,
    terms: &[ProductTerm],
    tag: Option<&str>,
) -> CircuitResult<()> {
    for term in terms {
        append_basis_change(result, *term, tag)?;
    }
    Ok(())
}

fn append_product_basis_change_reversed(
    result: &mut Circuit,
    terms: &[ProductTerm],
    tag: Option<&str>,
) -> CircuitResult<()> {
    for term in terms.iter().rev() {
        append_basis_change(result, *term, tag)?;
    }
    Ok(())
}

fn append_basis_change(
    result: &mut Circuit,
    term: ProductTerm,
    tag: Option<&str>,
) -> CircuitResult<()> {
    match term.pauli {
        Pauli::X => append_gate_on_qubit(result, Gate::from_name("H")?, term.qubit, tag),
        Pauli::Y => append_single_target_sequence(
            result,
            &shortest_single_qubit_base_sequence(
                SingleQubitClifford::from_gate(Gate::from_name("H_YZ")?)
                    .map_err(stabilizer_to_simplify_error)?,
            )?,
            Target::qubit(term.qubit, false),
            tag,
        ),
        Pauli::Z => Ok(()),
    }
}

fn append_product_cx_fanout(
    result: &mut Circuit,
    terms: &[ProductTerm],
    tag: Option<&str>,
) -> CircuitResult<()> {
    let Some(accumulator) = terms.first().map(|term| term.qubit) else {
        return Ok(());
    };
    let cx = Gate::from_name("CX")?;
    for term in terms.iter().skip(1) {
        append_gate_targets(
            result,
            cx,
            vec![
                Target::qubit(term.qubit, false),
                Target::qubit(accumulator, false),
            ],
            tag,
        )?;
    }
    Ok(())
}

fn append_gate_on_qubit(
    result: &mut Circuit,
    gate: Gate,
    qubit: QubitId,
    tag: Option<&str>,
) -> CircuitResult<()> {
    append_gate_targets(result, gate, vec![Target::qubit(qubit, false)], tag)
}

fn append_gate_targets(
    result: &mut Circuit,
    gate: Gate,
    targets: Vec<Target>,
    tag: Option<&str>,
) -> CircuitResult<()> {
    if targets.is_empty() {
        return Ok(());
    }
    result.append_instruction(CircuitInstruction::new(
        gate,
        Vec::new(),
        targets,
        tag.map(ToOwned::to_owned),
    )?);
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProductTerm {
    qubit: QubitId,
    pauli: Pauli,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReducedProduct {
    negative: bool,
    terms: Vec<ProductTerm>,
}

fn reduce_pauli_product(group: &[Target]) -> CircuitResult<ReducedProduct> {
    let mut phase = 0_u8;
    let mut terms = BTreeMap::<QubitId, Pauli>::new();
    let mut order = Vec::<QubitId>::new();

    for target in group {
        match target {
            Target::Pauli {
                pauli,
                id,
                inverted,
            } => {
                if *inverted {
                    phase = (phase + 2) % 4;
                }
                if !terms.contains_key(id) && !order.contains(id) {
                    order.push(*id);
                }
                let current = terms.remove(id);
                let (next_phase, next_pauli) = multiply_pauli(current, *pauli);
                phase = (phase + next_phase) % 4;
                if let Some(next_pauli) = next_pauli {
                    terms.insert(*id, next_pauli);
                }
            }
            Target::Combiner => {}
            _ => {
                return Err(invalid_simplification(format!(
                    "Pauli product decomposition expected Pauli targets, got {target}"
                )));
            }
        }
    }

    if !phase.is_multiple_of(2) {
        return Err(invalid_simplification(
            "Pauli product decomposition encountered an anti-Hermitian product",
        ));
    }

    Ok(ReducedProduct {
        negative: phase == 2,
        terms: order
            .into_iter()
            .filter_map(|qubit| {
                terms
                    .remove(&qubit)
                    .map(|pauli| ProductTerm { qubit, pauli })
            })
            .collect(),
    })
}

fn multiply_pauli(current: Option<Pauli>, next: Pauli) -> (u8, Option<Pauli>) {
    let Some(current) = current else {
        return (0, Some(next));
    };
    match (current, next) {
        (Pauli::X, Pauli::X) | (Pauli::Y, Pauli::Y) | (Pauli::Z, Pauli::Z) => (0, None),
        (Pauli::X, Pauli::Y) => (1, Some(Pauli::Z)),
        (Pauli::Y, Pauli::Z) => (1, Some(Pauli::X)),
        (Pauli::Z, Pauli::X) => (1, Some(Pauli::Y)),
        (Pauli::Y, Pauli::X) => (3, Some(Pauli::Z)),
        (Pauli::Z, Pauli::Y) => (3, Some(Pauli::X)),
        (Pauli::X, Pauli::Z) => (3, Some(Pauli::Y)),
    }
}

fn stabilizer_to_simplify_error(error: StabilizerError) -> CircuitError {
    invalid_simplification(error.to_string())
}

fn invalid_simplification(message: impl Into<String>) -> CircuitError {
    CircuitError::invalid_circuit_simplification(message)
}
