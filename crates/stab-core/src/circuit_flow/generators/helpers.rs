use crate::{
    CircuitError, CircuitInstruction, CircuitResult, Flow, Pauli, PauliBasis, PauliSign,
    PauliString, Target,
};

use super::single_pauli;

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
    let local = PauliString::from_bases(
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
    Ok(PauliString::from_bases(transformed.sign(), bases))
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

pub(super) fn sweep_controlled_pauli_is_sign_only_noop(instruction: &CircuitInstruction) -> bool {
    if !matches!(
        instruction.gate().canonical_name(),
        "CX" | "CY" | "CZ" | "XCZ" | "YCZ"
    ) {
        return false;
    }
    let groups = instruction.target_groups();
    !groups.is_empty()
        && groups.iter().all(|group| {
            let [left, right] = *group else {
                return false;
            };
            let left_is_sweep = left.is_sweep_bit_target();
            let right_is_sweep = right.is_sweep_bit_target();
            (left_is_sweep ^ right_is_sweep)
                && left.qubit_id().is_some() != right.qubit_id().is_some()
        })
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
        if qubits.contains(&qubit) {
            return None;
        }
        qubits.push(qubit);
    }
    Some(qubits)
}

pub(super) fn unique_measure_reset_targets(
    instruction: &CircuitInstruction,
) -> CircuitResult<Option<Vec<(usize, bool)>>> {
    let mut targets = Vec::with_capacity(instruction.targets().len());
    for target in instruction.targets() {
        let parsed = pair_measurement_target_index(target)?;
        if targets.iter().any(|&(qubit, _)| qubit == parsed.0) {
            return Ok(None);
        }
        targets.push(parsed);
    }
    Ok(Some(targets))
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

pub(super) fn input_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
    record_sign: PauliSign,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        single_pauli(qubit_count, qubit, basis),
        PauliString::from_bases(record_sign, []),
        [record_index_i32(record_index)?],
        [],
    ))
}

pub(super) fn reset_flow(qubit_count: usize, qubit: usize, basis: PauliBasis) -> Flow {
    Flow::new(
        PauliString::identity(0),
        single_pauli(qubit_count, qubit, basis),
        [],
        [],
    )
}

pub(super) fn positive_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::identity(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

pub(super) fn negative_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::from_bases(PauliSign::Minus, []),
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
