//! One- and two-qubit Cliffords compiled for the per-row and word-parallel update kernels.
//!
//! A local Pauli on at most two qubits is a four-bit code with qubit `i`'s X bit at bit `2i` and
//! its Z bit at bit `2i + 1`, using the Hermitian convention `Y = iXZ`. Conjugating a code by the
//! gate yields another code and a sign, so [`SmallGate::images`] is the complete signed action as
//! a sixteen-entry table. The output bits are linear in the input bits by construction, and
//! [`SmallGate::formula`] additionally expresses the sign as its algebraic normal form, a Boolean
//! polynomial in the input bits, so that the transposed appender can update 64 generator rows
//! per word operation. Some two-qubit signs need cubic monomials, so every monomial is kept.

use super::{PauliRows, Tableau};
use crate::{PauliPhase, StabilizerError, StabilizerResult};

/// Largest gate arity compiled into a lookup table.
pub(super) const MAX_SMALL_ARITY: usize = 2;

/// The signed image of one local Pauli code.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct LocalImage {
    pub(super) code: u8,
    pub(super) negative: bool,
}

/// A one- or two-qubit Clifford as its complete local action.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SmallGate {
    arity: usize,
    images: [LocalImage; 16],
}

/// A small gate's word formula packed into bits, cheap to cache and to compare.
///
/// Bit `4b + j` of `linear` says that output bit `b` includes input bit `j`, and bit `m` of
/// `sign` selects the monomial multiplying the input bits set in `m`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct PackedFormula {
    linear: u16,
    sign: u16,
}

/// Word masks that compute a small gate's action on four input bit planes at once.
///
/// Input plane `j` is local bit `j` of the code. Output bit `b` is the XOR of the input planes
/// selected by `linear[b]`, and the sign flip is the XOR of the monomials selected by `sign`,
/// indexed by the code of the input bits they multiply. Every mask is all ones or all zeros, so
/// the kernels stay branch-free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SignFormula {
    linear: [[u64; 4]; 4],
    sign: [u64; 16],
}

/// Positions whose code has bit `j` set, for the binary Moebius transform of a 16-entry table.
const MOEBIUS_MASKS: [u16; 4] = [0xAAAA, 0xCCCC, 0xF0F0, 0xFF00];

impl SmallGate {
    /// Compiles `gate`, or returns `None` when it acts on more than two qubits.
    ///
    /// Each code's image is the image of the code without its lowest bit, left-multiplied by the
    /// generator of that bit, so the table costs one local product per code.
    pub(super) fn compile(gate: &Tableau) -> StabilizerResult<Option<Self>> {
        let arity = gate.len();
        if arity > MAX_SMALL_ARITY {
            return Ok(None);
        }
        let rows = gate.rows();
        let mut generators = [LocalPauli::IDENTITY; 4];
        for (index, generator) in generators.iter_mut().enumerate().take(2 * arity) {
            // Row order is X_0.., Z_0..; code bit order is X_0, Z_0, X_1, Z_1.
            let row = (index % 2) * arity + index / 2;
            *generator = LocalPauli::from_row(rows, row)?;
        }
        let code_count = 1_usize << (2 * arity);
        let mut products = [LocalPauli::IDENTITY; 16];
        let mut images = [LocalImage::default(); 16];
        for code in 1..code_count {
            let lowest = code.trailing_zeros() as usize;
            let rest = *products
                .get(code & (code - 1))
                .ok_or_else(|| out_of_table(code))?;
            let generator = *generators.get(lowest).ok_or_else(|| out_of_table(code))?;
            let product = generator.multiply(rest);
            *products.get_mut(code).ok_or_else(|| out_of_table(code))? = product;
            let y_terms = (code & (code >> 1) & 0b0101).count_ones();
            let log_i = (u32::from(product.log_i) + y_terms) & 3;
            if log_i & 1 != 0 {
                return Err(StabilizerError::ImaginaryProduct {
                    phase: phase_from_log_i(log_i),
                });
            }
            *images.get_mut(code).ok_or_else(|| out_of_table(code))? = LocalImage {
                code: product.code(),
                negative: log_i == 2,
            };
        }
        Ok(Some(Self { arity, images }))
    }

    pub(super) const fn arity(&self) -> usize {
        self.arity
    }

    pub(super) const fn images(&self) -> &[LocalImage; 16] {
        &self.images
    }

    /// The word formula of this gate: its linear bit map and the algebraic normal form of its
    /// sign. Some two-qubit signs need cubic monomials, so every monomial is kept.
    pub(super) fn formula(&self) -> PackedFormula {
        let mut linear = 0_u16;
        let mut table = 0_u16;
        for (code, image) in self.images.iter().enumerate() {
            table |= u16::from(image.negative) << code;
            if code.is_power_of_two() {
                let input = code.trailing_zeros();
                for output in 0..4 {
                    linear |= u16::from(image.code >> output & 1) << (4 * output + input);
                }
            }
        }
        for (bit, mask) in MOEBIUS_MASKS.iter().enumerate() {
            table ^= (table << (1 << bit)) & mask;
        }
        PackedFormula {
            linear,
            sign: table,
        }
    }

    /// A key that determines the action of a gate on at most two qubits, or `None` for a wider
    /// gate: the arity, then each generator row's X bits, Z bits, and sign.
    pub(super) fn key(gate: &Tableau) -> Option<u64> {
        let arity = gate.len();
        if arity > MAX_SMALL_ARITY {
            return None;
        }
        let (words, signs) = gate.rows().words_and_signs();
        let mut key = u64::from(arity == 2) + 1;
        for (row, [x, z]) in words.as_chunks::<2>().0.iter().enumerate() {
            key |= ((x & 3) | ((z & 3) << 2)) << (2 + 4 * row);
        }
        let sign_bits = signs.first().copied().unwrap_or(0) & 0xF;
        Some(key | sign_bits << 18)
    }
}

impl PackedFormula {
    /// Expands the packed bits into word masks.
    pub(super) fn expand(self) -> SignFormula {
        let mut linear = [[0_u64; 4]; 4];
        for (output, row) in linear.iter_mut().enumerate() {
            for (input, slot) in row.iter_mut().enumerate() {
                *slot = mask(self.linear >> (4 * output + input) & 1 != 0);
            }
        }
        let mut sign = [0_u64; 16];
        for (monomial, slot) in sign.iter_mut().enumerate() {
            *slot = mask(self.sign >> monomial & 1 != 0);
        }
        SignFormula { linear, sign }
    }
}

impl SignFormula {
    /// A placeholder for an empty cache slot; it maps every input to zero.
    pub(super) const UNUSED: Self = Self {
        linear: [[0; 4]; 4],
        sign: [0; 16],
    };

    /// Applies the formula to four input words and returns the output words and sign flips.
    #[inline(always)]
    pub(super) fn apply(&self, inputs: [u64; 4]) -> ([u64; 4], u64) {
        let [v0, v1, v2, v3] = inputs;
        let mut outputs = [0_u64; 4];
        for (output, row) in outputs.iter_mut().zip(&self.linear) {
            let [m0, m1, m2, m3] = *row;
            *output = (v0 & m0) ^ (v1 & m1) ^ (v2 & m2) ^ (v3 & m3);
        }
        let [
            _,
            c0,
            c1,
            c01,
            c2,
            c02,
            c12,
            c012,
            c3,
            c03,
            c13,
            c013,
            c23,
            c023,
            c123,
            c0123,
        ] = self.sign;
        let v01 = v0 & v1;
        let v23 = v2 & v3;
        let sign = (v0 & c0)
            ^ (v1 & c1)
            ^ (v2 & c2)
            ^ (v3 & c3)
            ^ (v01 & c01)
            ^ (v0 & v2 & c02)
            ^ (v1 & v2 & c12)
            ^ (v0 & v3 & c03)
            ^ (v1 & v3 & c13)
            ^ (v23 & c23)
            ^ (v01 & v2 & c012)
            ^ (v01 & v3 & c013)
            ^ (v0 & v23 & c023)
            ^ (v1 & v23 & c123)
            ^ (v01 & v23 & c0123);
        (outputs, sign)
    }
}

const fn out_of_table(index: usize) -> StabilizerError {
    StabilizerError::TableauIndexOutOfRange { index, len: 16 }
}

const fn mask(bit: bool) -> u64 {
    if bit { u64::MAX } else { 0 }
}

fn phase_from_log_i(log_i: u32) -> PauliPhase {
    match log_i & 3 {
        0 => PauliPhase::Plus,
        1 => PauliPhase::PlusI,
        2 => PauliPhase::Minus,
        _ => PauliPhase::MinusI,
    }
}

/// A Pauli on at most two local qubits with a base-`i` phase exponent.
#[derive(Clone, Copy, Debug)]
struct LocalPauli {
    x: u8,
    z: u8,
    log_i: u8,
}

impl LocalPauli {
    const IDENTITY: Self = Self {
        x: 0,
        z: 0,
        log_i: 0,
    };

    fn from_row(rows: &PauliRows, row: usize) -> StabilizerResult<Self> {
        let invalid = StabilizerError::TableauIndexOutOfRange {
            index: row,
            len: rows.row_count(),
        };
        let [x, z] = rows.row(row)? else {
            return Err(invalid);
        };
        let x = u8::try_from(*x).map_err(|_| invalid.clone())?;
        let z = u8::try_from(*z).map_err(|_| invalid)?;
        Ok(Self {
            x,
            z,
            log_i: if rows.sign(row)? { 2 } else { 0 },
        })
    }

    /// Right-multiplies `other`, accumulating the phase of each anticommuting qubit term.
    fn multiply(self, other: Self) -> Self {
        let x = self.x ^ other.x;
        let z = self.z ^ other.z;
        let x1z2 = self.x & other.z;
        let anticommutes = (other.x & self.z) ^ x1z2;
        let minus = (x ^ z ^ x1z2) & anticommutes;
        let quarter_turns = anticommutes.count_ones() + 2 * minus.count_ones();
        let log_i = (u32::from(self.log_i) + u32::from(other.log_i) + quarter_turns) & 3;
        Self {
            x,
            z,
            log_i: log_i as u8,
        }
    }

    /// The code with qubit `i`'s X bit at `2i` and Z bit at `2i + 1`.
    fn code(self) -> u8 {
        let mut code = 0_u8;
        for qubit in 0..MAX_SMALL_ARITY {
            code |= (self.x >> qubit & 1) << (2 * qubit);
            code |= (self.z >> qubit & 1) << (2 * qubit + 1);
        }
        code
    }
}
