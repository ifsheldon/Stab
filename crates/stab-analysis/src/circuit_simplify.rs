use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ops::ControlFlow;

use stab_algebra::{SingleQubitClifford, StabilizerError};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, Gate, GateKind, Pauli, PauliProduct, PauliTarget,
    QubitId, Target, TargetKind,
};

use crate::instruction_shape;
use crate::{AnalysisError, AnalysisOperation, AnalysisResult};

/// Rewrites supported operations into Stim's public `Circuit.decomposed()` base-gate set.
pub fn decomposed_circuit(circuit: &Circuit) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    append_decomposed_circuit(circuit, &mut result)?;
    Ok(result)
}

/// Decomposes one instruction for execution and higher-level analysis lowering.
#[doc(hidden)]
pub(crate) fn decomposed_single_instruction(
    instruction: &CircuitInstruction,
) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    append_decomposed_instruction(instruction, &mut result)?;
    Ok(result)
}

/// Visits the base-gate decomposition of one `SPP` or `SPP_DAG` instruction.
///
/// The visitor receives each lowered instruction as soon as it is produced. Returning
/// [`ControlFlow::Break`] stops lowering before later instructions are materialized.
#[doc(hidden)]
pub fn visit_decomposed_spp_instructions<Break>(
    instruction: &CircuitInstruction,
    mut visitor: impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
) -> AnalysisResult<ControlFlow<Break>> {
    let dagger = match instruction.gate().kind() {
        GateKind::SPP => false,
        GateKind::SPP_DAG => true,
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
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
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::MPP
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => {
            return Err(invalid_simplification(format!(
                "SPP decomposition expected SPP or SPP_DAG, got {}",
                instruction.gate().canonical_name()
            )));
        }
    };
    visit_decomposed_spp(instruction, dagger, &mut visitor)
}

fn append_decomposed_circuit(circuit: &Circuit, result: &mut Circuit) -> AnalysisResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                append_decomposed_instruction(instruction, result)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                result.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    decomposed_circuit(repeat.body())?,
                    repeat.tag_bytes(),
                ));
            }
        }
    }
    Ok(())
}

fn append_decomposed_instruction(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
) -> AnalysisResult<()> {
    match instruction.gate().kind() {
        GateKind::I | GateKind::II => return Ok(()),
        GateKind::MPP => return append_decomposed_mpp(instruction, result),
        GateKind::SPP => return append_decomposed_spp(instruction, result, false),
        GateKind::SPP_DAG => return append_decomposed_spp(instruction, result, true),
        GateKind::MPAD
        | GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS => {
            result.append_instruction(instruction.clone());
            return Ok(());
        }
        // Every other gate is kept or decomposed by its table classification below.
        GateKind::REPEAT
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
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
        | GateKind::S_DAG
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
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => {}
    }
    if (instruction.gate().is_noisy() && !instruction.gate().produces_measurements())
        || !crate::gate_has_h_s_cx_m_r_decomposition(instruction.gate())
    {
        result.append_instruction(instruction.clone());
        return Ok(());
    }

    if instruction.gate().is_single_qubit_gate() || instruction.gate().is_two_qubit_gate() {
        for segment in instruction.disjoint_target_segments() {
            append_template_decomposition(instruction, &segment, result)?;
        }
        return Ok(());
    }

    result.append_instruction(instruction.clone());
    Ok(())
}

fn append_template_decomposition(
    instruction: &CircuitInstruction,
    actual_segment: &CircuitInstruction,
    result: &mut Circuit,
) -> AnalysisResult<()> {
    let decomposition = crate::gate_h_s_cx_m_r_decomposition(instruction.gate())
        .map_err(|error| invalid_simplification(error.to_string()))?;
    let template = crate::gate_decomposition_to_circuit(decomposition)?;
    let actual_groups = actual_segment.target_groups();
    for item in template.items() {
        let CircuitItem::Instruction(template_instruction) = item else {
            return Err(invalid_simplification(format!(
                "{} decomposition metadata unexpectedly contained a repeat block",
                instruction.gate().canonical_name()
            )));
        };
        let mut targets = Vec::new();
        for template_group in template_instruction.target_groups() {
            for actual_group in actual_groups.clone() {
                for template_target in template_group {
                    let target = substitute_template_target(
                        template_target,
                        actual_group,
                        template_instruction.gate().produces_measurements(),
                    )?;
                    if !target.is_classical_bit_target()
                        || template_instruction
                            .gate()
                            .takes_measurement_record_targets()
                    {
                        targets.push(target);
                    }
                }
            }
        }
        result.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
            template_instruction.gate(),
            Vec::new(),
            targets,
            instruction.tag_bytes(),
        )?);
    }
    Ok(())
}

fn substitute_template_target(
    target: &Target,
    actual_targets: &[Target],
    preserves_inversion: bool,
) -> AnalysisResult<Target> {
    let TargetKind::Qubit { id, .. } = target.kind() else {
        return Ok(*target);
    };
    let index = id.get() as usize;
    let actual = actual_targets.get(index).ok_or_else(|| {
        invalid_simplification(format!(
            "decomposition template referenced missing target {index}"
        ))
    })?;
    Ok(match actual.kind() {
        TargetKind::Qubit {
            id,
            inverted: actual_inverted,
        } => Target::qubit(id, preserves_inversion && actual_inverted),
        TargetKind::MeasurementRecord { .. } | TargetKind::SweepBit { .. } => *actual,
        TargetKind::Pauli { id, .. } => Target::qubit(id, false),
        TargetKind::Combiner => {
            return Err(invalid_simplification(
                "decomposition template cannot substitute a combiner target",
            ));
        }
    })
}

fn append_single_target_sequence(
    result: &mut Circuit,
    sequence: &[Gate],
    target: Target,
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    for gate in sequence {
        result.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
            *gate,
            Vec::new(),
            vec![target],
            tag,
        )?);
    }
    Ok(())
}

fn append_decomposed_mpp(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
) -> AnalysisResult<()> {
    let products =
        instruction_shape::pauli_products(instruction, AnalysisOperation::CircuitSimplification)?;
    for group in products {
        let product = reduce_pauli_product(instruction.gate().kind(), group)?;
        if product.terms.is_empty() {
            append_gate_targets(
                result,
                GateKind::MPAD.gate(),
                vec![Target::qubit(
                    QubitId::new(u32::from(product.negative))?,
                    false,
                )],
                instruction.tag_bytes(),
            )?;
            continue;
        }
        append_product_basis_change(result, &product.terms, instruction.tag_bytes())?;
        append_product_cx_fanout(result, &product.terms, instruction.tag_bytes())?;
        let accumulator = product
            .terms
            .first()
            .ok_or_else(|| invalid_simplification("missing MPP accumulator"))?
            .qubit;
        append_gate_targets(
            result,
            GateKind::M.gate(),
            vec![Target::qubit(accumulator, product.negative)],
            instruction.tag_bytes(),
        )?;
        append_product_cx_fanout(result, &product.terms, instruction.tag_bytes())?;
        append_product_basis_change_reversed(result, &product.terms, instruction.tag_bytes())?;
    }
    Ok(())
}

fn append_decomposed_spp(
    instruction: &CircuitInstruction,
    result: &mut Circuit,
    dagger: bool,
) -> AnalysisResult<()> {
    let completed: ControlFlow<()> = visit_decomposed_spp(instruction, dagger, &mut |lowered| {
        result.append_instruction(lowered);
        ControlFlow::Continue(())
    })?;
    if completed.is_break() {
        return Err(invalid_simplification(
            "circuit-owned SPP decomposition stopped unexpectedly",
        ));
    }
    Ok(())
}

fn visit_decomposed_spp<Break>(
    instruction: &CircuitInstruction,
    dagger: bool,
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
) -> AnalysisResult<ControlFlow<Break>> {
    let products =
        instruction_shape::pauli_products(instruction, AnalysisOperation::CircuitSimplification)?;
    for group in products {
        let product = reduce_pauli_product(instruction.gate().kind(), group)?;
        if product.terms.is_empty() {
            continue;
        }
        let completion =
            visit_product_basis_change(visitor, &product.terms, instruction.tag_bytes())?;
        if completion.is_break() {
            return Ok(completion);
        }
        let completion = visit_product_cx_fanout(visitor, &product.terms, instruction.tag_bytes())?;
        if completion.is_break() {
            return Ok(completion);
        }
        let phase_gate = if product.negative ^ dagger {
            GateKind::S_DAG.gate()
        } else {
            GateKind::S.gate()
        };
        let completion = visit_single_target_sequence(
            visitor,
            &shortest_single_qubit_base_sequence(
                crate::single_qubit_clifford_for_gate(phase_gate)
                    .map_err(stabilizer_to_simplify_error)?,
            )?,
            Target::qubit(
                product
                    .terms
                    .first()
                    .ok_or_else(|| invalid_simplification("missing SPP accumulator"))?
                    .qubit,
                false,
            ),
            instruction.tag_bytes(),
        )?;
        if completion.is_break() {
            return Ok(completion);
        }
        let completion = visit_product_cx_fanout(visitor, &product.terms, instruction.tag_bytes())?;
        if completion.is_break() {
            return Ok(completion);
        }
        let completion =
            visit_product_basis_change_reversed(visitor, &product.terms, instruction.tag_bytes())?;
        if completion.is_break() {
            return Ok(completion);
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn visit_product_basis_change<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    terms: &[ProductTerm],
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    for term in terms {
        let completion = visit_basis_change(visitor, *term, tag)?;
        if completion.is_break() {
            return Ok(completion);
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn visit_product_basis_change_reversed<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    terms: &[ProductTerm],
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    for term in terms.iter().rev() {
        let completion = visit_basis_change(visitor, *term, tag)?;
        if completion.is_break() {
            return Ok(completion);
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn visit_basis_change<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    term: ProductTerm,
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    match term.pauli {
        Pauli::X => visit_gate_targets(
            visitor,
            GateKind::H.gate(),
            vec![Target::qubit(term.qubit, false)],
            tag,
        ),
        Pauli::Y => visit_single_target_sequence(
            visitor,
            &shortest_single_qubit_base_sequence(
                crate::single_qubit_clifford_for_gate(GateKind::H_YZ.gate())
                    .map_err(stabilizer_to_simplify_error)?,
            )?,
            Target::qubit(term.qubit, false),
            tag,
        ),
        Pauli::Z => Ok(ControlFlow::Continue(())),
    }
}

fn visit_product_cx_fanout<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    terms: &[ProductTerm],
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    let Some(accumulator) = terms.first().map(|term| term.qubit) else {
        return Ok(ControlFlow::Continue(()));
    };
    let cx = GateKind::CX.gate();
    for term in terms.iter().skip(1) {
        let completion = visit_gate_targets(
            visitor,
            cx,
            vec![
                Target::qubit(term.qubit, false),
                Target::qubit(accumulator, false),
            ],
            tag,
        )?;
        if completion.is_break() {
            return Ok(completion);
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn visit_single_target_sequence<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    sequence: &[Gate],
    target: Target,
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    for gate in sequence {
        let completion = visit_gate_targets(visitor, *gate, vec![target], tag)?;
        if completion.is_break() {
            return Ok(completion);
        }
    }
    Ok(ControlFlow::Continue(()))
}

fn visit_gate_targets<Break>(
    visitor: &mut impl FnMut(CircuitInstruction) -> ControlFlow<Break>,
    gate: Gate,
    targets: Vec<Target>,
    tag: Option<&[u8]>,
) -> AnalysisResult<ControlFlow<Break>> {
    if targets.is_empty() {
        return Ok(ControlFlow::Continue(()));
    }
    let instruction =
        stab_model::advanced::circuit_instruction_with_tag_bytes(gate, Vec::new(), targets, tag)?;
    Ok(visitor(instruction))
}

fn append_product_basis_change(
    result: &mut Circuit,
    terms: &[ProductTerm],
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    for term in terms {
        append_basis_change(result, *term, tag)?;
    }
    Ok(())
}

fn append_product_basis_change_reversed(
    result: &mut Circuit,
    terms: &[ProductTerm],
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    for term in terms.iter().rev() {
        append_basis_change(result, *term, tag)?;
    }
    Ok(())
}

fn append_basis_change(
    result: &mut Circuit,
    term: ProductTerm,
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    match term.pauli {
        Pauli::X => append_gate_on_qubit(result, GateKind::H.gate(), term.qubit, tag),
        Pauli::Y => append_single_target_sequence(
            result,
            &shortest_single_qubit_base_sequence(
                crate::single_qubit_clifford_for_gate(GateKind::H_YZ.gate())
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
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    let Some(accumulator) = terms.first().map(|term| term.qubit) else {
        return Ok(());
    };
    let cx = GateKind::CX.gate();
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
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    append_gate_targets(result, gate, vec![Target::qubit(qubit, false)], tag)
}

fn append_gate_targets(
    result: &mut Circuit,
    gate: Gate,
    targets: Vec<Target>,
    tag: Option<&[u8]>,
) -> AnalysisResult<()> {
    if targets.is_empty() {
        return Ok(());
    }
    result.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
        gate,
        Vec::new(),
        targets,
        tag,
    )?);
    Ok(())
}

fn shortest_single_qubit_base_sequence(clifford: SingleQubitClifford) -> AnalysisResult<Vec<Gate>> {
    let target = clifford.tableau();
    let h = (
        GateKind::H.gate(),
        crate::single_qubit_clifford_for_gate(GateKind::H.gate())
            .map_err(stabilizer_to_simplify_error)?
            .tableau(),
    );
    let s = (
        GateKind::S.gate(),
        crate::single_qubit_clifford_for_gate(GateKind::S.gate())
            .map_err(stabilizer_to_simplify_error)?
            .tableau(),
    );
    let mut queue = VecDeque::from([(
        stab_algebra::Tableau::identity(1).map_err(stabilizer_to_simplify_error)?,
        Vec::<Gate>::new(),
    )]);
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
struct ProductTerm {
    qubit: QubitId,
    pauli: Pauli,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReducedProduct {
    negative: bool,
    terms: Vec<ProductTerm>,
}

fn reduce_pauli_product(gate: GateKind, group: PauliProduct<'_>) -> AnalysisResult<ReducedProduct> {
    #[cfg(test)]
    {
        reduce_pauli_product_impl(gate, group, None)
    }
    #[cfg(not(test))]
    {
        reduce_pauli_product_impl(gate, group)
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PauliReductionWork {
    index_lookups: usize,
}

fn reduce_pauli_product_impl(
    gate: GateKind,
    group: PauliProduct<'_>,
    #[cfg(test)] mut work: Option<&mut PauliReductionWork>,
) -> AnalysisResult<ReducedProduct> {
    let mut phase = 0_u8;
    let mut term_indexes = BTreeMap::<QubitId, usize>::new();
    let mut terms = Vec::<(QubitId, Option<Pauli>)>::new();

    for PauliTarget {
        pauli,
        id,
        inverted,
    } in group
    {
        if inverted {
            phase = (phase + 2) % 4;
        }
        #[cfg(test)]
        if let Some(work) = work.as_deref_mut() {
            work.index_lookups += 1;
        }
        let index = if let Some(index) = term_indexes.get(&id) {
            *index
        } else {
            let index = terms.len();
            terms.push((id, None));
            term_indexes.insert(id, index);
            index
        };
        let slot = terms.get_mut(index).ok_or_else(|| {
            invalid_simplification("Pauli product index disagreed with retained terms")
        })?;
        let current = slot.1.take();
        let (next_phase, next_pauli) = multiply_pauli(current, pauli);
        phase = (phase + next_phase) % 4;
        slot.1 = next_pauli;
    }

    if !phase.is_multiple_of(2) {
        return Err(AnalysisError::AntiHermitianPauliProduct {
            operation: AnalysisOperation::CircuitSimplification,
            gate,
            phase: None,
        });
    }

    Ok(ReducedProduct {
        negative: phase == 2,
        terms: terms
            .into_iter()
            .filter_map(|(qubit, pauli)| pauli.map(|pauli| ProductTerm { qubit, pauli }))
            .collect(),
    })
}

#[cfg(test)]
fn reduce_pauli_product_with_work(
    group: PauliProduct<'_>,
) -> AnalysisResult<(ReducedProduct, PauliReductionWork)> {
    let mut work = PauliReductionWork::default();
    let product = reduce_pauli_product_impl(GateKind::MPP, group, Some(&mut work))?;
    Ok((product, work))
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

fn stabilizer_to_simplify_error(error: StabilizerError) -> AnalysisError {
    invalid_simplification(error.to_string())
}

/// A decomposition step the gate table and the validated circuit should have excluded.
fn invalid_simplification(message: impl Into<Cow<'static, str>>) -> AnalysisError {
    AnalysisError::internal(AnalysisOperation::CircuitSimplification, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "the generated fixture shape is an invariant of this focused work-count test"
    )]
    fn huge_single_line_mpp_reduces_without_quadratic_order_scanning() {
        const TARGET_COUNT: usize = 200_000;

        let mut text = String::from("MPP ");
        for index in 0..TARGET_COUNT {
            if index > 0 {
                text.push('*');
            }
            text.push('X');
            text.push_str(&index.to_string());
        }
        text.push('\n');

        let circuit = Circuit::from_stim_str(&text).expect("hostile MPP line parses");
        let instruction = circuit
            .items()
            .first()
            .and_then(|item| match item {
                CircuitItem::Instruction(instruction) => Some(instruction),
                CircuitItem::RepeatBlock(_) => None,
            })
            .expect("fixture contains one MPP instruction");
        let stab_model::GateTargets::PauliProducts(mut groups) = instruction.gate_targets() else {
            unreachable!("MPP targets are Pauli products");
        };
        let group = groups.next().expect("MPP instruction has one product");

        let (product, work) =
            reduce_pauli_product_with_work(group).expect("hostile MPP product reduces");

        assert_eq!(product.terms.len(), TARGET_COUNT);
        assert_eq!(
            work,
            PauliReductionWork {
                index_lookups: TARGET_COUNT,
            }
        );
    }
}
