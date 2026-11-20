//! Direct generators for a circuit that is one measurement, reset, or pad instruction.

use stab_algebra::{Flow, PauliBasis, PauliSign};
use stab_model::{CircuitInstruction, MemoryBudget};

use crate::{AnalysisError, AnalysisResult};

use super::canonicalize::final_canonicalize_measurement_generators;
use super::helpers::{
    final_measure_reset_occurrences, has_duplicate_measure_reset_targets, input_measurement_flow,
    instruction_qubit_count, internal_flow_error, measure_reset_targets, negative_record_flow,
    pair_measurement_target_index, positive_record_flow, record_index_i32,
    stabilizer_to_circuit_error, unique_measure_reset_qubits, unique_plain_target_indices,
};
use super::resources::admit_flow_generator_rows;
use super::rows::{
    add_pauli_product_measurement_flow, identity_flow_rows, measured_pauli_product, pair_pauli,
    remove_pair_anticommutations, remove_single_anticommutations, single_pauli_with_sign,
    undo_single_qubit_reset,
};

pub(super) fn simple_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    budget: MemoryBudget,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    admit_flow_generator_rows(qubit_count, instruction.targets().len(), budget)?;
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
                stab_algebra::advanced::pauli_identity_unchecked(qubit_count),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_targets.len())?;
    Ok(Some(flows))
}

pub(super) fn simple_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    budget: MemoryBudget,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    admit_flow_generator_rows(qubit_count, 0, budget)?;
    let qubits = match unique_plain_target_indices(instruction) {
        Some(qubits) => qubits,
        None => return Ok(None),
    };
    let mut flows = identity_flow_rows(qubit_count);
    for qubit in qubits {
        undo_single_qubit_reset(&mut flows, qubit, basis)?;
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, 0)?;
    Ok(Some(flows))
}

pub(super) fn simple_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    budget: MemoryBudget,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let targets = measure_reset_targets(instruction)?;
    if has_duplicate_measure_reset_targets(&targets) {
        return duplicate_measure_reset_flows(instruction, basis, &targets, budget).map(Some);
    }
    admit_flow_generator_rows(qubit_count, targets.len(), budget)?;
    let mut flows = identity_flow_rows(qubit_count);
    for (record_index, (qubit, inverted)) in targets.into_iter().enumerate() {
        undo_single_qubit_reset(&mut flows, qubit, basis)?;
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

pub(super) fn duplicate_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    targets: &[(usize, bool)],
    budget: MemoryBudget,
) -> AnalysisResult<Vec<Flow>> {
    let qubit_count = instruction_qubit_count(instruction);
    admit_flow_generator_rows(qubit_count, targets.len(), budget)?;
    let mut flows = identity_flow_rows(qubit_count);
    for qubit in unique_measure_reset_qubits(targets) {
        undo_single_qubit_reset(&mut flows, qubit, basis)?;
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
                    stab_algebra::advanced::pauli_identity_unchecked(qubit_count),
                    stab_algebra::advanced::pauli_from_bases_unchecked(
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

pub(super) fn simple_pair_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
    budget: MemoryBudget,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let groups = instruction.target_groups();
    admit_flow_generator_rows(qubit_count, groups.len(), budget)?;
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
                stab_algebra::advanced::pauli_identity_unchecked(qubit_count),
                [record_index],
                [],
            )
            .map_err(stabilizer_to_circuit_error)?,
        );
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_pairs.len())?;
    Ok(Some(flows))
}

pub(super) fn simple_pauli_product_measurement_flows(
    instruction: &CircuitInstruction,
    budget: MemoryBudget,
) -> AnalysisResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let groups = instruction.target_groups();
    admit_flow_generator_rows(qubit_count, groups.len(), budget)?;
    let mut measured_products = Vec::with_capacity(groups.len());
    for (record_index, group) in groups.into_iter().enumerate() {
        let product =
            measured_pauli_product(instruction.gate().canonical_name(), qubit_count, group)?;
        measured_products.push((product, record_index_i32(record_index)?));
    }

    let mut flows = identity_flow_rows(qubit_count);
    for (product, record_index) in measured_products.iter().rev() {
        add_pauli_product_measurement_flow(&mut flows, product, *record_index, qubit_count)?;
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_products.len())?;
    Ok(Some(flows))
}

pub(super) fn measurement_pad_flows(
    instruction: &CircuitInstruction,
    budget: MemoryBudget,
) -> AnalysisResult<Vec<Flow>> {
    admit_flow_generator_rows(0, instruction.targets().len(), budget)?;
    let mut positive_records = Vec::new();
    let mut negative_records = Vec::new();
    for (record_index, target) in instruction.targets().iter().rev().enumerate() {
        match target.qubit_id().map(|id| id.get()) {
            Some(0) => positive_records.push(record_index),
            Some(1) => negative_records.push(record_index),
            _ => {
                return Err(AnalysisError::invalid_tableau_conversion(format!(
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
