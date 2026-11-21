//! Generators of a measurement-rich circuit, undone instruction by instruction from identity
//! flows.

use stab_algebra::{Flow, PauliBasis, PauliSign};
use stab_model::advanced::CancellationCountdown;
use stab_model::{Cancellation, Circuit, CircuitInstruction, CircuitItem, MemoryBudget, Target};

use crate::AnalysisOperation;
use crate::{AnalysisError, AnalysisResult, gate_tableau};

use super::canonicalize::final_canonicalize_measurement_generators;
use super::classification::{circuit_has_instructions, circuit_requires_reverse_flow_solver};
use super::helpers::{
    apply_local_tableau_to_global_pauli, final_measure_reset_occurrences, internal_flow_error,
    measure_reset_targets, measurement_indices_reversed, pair_measurement_target_index,
    plain_tableau_targets, rows_matching, stabilizer_to_circuit_error, unique_measure_reset_qubits,
    unique_plain_target_indices,
};
use super::resources::{admit_flow_generator_rows, validate_flow_generator_repeat_nesting};
use super::rows::{
    add_pauli_product_measurement_flow, anticommutes_with_single_measurement, identity_flow_rows,
    measured_pauli_product, pair_pauli, remove_pair_anticommutations,
    remove_single_anticommutations, single_pauli, single_pauli_with_sign, undo_single_qubit_reset,
};
use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};

pub(super) fn composed_measurement_flow_generators(
    circuit: &Circuit,
    budget: MemoryBudget,
    cancellation: &Cancellation,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = circuit.simulated_qubit_count();
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FlowGeneration,
            "circuit measurement count does not fit usize during flow generation",
        )
    })?;
    admit_flow_generator_rows(qubit_count, measurement_count, budget)?;
    validate_flow_generator_repeat_nesting(circuit)?;
    if !circuit_requires_reverse_flow_solver(circuit) {
        return Ok(None);
    }

    let mut solver =
        MeasurementFeedbackFlowSolver::new(qubit_count, measurement_count, cancellation);
    if !solver.undo_circuit(circuit)? {
        return Ok(None);
    }
    solver.finalize().map(Some)
}

struct MeasurementFeedbackFlowSolver<'a> {
    flows: Vec<Flow>,
    qubit_count: usize,
    measurement_count: usize,
    measurements_in_past: usize,
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl<'a> MeasurementFeedbackFlowSolver<'a> {
    fn new(qubit_count: usize, measurement_count: usize, cancellation: &'a Cancellation) -> Self {
        Self {
            flows: identity_flow_rows(qubit_count),
            qubit_count,
            measurement_count,
            measurements_in_past: measurement_count,
            cancellation,
            countdown: CancellationCountdown::new(),
        }
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<bool> {
        self.countdown.tick(self.cancellation)?;
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

    fn undo_circuit(&mut self, circuit: &Circuit) -> AnalysisResult<bool> {
        for item in circuit.items().iter().rev() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    if !self.undo_instruction(instruction)? {
                        return Ok(false);
                    }
                }
                CircuitItem::RepeatBlock(repeat) => {
                    if !circuit_has_instructions(repeat.body()) {
                        continue;
                    }
                    for _ in 0..repeat.repeat_count().get() {
                        if !self.undo_circuit(repeat.body())? {
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn undo_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> AnalysisResult<bool> {
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
                    stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
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
    ) -> AnalysisResult<bool> {
        let qubits = match unique_plain_target_indices(instruction) {
            Some(qubits) => qubits,
            None => return Ok(false),
        };
        for qubit in qubits {
            undo_single_qubit_reset(&mut self.flows, qubit, basis)?;
        }
        Ok(true)
    }

    fn undo_measure_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> AnalysisResult<bool> {
        let targets = measure_reset_targets(instruction)?;
        let mut record_indices =
            measurement_indices_reversed(&mut self.measurements_in_past, targets.len())?;
        record_indices.reverse();
        for qubit in unique_measure_reset_qubits(&targets) {
            undo_single_qubit_reset(&mut self.flows, qubit, basis)?;
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
                        stab_algebra::advanced::pauli_from_bases_unbudgeted(
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
                        stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
                        stab_algebra::advanced::pauli_from_bases_unbudgeted(
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
    ) -> AnalysisResult<bool> {
        let pairs = crate::instruction_shape::measured_qubit_pairs(
            instruction,
            AnalysisOperation::FlowGeneration,
        )?
        .map(|[left, right]| {
            (
                (left.id.get() as usize, left.inverted),
                (right.id.get() as usize, right.inverted),
            )
        })
        .collect::<Vec<_>>();
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
                    stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
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
    ) -> AnalysisResult<bool> {
        let groups = crate::instruction_shape::pauli_products(
            instruction,
            AnalysisOperation::FlowGeneration,
        )?;
        let mut products = Vec::with_capacity(groups.len());
        for group in groups {
            let product = measured_pauli_product(instruction.gate(), self.qubit_count, group)?;
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

    fn undo_measurement_pad(&mut self, instruction: &CircuitInstruction) -> AnalysisResult<bool> {
        let pads =
            crate::instruction_shape::pad_values(instruction, AnalysisOperation::FlowGeneration)?;
        let mut record_indices =
            measurement_indices_reversed(&mut self.measurements_in_past, pads.len())?;
        record_indices.reverse();
        for (pad, record_index) in pads.rev().zip(record_indices) {
            match pad {
                false => self.flows.push(
                    Flow::new(
                        stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
                        stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
                        [record_index],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                ),
                true => self.flows.push(
                    Flow::new(
                        stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
                        stab_algebra::advanced::pauli_from_bases_unbudgeted(
                            PauliSign::Minus,
                            vec![PauliBasis::I; self.qubit_count],
                        ),
                        [record_index],
                        [],
                    )
                    .map_err(stabilizer_to_circuit_error)?,
                ),
            }
        }
        Ok(true)
    }

    fn undo_heralded_flow_records(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> AnalysisResult<bool> {
        // Heralded noise targets plain qubits, and each target appends one herald record.
        let heralds =
            crate::instruction_shape::qubits(instruction, AnalysisOperation::FlowGeneration)?.len();
        for record_index in measurement_indices_reversed(&mut self.measurements_in_past, heralds)? {
            self.flows.push(
                Flow::new(
                    stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
                    stab_algebra::advanced::pauli_identity_unbudgeted(self.qubit_count),
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
    ) -> AnalysisResult<bool> {
        let pairs = crate::instruction_shape::controlled_pairs(
            instruction,
            AnalysisOperation::FlowGeneration,
        )?;
        for group in pairs.as_target_pairs().iter().rev() {
            let [left, right] = group;
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
    ) -> AnalysisResult<bool> {
        for group in instruction.target_groups().rev() {
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
    ) -> AnalysisResult<bool> {
        let Some(targets) = plain_tableau_targets(group) else {
            return Ok(false);
        };
        let local_inverse = gate_tableau(instruction.gate())?
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
    ) -> AnalysisResult<bool> {
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

    fn finalize(mut self) -> AnalysisResult<Vec<Flow>> {
        final_canonicalize_measurement_generators(
            &mut self.flows,
            self.qubit_count,
            self.measurement_count,
        )?;
        Ok(self.flows)
    }
}

fn absolute_record_index(measurements_in_past: usize, record_offset: i32) -> AnalysisResult<i32> {
    let measurements_in_past = i64::try_from(measurements_in_past).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FlowGeneration,
            "measurement count does not fit i64 during feedback flow generation",
        )
    })?;
    let index = measurements_in_past
        .checked_add(i64::from(record_offset))
        .ok_or_else(|| {
            AnalysisError::count_overflow(
                AnalysisOperation::FlowGeneration,
                "measurement record index overflowed during feedback flow generation",
            )
        })?;
    if index < 0 || index > measurements_in_past {
        return Err(AnalysisError::record_out_of_range(
            AnalysisOperation::FlowGeneration,
            format!(
                "measurement record offset {record_offset} is outside the flow generation history"
            ),
        ));
    }
    i32::try_from(index).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::FlowGeneration,
            format!("flow measurement record index {index} does not fit i32"),
        )
    })
}

fn flow_with_toggled_measurement(flow: &Flow, record_index: i32) -> AnalysisResult<Flow> {
    Flow::new(
        flow.input().clone(),
        flow.output().clone(),
        flow.measurements().chain([record_index]),
        flow.observables(),
    )
    .map_err(stabilizer_to_circuit_error)
}
