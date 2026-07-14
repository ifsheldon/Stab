use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CircuitError, CircuitInstruction, CircuitResult, Flow, Pauli, PauliBasis, PauliSign,
    PauliString, Target,
};

use super::single_pauli;

// Two rows per qubit, two Pauli strings per row, and two bit planes per string.
const MAX_IGNORED_ONLY_FLOW_GENERATOR_PAULI_BITS: usize = 8 * 4096 * 4096;

pub(super) fn validate_ignored_only_flow_generator_work(qubit_count: usize) -> CircuitResult<()> {
    let pauli_bits = qubit_count
        .checked_mul(qubit_count)
        .and_then(|bits| bits.checked_mul(8))
        .ok_or_else(|| {
            CircuitError::invalid_domain_value(
                "ignored-only flow-generator Pauli bits",
                "overflowed",
            )
        })?;
    if pauli_bits > MAX_IGNORED_ONLY_FLOW_GENERATOR_PAULI_BITS {
        return Err(CircuitError::invalid_domain_value(
            "ignored-only flow-generator Pauli bits",
            format!(
                "{pauli_bits} exceeds current limit {MAX_IGNORED_ONLY_FLOW_GENERATOR_PAULI_BITS}"
            ),
        ));
    }
    Ok(())
}

pub(super) fn plain_tableau_targets(targets: &[Target]) -> Option<Vec<usize>> {
    let mut qubits = Vec::with_capacity(targets.len());
    for target in targets {
        let qubit = plain_target_index(target)?;
        if qubits.contains(&qubit) {
            return None;
        }
        qubits.push(qubit);
    }
    Some(qubits)
}

pub(super) fn apply_local_tableau_to_global_pauli(
    input: &PauliString,
    targets: &[usize],
    local_tableau: &crate::Tableau,
    qubit_count: usize,
) -> CircuitResult<PauliString> {
    let local = PauliString::from_bases_unchecked(
        input.sign(),
        targets
            .iter()
            .map(|&qubit| input.get(qubit).unwrap_or(PauliBasis::I)),
    );
    let transformed = local_tableau
        .apply(&local)
        .map_err(stabilizer_to_circuit_error)?;
    let mut bases = (0..qubit_count)
        .map(|qubit| input.get(qubit).unwrap_or(PauliBasis::I))
        .collect::<Vec<_>>();
    for (local_index, &qubit) in targets.iter().enumerate() {
        let basis = transformed.get(local_index).ok_or_else(|| {
            internal_flow_error("local tableau output length did not match targets")
        })?;
        let Some(slot) = bases.get_mut(qubit) else {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "flow tableau target qubit {qubit} is outside {qubit_count}-qubit state"
            )));
        };
        *slot = basis;
    }
    Ok(PauliString::from_bases_unchecked(transformed.sign(), bases))
}

pub(super) fn instruction_qubit_count(instruction: &CircuitInstruction) -> usize {
    instruction
        .targets()
        .iter()
        .filter_map(Target::qubit_id)
        .map(|qubit| qubit.get() as usize + 1)
        .max()
        .unwrap_or(0)
}

pub(super) fn plain_target_index(target: &Target) -> Option<usize> {
    if target.is_inverted_result_target() {
        return None;
    }
    target.qubit_id().map(|qubit| qubit.get() as usize)
}

pub(super) fn unique_plain_target_indices(instruction: &CircuitInstruction) -> Option<Vec<usize>> {
    let mut qubits = Vec::with_capacity(instruction.targets().len());
    for target in instruction.targets() {
        let qubit = plain_target_index(target)?;
        if !qubits.contains(&qubit) {
            qubits.push(qubit);
        }
    }
    Some(qubits)
}

pub(super) fn measure_reset_targets(
    instruction: &CircuitInstruction,
) -> CircuitResult<Vec<(usize, bool)>> {
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for target in instruction.targets() {
        targets.push(pair_measurement_target_index(target)?);
    }
    Ok(targets)
}

pub(super) fn has_duplicate_measure_reset_targets(targets: &[(usize, bool)]) -> bool {
    let mut seen = BTreeSet::new();
    targets.iter().any(|&(qubit, _)| !seen.insert(qubit))
}

pub(super) fn unique_measure_reset_qubits(targets: &[(usize, bool)]) -> Vec<usize> {
    let mut seen = BTreeSet::new();
    targets
        .iter()
        .filter_map(|&(qubit, _)| seen.insert(qubit).then_some(qubit))
        .collect()
}

pub(super) fn final_measure_reset_occurrences(
    targets: &[(usize, bool)],
) -> BTreeMap<usize, (usize, bool)> {
    targets
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (qubit, inverted))| (qubit, (index, inverted)))
        .collect()
}

pub(super) fn measurement_indices_reversed(
    measurements_in_past: &mut usize,
    count: usize,
) -> CircuitResult<Vec<i32>> {
    let mut indices = Vec::with_capacity(count);
    for _ in 0..count {
        *measurements_in_past = measurements_in_past.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "measurement count underflowed during flow generation",
            )
        })?;
        indices.push(record_index_i32(*measurements_in_past)?);
    }
    Ok(indices)
}

pub(super) fn pair_measurement_target_index(target: &Target) -> CircuitResult<(usize, bool)> {
    let qubit = target.qubit_id().ok_or_else(|| {
        CircuitError::invalid_tableau_conversion(format!(
            "pair-measurement flow generator target {target} does not identify a qubit"
        ))
    })?;
    Ok((qubit.get() as usize, target.is_inverted_result_target()))
}

pub(super) fn record_index_i32(record_index: usize) -> CircuitResult<i32> {
    i32::try_from(record_index).map_err(|_| {
        CircuitError::invalid_tableau_conversion(format!(
            "flow measurement record index {record_index} does not fit i32"
        ))
    })
}

pub(super) fn stabilizer_to_circuit_error(error: crate::StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}

pub(super) fn internal_flow_error(message: &'static str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(message)
}

pub(super) fn rows_matching(flows: &[Flow], predicate: impl Fn(&Flow) -> bool) -> Vec<usize> {
    flows
        .iter()
        .enumerate()
        .filter_map(|(index, flow)| predicate(flow).then_some(index))
        .collect()
}

pub(super) fn input_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
    record_sign: PauliSign,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        single_pauli(qubit_count, qubit, basis),
        PauliString::from_bases_unchecked(record_sign, []),
        [record_index_i32(record_index)?],
        [],
    ))
}

pub(super) fn positive_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity_unchecked(0),
        PauliString::identity_unchecked(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

pub(super) fn negative_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity_unchecked(0),
        PauliString::from_bases_unchecked(PauliSign::Minus, []),
        [record_index_i32(record_index)?],
        [],
    ))
}

pub(super) fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}
