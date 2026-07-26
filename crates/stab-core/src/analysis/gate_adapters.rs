use std::str::FromStr;

use num_complex::Complex32;

use crate::{
    Circuit, CircuitError, CircuitResult, Flow, Gate, GateDecomposition, PauliBasis, PauliSign,
    PauliString, StabilizerError, Tableau, gate::GateUnitaryRows,
};

/// Fixed-shape unitary matrix metadata for a one- or two-qubit Stim gate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GateUnitaryMatrix {
    One([[Complex32; 2]; 2]),
    Two([[Complex32; 4]; 4]),
}

impl GateUnitaryMatrix {
    /// Returns the matrix width and height.
    pub fn dimension(self) -> usize {
        match self {
            Self::One(_) => 2,
            Self::Two(_) => 4,
        }
    }

    /// Returns the number of qubits acted on by this unitary.
    pub fn num_qubits(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Two(_) => 2,
        }
    }

    /// Returns the number of complex matrix entries.
    pub fn entry_count(self) -> usize {
        let dimension = self.dimension();
        dimension * dimension
    }

    /// Materializes the fixed-shape matrix as nested rows for generic matrix consumers.
    pub fn to_vecs(self) -> Vec<Vec<Complex32>> {
        match self {
            Self::One(rows) => rows.into_iter().map(Vec::from).collect(),
            Self::Two(rows) => rows.into_iter().map(Vec::from).collect(),
        }
    }
}

/// Returns the local Clifford tableau metadata for a gate with known tableau data.
///
/// Gates without fixed-arity Clifford tableau metadata return
/// [`CircuitError::InvalidTableauConversion`].
pub fn gate_tableau(gate: Gate) -> CircuitResult<Tableau> {
    crate::circuit_tableau::gate_tableau(gate.canonical_name())
}

/// Returns true when [`gate_tableau`] can produce local Clifford tableau metadata for `gate`.
pub fn gate_has_tableau(gate: Gate) -> bool {
    crate::circuit_tableau::gate_has_tableau(gate.canonical_name())
}

/// Returns Stim v1.16.0 gate-table stabilizer flow metadata.
///
/// This includes tableau-backed unitary gates plus the representative measurement-rich and
/// variable-target metadata that Stim exposes through `GateData.flows`. Execution support is
/// tracked separately; metadata for gates such as `SPP` does not imply sampler,
/// detector-conversion, or analyzer execution support.
pub fn gate_flows(gate: Gate) -> CircuitResult<Vec<Flow>> {
    let gate_name = gate.canonical_name();
    if let Some(descriptors) = crate::gate::gate_flow_descriptors(gate_name) {
        return descriptors
            .iter()
            .map(|descriptor| {
                Flow::from_str(descriptor).map_err(|error| {
                    CircuitError::invalid_tableau_conversion(format!(
                        "gate {gate_name} flow metadata is invalid: {error}"
                    ))
                })
            })
            .collect();
    }
    if !gate_has_flows(gate) {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "gate {} does not have flow metadata",
            gate.canonical_name()
        )));
    }
    let tableau = gate_tableau(gate)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in 0..tableau.len() {
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

/// Returns true when [`gate_flows`] can produce gate-table flow metadata.
pub fn gate_has_flows(gate: Gate) -> bool {
    gate_has_tableau(gate) || crate::gate::gate_flow_descriptors(gate.canonical_name()).is_some()
}

/// Returns Stim v1.16.0's fixed-shape one- or two-qubit unitary matrix metadata.
///
/// Variable-target unitary gate families, such as `SPP` and `SPP_DAG`, do not have fixed matrix
/// metadata in Stim's gate table and are rejected here.
pub fn gate_unitary_matrix(gate: Gate) -> CircuitResult<GateUnitaryMatrix> {
    crate::gate::gate_unitary_rows(gate.canonical_name())
        .map(|rows| match rows {
            GateUnitaryRows::One(rows) => GateUnitaryMatrix::One(complex_matrix(rows)),
            GateUnitaryRows::Two(rows) => GateUnitaryMatrix::Two(complex_matrix(rows)),
        })
        .ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "gate {} does not have fixed-shape unitary matrix data",
                gate.canonical_name()
            ))
        })
}

/// Returns true when [`gate_unitary_matrix`] can produce fixed-shape unitary metadata.
pub fn gate_has_unitary_matrix(gate: Gate) -> bool {
    crate::gate::gate_unitary_rows(gate.canonical_name()).is_some()
}

/// Returns Stim v1.16.0's H/S/CX/M/R decomposition metadata for `gate`.
///
/// This exposes the static gate-table metadata only. Full circuit decomposition is owned by the
/// circuit transform APIs and is not implied by this accessor.
pub fn gate_h_s_cx_m_r_decomposition(gate: Gate) -> CircuitResult<GateDecomposition> {
    crate::gate::gate_decomposition_text(gate.canonical_name())
        .map(GateDecomposition::new)
        .ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "gate {} does not have H/S/CX/M/R decomposition data",
                gate.canonical_name()
            ))
        })
}

/// Returns true when [`gate_h_s_cx_m_r_decomposition`] can produce gate-table metadata.
pub fn gate_has_h_s_cx_m_r_decomposition(gate: Gate) -> bool {
    crate::gate::gate_decomposition_text(gate.canonical_name()).is_some()
}

/// Parses gate decomposition metadata into a validated circuit.
pub fn gate_decomposition_to_circuit(decomposition: GateDecomposition) -> CircuitResult<Circuit> {
    Circuit::from_stim_str(decomposition.as_stim_str())
}

// Temporary pre-0.2 method adapters. The free semantic functions own the dependency between the
// closed gate model and algebra values.
impl Gate {
    /// Returns the local Clifford tableau metadata for gates with known tableau data.
    ///
    /// Gates without fixed-arity Clifford tableau metadata return
    /// [`CircuitError::InvalidTableauConversion`].
    pub fn tableau(self) -> CircuitResult<Tableau> {
        gate_tableau(self)
    }

    /// Returns true when [`Gate::tableau`] can produce local Clifford tableau metadata.
    pub fn has_tableau(self) -> bool {
        gate_has_tableau(self)
    }

    /// Returns Stim v1.16.0 gate-table stabilizer flow metadata.
    ///
    /// This includes tableau-backed unitary gates plus the representative measurement-rich and
    /// variable-target metadata that Stim exposes through `GateData.flows`. Execution support is
    /// tracked separately; metadata for gates such as `SPP` does not imply sampler,
    /// detector-conversion, or analyzer execution support.
    pub fn flows(self) -> CircuitResult<Vec<Flow>> {
        gate_flows(self)
    }

    /// Returns true when [`Gate::flows`] can produce Stim v1.16.0 gate-table flow metadata.
    pub fn has_flows(self) -> bool {
        gate_has_flows(self)
    }

    /// Returns Stim v1.16.0's fixed-shape one- or two-qubit unitary matrix metadata.
    ///
    /// Variable-target unitary gate families, such as `SPP` and `SPP_DAG`, do not have fixed
    /// matrix metadata in Stim's gate table and are rejected here.
    pub fn unitary_matrix(self) -> CircuitResult<GateUnitaryMatrix> {
        gate_unitary_matrix(self)
    }

    /// Returns true when [`Gate::unitary_matrix`] can produce fixed-shape unitary metadata.
    pub fn has_unitary_matrix(self) -> bool {
        gate_has_unitary_matrix(self)
    }

    /// Returns Stim v1.16.0's H/S/CX/M/R decomposition metadata for this gate.
    ///
    /// This exposes the static gate-table metadata only. Full circuit decomposition is owned by
    /// the circuit transform APIs and is not implied by this accessor.
    pub fn h_s_cx_m_r_decomposition(self) -> CircuitResult<GateDecomposition> {
        gate_h_s_cx_m_r_decomposition(self)
    }

    /// Returns true when [`Gate::h_s_cx_m_r_decomposition`] can produce gate-table metadata.
    pub fn has_h_s_cx_m_r_decomposition(self) -> bool {
        gate_has_h_s_cx_m_r_decomposition(self)
    }
}

// Temporary pre-0.2 method adapter. Parsing the model-owned descriptor into a circuit is an
// analysis operation and must move with this module during physical crate extraction.
impl GateDecomposition {
    /// Parses the decomposition text into a Stab circuit.
    pub fn to_circuit(self) -> CircuitResult<Circuit> {
        gate_decomposition_to_circuit(self)
    }
}

fn complex_matrix<const N: usize>(rows: [[(f32, f32); N]; N]) -> [[Complex32; N]; N] {
    rows.map(|row| row.map(|(real, imaginary)| Complex32::new(real, imaginary)))
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
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

fn stabilizer_to_circuit_error(error: StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}
