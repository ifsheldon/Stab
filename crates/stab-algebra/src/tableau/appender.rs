//! Batched appends on a transposed Tableau.
//!
//! [`Tableau::append`] visits every generator row once per gate, touching a few bits of each.
//! [`TableauAppender`] instead transposes the Tableau once so that each output qubit owns one
//! packed X plane and one packed Z plane over all `2n` generator rows, beside the packed sign
//! plane. Appending a gate then rewrites only its targets' planes, 64 rows per word operation,
//! so a one- or two-qubit gate costs `O(n / 64)` word operations instead of `O(n)` row visits.

use stab_bits::transpose_bit_tile;

use super::small_gate::{SignFormula, SmallGate};
use super::{Tableau, validate_tableau_targets};
use crate::MemoryBudget;
use crate::budget::admit;
use crate::budget::{
    StabilizerResource, tableau_bytes, try_filled, try_zeroed_words, words_for_bits,
};
use crate::{PauliBasis, PauliPhase, PauliSign, StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;
/// Compiled small gates remembered by an appender, in two-way sets indexed by a hash of the
/// gate's key.
const FORMULA_CACHE_SLOTS: usize = 32;

/// A Tableau held transposed for a run of appended local Clifford gates.
///
/// [`Self::new`] takes ownership of a Tableau and transposes it in `O(n^2 / 64)` word operations,
/// [`Self::append`] has exactly the semantics of [`Tableau::append`], and [`Self::finish`]
/// transposes back and returns the Tableau. Use it when many gates are appended in a row; a
/// single append is cheaper on the Tableau itself. The transposed planes are no larger than the
/// Tableau's own rows, so like composition they are reserved fallibly without a separate memory
/// budget, and the source rows are released once the transpose is complete.
#[derive(Clone, Debug)]
pub struct TableauAppender {
    num_qubits: usize,
    /// Words per plane: one bit per generator row.
    plane_words: usize,
    /// For each output qubit, its X plane then its Z plane.
    planes: Vec<u64>,
    signs: Vec<u64>,
    /// Recently compiled one- and two-qubit gates by [`SmallGate::key`]; key 0 marks an empty
    /// slot because every key is nonzero.
    formulas: [(u64, SignFormula); FORMULA_CACHE_SLOTS],
}

impl TableauAppender {
    /// Creates an appender holding the identity on `num_qubits` qubits, without a transpose.
    ///
    /// The held planes are admitted against `budget` as the bytes of the equivalent Tableau, so
    /// this refuses exactly when [`Tableau::identity_with_budget`] would.
    pub fn identity_with_budget(num_qubits: usize, budget: MemoryBudget) -> StabilizerResult<Self> {
        admit(
            budget,
            StabilizerResource::Tableau,
            tableau_bytes(num_qubits),
        )?;
        let row_count = num_qubits
            .checked_mul(2)
            .ok_or(StabilizerError::ResourceSizeOverflow {
                resource: StabilizerResource::Tableau,
                item_count: num_qubits,
                repetitions: 2,
            })?;
        let plane_words = words_for_bits(row_count);
        let len = planes_len(num_qubits, plane_words)?;
        let mut planes = try_zeroed_words(StabilizerResource::Tableau, len)?;
        for qubit in 0..num_qubits {
            for (plane, row) in [(0, qubit), (1, num_qubits + qubit)] {
                let index = (2 * qubit + plane) * plane_words + row / WORD_BITS;
                *planes
                    .get_mut(index)
                    .ok_or_else(|| out_of_range(index, len))? |= 1 << (row % WORD_BITS);
            }
        }
        Ok(Self {
            num_qubits,
            plane_words,
            planes,
            signs: try_zeroed_words(StabilizerResource::Tableau, plane_words)?,
            formulas: [(0, SignFormula::UNUSED); FORMULA_CACHE_SLOTS],
        })
    }

    /// Transposes `tableau` into a batched appender.
    pub fn new(tableau: Tableau) -> StabilizerResult<Self> {
        let rows = tableau.rows;
        let num_qubits = rows.num_qubits();
        let row_count = rows.row_count();
        let plane_words = words_for_bits(row_count);
        let half_words = rows.half_words();
        let row_words = rows.row_words();
        let len = planes_len(num_qubits, plane_words)?;
        let mut planes = try_zeroed_words(StabilizerResource::Tableau, len)?;
        let (words, sign_words) = rows.words_and_signs();
        for block in 0..plane_words {
            let block_rows = words
                .chunks_exact(row_words.max(1))
                .skip(block * WORD_BITS)
                .take(WORD_BITS);
            for plane in 0..2 {
                for word in 0..half_words {
                    let mut tile = [0_u64; WORD_BITS];
                    for (slot, row) in tile.iter_mut().zip(block_rows.clone()) {
                        *slot = *row
                            .get(plane * half_words + word)
                            .ok_or_else(|| out_of_range(word, row_words))?;
                    }
                    transpose_bit_tile(&mut tile);
                    let first_qubit = word * WORD_BITS;
                    for (qubit, value) in (first_qubit..num_qubits).zip(tile) {
                        let index = (2 * qubit + plane) * plane_words + block;
                        *planes
                            .get_mut(index)
                            .ok_or_else(|| out_of_range(index, len))? = value;
                    }
                }
            }
        }
        let mut signs = try_zeroed_words(StabilizerResource::Tableau, plane_words)?;
        signs.copy_from_slice(sign_words);
        Ok(Self {
            num_qubits,
            plane_words,
            planes,
            signs,
            formulas: [(0, SignFormula::UNUSED); FORMULA_CACHE_SLOTS],
        })
    }

    /// The number of qubits of the held Tableau.
    pub fn len(&self) -> usize {
        self.num_qubits
    }

    pub fn is_empty(&self) -> bool {
        self.num_qubits == 0
    }

    /// Returns the output Pauli basis on `output_qubit` of the `X` generator of `input_qubit`.
    pub fn x_output_basis(
        &self,
        input_qubit: usize,
        output_qubit: usize,
    ) -> StabilizerResult<PauliBasis> {
        self.check_qubit(input_qubit)?;
        self.check_qubit(output_qubit)?;
        self.row_basis(input_qubit, output_qubit)
    }

    /// Returns the output Pauli basis on `output_qubit` of the `Z` generator of `input_qubit`.
    pub fn z_output_basis(
        &self,
        input_qubit: usize,
        output_qubit: usize,
    ) -> StabilizerResult<PauliBasis> {
        self.check_qubit(input_qubit)?;
        self.check_qubit(output_qubit)?;
        self.row_basis(self.num_qubits + input_qubit, output_qubit)
    }

    /// Returns the sign of the `X` generator of `input_qubit`.
    pub fn x_output_sign(&self, input_qubit: usize) -> StabilizerResult<PauliSign> {
        self.check_qubit(input_qubit)?;
        self.row_sign(input_qubit)
    }

    /// Returns the sign of the `Z` generator of `input_qubit`.
    pub fn z_output_sign(&self, input_qubit: usize) -> StabilizerResult<PauliSign> {
        self.check_qubit(input_qubit)?;
        self.row_sign(self.num_qubits + input_qubit)
    }

    #[inline]
    fn check_qubit(&self, qubit: usize) -> StabilizerResult<()> {
        if qubit < self.num_qubits {
            Ok(())
        } else {
            Err(out_of_range(qubit, self.num_qubits))
        }
    }

    #[inline]
    fn row_basis(&self, row: usize, output_qubit: usize) -> StabilizerResult<PauliBasis> {
        let word = row / WORD_BITS;
        let shift = row % WORD_BITS;
        let base = 2 * output_qubit * self.plane_words + word;
        let x_word = *self
            .planes
            .get(base)
            .ok_or_else(|| out_of_range(base, self.planes.len()))?;
        let z_index = base + self.plane_words;
        let z_word = *self
            .planes
            .get(z_index)
            .ok_or_else(|| out_of_range(z_index, self.planes.len()))?;
        Ok(PauliBasis::from_xz(
            (x_word >> shift) & 1 != 0,
            (z_word >> shift) & 1 != 0,
        ))
    }

    #[inline]
    fn row_sign(&self, row: usize) -> StabilizerResult<PauliSign> {
        let word_index = row / WORD_BITS;
        let word = *self
            .signs
            .get(word_index)
            .ok_or_else(|| out_of_range(word_index, self.signs.len()))?;
        Ok(if (word >> (row % WORD_BITS)) & 1 != 0 {
            PauliSign::Minus
        } else {
            PauliSign::Plus
        })
    }

    /// Appends `gate` on `targets`, exactly as [`Tableau::append`] would.
    ///
    /// Targets are validated before any change, with the same errors as [`Tableau::append`]. One-
    /// and two-qubit gates rewrite their targets' planes with a branch-free word formula in
    /// `O(n / 64)` word operations; a `k`-qubit gate costs `O(k^2 n / 64)`.
    pub fn append(&mut self, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
        validate_tableau_targets(gate.len(), self.num_qubits, targets)?;
        let Some(key) = SmallGate::key(gate) else {
            return self.append_product(gate, targets);
        };
        let slot = self.cache_small_formula(gate, key)?;
        let Self {
            plane_words,
            planes,
            signs,
            formulas,
            ..
        } = self;
        let (_, formula) = formulas
            .get(slot)
            .ok_or_else(|| out_of_range(slot, FORMULA_CACHE_SLOTS))?;
        match targets {
            [target] => append_one(planes, signs, *plane_words, formula, *target),
            [first, second] => append_two(planes, signs, *plane_words, formula, *first, *second),
            _ => Ok(()),
        }
    }

    /// Transposes back and returns the Tableau with every appended gate applied.
    pub fn finish(self) -> StabilizerResult<Tableau> {
        let num_qubits = self.num_qubits;
        let mut result = Tableau::zeroed(num_qubits)?;
        let rows = &mut result.rows;
        let half_words = rows.half_words();
        let row_words = rows.row_words();
        let row_count = rows.row_count();
        let (words, signs) = rows.words_and_signs_mut();
        for block in 0..self.plane_words {
            for plane in 0..2 {
                for word in 0..half_words {
                    let mut tile = [0_u64; WORD_BITS];
                    for (qubit, slot) in (word * WORD_BITS..num_qubits).zip(tile.iter_mut()) {
                        let index = (2 * qubit + plane) * self.plane_words + block;
                        *slot = *self
                            .planes
                            .get(index)
                            .ok_or_else(|| out_of_range(index, self.planes.len()))?;
                    }
                    transpose_bit_tile(&mut tile);
                    let first_row = block * WORD_BITS;
                    for (row, value) in (first_row..row_count).zip(tile) {
                        let index = row * row_words + plane * half_words + word;
                        let len = words.len();
                        *words
                            .get_mut(index)
                            .ok_or_else(|| out_of_range(index, len))? = value;
                    }
                }
            }
        }
        if signs.len() != self.signs.len() {
            return Err(out_of_range(self.signs.len(), signs.len()));
        }
        signs.copy_from_slice(&self.signs);
        Ok(result)
    }

    /// Ensures the formula of the small gate `gate` with key `key` is cached and returns its slot.
    ///
    /// A miss compiles the gate into the first slot of its two-way set and moves the previous
    /// occupant to the second slot.
    fn cache_small_formula(&mut self, gate: &Tableau, key: u64) -> StabilizerResult<usize> {
        let hash = key.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 60;
        let set = usize::try_from(hash).map_err(|_| out_of_range(0, FORMULA_CACHE_SLOTS))?;
        let first = 2 * set;
        for slot in [first, first + 1] {
            let (cached, _) = self
                .formulas
                .get(slot)
                .ok_or_else(|| out_of_range(slot, FORMULA_CACHE_SLOTS))?;
            if *cached == key {
                return Ok(slot);
            }
        }
        let compiled = SmallGate::compile(gate)?.ok_or_else(|| out_of_range(gate.len(), 2))?;
        let pair = self
            .formulas
            .get_mut(first..first + 2)
            .ok_or_else(|| out_of_range(first, FORMULA_CACHE_SLOTS))?;
        pair.swap(0, 1);
        if let Some(slot) = pair.first_mut() {
            *slot = (key, compiled.formula().expand());
        }
        Ok(first)
    }

    /// Conjugates each 64-row word of local Paulis through the gate's generator images.
    ///
    /// Every lane accumulates the ordered product `X_0, Z_0, X_1, Z_1, ...` of the generators its
    /// input selects, with a two-bit phase counter per lane, exactly as the packed Pauli product
    /// kernel does, plus one quarter turn per input `Y`. A first pass proves every lane's phase is
    /// real before the second pass writes anything, so a failure leaves the planes unchanged.
    fn append_product(&mut self, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
        let product = GeneratorProduct::new(gate)?;
        let arity = targets.len();
        let mut inputs = try_zeroed_words(StabilizerResource::Tableau, 2 * arity)?;
        let mut outputs = try_zeroed_words(StabilizerResource::Tableau, 2 * arity)?;
        for write in [false, true] {
            for word in 0..self.plane_words {
                for (local, target) in targets.iter().enumerate() {
                    for plane in 0..2 {
                        let index = (2 * target + plane) * self.plane_words + word;
                        let value = *self
                            .planes
                            .get(index)
                            .ok_or_else(|| out_of_range(index, self.planes.len()))?;
                        *inputs
                            .get_mut(2 * local + plane)
                            .ok_or_else(|| out_of_range(local, arity))? = value;
                    }
                }
                let (negative, imaginary) = product.apply(&inputs, &mut outputs)?;
                if imaginary != 0 {
                    return Err(StabilizerError::ImaginaryProduct {
                        phase: PauliPhase::PlusI,
                    });
                }
                if !write {
                    continue;
                }
                for (local, target) in targets.iter().enumerate() {
                    for plane in 0..2 {
                        let index = (2 * target + plane) * self.plane_words + word;
                        let value = *outputs
                            .get(2 * local + plane)
                            .ok_or_else(|| out_of_range(local, arity))?;
                        let len = self.planes.len();
                        *self
                            .planes
                            .get_mut(index)
                            .ok_or_else(|| out_of_range(index, len))? = value;
                    }
                }
                let len = self.signs.len();
                *self
                    .signs
                    .get_mut(word)
                    .ok_or_else(|| out_of_range(word, len))? ^= negative;
            }
        }
        Ok(())
    }
}

/// The X and Z planes of `qubit`, each `plane_words` long.
fn qubit_planes(planes: &mut [u64], plane_words: usize, qubit: usize) -> Option<&mut [u64]> {
    let start = qubit.checked_mul(2 * plane_words)?;
    planes.get_mut(start..start.checked_add(2 * plane_words)?)
}

fn append_one(
    planes: &mut [u64],
    signs: &mut [u64],
    plane_words: usize,
    formula: &SignFormula,
    target: usize,
) -> StabilizerResult<()> {
    let len = planes.len();
    let target_planes =
        qubit_planes(planes, plane_words, target).ok_or_else(|| out_of_range(target, len))?;
    let (xs, zs) = target_planes.split_at_mut(plane_words);
    for ((x, z), sign) in xs.iter_mut().zip(zs.iter_mut()).zip(signs.iter_mut()) {
        let ([new_x, new_z, _, _], flip) = formula.apply([*x, *z, 0, 0]);
        *x = new_x;
        *z = new_z;
        *sign ^= flip;
    }
    Ok(())
}

fn append_two(
    planes: &mut [u64],
    signs: &mut [u64],
    plane_words: usize,
    formula: &SignFormula,
    first: usize,
    second: usize,
) -> StabilizerResult<()> {
    let width = 2 * plane_words;
    let (low, high) = (first.min(second), first.max(second));
    let len = planes.len();
    let (head, tail) = planes
        .split_at_mut_checked(high * width)
        .ok_or_else(|| out_of_range(high, len))?;
    let low_planes = qubit_planes(head, plane_words, low).ok_or_else(|| out_of_range(low, len))?;
    let high_planes = tail
        .get_mut(..width)
        .ok_or_else(|| out_of_range(high, len))?;
    let (first_planes, second_planes) = if first < second {
        (low_planes, high_planes)
    } else {
        (high_planes, low_planes)
    };
    let (x0s, z0s) = first_planes.split_at_mut(plane_words);
    let (x1s, z1s) = second_planes.split_at_mut(plane_words);
    for ((((x0, z0), x1), z1), sign) in x0s
        .iter_mut()
        .zip(z0s.iter_mut())
        .zip(x1s.iter_mut())
        .zip(z1s.iter_mut())
        .zip(signs.iter_mut())
    {
        let ([new_x0, new_z0, new_x1, new_z1], flip) = formula.apply([*x0, *z0, *x1, *z1]);
        *x0 = new_x0;
        *z0 = new_z0;
        *x1 = new_x1;
        *z1 = new_z1;
        *sign ^= flip;
    }
    Ok(())
}

/// The generator images of a gate as all-ones or all-zero lane masks.
///
/// Generator `b` is local qubit `b / 2` in plane `b % 2`; its image has bits
/// `masks[b * 2k + 2j]` (X) and `masks[b * 2k + 2j + 1]` (Z) on local qubit `j`.
struct GeneratorProduct {
    arity: usize,
    masks: Vec<u64>,
    signs: Vec<u64>,
}

impl GeneratorProduct {
    fn new(gate: &Tableau) -> StabilizerResult<Self> {
        let arity = gate.len();
        let local = gate.rows();
        let mut masks = try_zeroed_words(StabilizerResource::Tableau, 4 * arity * arity)?;
        let mut signs = try_filled(StabilizerResource::Tableau, 2 * arity, 0_u64)?;
        for (generator, sign) in signs.iter_mut().enumerate() {
            let row = (generator % 2) * arity + generator / 2;
            *sign = lane_mask(local.sign(row)?);
            for qubit in 0..arity {
                let (x, z) = local.bits(row, qubit)?;
                for (plane, bit) in [(0, x), (1, z)] {
                    let index = generator * 2 * arity + 2 * qubit + plane;
                    let len = masks.len();
                    *masks
                        .get_mut(index)
                        .ok_or_else(|| out_of_range(index, len))? = lane_mask(bit);
                }
            }
        }
        Ok(Self {
            arity,
            masks,
            signs,
        })
    }

    /// Writes the image planes of one word of inputs and returns its sign and imaginary lanes.
    fn apply(&self, inputs: &[u64], outputs: &mut [u64]) -> StabilizerResult<(u64, u64)> {
        let stride = 2 * self.arity;
        outputs.fill(0);
        let mut count_bit_1 = 0_u64;
        let mut count_bit_2 = 0_u64;
        for (generator, select) in inputs.iter().copied().enumerate() {
            let image = self
                .masks
                .get(generator * stride..(generator + 1) * stride)
                .ok_or_else(|| out_of_range(generator, stride))?;
            let sign = self
                .signs
                .get(generator)
                .ok_or_else(|| out_of_range(generator, stride))?;
            count_bit_2 ^= select & sign;
            for ([x, z], [image_x, image_z]) in outputs
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .zip(image.as_chunks::<2>().0)
            {
                let right_x = image_x & select;
                let right_z = image_z & select;
                let x1z2 = *x & right_z;
                let anticommutes = (right_x & *z) ^ x1z2;
                *x ^= right_x;
                *z ^= right_z;
                count_bit_2 ^= (count_bit_1 ^ *x ^ *z ^ x1z2) & anticommutes;
                count_bit_1 ^= anticommutes;
            }
        }
        for [x, z] in inputs.as_chunks::<2>().0 {
            let y = x & z;
            count_bit_2 ^= count_bit_1 & y;
            count_bit_1 ^= y;
        }
        Ok((count_bit_2, count_bit_1))
    }
}

fn planes_len(num_qubits: usize, plane_words: usize) -> StabilizerResult<usize> {
    num_qubits
        .checked_mul(2)
        .and_then(|planes| planes.checked_mul(plane_words))
        .ok_or(StabilizerError::ResourceSizeOverflow {
            resource: StabilizerResource::Tableau,
            item_count: num_qubits,
            repetitions: 2 * plane_words,
        })
}

const fn lane_mask(bit: bool) -> u64 {
    if bit { u64::MAX } else { 0 }
}

const fn out_of_range(index: usize, len: usize) -> StabilizerError {
    StabilizerError::TableauIndexOutOfRange { index, len }
}
