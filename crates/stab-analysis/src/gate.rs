use std::str::FromStr;

use num_complex::Complex32;
use stab_algebra::{
    Flow, PauliBasis, PauliSign, PauliString, SingleQubitClifford, StabilizerError,
    StabilizerResult, Tableau,
};
use stab_model::advanced::{
    GateUnitaryRows, gate_decomposition, gate_flow_descriptors, gate_unitary_rows,
};
use stab_model::{Circuit, Gate, GateDecomposition};

use crate::{AnalysisError, AnalysisResult};

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

/// Resolves a closed-dialect gate into its single-qubit Clifford algebra value.
pub fn single_qubit_clifford_for_gate(gate: Gate) -> StabilizerResult<SingleQubitClifford> {
    let clifford = match gate.canonical_name() {
        "I" => SingleQubitClifford::I,
        "X" => SingleQubitClifford::X,
        "Y" => SingleQubitClifford::Y,
        "Z" => SingleQubitClifford::Z,
        "H" => SingleQubitClifford::H,
        "SQRT_Y_DAG" => SingleQubitClifford::SqrtYDag,
        "H_NXZ" => SingleQubitClifford::Hnxz,
        "SQRT_Y" => SingleQubitClifford::SqrtY,
        "S" => SingleQubitClifford::S,
        "H_XY" => SingleQubitClifford::Hxy,
        "H_NXY" => SingleQubitClifford::Hnxy,
        "S_DAG" => SingleQubitClifford::SDag,
        "SQRT_X_DAG" => SingleQubitClifford::SqrtXDag,
        "SQRT_X" => SingleQubitClifford::SqrtX,
        "H_NYZ" => SingleQubitClifford::Hnyz,
        "H_YZ" => SingleQubitClifford::Hyz,
        "C_XYZ" => SingleQubitClifford::Cxyz,
        "C_XYNZ" => SingleQubitClifford::Cxynz,
        "C_NXYZ" => SingleQubitClifford::Cnxyz,
        "C_XNYZ" => SingleQubitClifford::Cxnyz,
        "C_ZYX" => SingleQubitClifford::Czyx,
        "C_ZNYX" => SingleQubitClifford::Cznyx,
        "C_NZYX" => SingleQubitClifford::Cnzyx,
        "C_ZYNX" => SingleQubitClifford::Czynx,
        _ => {
            return Err(StabilizerError::InvalidSingleQubitCliffordGate {
                gate: gate.canonical_name().to_owned(),
            });
        }
    };
    Ok(clifford)
}

/// Returns the local Clifford tableau metadata for a gate with known tableau data.
pub fn gate_tableau(gate: Gate) -> AnalysisResult<Tableau> {
    if let Ok(clifford) = single_qubit_clifford_for_gate(gate) {
        return Ok(clifford.tableau());
    }
    let outputs = two_qubit_outputs(gate.canonical_name()).ok_or_else(|| {
        AnalysisError::invalid_tableau_conversion(format!(
            "gate {} does not have tableau data",
            gate.canonical_name()
        ))
    })?;
    Tableau::gate2(outputs[0], outputs[1], outputs[2], outputs[3])
        .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))
}

/// Returns true when [`gate_tableau`] can produce local Clifford tableau metadata for `gate`.
pub fn gate_has_tableau(gate: Gate) -> bool {
    single_qubit_clifford_for_gate(gate).is_ok()
        || two_qubit_outputs(gate.canonical_name()).is_some()
}

/// Returns Stim v1.16.0 gate-table stabilizer flow metadata.
pub fn gate_flows(gate: Gate) -> AnalysisResult<Vec<Flow>> {
    let gate_name = gate.canonical_name();
    if let Some(descriptors) = gate_flow_descriptors(gate) {
        return descriptors
            .iter()
            .map(|descriptor| {
                Flow::from_str(descriptor).map_err(|error| {
                    AnalysisError::invalid_tableau_conversion(format!(
                        "gate {gate_name} flow metadata is invalid: {error}"
                    ))
                })
            })
            .collect();
    }
    if !gate_has_flows(gate) {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "gate {gate_name} does not have flow metadata"
        )));
    }
    let tableau = gate_tableau(gate)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in 0..tableau.len() {
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_analysis_error)?
                .clone(),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_analysis_error)?
                .clone(),
        ));
    }
    Ok(flows)
}

/// Returns true when [`gate_flows`] can produce gate-table flow metadata.
pub fn gate_has_flows(gate: Gate) -> bool {
    gate_has_tableau(gate) || gate_flow_descriptors(gate).is_some()
}

/// Returns Stim v1.16.0's fixed-shape one- or two-qubit unitary matrix metadata.
pub fn gate_unitary_matrix(gate: Gate) -> AnalysisResult<GateUnitaryMatrix> {
    gate_unitary_rows(gate)
        .map(|rows| match rows {
            GateUnitaryRows::One(rows) => GateUnitaryMatrix::One(complex_matrix(rows)),
            GateUnitaryRows::Two(rows) => GateUnitaryMatrix::Two(complex_matrix(rows)),
        })
        .ok_or_else(|| {
            AnalysisError::invalid_tableau_conversion(format!(
                "gate {} does not have fixed-shape unitary matrix data",
                gate.canonical_name()
            ))
        })
}

/// Returns true when [`gate_unitary_matrix`] can produce fixed-shape unitary metadata.
pub fn gate_has_unitary_matrix(gate: Gate) -> bool {
    gate_unitary_rows(gate).is_some()
}

/// Returns Stim v1.16.0's H/S/CX/M/R decomposition metadata for `gate`.
pub fn gate_h_s_cx_m_r_decomposition(gate: Gate) -> AnalysisResult<GateDecomposition> {
    gate_decomposition(gate).ok_or_else(|| {
        AnalysisError::invalid_tableau_conversion(format!(
            "gate {} does not have H/S/CX/M/R decomposition data",
            gate.canonical_name()
        ))
    })
}

/// Returns true when [`gate_h_s_cx_m_r_decomposition`] can produce gate-table metadata.
pub fn gate_has_h_s_cx_m_r_decomposition(gate: Gate) -> bool {
    gate_decomposition(gate).is_some()
}

/// Parses gate decomposition metadata into a validated circuit.
pub fn gate_decomposition_to_circuit(decomposition: GateDecomposition) -> AnalysisResult<Circuit> {
    Circuit::from_stim_str(decomposition.as_stim_str()).map_err(Into::into)
}

fn complex_matrix<const N: usize>(rows: [[(f32, f32); N]; N]) -> [[Complex32; N]; N] {
    rows.map(|row| row.map(|(real, imaginary)| Complex32::new(real, imaginary)))
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    stab_algebra::advanced::pauli_from_bases_unchecked(
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

fn stabilizer_to_analysis_error(error: StabilizerError) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(error.to_string())
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
