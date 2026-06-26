use std::fmt::{Display, Formatter};
use std::str::FromStr;

use thiserror::Error;

use crate::{BitError, BitVec, Gate};

pub type StabilizerResult<T> = Result<T, StabilizerError>;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum StabilizerError {
    #[error(transparent)]
    Bit(#[from] BitError),

    #[error("Pauli string length mismatch: left={left} right={right}")]
    LengthMismatch { left: usize, right: usize },

    #[error("unrecognized Pauli character {character:?} at offset {offset}")]
    InvalidPauliCharacter { character: char, offset: usize },

    #[error("invalid sparse Pauli string shorthand {text:?}")]
    InvalidSparsePauliString { text: String },

    #[error("Pauli product has imaginary phase {phase}")]
    ImaginaryProduct { phase: PauliPhase },

    #[error("gate {gate} is not a single-qubit Clifford gate")]
    InvalidSingleQubitCliffordGate { gate: String },

    #[error("Clifford index {index} is outside length {len}")]
    CliffordIndexOutOfRange { index: usize, len: usize },

    #[error("invalid single-qubit Clifford product")]
    InvalidSingleQubitCliffordProduct,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauliSign {
    Plus,
    Minus,
}

impl PauliSign {
    pub fn is_negative(self) -> bool {
        matches!(self, Self::Minus)
    }

    fn to_phase(self) -> PauliPhase {
        match self {
            Self::Plus => PauliPhase::Plus,
            Self::Minus => PauliPhase::Minus,
        }
    }
}

impl Display for PauliSign {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plus => f.write_str("+"),
            Self::Minus => f.write_str("-"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauliPhase {
    Plus,
    PlusI,
    Minus,
    MinusI,
}

impl PauliPhase {
    pub fn is_real(self) -> bool {
        matches!(self, Self::Plus | Self::Minus)
    }

    pub fn is_imaginary(self) -> bool {
        !self.is_real()
    }

    pub fn is_negative(self) -> bool {
        matches!(self, Self::Minus | Self::MinusI)
    }

    pub fn sign(self) -> PauliSign {
        match self {
            Self::Plus | Self::PlusI => PauliSign::Plus,
            Self::Minus | Self::MinusI => PauliSign::Minus,
        }
    }

    fn from_sign_and_imaginary(sign: PauliSign, imaginary: bool) -> Self {
        match (sign, imaginary) {
            (PauliSign::Plus, false) => Self::Plus,
            (PauliSign::Plus, true) => Self::PlusI,
            (PauliSign::Minus, false) => Self::Minus,
            (PauliSign::Minus, true) => Self::MinusI,
        }
    }

    fn exponent(self) -> u8 {
        match self {
            Self::Plus => 0,
            Self::PlusI => 1,
            Self::Minus => 2,
            Self::MinusI => 3,
        }
    }

    fn from_exponent(exponent: u8) -> Self {
        match exponent & 3 {
            0 => Self::Plus,
            1 => Self::PlusI,
            2 => Self::Minus,
            _ => Self::MinusI,
        }
    }

    fn multiply(self, rhs: Self) -> Self {
        Self::from_exponent(self.exponent().wrapping_add(rhs.exponent()))
    }
}

impl Display for PauliPhase {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plus => f.write_str("+"),
            Self::PlusI => f.write_str("+i"),
            Self::Minus => f.write_str("-"),
            Self::MinusI => f.write_str("-i"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauliBasis {
    I,
    X,
    Y,
    Z,
}

impl PauliBasis {
    pub fn from_xz(x: bool, z: bool) -> Self {
        match (x, z) {
            (false, false) => Self::I,
            (true, false) => Self::X,
            (true, true) => Self::Y,
            (false, true) => Self::Z,
        }
    }

    pub fn x_bit(self) -> bool {
        matches!(self, Self::X | Self::Y)
    }

    pub fn z_bit(self) -> bool {
        matches!(self, Self::Y | Self::Z)
    }

    pub fn log_i_scalar_byproduct(self, rhs: Self) -> u8 {
        match (self, rhs) {
            (Self::X, Self::Y) | (Self::Y, Self::Z) | (Self::Z, Self::X) => 1,
            (Self::X, Self::Z) | (Self::Y, Self::X) | (Self::Z, Self::Y) => 3,
            _ => 0,
        }
    }

    pub fn multiply(self, rhs: Self) -> (Self, PauliPhase) {
        let basis = Self::from_xz(self.x_bit() ^ rhs.x_bit(), self.z_bit() ^ rhs.z_bit());
        let phase = PauliPhase::from_exponent(self.log_i_scalar_byproduct(rhs));
        (basis, phase)
    }

    fn from_dense_char(
        character: char,
        offset: usize,
        allow_lowercase: bool,
    ) -> StabilizerResult<Self> {
        match character {
            'I' | '_' => Ok(Self::I),
            'X' => Ok(Self::X),
            'Y' => Ok(Self::Y),
            'Z' => Ok(Self::Z),
            'x' if allow_lowercase => Ok(Self::X),
            'y' if allow_lowercase => Ok(Self::Y),
            'z' if allow_lowercase => Ok(Self::Z),
            _ => Err(StabilizerError::InvalidPauliCharacter { character, offset }),
        }
    }

    fn dense_char(self) -> char {
        match self {
            Self::I => '_',
            Self::X => 'X',
            Self::Y => 'Y',
            Self::Z => 'Z',
        }
    }

    fn sparse_char(self) -> Option<char> {
        match self {
            Self::I => None,
            Self::X => Some('X'),
            Self::Y => Some('Y'),
            Self::Z => Some('Z'),
        }
    }
}

impl Display for PauliBasis {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::I => f.write_str("_"),
            Self::X => f.write_str("X"),
            Self::Y => f.write_str("Y"),
            Self::Z => f.write_str("Z"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauliString {
    sign: PauliSign,
    xs: BitVec,
    zs: BitVec,
}

impl PauliString {
    pub fn identity(num_qubits: usize) -> Self {
        Self {
            sign: PauliSign::Plus,
            xs: BitVec::zeros(num_qubits),
            zs: BitVec::zeros(num_qubits),
        }
    }

    pub fn from_bases(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> StabilizerResult<Self> {
        let bases = bases.into_iter().collect::<Vec<_>>();
        let mut result = Self::identity(bases.len());
        result.sign = sign;
        for (index, basis) in bases.into_iter().enumerate() {
            result.set(index, basis)?;
        }
        Ok(result)
    }

    pub fn sign(&self) -> PauliSign {
        self.sign
    }

    pub fn phase(&self) -> PauliPhase {
        self.sign.to_phase()
    }

    pub fn len(&self) -> usize {
        self.xs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<PauliBasis> {
        Some(PauliBasis::from_xz(
            self.xs.get(index)?,
            self.zs.get(index)?,
        ))
    }

    pub fn set(&mut self, index: usize, basis: PauliBasis) -> StabilizerResult<()> {
        self.xs.set(index, basis.x_bit())?;
        self.zs.set(index, basis.z_bit())?;
        Ok(())
    }

    pub fn x_bits(&self) -> &[u64] {
        self.xs.words()
    }

    pub fn z_bits(&self) -> &[u64] {
        self.zs.words()
    }

    pub fn weight(&self) -> usize {
        self.xs
            .words()
            .iter()
            .zip(self.zs.words())
            .map(|(x_word, z_word)| (x_word | z_word).count_ones() as usize)
            .sum()
    }

    pub fn has_no_pauli_terms(&self) -> bool {
        self.weight() == 0
    }

    pub fn intersects(&self, rhs: &Self) -> StabilizerResult<bool> {
        Ok(self
            .xs
            .words()
            .iter()
            .zip(self.zs.words())
            .zip(rhs.xs.words().iter().zip(rhs.zs.words()))
            .any(|((left_x, left_z), (right_x, right_z))| {
                (left_x | left_z) & (right_x | right_z) != 0
            }))
    }

    pub fn commutes(&self, rhs: &Self) -> StabilizerResult<bool> {
        Ok(self.symplectic_product(rhs) == 0)
    }

    pub fn log_i_scalar_byproduct(&self, rhs: &Self) -> StabilizerResult<u8> {
        let mut log_i = 0_u8;
        for index in 0..self.len().max(rhs.len()) {
            let left = self.get_or_identity(index);
            let right = rhs.get_or_identity(index);
            log_i = log_i.wrapping_add(left.log_i_scalar_byproduct(right));
        }
        Ok(log_i & 3)
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<FlexPauliString> {
        let len = self.len().max(rhs.len());
        let mut bases = Vec::with_capacity(len);
        let mut phase = self.phase().multiply(rhs.phase());
        for index in 0..len {
            let left = self.get_or_identity(index);
            let right = rhs.get_or_identity(index);
            let (basis, basis_phase) = left.multiply(right);
            bases.push(basis);
            phase = phase.multiply(basis_phase);
        }
        FlexPauliString::from_phase_and_bases(phase, bases)
    }

    pub fn multiply_real(&self, rhs: &Self) -> StabilizerResult<Self> {
        let product = self.multiply(rhs)?;
        product.try_into_real()
    }

    pub fn sparse_string(&self) -> String {
        let mut out = self.sign.to_string();
        let mut has_term = false;
        for index in 0..self.len() {
            if let Some(character) = self.get_or_identity(index).sparse_char() {
                if has_term {
                    out.push('*');
                }
                out.push(character);
                out.push_str(&index.to_string());
                has_term = true;
            }
        }
        if !has_term {
            out.push('I');
        }
        out
    }

    pub fn active_terms(&self) -> impl Iterator<Item = (usize, PauliBasis)> + '_ {
        (0..self.len()).filter_map(|index| {
            let basis = self.get_or_identity(index);
            if basis == PauliBasis::I {
                None
            } else {
                Some((index, basis))
            }
        })
    }

    fn parse_dense(text: &str, allow_lowercase: bool) -> StabilizerResult<Self> {
        let (sign, body) = parse_real_prefix(text);
        let mut result = Self::identity(body.chars().count());
        result.sign = sign;
        for (offset, character) in body.chars().enumerate() {
            result.set(
                offset,
                PauliBasis::from_dense_char(character, offset, allow_lowercase)?,
            )?;
        }
        Ok(result)
    }

    fn from_parts(sign: PauliSign, xs: BitVec, zs: BitVec) -> StabilizerResult<Self> {
        if xs.len() != zs.len() {
            return Err(StabilizerError::LengthMismatch {
                left: xs.len(),
                right: zs.len(),
            });
        }
        Ok(Self { sign, xs, zs })
    }

    fn ensure_len(&mut self, len: usize) {
        if len <= self.len() {
            return;
        }
        self.xs = BitVec::from_words_truncated(len, self.xs.words().to_vec());
        self.zs = BitVec::from_words_truncated(len, self.zs.words().to_vec());
    }

    fn get_or_identity(&self, index: usize) -> PauliBasis {
        self.get(index).unwrap_or(PauliBasis::I)
    }

    fn get_or_error(&self, index: usize) -> StabilizerResult<PauliBasis> {
        self.get(index)
            .ok_or(StabilizerError::Bit(BitError::BitIndexOutOfRange {
                index,
                len: self.len(),
            }))
    }

    fn symplectic_product(&self, rhs: &Self) -> u32 {
        self.xs
            .words()
            .iter()
            .zip(self.zs.words())
            .zip(rhs.xs.words().iter().zip(rhs.zs.words()))
            .map(|((left_x, left_z), (right_x, right_z))| {
                ((left_x & right_z) ^ (left_z & right_x)).count_ones()
            })
            .fold(0, |parity, word_count| parity ^ (word_count & 1))
            & 1
    }
}

impl Display for PauliString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.sign)?;
        for index in 0..self.len() {
            write!(f, "{}", self.get_or_identity(index).dense_char())?;
        }
        Ok(())
    }
}

impl FromStr for PauliString {
    type Err = StabilizerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::parse_dense(text, false)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlexPauliString {
    value: PauliString,
    imaginary: bool,
}

impl FlexPauliString {
    pub fn identity(num_qubits: usize) -> Self {
        Self {
            value: PauliString::identity(num_qubits),
            imaginary: false,
        }
    }

    pub fn value(&self) -> &PauliString {
        &self.value
    }

    pub fn phase(&self) -> PauliPhase {
        PauliPhase::from_sign_and_imaginary(self.value.sign(), self.imaginary)
    }

    pub fn len(&self) -> usize {
        self.value.len()
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<PauliBasis> {
        self.value.get(index)
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<Self> {
        let product = self.value.multiply(&rhs.value)?;
        let phase = product
            .phase()
            .multiply(self.extra_imaginary_phase())
            .multiply(rhs.extra_imaginary_phase());
        Self::from_phase_and_bits(phase, product.value.xs, product.value.zs)
    }

    pub fn try_into_real(self) -> StabilizerResult<PauliString> {
        if self.imaginary {
            Err(StabilizerError::ImaginaryProduct {
                phase: self.phase(),
            })
        } else {
            Ok(self.value)
        }
    }

    pub fn from_phase_and_bases(
        phase: PauliPhase,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> StabilizerResult<Self> {
        let imaginary = phase.is_imaginary();
        let value = PauliString::from_bases(phase.sign(), bases)?;
        Ok(Self { value, imaginary })
    }

    fn from_phase_and_bits(phase: PauliPhase, xs: BitVec, zs: BitVec) -> StabilizerResult<Self> {
        let value = PauliString::from_parts(phase.sign(), xs, zs)?;
        Ok(Self {
            value,
            imaginary: phase.is_imaginary(),
        })
    }

    fn extra_imaginary_phase(&self) -> PauliPhase {
        if self.imaginary {
            PauliPhase::PlusI
        } else {
            PauliPhase::Plus
        }
    }

    fn right_mul_basis(&mut self, index: usize, basis: PauliBasis) -> StabilizerResult<()> {
        if basis == PauliBasis::I {
            return Ok(());
        }
        self.value.ensure_len(index.saturating_add(1));
        let old_basis = self.value.get_or_error(index)?;
        let (new_basis, phase) = old_basis.multiply(basis);
        let phase = self.phase().multiply(phase);
        self.imaginary = phase.is_imaginary();
        self.value.sign = phase.sign();
        self.value.set(index, new_basis)
    }

    fn parse_sparse_body(
        phase: PauliPhase,
        body: &str,
        num_qubits: usize,
        original_text: &str,
    ) -> StabilizerResult<Self> {
        let mut result = Self::from_phase_and_bases(phase, vec![PauliBasis::I; num_qubits])?;
        let mut current_basis = None;
        let mut current_index = None;

        for character in body.chars() {
            match character {
                '*' => {
                    flush_sparse_term(
                        &mut result,
                        &mut current_basis,
                        &mut current_index,
                        original_text,
                    )?;
                }
                'I' | 'X' | 'Y' | 'Z' | 'x' | 'y' | 'z' => {
                    if current_basis.is_some() {
                        return Err(StabilizerError::InvalidSparsePauliString {
                            text: original_text.to_owned(),
                        });
                    }
                    current_basis = Some(PauliBasis::from_dense_char(character, 0, true)?);
                }
                '0'..='9' => {
                    if current_basis.is_none() {
                        return Err(StabilizerError::InvalidSparsePauliString {
                            text: original_text.to_owned(),
                        });
                    }
                    let digit = character.to_digit(10).ok_or_else(|| {
                        StabilizerError::InvalidSparsePauliString {
                            text: original_text.to_owned(),
                        }
                    })?;
                    let next = current_index
                        .unwrap_or(0_usize)
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(digit as usize))
                        .ok_or_else(|| StabilizerError::InvalidSparsePauliString {
                            text: original_text.to_owned(),
                        })?;
                    current_index = Some(next);
                }
                _ => {
                    return Err(StabilizerError::InvalidSparsePauliString {
                        text: original_text.to_owned(),
                    });
                }
            }
        }
        flush_sparse_term(
            &mut result,
            &mut current_basis,
            &mut current_index,
            original_text,
        )?;
        Ok(result)
    }
}

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

impl Display for FlexPauliString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.phase())?;
        for index in 0..self.len() {
            write!(f, "{}", self.value.get_or_identity(index).dense_char())?;
        }
        Ok(())
    }
}

impl FromStr for FlexPauliString {
    type Err = StabilizerError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (phase, body) = parse_flex_prefix(text);
        let sparse_size = sparse_size_if_sparse(body)?;
        if sparse_size > 0 {
            Self::parse_sparse_body(phase, body, sparse_size, text)
        } else {
            let mut value = PauliString::parse_dense(body, true)?;
            value.sign = phase.sign();
            Ok(Self {
                value,
                imaginary: phase.is_imaginary(),
            })
        }
    }
}

fn parse_real_prefix(text: &str) -> (PauliSign, &str) {
    if let Some(rest) = text.strip_prefix('-') {
        (PauliSign::Minus, rest)
    } else if let Some(rest) = text.strip_prefix('+') {
        (PauliSign::Plus, rest)
    } else {
        (PauliSign::Plus, text)
    }
}

fn sign_from_bit(negative: bool) -> PauliSign {
    if negative {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}

fn parse_flex_prefix(text: &str) -> (PauliPhase, &str) {
    let (sign, body) = parse_real_prefix(text);
    if let Some(rest) = body.strip_prefix('i') {
        (PauliPhase::from_sign_and_imaginary(sign, true), rest)
    } else {
        (PauliPhase::from_sign_and_imaginary(sign, false), body)
    }
}

fn sparse_size_if_sparse(text: &str) -> StabilizerResult<usize> {
    let mut current_index = None;
    let mut num_qubits = 0_usize;
    for character in text.chars() {
        if let Some(digit) = character.to_digit(10) {
            let next = current_index
                .unwrap_or(0_usize)
                .checked_mul(10)
                .and_then(|value| value.checked_add(digit as usize))
                .ok_or_else(|| StabilizerError::InvalidSparsePauliString {
                    text: text.to_owned(),
                })?;
            current_index = Some(next);
        } else if let Some(index) = current_index.take() {
            num_qubits = num_qubits.max(index.saturating_add(1));
        }
    }
    if let Some(index) = current_index {
        num_qubits = num_qubits.max(index.saturating_add(1));
    }
    Ok(num_qubits)
}

fn flush_sparse_term(
    result: &mut FlexPauliString,
    basis: &mut Option<PauliBasis>,
    index: &mut Option<usize>,
    original_text: &str,
) -> StabilizerResult<()> {
    let Some(basis) = basis.take() else {
        return Err(StabilizerError::InvalidSparsePauliString {
            text: original_text.to_owned(),
        });
    };
    let Some(index) = index.take() else {
        return Err(StabilizerError::InvalidSparsePauliString {
            text: original_text.to_owned(),
        });
    };
    if index >= result.len() {
        return Err(StabilizerError::InvalidSparsePauliString {
            text: original_text.to_owned(),
        });
    }
    result.right_mul_basis(index, basis)
}
