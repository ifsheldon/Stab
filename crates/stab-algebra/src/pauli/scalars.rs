//! The single-qubit scalars of Pauli algebra: real signs, base-`i` phases, and Pauli bases.

use std::fmt::{Display, Formatter};

use crate::{StabilizerError, StabilizerResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PauliSign {
    Plus,
    Minus,
}

impl PauliSign {
    pub fn is_negative(self) -> bool {
        matches!(self, Self::Minus)
    }

    pub(crate) fn to_phase(self) -> PauliPhase {
        match self {
            Self::Plus => PauliPhase::Plus,
            Self::Minus => PauliPhase::Minus,
        }
    }

    pub(super) fn flipped(self) -> Self {
        match self {
            Self::Plus => Self::Minus,
            Self::Minus => Self::Plus,
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

    pub(crate) fn from_sign_and_imaginary(sign: PauliSign, imaginary: bool) -> Self {
        match (sign, imaginary) {
            (PauliSign::Plus, false) => Self::Plus,
            (PauliSign::Plus, true) => Self::PlusI,
            (PauliSign::Minus, false) => Self::Minus,
            (PauliSign::Minus, true) => Self::MinusI,
        }
    }

    /// The exponent `k` in `i^k`, in `0..4`.
    pub fn log_i(self) -> u8 {
        match self {
            Self::Plus => 0,
            Self::PlusI => 1,
            Self::Minus => 2,
            Self::MinusI => 3,
        }
    }

    /// The phase `i^log_i`; only `log_i` modulo 4 matters.
    pub fn from_log_i(log_i: u8) -> Self {
        match log_i & 3 {
            0 => Self::Plus,
            1 => Self::PlusI,
            2 => Self::Minus,
            _ => Self::MinusI,
        }
    }

    /// Multiplies two Pauli phases.
    pub fn multiply(self, rhs: Self) -> Self {
        Self::from_log_i(self.log_i().wrapping_add(rhs.log_i()))
    }
}

impl From<PauliSign> for PauliPhase {
    fn from(sign: PauliSign) -> Self {
        sign.to_phase()
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

    /// The phase `c` with `self * rhs = c * (self ^ rhs)`, such as `+i` for `X * Y = iZ`.
    pub fn log_i_scalar_byproduct(self, rhs: Self) -> PauliPhase {
        match (self, rhs) {
            (Self::X, Self::Y) | (Self::Y, Self::Z) | (Self::Z, Self::X) => PauliPhase::PlusI,
            (Self::X, Self::Z) | (Self::Y, Self::X) | (Self::Z, Self::Y) => PauliPhase::MinusI,
            _ => PauliPhase::Plus,
        }
    }

    pub fn multiply(self, rhs: Self) -> (Self, PauliPhase) {
        let basis = Self::from_xz(self.x_bit() ^ rhs.x_bit(), self.z_bit() ^ rhs.z_bit());
        (basis, self.log_i_scalar_byproduct(rhs))
    }

    pub(super) fn from_dense_char(
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

    pub(super) fn dense_char(self) -> char {
        match self {
            Self::I => '_',
            Self::X => 'X',
            Self::Y => 'Y',
            Self::Z => 'Z',
        }
    }

    pub(super) fn sparse_char(self) -> Option<char> {
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
