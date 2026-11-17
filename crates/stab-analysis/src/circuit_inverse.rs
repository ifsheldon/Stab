use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString, Tableau};
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, GateKind, MemoryBudget, RepeatBlock,
    Target,
};

use crate::{
    AnalysisError, AnalysisResult,
    circuit_flow::{
        FlowCheckAlgorithm, FlowCheckCost, UnsignedFlowCheckOptions,
        check_unsigned_flows_with_sparse_tracker, stops_flow_check,
    },
    sparse_rev_frame_tracker::folds_unitary_repeat_body,
};

mod reverse_flow;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InverseQecOptions {
    /// Preserve measurements instead of turning eligible measurements into resets.
    pub keep_measurements: bool,
    /// Budget for the expanded measurement-rich reversal, as
    /// [`TimeReversedForFlowsOptions::memory_budget`].
    pub memory_budget: MemoryBudget,
    /// Signal polled once per reversed instruction of the expanded measurement-rich reversal.
    pub cancellation: Cancellation,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimeReversedForFlowsOptions {
    /// Keep measurements as measurements instead of converting eligible ones to resets.
    ///
    /// This matches Stim v1.16.0's `dont_turn_measurements_into_resets` option
    /// across the supported tracker-driven measurement reversal surface.
    pub dont_turn_measurements_into_resets: bool,
    /// Budget for the retained copies of the expanded measurement-rich reversal, and for the
    /// dense Tableau that validates the input flows of a unitary circuit.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per reversed instruction of the expanded measurement-rich reversal, and
    /// by the flow checks that validate the input and reversed flows.
    pub cancellation: Cancellation,
}

impl TimeReversedForFlowsOptions {
    /// The budget and signal of the flow checks that validate the reversal.
    pub(crate) fn flow_check_options(&self) -> UnsignedFlowCheckOptions {
        UnsignedFlowCheckOptions {
            memory_budget: self.memory_budget,
            cancellation: self.cancellation.clone(),
        }
    }
}

/// Returns the strict inverse of a unitary circuit and its invertible annotations.
///
/// Leading `QUBIT_COORDS` instructions are preserved, `TICK` is self-inverse,
/// `SHIFT_COORDS` arguments are negated, and repeat blocks are inverted at every nesting depth
/// without native recursion.
/// Measurements, resets, noise, detectors, observables, and non-leading qubit
/// coordinates return an error instead of being skipped or approximated.
pub fn circuit_inverse_unitary(circuit: &Circuit) -> AnalysisResult<Circuit> {
    rebuild_reversed(
        circuit,
        |body| {
            let items = body.items();
            let leading_coordinates = items
                .iter()
                .take_while(|item| {
                    matches!(
                        item,
                        CircuitItem::Instruction(instruction)
                            if instruction.gate().kind() == GateKind::QUBIT_COORDS
                    )
                })
                .count();
            let (prefix, rest) = items.split_at(leading_coordinates);
            let mut result = Circuit::new();
            for item in prefix {
                let CircuitItem::Instruction(instruction) = item else {
                    return Err(AnalysisError::invalid_tableau_conversion(
                        "leading-coordinate prefix unexpectedly contained a repeat block",
                    ));
                };
                result.append_instruction(inverse_coordinate_instruction(instruction)?);
            }
            Ok((result, rest))
        },
        |instruction| inverse_public_instruction(instruction).map(Some),
        RepeatBodies::KeepEmpty,
    )
}

fn circuit_inverse_unitary_strict(circuit: &Circuit) -> AnalysisResult<Circuit> {
    rebuild_reversed(
        circuit,
        |body| Ok((Circuit::new(), body.items())),
        |instruction| inverse_unitary_instruction(instruction).map(Some),
        RepeatBodies::KeepEmpty,
    )
}

/// Returns the inverse of a circuit's unitary instructions, dropping every other instruction.
///
/// The caller guarantees the dropped instructions cannot act on the Pauli strings it tracks, for
/// example annotations and noise around Pauli-only flows; a repeat whose inverse body is empty is
/// dropped too.
pub(crate) fn unitary_part_inverse(circuit: &Circuit) -> AnalysisResult<Circuit> {
    rebuild_reversed(
        circuit,
        |body| Ok((Circuit::new(), body.items())),
        |instruction| {
            if instruction.gate().is_unitary() {
                inverse_unitary_instruction(instruction).map(Some)
            } else {
                Ok(None)
            }
        },
        RepeatBodies::DropEmpty,
    )
}

/// Whether a rebuilt repeat whose body came out empty is kept.
#[derive(Clone, Copy, Eq, PartialEq)]
enum RepeatBodies {
    KeepEmpty,
    DropEmpty,
}

/// One body being rebuilt in reverse: its items left to visit, its rebuilt part, and the repeat
/// block that owns it, `None` at the top level.
struct ReversedBody<'a> {
    items: std::iter::Rev<std::slice::Iter<'a, CircuitItem>>,
    result: Circuit,
    repeat: Option<&'a RepeatBlock>,
}

/// Rebuilds `circuit` with the items of every body in reverse order, keeping each repeat block's
/// count and tag around its rebuilt body.
///
/// The walk keeps its own stack, so nesting depth costs heap rather than native stack. `begin`
/// starts each body, the top level and every repeat body, returning its rebuilt prefix and the
/// items still to reverse; `instruction` maps each such instruction, dropping it on `None`.
fn rebuild_reversed<'a>(
    circuit: &'a Circuit,
    mut begin: impl FnMut(&'a Circuit) -> AnalysisResult<(Circuit, &'a [CircuitItem])>,
    mut instruction: impl FnMut(&CircuitInstruction) -> AnalysisResult<Option<CircuitInstruction>>,
    repeat_bodies: RepeatBodies,
) -> AnalysisResult<Circuit> {
    let stack_error =
        || AnalysisError::invalid_tableau_conversion("circuit inversion lost its body stack");
    let (result, items) = begin(circuit)?;
    let mut stack = vec![ReversedBody {
        items: items.iter().rev(),
        result,
        repeat: None,
    }];
    loop {
        let top = stack.last_mut().ok_or_else(stack_error)?;
        match top.items.next() {
            Some(CircuitItem::Instruction(original)) => {
                if let Some(inverse) = instruction(original)? {
                    top.result.append_instruction(inverse);
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let (result, items) = begin(repeat.body())?;
                stack.push(ReversedBody {
                    items: items.iter().rev(),
                    result,
                    repeat: Some(repeat),
                });
            }
            None => {
                let finished = stack.pop().ok_or_else(stack_error)?;
                let Some(repeat) = finished.repeat else {
                    return Ok(finished.result);
                };
                if repeat_bodies == RepeatBodies::DropEmpty && finished.result.items().is_empty() {
                    continue;
                }
                stack
                    .last_mut()
                    .ok_or_else(stack_error)?
                    .result
                    .append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                        repeat.repeat_count(),
                        finished.result,
                        repeat.tag_bytes(),
                    ));
            }
        }
    }
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
        memory_budget: options.memory_budget,
        cancellation: options.cancellation,
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
    time_reversed_for_flows_using(circuit, flows, options, FlowValidation::cheaper_algorithm)
}

/// Time reversal whose unitary fast path validates the input flows with the algorithm `select`
/// picks; every choice returns the same result.
pub(crate) fn time_reversed_for_flows_using(
    circuit: &Circuit,
    flows: &[Flow],
    options: TimeReversedForFlowsOptions,
    select: fn(&Circuit, &[Flow]) -> FlowCheckAlgorithm,
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
    let validation = FlowValidation::build(select(circuit, flows), circuit, &options)?;
    for (index, (flow, satisfied)) in flows
        .iter()
        .zip(validation.check_all(circuit, flows, &options)?)
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
    let reversed_checks = check_unsigned_flows_with_sparse_tracker(
        &inverse,
        &reversed_flows,
        &options.flow_check_options(),
    )
    .map_err(|error| {
        if stops_flow_check(&error) {
            return error;
        }
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
    let gate = instruction.gate();
    let args = match gate.kind() {
        GateKind::XCX
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
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG => return inverse_unitary_instruction(instruction),
        GateKind::TICK => instruction.args().to_vec(),
        GateKind::SHIFT_COORDS => instruction.args().iter().map(|arg| -*arg).collect(),
        GateKind::QUBIT_COORDS => {
            return Err(AnalysisError::invalid_tableau_conversion(
                "inverting QUBIT_COORDS is supported only at the start of a circuit or repeat block",
            ));
        }
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
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
        | GateKind::MPP
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => {
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
    let inverse_gate = gate.generalized_inverse();
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
    /// Chooses how the unitary fast path validates input flows. A repeat-free circuit takes the
    /// algorithm [`FlowCheckCost`] estimates to be cheaper; the sparse tracker undoes every
    /// top-level unitary instruction. A circuit with repeats keeps the sparse tracker when it folds
    /// every repeat body without unrolling it, and otherwise raises the body's Tableau to the
    /// repeat count by squaring.
    fn cheaper_algorithm(circuit: &Circuit, flows: &[Flow]) -> FlowCheckAlgorithm {
        let mut repeats = circuit
            .items()
            .iter()
            .filter_map(|item| match item {
                CircuitItem::RepeatBlock(repeat) => Some(repeat),
                CircuitItem::Instruction(_) => None,
            })
            .peekable();
        if repeats.peek().is_none() {
            return FlowCheckCost::measure(circuit, flows).cheaper_algorithm();
        }
        if repeats.all(|repeat| folds_unitary_repeat_body(repeat.body())) {
            FlowCheckAlgorithm::SparseTracker
        } else {
            FlowCheckAlgorithm::DenseTableau
        }
    }

    fn build(
        algorithm: FlowCheckAlgorithm,
        circuit: &Circuit,
        reversal: &TimeReversedForFlowsOptions,
    ) -> AnalysisResult<Self> {
        if algorithm == FlowCheckAlgorithm::SparseTracker {
            return Ok(Self::SparseFolded);
        }
        let options = crate::CircuitToTableauOptions {
            memory_budget: reversal.memory_budget,
            cancellation: reversal.cancellation.clone(),
            ..crate::CircuitToTableauOptions::default()
        };
        Ok(Self::Tableau(crate::circuit_to_tableau_with_options(
            circuit, &options,
        )?))
    }

    fn check_all(
        &self,
        circuit: &Circuit,
        flows: &[Flow],
        reversal: &TimeReversedForFlowsOptions,
    ) -> AnalysisResult<Vec<bool>> {
        match self {
            Self::Tableau(tableau) => flows
                .iter()
                .map(|flow| {
                    reversal.cancellation.check()?;
                    unitary_flow_is_satisfied_by_tableau(tableau, flow)
                })
                .collect(),
            Self::SparseFolded => check_unsigned_flows_with_sparse_tracker(
                circuit,
                flows,
                &reversal.flow_check_options(),
            )
            .map_err(|error| {
                if stops_flow_check(&error) {
                    error
                } else {
                    AnalysisError::invalid_tableau_conversion(error.to_string())
                }
            }),
        }
    }
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
