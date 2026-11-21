use num_complex::Complex32;
use stab_algebra::{
    Flow, PauliBasis, PauliSign, PauliString, SingleQubitClifford, Tableau, TwoQubitClifford,
    TwoQubitPauli,
};
use stab_model::{Gate, GateKind, GatePauliString, GateUnitaryRows, Pauli};

use crate::{AnalysisError, AnalysisOperation, AnalysisResult};

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

/// Returns the single-qubit Clifford of a one-qubit unitary gate, or `None` for every other
/// gate.
pub fn single_qubit_clifford_for_gate(gate: Gate) -> Option<SingleQubitClifford> {
    Some(match gate.kind() {
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
        | GateKind::MZZ => return None,
    })
}

/// Returns the two-qubit Clifford of a fixed two-qubit unitary gate, such as `CX` or `ISWAP`, or
/// `None` for every other gate.
///
/// The images are Stim v1.16.0's tableau data for each gate, checked to form a Clifford when
/// this table is compiled.
pub fn two_qubit_clifford_for_gate(gate: Gate) -> Option<TwoQubitClifford> {
    use PauliBasis::{I, X, Y, Z};
    use PauliSign::{Minus as M, Plus as P};
    Some(match gate.kind() {
        GateKind::II => const { clifford([(P, X, I), (P, I, X)], [(P, Z, I), (P, I, Z)]) },
        GateKind::XCX => const { clifford([(P, X, I), (P, I, X)], [(P, Z, X), (P, X, Z)]) },
        GateKind::XCY => const { clifford([(P, X, I), (P, X, X)], [(P, Z, Y), (P, X, Z)]) },
        GateKind::XCZ => const { clifford([(P, X, I), (P, X, X)], [(P, Z, Z), (P, I, Z)]) },
        GateKind::YCX => const { clifford([(P, X, X), (P, I, X)], [(P, Z, X), (P, Y, Z)]) },
        GateKind::YCY => const { clifford([(P, X, Y), (P, Y, X)], [(P, Z, Y), (P, Y, Z)]) },
        GateKind::YCZ => const { clifford([(P, X, Z), (P, Y, X)], [(P, Z, Z), (P, I, Z)]) },
        GateKind::CX => CX_CLIFFORD,
        GateKind::CY => const { clifford([(P, X, Y), (P, Z, X)], [(P, Z, I), (P, Z, Z)]) },
        GateKind::CZ => const { clifford([(P, X, Z), (P, Z, X)], [(P, Z, I), (P, I, Z)]) },
        GateKind::SWAP => const { clifford([(P, I, X), (P, X, I)], [(P, I, Z), (P, Z, I)]) },
        GateKind::ISWAP => const { clifford([(P, Z, Y), (P, Y, Z)], [(P, I, Z), (P, Z, I)]) },
        GateKind::ISWAP_DAG => const { clifford([(M, Z, Y), (M, Y, Z)], [(P, I, Z), (P, Z, I)]) },
        GateKind::CXSWAP => const { clifford([(P, X, X), (P, X, I)], [(P, I, Z), (P, Z, Z)]) },
        GateKind::SWAPCX => const { clifford([(P, I, X), (P, X, X)], [(P, Z, Z), (P, Z, I)]) },
        GateKind::CZSWAP => const { clifford([(P, Z, X), (P, X, Z)], [(P, I, Z), (P, Z, I)]) },
        GateKind::SQRT_XX => const { clifford([(P, X, I), (P, I, X)], [(M, Y, X), (M, X, Y)]) },
        GateKind::SQRT_XX_DAG => const { clifford([(P, X, I), (P, I, X)], [(P, Y, X), (P, X, Y)]) },
        GateKind::SQRT_YY => const { clifford([(M, Z, Y), (M, Y, Z)], [(P, X, Y), (P, Y, X)]) },
        GateKind::SQRT_YY_DAG => const { clifford([(P, Z, Y), (P, Y, Z)], [(M, X, Y), (M, Y, X)]) },
        GateKind::SQRT_ZZ => const { clifford([(P, Y, Z), (P, Z, Y)], [(P, Z, I), (P, I, Z)]) },
        GateKind::SQRT_ZZ_DAG => const { clifford([(M, Y, Z), (M, Z, Y)], [(P, Z, I), (P, I, Z)]) },
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
        | GateKind::MZZ => return None,
    })
}

/// `CX`, which tableau synthesis also uses as a base gate.
pub(crate) const CX_CLIFFORD: TwoQubitClifford = {
    use PauliBasis::{I, X, Z};
    use PauliSign::Plus as P;
    clifford([(P, X, X), (P, I, X)], [(P, Z, I), (P, Z, Z)])
};

/// Builds a table entry from Stim's `X_0`, `X_1` and `Z_0`, `Z_1` images.
///
/// Only called in `const` blocks, so an entry that is not a Clifford fails compilation.
#[allow(
    clippy::panic,
    reason = "evaluated only at compile time, where a panic is a build error for a bad entry"
)]
const fn clifford(
    x_images: [(PauliSign, PauliBasis, PauliBasis); 2],
    z_images: [(PauliSign, PauliBasis, PauliBasis); 2],
) -> TwoQubitClifford {
    const fn image((sign, first, second): (PauliSign, PauliBasis, PauliBasis)) -> TwoQubitPauli {
        TwoQubitPauli::new(sign, [first, second])
    }
    let [x0, x1] = x_images;
    let [z0, z1] = z_images;
    match TwoQubitClifford::try_from_images([image(x0), image(x1)], [image(z0), image(z1)]) {
        Some(clifford) => clifford,
        None => panic!("a two-qubit gate table entry is not a Clifford"),
    }
}

/// Returns the local Clifford tableau of a fixed one- or two-qubit unitary gate, or `None` for
/// every other gate.
pub fn gate_tableau(gate: Gate) -> Option<Tableau> {
    if let Some(clifford) = single_qubit_clifford_for_gate(gate) {
        return Some(clifford.tableau());
    }
    two_qubit_clifford_for_gate(gate).map(TwoQubitClifford::tableau)
}

/// Returns the tableau of a unitary gate that `operation` has already routed as a fixed one- or
/// two-qubit Clifford, reporting a gate outside that table as a broken invariant.
pub(crate) fn fixed_unitary_tableau(
    gate: Gate,
    operation: AnalysisOperation,
) -> AnalysisResult<Tableau> {
    gate_tableau(gate).ok_or_else(|| {
        AnalysisError::internal(
            operation,
            format!(
                "gate {} has no fixed Clifford tableau",
                gate.canonical_name()
            ),
        )
    })
}

/// Returns Stim v1.16.0's stabilizer flows of `gate`, or `None` when Stim gives it none.
///
/// A fixed unitary gate's flows map each `X_q` and `Z_q` to its tableau image, in qubit order;
/// a non-unitary gate's flows come from the model's gate table.
pub fn gate_flows(gate: Gate) -> Option<Vec<Flow>> {
    if let Some(descriptors) = gate.flow_descriptors() {
        return Some(
            descriptors
                .iter()
                .map(|descriptor| {
                    let measurements: Vec<i32> = descriptor
                        .measurements()
                        .iter()
                        .map(|offset| offset.get())
                        .collect();
                    Flow::from_paulis_and_measurements(
                        pauli_string(descriptor.input()),
                        pauli_string(descriptor.output()),
                        &measurements,
                    )
                })
                .collect(),
        );
    }
    if let Some(clifford) = single_qubit_clifford_for_gate(gate) {
        return Some(vec![
            Flow::from_paulis(single_pauli(1, 0, PauliBasis::X), clifford.x_output()),
            Flow::from_paulis(single_pauli(1, 0, PauliBasis::Z), clifford.z_output()),
        ]);
    }
    let clifford = two_qubit_clifford_for_gate(gate)?;
    let mut flows = Vec::with_capacity(4);
    for (qubit, (x_image, z_image)) in clifford
        .x_images()
        .into_iter()
        .zip(clifford.z_images())
        .enumerate()
    {
        flows.push(Flow::from_paulis(
            single_pauli(2, qubit, PauliBasis::X),
            x_image.to_pauli_string(),
        ));
        flows.push(Flow::from_paulis(
            single_pauli(2, qubit, PauliBasis::Z),
            z_image.to_pauli_string(),
        ));
    }
    Some(flows)
}

/// Returns Stim v1.16.0's fixed-shape unitary matrix of a one- or two-qubit unitary gate, or
/// `None` for every other gate.
pub fn gate_unitary_matrix(gate: Gate) -> Option<GateUnitaryMatrix> {
    gate.unitary_rows().map(|rows| match rows {
        GateUnitaryRows::One(rows) => GateUnitaryMatrix::One(complex_matrix(rows)),
        GateUnitaryRows::Two(rows) => GateUnitaryMatrix::Two(complex_matrix(rows)),
    })
}

fn complex_matrix<const N: usize>(rows: [[(f32, f32); N]; N]) -> [[Complex32; N]; N] {
    rows.map(|row| row.map(|(real, imaginary)| Complex32::new(real, imaginary)))
}

fn pauli_string(product: GatePauliString) -> PauliString {
    let sign = if product.is_negative() {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    };
    stab_algebra::advanced::pauli_from_bases_unbudgeted(
        sign,
        product.paulis().iter().map(|pauli| match pauli {
            None => PauliBasis::I,
            Some(Pauli::X) => PauliBasis::X,
            Some(Pauli::Y) => PauliBasis::Y,
            Some(Pauli::Z) => PauliBasis::Z,
        }),
    )
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    stab_algebra::advanced::pauli_from_bases_unbudgeted(
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
