use std::str::FromStr;

use num_complex::Complex32;
use stab_algebra::{
    Flow, PauliBasis, PauliSign, PauliString, SingleQubitClifford, StabilizerError,
    StabilizerResult, Tableau,
};
use stab_model::advanced::{
    GateUnitaryRows, gate_decomposition, gate_flow_descriptors, gate_unitary_rows,
};
use stab_model::{Circuit, Gate, GateDecomposition, GateKind};

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
    let clifford = match gate.kind() {
        GateKind::I => SingleQubitClifford::I,
        GateKind::X => SingleQubitClifford::X,
        GateKind::Y => SingleQubitClifford::Y,
        GateKind::Z => SingleQubitClifford::Z,
        GateKind::H => SingleQubitClifford::H,
        GateKind::SQRT_Y_DAG => SingleQubitClifford::SqrtYDag,
        GateKind::H_NXZ => SingleQubitClifford::Hnxz,
        GateKind::SQRT_Y => SingleQubitClifford::SqrtY,
        GateKind::S => SingleQubitClifford::S,
        GateKind::H_XY => SingleQubitClifford::Hxy,
        GateKind::H_NXY => SingleQubitClifford::Hnxy,
        GateKind::S_DAG => SingleQubitClifford::SDag,
        GateKind::SQRT_X_DAG => SingleQubitClifford::SqrtXDag,
        GateKind::SQRT_X => SingleQubitClifford::SqrtX,
        GateKind::H_NYZ => SingleQubitClifford::Hnyz,
        GateKind::H_YZ => SingleQubitClifford::Hyz,
        GateKind::C_XYZ => SingleQubitClifford::Cxyz,
        GateKind::C_XYNZ => SingleQubitClifford::Cxynz,
        GateKind::C_NXYZ => SingleQubitClifford::Cnxyz,
        GateKind::C_XNYZ => SingleQubitClifford::Cxnyz,
        GateKind::C_ZYX => SingleQubitClifford::Czyx,
        GateKind::C_ZNYX => SingleQubitClifford::Cznyx,
        GateKind::C_NZYX => SingleQubitClifford::Cnzyx,
        GateKind::C_ZYNX => SingleQubitClifford::Czynx,
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => {
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
    let outputs = two_qubit_outputs(gate.kind()).ok_or_else(|| {
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
    single_qubit_clifford_for_gate(gate).is_ok() || two_qubit_outputs(gate.kind()).is_some()
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

fn two_qubit_outputs(kind: GateKind) -> Option<[&'static str; 4]> {
    match kind {
        GateKind::II => Some(["+X_", "+Z_", "+_X", "+_Z"]),
        GateKind::XCX => Some(["+X_", "+ZX", "+_X", "+XZ"]),
        GateKind::XCY => Some(["+X_", "+ZY", "+XX", "+XZ"]),
        GateKind::XCZ => Some(["+X_", "+ZZ", "+XX", "+_Z"]),
        GateKind::YCX => Some(["+XX", "+ZX", "+_X", "+YZ"]),
        GateKind::YCY => Some(["+XY", "+ZY", "+YX", "+YZ"]),
        GateKind::YCZ => Some(["+XZ", "+ZZ", "+YX", "+_Z"]),
        GateKind::CX => Some(["+XX", "+Z_", "+_X", "+ZZ"]),
        GateKind::CY => Some(["+XY", "+Z_", "+ZX", "+ZZ"]),
        GateKind::CZ => Some(["+XZ", "+Z_", "+ZX", "+_Z"]),
        GateKind::SWAP => Some(["+_X", "+_Z", "+X_", "+Z_"]),
        GateKind::ISWAP => Some(["+ZY", "+_Z", "+YZ", "+Z_"]),
        GateKind::ISWAP_DAG => Some(["-ZY", "+_Z", "-YZ", "+Z_"]),
        GateKind::CXSWAP => Some(["+XX", "+_Z", "+X_", "+ZZ"]),
        GateKind::SWAPCX => Some(["+_X", "+ZZ", "+XX", "+Z_"]),
        GateKind::CZSWAP => Some(["+ZX", "+_Z", "+XZ", "+Z_"]),
        GateKind::SQRT_XX => Some(["+X_", "-YX", "+_X", "-XY"]),
        GateKind::SQRT_XX_DAG => Some(["+X_", "+YX", "+_X", "+XY"]),
        GateKind::SQRT_YY => Some(["-ZY", "+XY", "-YZ", "+YX"]),
        GateKind::SQRT_YY_DAG => Some(["+ZY", "-XY", "+YZ", "-YX"]),
        GateKind::SQRT_ZZ => Some(["+YZ", "+Z_", "+ZY", "+_Z"]),
        GateKind::SQRT_ZZ_DAG => Some(["-YZ", "+Z_", "-ZY", "+_Z"]),
        GateKind::DETECTOR
        | GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::QUBIT_COORDS
        | GateKind::SHIFT_COORDS
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => None,
    }
}
