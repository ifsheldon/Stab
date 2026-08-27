use stab_algebra::{PauliBasis, PauliSign, Tableau};
use stab_model::{Circuit, Gate, QubitId, Target};

use crate::{AnalysisError, AnalysisResult, gate_tableau};

/// Synthesizes a Clifford circuit using only `H`, `S`, and `CX` instructions.
///
/// The elimination follows Stim's canonical generator-pivot strategy, but the public contract is
/// semantic: converting the result back to a tableau reproduces `tableau` exactly.
pub fn tableau_to_circuit(tableau: &Tableau) -> AnalysisResult<Circuit> {
    if !tableau.satisfies_invariants().map_err(map_tableau_error)? {
        return Err(AnalysisError::invalid_tableau_conversion(
            "tableau synthesis requires canonical symplectic generators",
        ));
    }

    let mut remaining = tableau.inverse().map_err(map_tableau_error)?;
    let mut circuit = Circuit::new();
    let num_qubits = remaining.len();

    for column in 0..num_qubits {
        let pivot = (column..num_qubits)
            .find(|&row| {
                let x = output_code(&remaining, PauliBasis::X, column, row);
                let z = output_code(&remaining, PauliBasis::Z, column, row);
                matches!((x, z), (Ok(x), Ok(z)) if x != 0 && z != 0 && x != z)
            })
            .ok_or_else(|| {
                AnalysisError::invalid_tableau_conversion(format!(
                    "tableau synthesis could not find a pivot for generator {column}"
                ))
            })?;

        if pivot != column {
            apply_gate(&mut remaining, &mut circuit, "CX", &[pivot, column])?;
            apply_gate(&mut remaining, &mut circuit, "CX", &[column, pivot])?;
            apply_gate(&mut remaining, &mut circuit, "CX", &[pivot, column])?;
        }

        if output_code(&remaining, PauliBasis::Z, column, column)? == 3 {
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
        }
        if output_code(&remaining, PauliBasis::Z, column, column)? != 2 {
            apply_gate(&mut remaining, &mut circuit, "H", &[column])?;
        }
        if output_code(&remaining, PauliBasis::X, column, column)? != 1 {
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
        }

        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::X, column, row)? == 3 {
                apply_gate(&mut remaining, &mut circuit, "S", &[row])?;
            }
        }
        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::X, column, row)? == 2 {
                apply_gate(&mut remaining, &mut circuit, "H", &[row])?;
            }
        }
        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::X, column, row)? != 0 {
                apply_gate(&mut remaining, &mut circuit, "CX", &[column, row])?;
            }
        }

        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::Z, column, row)? == 3 {
                apply_gate(&mut remaining, &mut circuit, "S", &[row])?;
            }
        }
        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::Z, column, row)? == 1 {
                apply_gate(&mut remaining, &mut circuit, "H", &[row])?;
            }
        }
        for row in column + 1..num_qubits {
            if output_code(&remaining, PauliBasis::Z, column, row)? != 0 {
                apply_gate(&mut remaining, &mut circuit, "CX", &[row, column])?;
            }
        }
    }

    let negative_zs = (0..num_qubits)
        .map(|column| {
            remaining
                .z_output(column)
                .map(|output| output.sign() == PauliSign::Minus)
                .map_err(map_tableau_error)
        })
        .collect::<AnalysisResult<Vec<_>>>()?;
    for (column, negative) in negative_zs.iter().copied().enumerate() {
        if negative {
            apply_gate(&mut remaining, &mut circuit, "H", &[column])?;
        }
    }
    for (column, negative) in negative_zs.iter().copied().enumerate() {
        if negative {
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
        }
    }
    for (column, negative) in negative_zs.into_iter().enumerate() {
        if negative {
            apply_gate(&mut remaining, &mut circuit, "H", &[column])?;
        }
    }
    for column in 0..num_qubits {
        if remaining
            .x_output(column)
            .map_err(map_tableau_error)?
            .sign()
            == PauliSign::Minus
        {
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
            apply_gate(&mut remaining, &mut circuit, "S", &[column])?;
        }
    }

    if circuit.count_qubits() < num_qubits && num_qubits != 0 {
        apply_gate(&mut remaining, &mut circuit, "H", &[num_qubits - 1])?;
        apply_gate(&mut remaining, &mut circuit, "H", &[num_qubits - 1])?;
    }

    let identity = Tableau::identity(num_qubits).map_err(map_tableau_error)?;
    if remaining != identity {
        return Err(AnalysisError::invalid_tableau_conversion(
            "tableau synthesis did not eliminate every generator",
        ));
    }
    Ok(circuit)
}

fn output_code(
    tableau: &Tableau,
    input_basis: PauliBasis,
    input: usize,
    output: usize,
) -> AnalysisResult<u8> {
    let pauli = match input_basis {
        PauliBasis::X => tableau.x_output(input).map_err(map_tableau_error)?,
        PauliBasis::Z => tableau.z_output(input).map_err(map_tableau_error)?,
        PauliBasis::I | PauliBasis::Y => {
            return Err(AnalysisError::invalid_tableau_conversion(
                "tableau synthesis requested a noncanonical generator basis",
            ));
        }
    };
    let basis = pauli.get(output).ok_or_else(|| {
        AnalysisError::invalid_tableau_conversion(format!(
            "tableau synthesis output index {output} is outside width {}",
            tableau.len()
        ))
    })?;
    Ok(u8::from(basis.x_bit()) + 2 * u8::from(basis.z_bit()))
}

fn apply_gate(
    remaining: &mut Tableau,
    circuit: &mut Circuit,
    gate_name: &'static str,
    targets: &[usize],
) -> AnalysisResult<()> {
    let gate = Gate::from_name(gate_name)?;
    let local = gate_tableau(gate)?;
    remaining
        .append(&local, targets)
        .map_err(map_tableau_error)?;

    let targets = targets
        .iter()
        .copied()
        .map(qubit_target)
        .collect::<AnalysisResult<Vec<_>>>()?;
    circuit.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
        gate,
        Vec::new(),
        targets,
        None,
    )?);
    Ok(())
}

fn qubit_target(index: usize) -> AnalysisResult<Target> {
    let index = u32::try_from(index).map_err(|_| {
        AnalysisError::invalid_tableau_conversion(format!(
            "tableau synthesis qubit index {index} does not fit in a Stim target"
        ))
    })?;
    Ok(Target::qubit(QubitId::new(index)?, false))
}

fn map_tableau_error(error: stab_algebra::StabilizerError) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(error.to_string())
}
