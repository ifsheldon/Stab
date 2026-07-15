use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Flow, PauliBasis,
    PauliSign, PauliString, Target,
};

mod canonicalize;
mod helpers;

use canonicalize::final_canonicalize_measurement_generators;
use helpers::{
    apply_local_tableau_to_global_pauli, final_measure_reset_occurrences,
    has_duplicate_measure_reset_targets, input_measurement_flow, instruction_qubit_count,
    internal_flow_error, measure_reset_targets, measurement_indices_reversed, negative_record_flow,
    pair_measurement_target_index, pauli_basis, plain_tableau_targets, positive_record_flow,
    record_index_i32, rows_matching, stabilizer_to_circuit_error, unique_measure_reset_qubits,
    unique_plain_target_indices, validate_ignored_only_flow_generator_work,
};

use super::transitions::{ReverseFlowTransition, reverse_flow_transition};

const MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS: usize = 4096;

/// Returns unsigned stabilizer-flow generators for the supported tableau and PFM5 measurement subset.
///
/// Repeat-contained measurement-rich circuits use bounded flattened operations plus a flow-row cap.
/// Annotation-only identity output is guarded by an aggregate Pauli-bit budget; broader semantics fail closed.
pub fn circuit_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    if circuit_is_ignored_only(circuit) {
        let qubit_count = circuit.count_simulated_qubits();
        validate_ignored_only_flow_generator_work(qubit_count)?;
        return Ok(reverse_ordered_identity_flow_rows(qubit_count));
    }
    if circuit_requires_reverse_flow_solver(circuit) {
        return simple_measurement_rich_flow_generators(circuit)?
            .ok_or_else(|| unsupported_flow_generator_error(circuit));
    }
    unitary_flow_generators(circuit)
}

fn unitary_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    let tableau = circuit.to_tableau(true, false, false)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in (0..tableau.len()).rev() {
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
        ));
    }
    Ok(flows)
}

fn circuit_is_ignored_only(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            matches!(
                reverse_flow_transition(instruction),
                ReverseFlowTransition::Ignored
            )
        }
        CircuitItem::RepeatBlock(repeat) => circuit_is_ignored_only(repeat.body()),
    })
}

fn circuit_requires_reverse_flow_solver(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => {
            instruction_requires_reverse_flow_solver(instruction)
        }
        CircuitItem::RepeatBlock(repeat) => circuit_requires_reverse_flow_solver(repeat.body()),
    })
}

fn instruction_requires_reverse_flow_solver(instruction: &CircuitInstruction) -> bool {
    let transition = reverse_flow_transition(instruction);
    transition.is_measurement_rich()
        || (matches!(transition, ReverseFlowTransition::ControlledPauli(_))
            && instruction
                .targets()
                .iter()
                .any(Target::is_classical_bit_target))
}

fn simple_measurement_rich_flow_generators(circuit: &Circuit) -> CircuitResult<Option<Vec<Flow>>> {
    if let [CircuitItem::Instruction(instruction)] = circuit.items() {
        let transition = reverse_flow_transition(instruction);
        let simple = match transition {
            ReverseFlowTransition::Measurement(basis) => {
                simple_measurement_flows(instruction, basis)?
            }
            ReverseFlowTransition::Reset(basis) => simple_reset_flows(instruction, basis)?,
            ReverseFlowTransition::MeasureReset(basis) => {
                simple_measure_reset_flows(instruction, basis)?
            }
            ReverseFlowTransition::PairMeasurement(basis) => {
                simple_pair_measurement_flows(instruction, basis)?
            }
            ReverseFlowTransition::PauliProductMeasurement => {
                simple_pauli_product_measurement_flows(instruction)?
            }
            ReverseFlowTransition::MeasurementPad => Some(measurement_pad_flows(instruction)?),
            ReverseFlowTransition::SweepControlledPauliNoop => {
                Some(identity_flow_rows(instruction_qubit_count(instruction)))
            }
            _ => None,
        };
        if simple.is_some() {
            return Ok(simple);
        }
    }
    scoped_composed_measurement_flow_generators(circuit)
}

fn simple_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    validate_measurement_rich_flow_generator_rows(qubit_count, instruction.targets().len())?;
    let mut measured_targets = Vec::with_capacity(instruction.targets().len());
    for (record_index, target) in instruction.targets().iter().enumerate() {
        measured_targets.push((
            pair_measurement_target_index(target)?,
            record_index_i32(record_index)?,
        ));
    }

    let mut flows = identity_flow_rows(qubit_count);
    for &((qubit, inverted), record_index) in measured_targets.iter().rev() {
        remove_single_anticommutations(&mut flows, qubit, basis)?;
        flows.push(
            Flow::new(
                single_pauli_with_sign(
                    qubit_count,
                    qubit,
                    basis,
                    if inverted {
                        PauliSign::Minus
                    } else {
                        PauliSign::Plus
                    },
                ),
                PauliString::identity_unchecked(qubit_count),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_targets.len())?;
    Ok(Some(flows))
}

fn simple_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    validate_measurement_rich_flow_generator_rows(qubit_count, 0)?;
    let qubits = match unique_plain_target_indices(instruction) {
        Some(qubits) => qubits,
        None => return Ok(None),
    };
    let mut flows = identity_flow_rows(qubit_count);
    for qubit in qubits {
        remove_single_anticommutations(&mut flows, qubit, basis)?;
        clear_input_term(&mut flows, qubit)?;
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, 0)?;
    Ok(Some(flows))
}

fn simple_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let targets = measure_reset_targets(instruction)?;
    if has_duplicate_measure_reset_targets(&targets) {
        return duplicate_measure_reset_flows(instruction, basis, &targets).map(Some);
    }
    validate_measurement_rich_flow_generator_rows(qubit_count, targets.len())?;
    let mut flows = identity_flow_rows(qubit_count);
    for (record_index, (qubit, inverted)) in targets.into_iter().enumerate() {
        remove_single_anticommutations(&mut flows, qubit, basis)?;
        clear_input_term(&mut flows, qubit)?;
        flows.push(input_measurement_flow(
            qubit_count,
            qubit,
            basis,
            record_index,
            if inverted {
                PauliSign::Minus
            } else {
                PauliSign::Plus
            },
        )?);
    }
    final_canonicalize_measurement_generators(
        &mut flows,
        qubit_count,
        instruction.targets().len(),
    )?;
    Ok(Some(flows))
}

fn duplicate_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    targets: &[(usize, bool)],
) -> CircuitResult<Vec<Flow>> {
    let qubit_count = instruction_qubit_count(instruction);
    validate_measurement_rich_flow_generator_rows(qubit_count, targets.len())?;
    let mut flows = identity_flow_rows(qubit_count);
    for qubit in unique_measure_reset_qubits(targets) {
        remove_single_anticommutations(&mut flows, qubit, basis)?;
        clear_input_term(&mut flows, qubit)?;
    }
    let final_occurrences = final_measure_reset_occurrences(targets);
    for (record_index, &(qubit, inverted)) in targets.iter().enumerate() {
        let &(final_index, final_inverted) = final_occurrences
            .get(&qubit)
            .ok_or_else(|| internal_flow_error("missing final measure-reset target"))?;
        if record_index == final_index {
            flows.push(input_measurement_flow(
                qubit_count,
                qubit,
                basis,
                record_index,
                if inverted {
                    PauliSign::Minus
                } else {
                    PauliSign::Plus
                },
            )?);
        } else {
            flows.push(
                Flow::new(
                    PauliString::identity_unchecked(qubit_count),
                    PauliString::from_bases_unchecked(
                        if inverted ^ final_inverted {
                            PauliSign::Minus
                        } else {
                            PauliSign::Plus
                        },
                        vec![PauliBasis::I; qubit_count],
                    ),
                    [
                        record_index_i32(record_index)?,
                        record_index_i32(final_index)?,
                    ],
                    [],
                )
                .map_err(stabilizer_to_circuit_error)?,
            );
        }
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, targets.len())?;
    Ok(flows)
}

fn simple_pair_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let groups = instruction.target_groups();
    validate_measurement_rich_flow_generator_rows(qubit_count, groups.len())?;
    let mut measured_pairs = Vec::with_capacity(groups.len());
    for (record_index, group) in groups.iter().enumerate() {
        let [left, right] = *group else {
            return Ok(None);
        };
        measured_pairs.push((
            pair_measurement_target_index(left)?,
            pair_measurement_target_index(right)?,
            record_index_i32(record_index)?,
        ));
    }

    let mut flows = identity_flow_rows(qubit_count);
    for &((left, left_inverted), (right, right_inverted), record_index) in
        measured_pairs.iter().rev()
    {
        remove_pair_anticommutations(&mut flows, left, right, basis)?;
        flows.push(
            Flow::new(
                pair_pauli(
                    qubit_count,
                    left,
                    right,
                    basis,
                    if left_inverted ^ right_inverted {
                        PauliSign::Minus
                    } else {
                        PauliSign::Plus
                    },
                ),
                PauliString::identity_unchecked(qubit_count),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_pairs.len())?;
    Ok(Some(flows))
}

fn simple_pauli_product_measurement_flows(
    instruction: &CircuitInstruction,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let mut measured_products = Vec::with_capacity(instruction.target_groups().len());
    for (record_index, group) in instruction.target_groups().into_iter().enumerate() {
        let product =
            measured_pauli_product(instruction.gate().canonical_name(), qubit_count, group)?;
        measured_products.push((product, record_index_i32(record_index)?));
    }
    validate_measurement_rich_flow_generator_rows(qubit_count, measured_products.len())?;

    let mut flows = identity_flow_rows(qubit_count);
    for (product, record_index) in measured_products.iter().rev() {
        add_pauli_product_measurement_flow(&mut flows, product, *record_index, qubit_count)?;
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_products.len())?;
    Ok(Some(flows))
}

fn measurement_pad_flows(instruction: &CircuitInstruction) -> CircuitResult<Vec<Flow>> {
    validate_measurement_rich_flow_generator_rows(0, instruction.targets().len())?;
    let mut positive_records = Vec::new();
    let mut negative_records = Vec::new();
    for (record_index, target) in instruction.targets().iter().rev().enumerate() {
        match target.qubit_id().map(|id| id.get()) {
            Some(0) => positive_records.push(record_index),
            Some(1) => negative_records.push(record_index),
            _ => {
                return Err(CircuitError::invalid_tableau_conversion(format!(
                    "MPAD flow generator has invalid pad target {target}"
                )));
            }
        }
    }
    let mut flows = Vec::with_capacity(positive_records.len() + negative_records.len());
    for record in positive_records {
        flows.push(positive_record_flow(record)?);
    }
    for record in negative_records {
        flows.push(negative_record_flow(record)?);
    }
    Ok(flows)
}

fn scoped_composed_measurement_flow_generators(
    circuit: &Circuit,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = circuit.count_simulated_qubits();
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "circuit measurement count does not fit usize during flow generation",
        )
    })?;
    validate_measurement_rich_flow_generator_rows(qubit_count, measurement_count)?;
    let instructions = flattened_measurement_generator_instructions(circuit)?;
    if !instructions
        .iter()
        .any(instruction_requires_reverse_flow_solver)
    {
        return Ok(None);
    }

    let mut solver = MeasurementFeedbackFlowSolver::new(qubit_count, measurement_count);
    for instruction in instructions.iter().rev() {
        if !solver.undo_instruction(instruction)? {
            return Ok(None);
        }
    }
    solver.finalize().map(Some)
}

fn validate_measurement_rich_flow_generator_rows(
    qubit_count: usize,
    measurement_count: usize,
) -> CircuitResult<()> {
    let rows = qubit_count
        .checked_mul(2)
        .and_then(|rows| rows.checked_add(measurement_count))
        .ok_or_else(|| {
            CircuitError::invalid_domain_value("measurement-rich flow-generator rows", "overflowed")
        })?;
    if rows > MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS {
        return Err(CircuitError::invalid_domain_value(
            "measurement-rich flow-generator rows",
            format!("{rows} exceeds current limit {MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS}"),
        ));
    }
    Ok(())
}

fn flattened_measurement_generator_instructions(
    circuit: &Circuit,
) -> CircuitResult<Vec<CircuitInstruction>> {
    if circuit
        .items()
        .iter()
        .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        circuit.flattened_operations()
    } else {
        Ok(circuit
            .items()
            .iter()
            .filter_map(|item| match item {
                CircuitItem::Instruction(instruction) => Some(instruction.clone()),
                CircuitItem::RepeatBlock(_) => None,
            })
            .collect())
    }
}

struct MeasurementFeedbackFlowSolver {
    flows: Vec<Flow>,
    qubit_count: usize,
    measurement_count: usize,
    measurements_in_past: usize,
}

impl MeasurementFeedbackFlowSolver {
    fn new(qubit_count: usize, measurement_count: usize) -> Self {
        Self {
            flows: identity_flow_rows(qubit_count),
            qubit_count,
            measurement_count,
            measurements_in_past: measurement_count,
        }
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<bool> {
        match reverse_flow_transition(instruction) {
            ReverseFlowTransition::Measurement(basis) => self.undo_measurement(instruction, basis),
            ReverseFlowTransition::Reset(basis) => self.undo_reset(instruction, basis),
            ReverseFlowTransition::MeasureReset(basis) => {
                self.undo_measure_reset(instruction, basis)
            }
            ReverseFlowTransition::PairMeasurement(basis) => {
                self.undo_pair_measurement(instruction, basis)
            }
            ReverseFlowTransition::PauliProductMeasurement => {
                self.undo_pauli_product_measurement(instruction)
            }
            ReverseFlowTransition::MeasurementPad => self.undo_measurement_pad(instruction),
            ReverseFlowTransition::HeraldedMeasurement => {
                self.undo_heralded_flow_records(instruction)
            }
            ReverseFlowTransition::PauliProductUnitary => {
                self.undo_decomposed_instruction(instruction)
            }
            ReverseFlowTransition::SweepControlledPauliNoop
            | ReverseFlowTransition::Detector
            | ReverseFlowTransition::Observable
            | ReverseFlowTransition::Ignored => Ok(true),
            ReverseFlowTransition::ControlledPauli(basis) => {
                self.undo_feedback_capable_instruction(instruction, basis)
            }
            ReverseFlowTransition::Tableau => self.undo_tableau_instruction(instruction),
            ReverseFlowTransition::Unsupported => Ok(false),
        }
    }

    fn undo_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let mut targets = Vec::with_capacity(instruction.targets().len());
        for target in instruction.targets() {
            targets.push(pair_measurement_target_index(target)?);
        }
        for (&(qubit, inverted), record_index) in targets.iter().rev().zip(
            measurement_indices_reversed(&mut self.measurements_in_past, targets.len())?,
        ) {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            self.flows.push(
                Flow::new(
                    single_pauli_with_sign(
                        self.qubit_count,
                        qubit,
                        basis,
                        if inverted {
                            PauliSign::Minus
                        } else {
                            PauliSign::Plus
                        },
                    ),
                    PauliString::identity_unchecked(self.qubit_count),
                    [record_index],
                    [],
                )
                .map_err(stabilizer_to_circuit_error)?,
            );
        }
        Ok(true)
    }

    fn undo_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let qubits = match unique_plain_target_indices(instruction) {
            Some(qubits) => qubits,
            None => return Ok(false),
        };
        for qubit in qubits {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            clear_input_term(&mut self.flows, qubit)?;
        }
        Ok(true)
    }

    fn undo_measure_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let targets = measure_reset_targets(instruction)?;
        let mut record_indices =
            measurement_indices_reversed(&mut self.measurements_in_past, targets.len())?;
        record_indices.reverse();
        for qubit in unique_measure_reset_qubits(&targets) {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            clear_input_term(&mut self.flows, qubit)?;
        }
        let final_occurrences = final_measure_reset_occurrences(&targets);
        for (local_index, &(qubit, inverted)) in targets.iter().enumerate() {
            let &(final_index, final_inverted) = final_occurrences
                .get(&qubit)
                .ok_or_else(|| internal_flow_error("missing final measure-reset target"))?;
            let record_index = record_indices
                .get(local_index)
                .copied()
                .ok_or_else(|| internal_flow_error("missing measure-reset record index"))?;
            let final_record = record_indices
                .get(final_index)
                .copied()
                .ok_or_else(|| internal_flow_error("missing final measure-reset record index"))?;
            if local_index == final_index {
                self.flows.push(
                    Flow::new(
                        single_pauli(self.qubit_count, qubit, basis),
                        PauliString::from_bases_unchecked(
                            if inverted {
                                PauliSign::Minus
                            } else {
                                PauliSign::Plus
                            },
                            vec![PauliBasis::I; self.qubit_count],
                        ),
                        [record_index],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                );
            } else {
                self.flows.push(
                    Flow::new(
                        PauliString::identity_unchecked(self.qubit_count),
                        PauliString::from_bases_unchecked(
                            if inverted ^ final_inverted {
                                PauliSign::Minus
                            } else {
                                PauliSign::Plus
                            },
                            vec![PauliBasis::I; self.qubit_count],
                        ),
                        [record_index, final_record],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                );
            }
        }
        Ok(true)
    }

    fn undo_pair_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let groups = instruction.target_groups();
        let mut pairs = Vec::with_capacity(groups.len());
        for group in &groups {
            let [left, right] = *group else {
                return Ok(false);
            };
            pairs.push((
                pair_measurement_target_index(left)?,
                pair_measurement_target_index(right)?,
            ));
        }
        for (&((left, left_inverted), (right, right_inverted)), record_index) in
            pairs.iter().rev().zip(measurement_indices_reversed(
                &mut self.measurements_in_past,
                pairs.len(),
            )?)
        {
            remove_pair_anticommutations(&mut self.flows, left, right, basis)?;
            self.flows.push(
                Flow::new(
                    pair_pauli(
                        self.qubit_count,
                        left,
                        right,
                        basis,
                        if left_inverted ^ right_inverted {
                            PauliSign::Minus
                        } else {
                            PauliSign::Plus
                        },
                    ),
                    PauliString::identity_unchecked(self.qubit_count),
                    [record_index],
                    [],
                )
                .map_err(stabilizer_to_circuit_error)?,
            );
        }
        Ok(true)
    }

    fn undo_pauli_product_measurement(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        let mut products = Vec::with_capacity(instruction.target_groups().len());
        for group in instruction.target_groups() {
            let product = measured_pauli_product(
                instruction.gate().canonical_name(),
                self.qubit_count,
                group,
            )?;
            products.push(product);
        }
        for (product, record_index) in products.iter().rev().zip(measurement_indices_reversed(
            &mut self.measurements_in_past,
            products.len(),
        )?) {
            add_pauli_product_measurement_flow(
                &mut self.flows,
                product,
                record_index,
                self.qubit_count,
            )?;
        }
        Ok(true)
    }

    fn undo_measurement_pad(&mut self, instruction: &CircuitInstruction) -> CircuitResult<bool> {
        let mut record_indices = measurement_indices_reversed(
            &mut self.measurements_in_past,
            instruction.targets().len(),
        )?;
        record_indices.reverse();
        for (target, record_index) in instruction.targets().iter().rev().zip(record_indices) {
            match target.qubit_id().map(|id| id.get()) {
                Some(0) => self.flows.push(
                    Flow::new(
                        PauliString::identity_unchecked(self.qubit_count),
                        PauliString::identity_unchecked(self.qubit_count),
                        [record_index],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                ),
                Some(1) => self.flows.push(
                    Flow::new(
                        PauliString::identity_unchecked(self.qubit_count),
                        PauliString::from_bases_unchecked(
                            PauliSign::Minus,
                            vec![PauliBasis::I; self.qubit_count],
                        ),
                        [record_index],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                ),
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn undo_heralded_flow_records(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        for (target, record_index) in
            instruction
                .targets()
                .iter()
                .rev()
                .zip(measurement_indices_reversed(
                    &mut self.measurements_in_past,
                    instruction.targets().len(),
                )?)
        {
            if target.qubit_id().is_none() || target.is_inverted_result_target() {
                return Ok(false);
            }
            self.flows.push(
                Flow::new(
                    PauliString::identity_unchecked(self.qubit_count),
                    PauliString::identity_unchecked(self.qubit_count),
                    [record_index],
                    [],
                )
                .map_err(stabilizer_to_circuit_error)?,
            );
        }
        Ok(true)
    }

    fn undo_feedback_capable_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        for group in instruction.target_groups().into_iter().rev() {
            let [left, right] = group else {
                return Ok(false);
            };
            let feedback = match (
                left.measurement_record_offset(),
                right.measurement_record_offset(),
            ) {
                (Some(record), None) => Some((record.get(), right)),
                (None, Some(record)) => Some((record.get(), left)),
                _ => None,
            };
            let Some((record_offset, target)) = feedback else {
                if left.qubit_id().is_some()
                    && right.qubit_id().is_some()
                    && !self.undo_tableau_target_group(instruction, group)?
                {
                    return Ok(false);
                }
                continue;
            };
            let Some(qubit) = target.qubit_id().map(|qubit| qubit.get() as usize) else {
                continue;
            };
            let record_index = absolute_record_index(self.measurements_in_past, record_offset)?;
            for row in rows_matching(&self.flows, |flow| {
                anticommutes_with_single_measurement(flow.input(), qubit, basis)
            }) {
                let updated = flow_with_toggled_measurement(
                    self.flows
                        .get(row)
                        .ok_or_else(|| internal_flow_error("feedback row is out of bounds"))?,
                    record_index,
                )?;
                if let Some(slot) = self.flows.get_mut(row) {
                    *slot = updated;
                }
            }
        }
        Ok(true)
    }

    fn undo_tableau_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        for group in instruction.target_groups().into_iter().rev() {
            if !self.undo_tableau_target_group(instruction, group)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn undo_tableau_target_group(
        &mut self,
        instruction: &CircuitInstruction,
        group: &[Target],
    ) -> CircuitResult<bool> {
        let Some(targets) = plain_tableau_targets(group) else {
            return Ok(false);
        };
        let local_inverse =
            crate::circuit_tableau::gate_tableau(instruction.gate().canonical_name())?
                .inverse()
                .map_err(stabilizer_to_circuit_error)?;
        if local_inverse.len() != targets.len() {
            return Ok(false);
        }
        for row in &mut self.flows {
            let input = apply_local_tableau_to_global_pauli(
                row.input(),
                &targets,
                &local_inverse,
                self.qubit_count,
            )?;
            *row = Flow::new(
                input,
                row.output().clone(),
                row.measurements(),
                row.observables(),
            )
            .map_err(stabilizer_to_circuit_error)?;
        }
        Ok(true)
    }

    fn undo_decomposed_instruction(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        let decomposed = crate::circuit_simplify::decomposed_single_instruction(instruction)?;
        for item in decomposed.items().iter().rev() {
            let CircuitItem::Instruction(instruction) = item else {
                return Ok(false);
            };
            if !self.undo_instruction(instruction)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn finalize(mut self) -> CircuitResult<Vec<Flow>> {
        final_canonicalize_measurement_generators(
            &mut self.flows,
            self.qubit_count,
            self.measurement_count,
        )?;
        Ok(self.flows)
    }
}

fn measured_pauli_product(
    gate_name: &'static str,
    qubit_count: usize,
    targets: &[Target],
) -> CircuitResult<PauliString> {
    let mut product = PauliString::identity_unchecked(qubit_count);
    for target in targets {
        if target.is_combiner() {
            continue;
        }
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} does not identify a qubit"
            ))
        })?;
        let pauli = target.pauli_type().ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} is not a Pauli target"
            ))
        })?;
        let term = single_pauli_with_sign(
            qubit_count,
            qubit.get() as usize,
            pauli_basis(pauli),
            if target.is_inverted_result_target() {
                PauliSign::Minus
            } else {
                PauliSign::Plus
            },
        );
        product = product.multiply_real(&term).map_err(|error| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator Pauli product is not Hermitian: {error}"
            ))
        })?;
    }
    Ok(product)
}

fn identity_flow_rows(qubit_count: usize) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(qubit_count.saturating_mul(2));
    for qubit in 0..qubit_count {
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::X),
            single_pauli(qubit_count, qubit, PauliBasis::X),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            single_pauli(qubit_count, qubit, PauliBasis::Z),
        ));
    }
    flows
}

fn reverse_ordered_identity_flow_rows(qubit_count: usize) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(qubit_count.saturating_mul(2));
    for qubit in (0..qubit_count).rev() {
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::X),
            single_pauli(qubit_count, qubit, PauliBasis::X),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            single_pauli(qubit_count, qubit, PauliBasis::Z),
        ));
    }
    flows
}

fn remove_single_anticommutations(
    flows: &mut Vec<Flow>,
    qubit: usize,
    basis: PauliBasis,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_single_measurement(flow.input(), qubit, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("single-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("single-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn remove_pair_anticommutations(
    flows: &mut Vec<Flow>,
    left: usize,
    right: usize,
    basis: PauliBasis,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pair_measurement(flow.input(), left, right, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("pair-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("pair-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn add_pauli_product_measurement_flow(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
    record_index: i32,
    qubit_count: usize,
) -> CircuitResult<()> {
    if measured_product.has_no_pauli_terms() {
        flows.push(
            Flow::new(
                PauliString::identity_unchecked(qubit_count),
                PauliString::from_bases_unchecked(
                    measured_product.sign(),
                    vec![PauliBasis::I; qubit_count],
                ),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
        return Ok(());
    }
    remove_pauli_product_anticommutations(flows, measured_product)?;
    flows.push(
        Flow::new(
            measured_product.clone(),
            PauliString::identity_unchecked(qubit_count),
            [record_index],
            [],
        )
        .map_err(stabilizer_to_circuit_error)?,
    );
    Ok(())
}

fn remove_pauli_product_anticommutations(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pauli_product(flow.input(), measured_product)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows.get(pivot).cloned().ok_or_else(|| {
        internal_flow_error("Pauli-product measurement pivot row is out of bounds")
    })?;
    for &row in rest {
        let target = flows.get(row).ok_or_else(|| {
            internal_flow_error("Pauli-product measurement target row is out of bounds")
        })?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn anticommutes_with_pauli_product(input: &PauliString, measured_product: &PauliString) -> bool {
    (0..input.len().max(measured_product.len())).fold(false, |anticommutes, qubit| {
        let input_basis = input.get(qubit).unwrap_or(PauliBasis::I);
        let measured_basis = measured_product.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes
            ^ (input_basis.x_bit() & measured_basis.z_bit())
            ^ (input_basis.z_bit() & measured_basis.x_bit())
    })
}

fn clear_input_term(flows: &mut [Flow], qubit: usize) -> CircuitResult<()> {
    for flow in flows {
        let mut input = flow.input().clone();
        if qubit < input.len() {
            input
                .set(qubit, PauliBasis::I)
                .map_err(stabilizer_to_circuit_error)?;
        }
        *flow = Flow::new(
            input,
            flow.output().clone(),
            flow.measurements(),
            flow.observables(),
        )
        .map_err(stabilizer_to_circuit_error)?;
    }
    Ok(())
}

fn anticommutes_with_pair_measurement(
    input: &PauliString,
    left: usize,
    right: usize,
    measured_basis: PauliBasis,
) -> bool {
    let x = measured_basis.x_bit();
    let z = measured_basis.z_bit();
    let mut anticommutes = false;
    for qubit in [left, right] {
        let basis = input.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes ^= basis.x_bit() & z;
        anticommutes ^= basis.z_bit() & x;
    }
    anticommutes
}

fn anticommutes_with_single_measurement(
    input: &PauliString,
    qubit: usize,
    measured_basis: PauliBasis,
) -> bool {
    let basis = input.get(qubit).unwrap_or(PauliBasis::I);
    (basis.x_bit() & measured_basis.z_bit()) ^ (basis.z_bit() & measured_basis.x_bit())
}

fn pair_pauli(
    qubit_count: usize,
    left: usize,
    right: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    PauliString::from_bases_unchecked(
        sign,
        (0..qubit_count).map(|qubit| {
            if qubit == left || qubit == right {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn single_pauli_with_sign(
    len: usize,
    index: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    PauliString::from_bases_unchecked(
        sign,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn absolute_record_index(measurements_in_past: usize, record_offset: i32) -> CircuitResult<i32> {
    let measurements_in_past = i64::try_from(measurements_in_past).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "measurement count does not fit i64 during feedback flow generation",
        )
    })?;
    let index = measurements_in_past
        .checked_add(i64::from(record_offset))
        .ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "measurement record index overflowed during feedback flow generation",
            )
        })?;
    if index < 0 || index >= measurements_in_past {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "measurement record offset {record_offset} is outside the flow generation history"
        )));
    }
    i32::try_from(index).map_err(|_| {
        CircuitError::invalid_tableau_conversion(format!(
            "flow measurement record index {index} does not fit i32"
        ))
    })
}

fn flow_with_toggled_measurement(flow: &Flow, record_index: i32) -> CircuitResult<Flow> {
    Flow::new(
        flow.input().clone(),
        flow.output().clone(),
        flow.measurements().chain([record_index]),
        flow.observables(),
    )
    .map_err(stabilizer_to_circuit_error)
}

fn unsupported_flow_generator_error(circuit: &Circuit) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "circuit_flow_generators only supports unitary tableau circuits, supported measurement/reset/pair-measurement/MPP/MPAD circuits, scoped composed measurement-rich circuits, scoped measurement-record feedback circuits, and scoped heralded-noise record circuits; got {} top-level item(s)",
        circuit.items().len()
    ))
}

pub(super) fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases_unchecked(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}
