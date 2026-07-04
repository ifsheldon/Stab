use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, GateCategory,
    PauliBasis, PauliSign, PauliString, QubitId, SingleQubitClifford, Tableau, Target,
};

pub fn circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
) -> CircuitResult<Tableau> {
    let mut result = Tableau::identity(circuit.count_qubits());
    apply_circuit_to_tableau(
        circuit,
        ignore_noise,
        ignore_measurement,
        ignore_reset,
        &mut result,
    )?;
    Ok(result)
}

fn apply_circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
    result: &mut Tableau,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => apply_instruction_to_tableau(
                instruction,
                ignore_noise,
                ignore_measurement,
                ignore_reset,
                result,
            )?,
            CircuitItem::RepeatBlock(repeat) => {
                for _ in 0..repeat.repeat_count().get() {
                    apply_circuit_to_tableau(
                        repeat.body(),
                        ignore_noise,
                        ignore_measurement,
                        ignore_reset,
                        result,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn apply_instruction_to_tableau(
    instruction: &CircuitInstruction,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
    result: &mut Tableau,
) -> CircuitResult<()> {
    let gate = instruction.gate();
    match gate.category() {
        GateCategory::Annotation => Ok(()),
        GateCategory::Noise | GateCategory::HeraldedNoise => {
            if ignore_noise || instruction.args().iter().all(|arg| *arg == 0.0) {
                Ok(())
            } else {
                Err(CircuitError::invalid_tableau_conversion(format!(
                    "noisy operation {}",
                    gate.canonical_name()
                )))
            }
        }
        GateCategory::Collapsing | GateCategory::PairMeasurement => {
            let name = gate.canonical_name();
            let is_reset = matches!(name, "R" | "RX" | "RY" | "MR" | "MRX" | "MRY");
            let is_measurement = matches!(
                name,
                "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" | "MPAD" | "MXX" | "MYY" | "MZZ"
            );
            if (!is_reset || ignore_reset) && (!is_measurement || ignore_measurement) {
                Ok(())
            } else {
                Err(CircuitError::invalid_tableau_conversion(format!(
                    "non-unitary operation {}",
                    gate.canonical_name()
                )))
            }
        }
        GateCategory::ControlFlow => Ok(()),
        GateCategory::Controlled
        | GateCategory::HadamardLike
        | GateCategory::Pauli
        | GateCategory::Period3
        | GateCategory::Period4
        | GateCategory::ParityPhasing
        | GateCategory::Swap => {
            for group in instruction.target_groups() {
                apply_unitary_group_to_tableau(gate.canonical_name(), group, result)?;
            }
            Ok(())
        }
        GateCategory::PauliProduct => Err(CircuitError::invalid_tableau_conversion(format!(
            "unsupported unitary operation {}",
            gate.canonical_name()
        ))),
    }
}

fn apply_unitary_group_to_tableau(
    gate_name: &str,
    targets: &[Target],
    result: &mut Tableau,
) -> CircuitResult<()> {
    let target_ids = target_qubit_ids(gate_name, targets)?;
    let local = gate_tableau(gate_name)?;
    if local.len() != target_ids.len() {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "gate {gate_name} expected {} tableau targets but got {}",
            local.len(),
            target_ids.len()
        )));
    }
    let expanded = scatter_tableau(&local, &target_ids, result.len())?;
    *result = result
        .then(&expanded)
        .map_err(|error| CircuitError::invalid_tableau_conversion(error.to_string()))?;
    Ok(())
}

fn target_qubit_ids(gate_name: &str, targets: &[Target]) -> CircuitResult<Vec<QubitId>> {
    targets
        .iter()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                CircuitError::invalid_tableau_conversion(format!(
                    "gate {gate_name} has non-qubit tableau target {target}"
                ))
            })
        })
        .collect()
}

pub(crate) fn gate_tableau(gate_name: &str) -> CircuitResult<Tableau> {
    if let Ok(gate) = crate::Gate::from_name(gate_name)
        && let Ok(clifford) = SingleQubitClifford::from_gate(gate)
    {
        return Ok(clifford.tableau());
    }
    let outputs = two_qubit_outputs(gate_name).ok_or_else(|| {
        CircuitError::invalid_tableau_conversion(format!(
            "gate {gate_name} does not have tableau data"
        ))
    })?;
    Tableau::gate2(outputs[0], outputs[1], outputs[2], outputs[3])
        .map_err(|error| CircuitError::invalid_tableau_conversion(error.to_string()))
}

pub(crate) fn gate_has_tableau(gate_name: &str) -> bool {
    if let Ok(gate) = crate::Gate::from_name(gate_name)
        && SingleQubitClifford::from_gate(gate).is_ok()
    {
        return true;
    }
    two_qubit_outputs(gate_name).is_some()
}

fn two_qubit_outputs(gate_name: &str) -> Option<[&'static str; 4]> {
    match gate_name {
        "II" => Some(["+X_", "+Z_", "+_X", "+_Z"]),
        "XCX" => Some(["+X_", "+ZX", "+_X", "+XZ"]),
        "XCY" => Some(["+X_", "+ZY", "+XX", "+XZ"]),
        "XCZ" => Some(["+X_", "+ZZ", "+XX", "+_Z"]),
        "YCX" => Some(["+XX", "+ZX", "+_X", "+YZ"]),
        "YCY" => Some(["+XY", "+ZY", "+YX", "+YZ"]),
        "YCZ" => Some(["+XZ", "+ZZ", "+YX", "+_Z"]),
        "CX" => Some(["+XX", "+Z_", "+_X", "+ZZ"]),
        "CY" => Some(["+XY", "+Z_", "+ZX", "+ZZ"]),
        "CZ" => Some(["+XZ", "+Z_", "+ZX", "+_Z"]),
        "SWAP" => Some(["+_X", "+_Z", "+X_", "+Z_"]),
        "ISWAP" => Some(["+ZY", "+_Z", "+YZ", "+Z_"]),
        "ISWAP_DAG" => Some(["-ZY", "+_Z", "-YZ", "+Z_"]),
        "CXSWAP" => Some(["+XX", "+_Z", "+X_", "+ZZ"]),
        "SWAPCX" => Some(["+_X", "+ZZ", "+XX", "+Z_"]),
        "CZSWAP" => Some(["+ZX", "+_Z", "+XZ", "+Z_"]),
        "SQRT_XX" => Some(["+X_", "-YX", "+_X", "-XY"]),
        "SQRT_XX_DAG" => Some(["+X_", "+YX", "+_X", "+XY"]),
        "SQRT_YY" => Some(["-ZY", "+XY", "-YZ", "+YX"]),
        "SQRT_YY_DAG" => Some(["+ZY", "-XY", "+YZ", "-YX"]),
        "SQRT_ZZ" => Some(["+YZ", "+Z_", "+ZY", "+_Z"]),
        "SQRT_ZZ_DAG" => Some(["-YZ", "+Z_", "-ZY", "+_Z"]),
        _ => None,
    }
}

fn scatter_tableau(
    local: &Tableau,
    targets: &[QubitId],
    num_qubits: usize,
) -> CircuitResult<Tableau> {
    let mut xs = Vec::with_capacity(num_qubits);
    let mut zs = Vec::with_capacity(num_qubits);
    for global_index in 0..num_qubits {
        if let Some(local_index) = local_index_for_global(targets, global_index) {
            xs.push(expand_pauli(
                local.x_output(local_index).map_err(map_tableau_error)?,
                targets,
                num_qubits,
            )?);
            zs.push(expand_pauli(
                local.z_output(local_index).map_err(map_tableau_error)?,
                targets,
                num_qubits,
            )?);
        } else {
            xs.push(single_pauli(
                num_qubits,
                global_index,
                PauliBasis::X,
                PauliSign::Plus,
            ));
            zs.push(single_pauli(
                num_qubits,
                global_index,
                PauliBasis::Z,
                PauliSign::Plus,
            ));
        }
    }
    Ok(Tableau::from_output_columns_unchecked(xs, zs))
}

fn expand_pauli(
    local: &PauliString,
    targets: &[QubitId],
    num_qubits: usize,
) -> CircuitResult<PauliString> {
    let mut bases = vec![PauliBasis::I; num_qubits];
    for (local_index, target) in targets.iter().enumerate() {
        let global_index = target.get() as usize;
        let basis = local.get(local_index).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion("local tableau Pauli length mismatch")
        })?;
        if let Some(slot) = bases.get_mut(global_index) {
            *slot = basis;
        } else {
            return Err(CircuitError::invalid_tableau_conversion(format!(
                "target qubit {global_index} outside tableau length {num_qubits}"
            )));
        }
    }
    Ok(PauliString::from_bases(local.sign(), bases))
}

fn local_index_for_global(targets: &[QubitId], global_index: usize) -> Option<usize> {
    targets
        .iter()
        .position(|target| target.get() as usize == global_index)
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis, sign: PauliSign) -> PauliString {
    PauliString::from_bases(
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

fn map_tableau_error(error: crate::StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}
