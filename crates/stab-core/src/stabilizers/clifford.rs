use std::fmt::{Display, Formatter};

use rand::{Rng, RngExt as _};

use super::{
    PauliBasis, PauliPhase, PauliSign, PauliString, StabilizerError, StabilizerResult, Tableau,
};
use crate::bits::{CliffordPlanes, CliffordPlanesMut, clifford_right_multiply_words};
use crate::{BitError, BitVec, Gate};

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

    /// Samples one of the 24 single-qubit Clifford gates uniformly using the caller-owned RNG.
    pub fn random<R>(rng: &mut R) -> Self
    where
        R: Rng + ?Sized,
    {
        let index = rng.random_range(0..SINGLE_QUBIT_CLIFFORDS.len());
        SINGLE_QUBIT_CLIFFORDS
            .get(index)
            .copied()
            .unwrap_or(Self::I)
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

    pub(crate) fn inverse(self) -> StabilizerResult<Self> {
        for candidate in Self::all() {
            if self.multiply(candidate)? == Self::I && candidate.multiply(self)? == Self::I {
                return Ok(candidate);
            }
        }
        Err(StabilizerError::InvalidSingleQubitCliffordProduct)
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
    z_signs: BitVec,
    x_signs: BitVec,
    inv_x2x: BitVec,
    x2z: BitVec,
    z2x: BitVec,
    inv_z2z: BitVec,
    non_identity_count: usize,
}

impl CliffordString {
    /// Creates an identity string within the [`crate::StabilizerResource::CliffordQubits`] limit.
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        super::StabilizerResource::CliffordQubits.ensure(num_qubits)?;
        Ok(Self::identity_unchecked(num_qubits))
    }

    pub(crate) fn identity_unchecked(num_qubits: usize) -> Self {
        debug_assert!(num_qubits <= super::StabilizerResource::CliffordQubits.limit());
        Self {
            z_signs: BitVec::zeros(num_qubits),
            x_signs: BitVec::zeros(num_qubits),
            inv_x2x: BitVec::zeros(num_qubits),
            x2z: BitVec::zeros(num_qubits),
            z2x: BitVec::zeros(num_qubits),
            inv_z2z: BitVec::zeros(num_qubits),
            non_identity_count: 0,
        }
    }

    /// Collects Clifford gates, rejecting the first item beyond the Clifford-qubit limit.
    pub fn from_gates(
        gates: impl IntoIterator<Item = SingleQubitClifford>,
    ) -> StabilizerResult<Self> {
        let limit = super::StabilizerResource::CliffordQubits.limit();
        let mut collected = Vec::new();
        for gate in gates {
            if collected.len() == limit {
                return Err(StabilizerError::ResourceLimitExceeded {
                    resource: super::StabilizerResource::CliffordQubits,
                    requested: limit.saturating_add(1),
                    limit,
                });
            }
            collected.push(gate);
        }
        Ok(Self::from_gates_within_limit(collected))
    }

    fn from_gates_within_limit(gates: Vec<SingleQubitClifford>) -> Self {
        debug_assert!(gates.len() <= super::StabilizerResource::CliffordQubits.limit());
        let table_indices = gates
            .into_iter()
            .map(SingleQubitClifford::table_index)
            .collect::<Vec<_>>();
        Self::from_table_indices(&table_indices)
    }

    fn from_table_indices(table_indices: &[u8]) -> Self {
        let plane = |bit| BitVec::from_bits(table_indices.iter().map(move |code| code & bit != 0));
        Self {
            z_signs: plane(1 << 0),
            x_signs: plane(1 << 1),
            inv_x2x: plane(1 << 2),
            x2z: plane(1 << 3),
            z2x: plane(1 << 4),
            inv_z2z: plane(1 << 5),
            non_identity_count: table_indices.iter().filter(|code| **code != 0).count(),
        }
    }

    /// Creates a random string of independent single-qubit Clifford gates.
    ///
    /// Passing a seeded `rand` RNG gives deterministic Stab output. The generated stream is not
    /// intended to match Stim's C++ RNG stream.
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        let mut result = Self::identity(num_qubits)?;
        result.randomize(rng);
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.z_signs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.z_signs.is_empty()
    }

    pub fn gate_at(&self, index: usize) -> Option<SingleQubitClifford> {
        if index >= self.len() {
            return None;
        }
        let table_index = u8::from(self.z_signs.get(index)?)
            | (u8::from(self.x_signs.get(index)?) << 1)
            | (u8::from(self.inv_x2x.get(index)?) << 2)
            | (u8::from(self.x2z.get(index)?) << 3)
            | (u8::from(self.z2x.get(index)?) << 4)
            | (u8::from(self.inv_z2z.get(index)?) << 5);
        SingleQubitClifford::from_table_index(table_index)
    }

    pub fn set_gate_at(&mut self, index: usize, gate: SingleQubitClifford) -> StabilizerResult<()> {
        let len = self.len();
        let old_gate = self
            .gate_at(index)
            .ok_or(StabilizerError::CliffordIndexOutOfRange { index, len })?;
        let non_identity_count = match (
            old_gate == SingleQubitClifford::I,
            gate == SingleQubitClifford::I,
        ) {
            (true, false) => self.non_identity_count + 1,
            (false, true) => self
                .non_identity_count
                .checked_sub(1)
                .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?,
            _ => self.non_identity_count,
        };
        self.write_gate_bits(index, gate)?;
        self.non_identity_count = non_identity_count;
        Ok(())
    }

    /// Replaces every position with a random single-qubit Clifford gate.
    pub fn randomize<R>(&mut self, rng: &mut R)
    where
        R: Rng + ?Sized,
    {
        let mut non_identity_count = 0_usize;
        let mut remaining = self.len();
        let words = self
            .z_signs
            .words_mut()
            .iter_mut()
            .zip(self.x_signs.words_mut())
            .zip(self.inv_x2x.words_mut())
            .zip(self.x2z.words_mut())
            .zip(self.z2x.words_mut())
            .zip(self.inv_z2z.words_mut());
        for (((((z_signs, x_signs), inv_x2x), x2z), z2x), inv_z2z) in words {
            let chunk_len = remaining.min(u64::BITS as usize);
            let mut packed = [0_u64; 6];
            for bit_index in 0..chunk_len {
                let code = SingleQubitClifford::random(rng).table_index();
                non_identity_count += usize::from(code != 0);
                let mask = 1_u64 << bit_index;
                packed[0] |= mask * u64::from(code & (1 << 0) != 0);
                packed[1] |= mask * u64::from(code & (1 << 1) != 0);
                packed[2] |= mask * u64::from(code & (1 << 2) != 0);
                packed[3] |= mask * u64::from(code & (1 << 3) != 0);
                packed[4] |= mask * u64::from(code & (1 << 4) != 0);
                packed[5] |= mask * u64::from(code & (1 << 5) != 0);
            }
            [*z_signs, *x_signs, *inv_x2x, *x2z, *z2x, *inv_z2z] = packed;
            remaining -= chunk_len;
        }
        debug_assert_eq!(remaining, 0);
        self.non_identity_count = non_identity_count;
    }

    pub fn concat(&self, rhs: &Self) -> StabilizerResult<Self> {
        let new_len = self
            .len()
            .checked_add(rhs.len())
            .ok_or(StabilizerError::LengthMismatch {
                left: self.len(),
                right: rhs.len(),
            })?;
        super::StabilizerResource::CliffordQubits.ensure(new_len)?;
        let mut table_indices = self.table_indices()?;
        table_indices.reserve(rhs.len());
        table_indices.extend(rhs.table_indices()?);
        Ok(Self::from_table_indices(&table_indices))
    }

    /// Repeats this string after checking multiplication overflow and the resulting size.
    pub fn repeat(&self, repetitions: usize) -> StabilizerResult<Self> {
        if repetitions == 0 || self.is_empty() {
            return Ok(Self::identity_unchecked(0));
        }
        let new_len =
            self.len()
                .checked_mul(repetitions)
                .ok_or(StabilizerError::ResourceSizeOverflow {
                    resource: super::StabilizerResource::CliffordQubits,
                    item_count: self.len(),
                    repetitions,
                })?;
        super::StabilizerResource::CliffordQubits.ensure(new_len)?;
        let source = self.table_indices()?;
        let table_indices = source
            .iter()
            .copied()
            .cycle()
            .take(new_len)
            .collect::<Vec<_>>();
        Ok(Self::from_table_indices(&table_indices))
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<Self> {
        let mut result = self.clone();
        result.right_multiply_in_place(rhs)?;
        Ok(result)
    }

    /// Right-multiplies `rhs` into this Clifford string in place.
    ///
    /// If `rhs` is longer than this string, this string is extended with identity rotations before
    /// multiplication.
    #[inline]
    pub fn right_multiply_in_place(&mut self, rhs: &Self) -> StabilizerResult<()> {
        self.ensure_len(rhs.len())?;
        if rhs.non_identity_count == 0 {
            return Ok(());
        }
        let word_count = rhs.z_signs.word_count();
        let tail_non_identity_count = self.tail_non_identity_count(word_count)?;
        let left = CliffordPlanesMut {
            z_signs: prefix_mut(self.z_signs.words_mut(), word_count)?,
            x_signs: prefix_mut(self.x_signs.words_mut(), word_count)?,
            inv_x2x: prefix_mut(self.inv_x2x.words_mut(), word_count)?,
            x2z: prefix_mut(self.x2z.words_mut(), word_count)?,
            z2x: prefix_mut(self.z2x.words_mut(), word_count)?,
            inv_z2z: prefix_mut(self.inv_z2z.words_mut(), word_count)?,
        };
        let right = CliffordPlanes {
            z_signs: rhs.z_signs.words(),
            x_signs: rhs.x_signs.words(),
            inv_x2x: rhs.inv_x2x.words(),
            x2z: rhs.x2z.words(),
            z2x: rhs.z2x.words(),
            inv_z2z: rhs.inv_z2z.words(),
        };
        self.non_identity_count = clifford_right_multiply_words(left, right)
            .checked_add(tail_non_identity_count)
            .ok_or(StabilizerError::InconsistentCliffordStringMetadata)?;
        Ok(())
    }

    fn ensure_len(&mut self, len: usize) -> StabilizerResult<()> {
        if len > self.len() {
            super::StabilizerResource::CliffordQubits.ensure(len)?;
            self.z_signs.resize_zeros(len);
            self.x_signs.resize_zeros(len);
            self.inv_x2x.resize_zeros(len);
            self.x2z.resize_zeros(len);
            self.z2x.resize_zeros(len);
            self.inv_z2z.resize_zeros(len);
        }
        Ok(())
    }

    fn write_gate_bits(&mut self, index: usize, gate: SingleQubitClifford) -> StabilizerResult<()> {
        let table_index = gate.table_index();
        self.z_signs.set(index, table_index & (1 << 0) != 0)?;
        self.x_signs.set(index, table_index & (1 << 1) != 0)?;
        self.inv_x2x.set(index, table_index & (1 << 2) != 0)?;
        self.x2z.set(index, table_index & (1 << 3) != 0)?;
        self.z2x.set(index, table_index & (1 << 4) != 0)?;
        self.inv_z2z.set(index, table_index & (1 << 5) != 0)?;
        Ok(())
    }

    fn table_indices(&self) -> StabilizerResult<Vec<u8>> {
        (0..self.len())
            .map(|index| {
                self.gate_at(index)
                    .map(SingleQubitClifford::table_index)
                    .ok_or(StabilizerError::InvalidSingleQubitCliffordProduct)
            })
            .collect()
    }

    fn tail_non_identity_count(&self, word_start: usize) -> StabilizerResult<usize> {
        let planes = [
            self.z_signs.words(),
            self.x_signs.words(),
            self.inv_x2x.words(),
            self.x2z.words(),
            self.z2x.words(),
            self.inv_z2z.words(),
        ];
        let word_count = self.z_signs.word_count();
        let mut count = 0_usize;
        for word_index in word_start..word_count {
            let mut combined = 0_u64;
            for plane in planes {
                combined |= *plane.get(word_index).ok_or(BitError::LengthMismatch {
                    left: word_index * u64::BITS as usize,
                    right: plane.len() * u64::BITS as usize,
                })?;
            }
            count += combined.count_ones() as usize;
        }
        Ok(count)
    }
}

fn prefix_mut(words: &mut [u64], len: usize) -> StabilizerResult<&mut [u64]> {
    let word_len = words.len();
    words.get_mut(..len).ok_or_else(|| {
        StabilizerError::Bit(BitError::LengthMismatch {
            left: word_len * u64::BITS as usize,
            right: len * u64::BITS as usize,
        })
    })
}

impl Display for CliffordString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for index in 0..self.len() {
            if index > 0 {
                f.write_str(" ")?;
            }
            let gate = self.gate_at(index).ok_or(std::fmt::Error)?;
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
    PauliString::from_bases_unchecked(pauli.sign, [pauli.basis])
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "stabilizer algebra tests use compact literal Clifford fixtures"
    )]

    use super::*;
    use rand::SeedableRng as _;
    use rand::rngs::SmallRng;

    #[test]
    fn right_multiply_in_place_matches_per_gate_products() {
        let mut left = CliffordString::from_gates([
            SingleQubitClifford::H,
            SingleQubitClifford::S,
            SingleQubitClifford::I,
        ])
        .unwrap();
        let right = CliffordString::from_gates([
            SingleQubitClifford::S,
            SingleQubitClifford::H,
            SingleQubitClifford::SqrtX,
        ])
        .unwrap();
        let expected = CliffordString::from_gates([
            SingleQubitClifford::H
                .multiply(SingleQubitClifford::S)
                .unwrap(),
            SingleQubitClifford::S
                .multiply(SingleQubitClifford::H)
                .unwrap(),
            SingleQubitClifford::I
                .multiply(SingleQubitClifford::SqrtX)
                .unwrap(),
        ])
        .unwrap();

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left, expected);
    }

    #[test]
    fn right_multiply_in_place_extends_shorter_left_side() {
        let mut left = CliffordString::from_gates([SingleQubitClifford::H]).unwrap();
        let right =
            CliffordString::from_gates([SingleQubitClifford::I, SingleQubitClifford::S]).unwrap();

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left.gate_at(0), Some(SingleQubitClifford::H));
        assert_eq!(left.gate_at(1), Some(SingleQubitClifford::S));
    }

    #[test]
    fn right_multiply_in_place_extends_for_longer_identity_rhs() {
        let mut left = CliffordString::from_gates([SingleQubitClifford::H]).unwrap();
        let right = CliffordString::identity_unchecked(3);

        left.right_multiply_in_place(&right).unwrap();

        assert_eq!(left.len(), 3);
        assert_eq!(left.gate_at(0), Some(SingleQubitClifford::H));
        assert_eq!(left.gate_at(1), Some(SingleQubitClifford::I));
        assert_eq!(left.gate_at(2), Some(SingleQubitClifford::I));
    }

    #[test]
    fn right_multiply_in_place_preserves_left_tail_across_word_boundaries() {
        for (left_len, right_len, tail_index) in [(63, 17, 62), (65, 63, 64), (129, 65, 128)] {
            let mut left = CliffordString::identity_unchecked(left_len);
            left.set_gate_at(0, SingleQubitClifford::H).unwrap();
            left.set_gate_at(tail_index, SingleQubitClifford::S)
                .unwrap();
            let mut right = CliffordString::identity_unchecked(right_len);
            right.set_gate_at(0, SingleQubitClifford::SqrtX).unwrap();

            let expected_head = SingleQubitClifford::H
                .multiply(SingleQubitClifford::SqrtX)
                .unwrap();
            left.right_multiply_in_place(&right).unwrap();

            assert_eq!(left.gate_at(0), Some(expected_head), "left_len={left_len}");
            assert_eq!(
                left.gate_at(tail_index),
                Some(SingleQubitClifford::S),
                "left_len={left_len}"
            );
            assert_ne!(left.non_identity_count, 0, "left_len={left_len}");
        }
    }

    #[test]
    fn clearing_last_gate_restores_identity_metadata() {
        let mut right = CliffordString::identity_unchecked(257);
        right.set_gate_at(256, SingleQubitClifford::SqrtY).unwrap();
        assert_eq!(right.non_identity_count, 1);

        right.set_gate_at(256, SingleQubitClifford::I).unwrap();
        assert_eq!(right.non_identity_count, 0);

        let mut left = CliffordString::from_gates((0..257).map(|index| {
            if index % 2 == 0 {
                SingleQubitClifford::H
            } else {
                SingleQubitClifford::S
            }
        }))
        .unwrap();
        let expected = left.clone();
        left.right_multiply_in_place(&right).unwrap();
        assert_eq!(left, expected);
    }

    #[test]
    fn clearing_many_gates_updates_non_identity_metadata_in_constant_time() {
        let width = 16_384;
        let mut value =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, width)).unwrap();
        assert_eq!(value.non_identity_count, width);

        for index in 0..width {
            value.set_gate_at(index, SingleQubitClifford::I).unwrap();
            assert_eq!(value.non_identity_count, width - index - 1);
        }
        assert_eq!(value, CliffordString::identity_unchecked(width));
    }

    #[test]
    fn randomize_reuses_packed_plane_storage_without_allocating() {
        let width = 65_537;
        let mut value = CliffordString::identity_unchecked(width);
        let mut rng = SmallRng::seed_from_u64(0xc11f_f07d);

        let allocations = allocation_counter::measure(|| value.randomize(&mut rng));

        assert_eq!(allocations.count_total, 0);
        assert_eq!(allocations.bytes_total, 0);
        let observed_count = (0..width)
            .filter(|&index| value.gate_at(index) != Some(SingleQubitClifford::I))
            .count();
        assert_eq!(value.non_identity_count, observed_count);
    }
}
