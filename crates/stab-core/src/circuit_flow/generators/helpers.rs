use crate::{CircuitError, CircuitInstruction, CircuitResult, PauliBasis, PauliString, Target};

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
