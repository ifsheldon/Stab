use std::fmt::{Display, Formatter};

use super::{
    PauliBasis, PauliPhase, PauliSign, PauliString, StabilizerError, StabilizerResult, Tableau,
};
use crate::Gate;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedPauli {
    sign: PauliSign,
    basis: PauliBasis,
}

impl SignedPauli {
    const fn new(sign: PauliSign, basis: PauliBasis) -> Self {
        Self { sign, basis }
    }

    fn try_from_phase_and_basis(phase: PauliPhase, basis: PauliBasis) -> StabilizerResult<Self> {
        if phase.is_imaginary() {
            Err(StabilizerError::ImaginaryProduct { phase })
        } else {
            Ok(Self {
                sign: phase.sign(),
                basis,
            })
        }
    }

    fn multiply(self, rhs: Self) -> (PauliBasis, PauliPhase) {
        let (basis, basis_phase) = self.basis.multiply(rhs.basis);
        let phase = self
            .sign
            .to_phase()
            .multiply(rhs.sign.to_phase())
            .multiply(basis_phase);
        (basis, phase)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleQubitClifford {
    I,
    X,
    Y,
    Z,
    H,
    SqrtYDag,
    Hnxz,
    SqrtY,
    S,
    Hxy,
    Hnxy,
    SDag,
    SqrtXDag,
    SqrtX,
    Hnyz,
    Hyz,
    Cxyz,
    Cxynz,
    Cnxyz,
    Cxnyz,
    Czyx,
    Cznyx,
    Cnzyx,
    Czynx,
}

const SINGLE_QUBIT_CLIFFORDS: [SingleQubitClifford; 24] = [
    SingleQubitClifford::I,
    SingleQubitClifford::X,
    SingleQubitClifford::Y,
    SingleQubitClifford::Z,
    SingleQubitClifford::H,
    SingleQubitClifford::SqrtYDag,
    SingleQubitClifford::Hnxz,
    SingleQubitClifford::SqrtY,
    SingleQubitClifford::S,
    SingleQubitClifford::Hxy,
    SingleQubitClifford::Hnxy,
    SingleQubitClifford::SDag,
    SingleQubitClifford::SqrtXDag,
    SingleQubitClifford::SqrtX,
    SingleQubitClifford::Hnyz,
    SingleQubitClifford::Hyz,
    SingleQubitClifford::Cxyz,
    SingleQubitClifford::Cxynz,
    SingleQubitClifford::Cnxyz,
    SingleQubitClifford::Cxnyz,
    SingleQubitClifford::Czyx,
    SingleQubitClifford::Cznyx,
    SingleQubitClifford::Cnzyx,
    SingleQubitClifford::Czynx,
];

impl SingleQubitClifford {
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        SINGLE_QUBIT_CLIFFORDS.into_iter()
    }

    pub fn from_gate(gate: Gate) -> StabilizerResult<Self> {
        match gate.canonical_name() {
            "I" => Ok(Self::I),
            "X" => Ok(Self::X),
            "Y" => Ok(Self::Y),
            "Z" => Ok(Self::Z),
            "H" => Ok(Self::H),
            "SQRT_Y_DAG" => Ok(Self::SqrtYDag),
            "H_NXZ" => Ok(Self::Hnxz),
            "SQRT_Y" => Ok(Self::SqrtY),
            "S" => Ok(Self::S),
            "H_XY" => Ok(Self::Hxy),
            "H_NXY" => Ok(Self::Hnxy),
            "S_DAG" => Ok(Self::SDag),
            "SQRT_X_DAG" => Ok(Self::SqrtXDag),
            "SQRT_X" => Ok(Self::SqrtX),
            "H_NYZ" => Ok(Self::Hnyz),
            "H_YZ" => Ok(Self::Hyz),
            "C_XYZ" => Ok(Self::Cxyz),
            "C_XYNZ" => Ok(Self::Cxynz),
            "C_NXYZ" => Ok(Self::Cnxyz),
            "C_XNYZ" => Ok(Self::Cxnyz),
            "C_ZYX" => Ok(Self::Czyx),
            "C_ZNYX" => Ok(Self::Cznyx),
            "C_NZYX" => Ok(Self::Cnzyx),
            "C_ZYNX" => Ok(Self::Czynx),
            _ => Err(StabilizerError::InvalidSingleQubitCliffordGate {
                gate: gate.canonical_name().to_owned(),
            }),
        }
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::I => "I",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::H => "H",
            Self::SqrtYDag => "SQRT_Y_DAG",
            Self::Hnxz => "H_NXZ",
            Self::SqrtY => "SQRT_Y",
            Self::S => "S",
            Self::Hxy => "H_XY",
            Self::Hnxy => "H_NXY",
            Self::SDag => "S_DAG",
            Self::SqrtXDag => "SQRT_X_DAG",
            Self::SqrtX => "SQRT_X",
            Self::Hnyz => "H_NYZ",
            Self::Hyz => "H_YZ",
            Self::Cxyz => "C_XYZ",
            Self::Cxynz => "C_XYNZ",
            Self::Cnxyz => "C_NXYZ",
            Self::Cxnyz => "C_XNYZ",
            Self::Czyx => "C_ZYX",
            Self::Cznyx => "C_ZNYX",
            Self::Cnzyx => "C_NZYX",
            Self::Czynx => "C_ZYNX",
        }
    }

    pub fn token(self) -> &'static str {
        match self {
            Self::I => "_I",
            Self::X => "_X",
            Self::Y => "_Y",
            Self::Z => "_Z",
            Self::H => "HI",
            Self::SqrtYDag => "HX",
            Self::Hnxz => "HY",
            Self::SqrtY => "HZ",
            Self::S => "SI",
            Self::Hxy => "SX",
            Self::Hnxy => "SY",
            Self::SDag => "SZ",
            Self::SqrtXDag => "VI",
            Self::SqrtX => "VX",
            Self::Hnyz => "VY",
            Self::Hyz => "VZ",
            Self::Cxyz => "uI",
            Self::Cxynz => "uX",
            Self::Cnxyz => "uY",
            Self::Cxnyz => "uZ",
            Self::Czyx => "dI",
            Self::Cznyx => "dX",
            Self::Cnzyx => "dY",
            Self::Czynx => "dZ",
        }
    }

    pub fn multiply(self, rhs: Self) -> StabilizerResult<Self> {
        let x_output = self.apply_signed(rhs.x_output())?;
        let z_output = self.apply_signed(rhs.z_output())?;
        Self::from_outputs(x_output, z_output)
            .ok_or(StabilizerError::InvalidSingleQubitCliffordProduct)
    }

    pub(crate) fn tableau(self) -> Tableau {
        Tableau::from_output_columns_unchecked(
            vec![signed_pauli_string(self.x_output())],
            vec![signed_pauli_string(self.z_output())],
        )
    }

    pub(crate) fn apply_basis(self, basis: PauliBasis) -> StabilizerResult<PauliBasis> {
        Ok(self
            .apply_signed(SignedPauli::new(PauliSign::Plus, basis))?
            .basis)
    }

    fn from_outputs(x_output: SignedPauli, z_output: SignedPauli) -> Option<Self> {
        let bits = [
            z_output.sign == PauliSign::Minus,
            x_output.sign == PauliSign::Minus,
            !x_output.basis.x_bit(),
            x_output.basis.z_bit(),
            z_output.basis.x_bit(),
            !z_output.basis.z_bit(),
        ];
        let index = bits.iter().enumerate().fold(0_u8, |index, (offset, bit)| {
            index | (u8::from(*bit) << offset)
        });
        Self::from_table_index(index)
    }

    fn from_table_index(index: u8) -> Option<Self> {
        match index {
            0 => Some(Self::I),
            1 => Some(Self::X),
            2 => Some(Self::Z),
            3 => Some(Self::Y),
            8 => Some(Self::S),
            9 => Some(Self::Hxy),
            10 => Some(Self::SDag),
            11 => Some(Self::Hnxy),
            16 => Some(Self::SqrtXDag),
            17 => Some(Self::SqrtX),
            18 => Some(Self::Hyz),
            19 => Some(Self::Hnyz),
            28 => Some(Self::Czyx),
            29 => Some(Self::Cznyx),
            30 => Some(Self::Czynx),
            31 => Some(Self::Cnzyx),
            56 => Some(Self::Cxyz),
            57 => Some(Self::Cxynz),
            58 => Some(Self::Cxnyz),
            59 => Some(Self::Cnxyz),
            60 => Some(Self::H),
            61 => Some(Self::SqrtYDag),
            62 => Some(Self::SqrtY),
            63 => Some(Self::Hnxz),
            _ => None,
        }
    }

    fn table_index(self) -> u8 {
        match self {
            Self::I => 0,
            Self::X => 1,
            Self::Z => 2,
            Self::Y => 3,
            Self::S => 8,
            Self::Hxy => 9,
            Self::SDag => 10,
            Self::Hnxy => 11,
            Self::SqrtXDag => 16,
            Self::SqrtX => 17,
            Self::Hyz => 18,
            Self::Hnyz => 19,
            Self::Czyx => 28,
            Self::Cznyx => 29,
            Self::Czynx => 30,
            Self::Cnzyx => 31,
            Self::Cxyz => 56,
            Self::Cxynz => 57,
            Self::Cxnyz => 58,
            Self::Cnxyz => 59,
            Self::H => 60,
            Self::SqrtYDag => 61,
            Self::SqrtY => 62,
            Self::Hnxz => 63,
        }
    }

    fn x_output(self) -> SignedPauli {
        let index = self.table_index();
        SignedPauli::new(
            sign_from_bit(index & 0b10 != 0),
            PauliBasis::from_xz(index & 0b100 == 0, index & 0b1000 != 0),
        )
    }

    fn z_output(self) -> SignedPauli {
        let index = self.table_index();
        SignedPauli::new(
            sign_from_bit(index & 0b1 != 0),
            PauliBasis::from_xz(index & 0b1_0000 != 0, index & 0b10_0000 == 0),
        )
    }

    fn y_output(self) -> StabilizerResult<SignedPauli> {
        let (basis, phase) = self.x_output().multiply(self.z_output());
        SignedPauli::try_from_phase_and_basis(PauliPhase::PlusI.multiply(phase), basis)
    }

    fn apply_signed(self, input: SignedPauli) -> StabilizerResult<SignedPauli> {
        let output = match input.basis {
            PauliBasis::I => SignedPauli::new(PauliSign::Plus, PauliBasis::I),
            PauliBasis::X => self.x_output(),
            PauliBasis::Y => self.y_output()?,
            PauliBasis::Z => self.z_output(),
        };
        let phase = input.sign.to_phase().multiply(output.sign.to_phase());
        SignedPauli::try_from_phase_and_basis(phase, output.basis)
    }
}

impl TryFrom<Gate> for SingleQubitClifford {
    type Error = StabilizerError;

    fn try_from(value: Gate) -> Result<Self, Self::Error> {
        Self::from_gate(value)
    }
}

impl Display for SingleQubitClifford {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.token())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CliffordString {
    gates: Vec<SingleQubitClifford>,
}

impl CliffordString {
    pub fn identity(num_qubits: usize) -> Self {
        Self {
            gates: vec![SingleQubitClifford::I; num_qubits],
        }
    }

    pub fn from_gates(gates: impl IntoIterator<Item = SingleQubitClifford>) -> Self {
        Self {
            gates: gates.into_iter().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.gates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.gates.is_empty()
    }

    pub fn gate_at(&self, index: usize) -> Option<SingleQubitClifford> {
        self.gates.get(index).copied()
    }

    pub fn set_gate_at(&mut self, index: usize, gate: SingleQubitClifford) -> StabilizerResult<()> {
        let len = self.len();
        let target = self
            .gates
            .get_mut(index)
            .ok_or(StabilizerError::CliffordIndexOutOfRange { index, len })?;
        *target = gate;
        Ok(())
    }

    pub fn concat(&self, rhs: &Self) -> StabilizerResult<Self> {
        let new_len = self
            .len()
            .checked_add(rhs.len())
            .ok_or(StabilizerError::LengthMismatch {
                left: self.len(),
                right: rhs.len(),
            })?;
        let mut gates = Vec::with_capacity(new_len);
        gates.extend_from_slice(&self.gates);
        gates.extend_from_slice(&rhs.gates);
        Ok(Self { gates })
    }

    pub fn repeat(&self, repetitions: usize) -> StabilizerResult<Self> {
        let new_len =
            self.len()
                .checked_mul(repetitions)
                .ok_or(StabilizerError::LengthMismatch {
                    left: self.len(),
                    right: repetitions,
                })?;
        let mut gates = Vec::with_capacity(new_len);
        for _ in 0..repetitions {
            gates.extend_from_slice(&self.gates);
        }
        Ok(Self { gates })
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<Self> {
        let len = self.len().max(rhs.len());
        let mut gates = Vec::with_capacity(len);
        for index in 0..len {
            let left = self.gate_at(index).unwrap_or(SingleQubitClifford::I);
            let right = rhs.gate_at(index).unwrap_or(SingleQubitClifford::I);
            gates.push(left.multiply(right)?);
        }
        Ok(Self { gates })
    }
}

impl Display for CliffordString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for (index, gate) in self.gates.iter().enumerate() {
            if index > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{gate}")?;
        }
        Ok(())
    }
}

fn sign_from_bit(negative: bool) -> PauliSign {
    if negative {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}

fn signed_pauli_string(pauli: SignedPauli) -> PauliString {
    PauliString::from_bases(pauli.sign, [pauli.basis])
}
