use std::fmt::{Display, Formatter};

use rand::Rng;

mod action;
mod appender;
mod local;
mod prepend;
mod random;
#[cfg(test)]
mod reference;
pub(crate) mod rows;
mod small_gate;
#[cfg(test)]
mod tests;

pub use appender::TableauAppender;
pub(crate) use rows::PauliRows;

use super::budget::{AlgebraMemoryBudget, StabilizerResource, tableau_bytes};
use super::{PauliBasis, PauliPhase, PauliSign, PauliString, StabilizerError, StabilizerResult};

/// A Clifford operation stored as the signed images of its `2n` canonical generators.
///
/// Row `k` holds the image of `X_k` and row `n + k` the image of `Z_k`, each as packed X and Z
/// words with one packed sign plane, so composition, inversion, and Pauli actions run over whole
/// words. Every constructor validates or synthesizes a Clifford, so a value always satisfies the
/// symplectic commutation relations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Tableau {
    rows: PauliRows,
}

impl Tableau {
    /// Creates an identity Tableau admitted against [`AlgebraMemoryBudget::DEFAULT`].
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        Self::identity_with_budget(num_qubits, AlgebraMemoryBudget::DEFAULT)
    }

    /// Creates an identity Tableau after admitting its packed rows against `budget`.
    pub fn identity_with_budget(
        num_qubits: usize,
        budget: AlgebraMemoryBudget,
    ) -> StabilizerResult<Self> {
        budget.admit(StabilizerResource::Tableau, tableau_bytes(num_qubits))?;
        Self::identity_unadmitted(num_qubits)
    }

    /// Creates an identity Tableau whose size the caller has already bounded.
    pub(crate) fn identity_unadmitted(num_qubits: usize) -> StabilizerResult<Self> {
        let mut rows =
            PauliRows::try_zeroed(2 * num_qubits, num_qubits, StabilizerResource::Tableau)?;
        for qubit in 0..num_qubits {
            rows.set_bits(qubit, qubit, true, false)?;
            rows.set_bits(num_qubits + qubit, qubit, false, true)?;
        }
        Ok(Self { rows })
    }

    fn zeroed(num_qubits: usize) -> StabilizerResult<Self> {
        Ok(Self {
            rows: PauliRows::try_zeroed(2 * num_qubits, num_qubits, StabilizerResource::Tableau)?,
        })
    }

    pub(crate) const fn from_rows(rows: PauliRows) -> Self {
        Self { rows }
    }

    pub(crate) const fn rows(&self) -> &PauliRows {
        &self.rows
    }

    /// Creates a Tableau from the images of its canonical X and Z generators.
    ///
    /// [`PauliString`] makes Hermiticity explicit by construction: each generator has a real
    /// positive or negative sign. This constructor additionally checks the generator counts and
    /// widths, then requires the complete canonical symplectic commutation relation. That relation
    /// proves the output map is invertible, so a second rank calculation would be redundant. The
    /// packed rows are the same size as the supplied generators, so they are reserved fallibly
    /// without a separate budget.
    pub fn from_conjugated_generators(
        xs: Vec<PauliString>,
        zs: Vec<PauliString>,
    ) -> StabilizerResult<Self> {
        let result = Self::from_generators_unvalidated(&xs, &zs)?;
        if let Some(error) = result.first_symplectic_error()? {
            return Err(error);
        }
        Ok(result)
    }

    /// Packs generator images after checking only their counts and widths.
    pub(crate) fn from_generators_unvalidated(
        xs: &[PauliString],
        zs: &[PauliString],
    ) -> StabilizerResult<Self> {
        if xs.len() != zs.len() {
            return Err(StabilizerError::TableauGeneratorCountMismatch {
                x_generators: xs.len(),
                z_generators: zs.len(),
            });
        }
        let expected = xs.len();
        for (basis, generators) in [(PauliBasis::X, xs), (PauliBasis::Z, zs)] {
            for (index, generator) in generators.iter().enumerate() {
                if generator.len() != expected {
                    return Err(StabilizerError::TableauGeneratorWidthMismatch {
                        basis,
                        index,
                        width: generator.len(),
                        expected,
                    });
                }
            }
        }
        let mut result = Self::zeroed(expected)?;
        for (index, (x, z)) in xs.iter().zip(zs).enumerate() {
            result.rows.set_from_pauli(index, x)?;
            result.rows.set_from_pauli(expected + index, z)?;
        }
        Ok(result)
    }

    /// Samples a uniformly random Clifford tableau admitted against
    /// [`AlgebraMemoryBudget::DEFAULT`].
    ///
    /// See [`Self::random_with_budget`].
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        Self::random_with_budget(num_qubits, AlgebraMemoryBudget::DEFAULT, rng)
    }

    /// Samples a uniformly random Clifford tableau using the caller-owned RNG.
    ///
    /// Every Clifford operation, including its Pauli signs, is equally likely, as in Stim's
    /// `Tableau::random`: the symplectic part is Bravyi and Maslov's uniform sampler and every
    /// generator sign is an independent fair bit. Passing a seeded `rand` RNG gives deterministic
    /// Stab output, but the stream differs from Stim's C++ RNG. The output and the sampling
    /// matrices are admitted against `budget` before any randomness is consumed. Sampling costs
    /// `O(n^3 / 64)` word operations.
    pub fn random_with_budget<R>(
        num_qubits: usize,
        budget: AlgebraMemoryBudget,
        rng: &mut R,
    ) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        budget.admit(
            StabilizerResource::RandomTableau,
            random::random_tableau_bytes(num_qubits),
        )?;
        random::random_tableau(num_qubits, rng)
    }

    /// Builds a one-qubit Tableau from small fixed generator images.
    pub(crate) fn from_small_generators(
        xs: &[PauliString],
        zs: &[PauliString],
    ) -> StabilizerResult<Self> {
        Self::from_generators_unvalidated(xs, zs)
    }

    pub(crate) fn with_output_sign_mask(&self, mask: u128) -> StabilizerResult<Self> {
        let mut result = self.clone();
        for row in 0..result.rows.row_count() {
            let bit = u32::try_from(row)
                .ok()
                .and_then(|shift| mask.checked_shr(shift));
            result
                .rows
                .set_sign(row, bit.is_some_and(|bits| bits & 1 != 0))?;
        }
        Ok(result)
    }

    pub fn gate1(x_output: &str, z_output: &str) -> StabilizerResult<Self> {
        let x = x_output.parse::<PauliString>()?;
        let z = z_output.parse::<PauliString>()?;
        ensure_pauli_len(&x, 1)?;
        ensure_pauli_len(&z, 1)?;
        Self::from_conjugated_generators(vec![x], vec![z])
    }

    pub fn gate2(
        x1_output: &str,
        z1_output: &str,
        x2_output: &str,
        z2_output: &str,
    ) -> StabilizerResult<Self> {
        let x1 = x1_output.parse::<PauliString>()?;
        let z1 = z1_output.parse::<PauliString>()?;
        let x2 = x2_output.parse::<PauliString>()?;
        let z2 = z2_output.parse::<PauliString>()?;
        for pauli in [&x1, &z1, &x2, &z2] {
            ensure_pauli_len(pauli, 2)?;
        }
        Self::from_conjugated_generators(vec![x1, x2], vec![z1, z2])
    }

    /// Returns the Tableau of conjugation by `pauli`, admitted against
    /// [`AlgebraMemoryBudget::DEFAULT`].
    ///
    /// See [`Self::from_pauli_string_with_budget`].
    pub fn from_pauli_string(pauli: &PauliString) -> StabilizerResult<Self> {
        Self::from_pauli_string_with_budget(pauli, AlgebraMemoryBudget::DEFAULT)
    }

    /// Returns the Tableau of conjugation by `pauli`, admitted against `budget` because its size
    /// is quadratic in the string's width.
    pub fn from_pauli_string_with_budget(
        pauli: &PauliString,
        budget: AlgebraMemoryBudget,
    ) -> StabilizerResult<Self> {
        let num_qubits = pauli.len();
        budget.admit(StabilizerResource::Tableau, tableau_bytes(num_qubits))?;
        let mut result = Self::identity_unadmitted(num_qubits)?;
        for (index, basis) in pauli.active_terms() {
            if basis.z_bit() {
                result.rows.flip_sign(index)?;
            }
            if basis.x_bit() {
                result.rows.flip_sign(num_qubits + index)?;
            }
        }
        Ok(result)
    }

    pub fn len(&self) -> usize {
        self.rows.num_qubits()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Reports whether this Tableau is the identity operation, signs included.
    pub fn is_identity(&self) -> bool {
        let num_qubits = self.len();
        if self.rows.sign_words().iter().any(|word| *word != 0) {
            return false;
        }
        (0..2 * num_qubits).all(|row| {
            let qubit = row % num_qubits.max(1);
            let x_half = row < num_qubits;
            self.rows.row(row).is_ok_and(|words| {
                let (xs, zs) = words.split_at(self.rows.half_words());
                let (diagonal, other) = if x_half { (xs, zs) } else { (zs, xs) };
                other.iter().all(|word| *word == 0)
                    && diagonal.iter().enumerate().all(|(index, word)| {
                        let expected = if index == qubit / 64 {
                            1_u64 << (qubit % 64)
                        } else {
                            0
                        };
                        *word == expected
                    })
            })
        })
    }

    /// Returns the signed image of `X_index`.
    pub fn x_output(&self, index: usize) -> StabilizerResult<PauliString> {
        self.rows.to_pauli(self.x_row(index)?)
    }

    /// Returns the signed image of `Z_index`.
    pub fn z_output(&self, index: usize) -> StabilizerResult<PauliString> {
        self.rows.to_pauli(self.z_row(index)?)
    }

    pub fn y_output(&self, index: usize) -> StabilizerResult<PauliString> {
        self.x_output(index)?
            .multiply(&self.z_output(index)?)?
            .multiply_phase(PauliPhase::PlusI)?
            .try_into_real()
    }

    fn x_row(&self, index: usize) -> StabilizerResult<usize> {
        if index < self.len() {
            Ok(index)
        } else {
            Err(StabilizerError::TableauIndexOutOfRange {
                index,
                len: self.len(),
            })
        }
    }

    fn z_row(&self, index: usize) -> StabilizerResult<usize> {
        Ok(self.len() + self.x_row(index)?)
    }

    /// Conjugates `input` by this Tableau in `O(w n / 64)` word operations for an input of weight
    /// `w`.
    pub fn apply(&self, input: &PauliString) -> StabilizerResult<PauliString> {
        if input.len() != self.len() {
            return Err(StabilizerError::LengthMismatch {
                left: input.len(),
                right: self.len(),
            });
        }
        let mut output = PauliRows::try_zeroed(1, self.len(), StabilizerResource::PauliString)?;
        let negative = action::apply_words(
            &self.rows,
            None,
            input.x_bits(),
            input.z_bits(),
            input.sign().is_negative(),
            output.row_mut(0)?,
        )?;
        output.set_sign(0, negative)?;
        output.to_pauli(0)
    }

    /// Returns the operation that applies `self` and then `second`, in `O(n^3 / 64)` word
    /// operations.
    pub fn then(&self, second: &Self) -> StabilizerResult<Self> {
        if self.len() != second.len() {
            return Err(StabilizerError::LengthMismatch {
                left: self.len(),
                right: second.len(),
            });
        }
        let mut result = Self::zeroed(self.len())?;
        action::apply_rows(&second.rows, &self.rows, &mut result.rows)?;
        Ok(result)
    }

    /// Appends a local Clifford action to the selected output qubits in place.
    ///
    /// This is semantically equivalent to `self.then(&gate.embedded(self.len(), targets)?)`.
    /// One- and two-qubit gates rewrite the target bits of every row from a precomputed local
    /// image table without allocating; wider gates conjugate each row's gathered local Pauli.
    /// Either way the cost is `O(n)` row visits; for many consecutive appends,
    /// [`TableauAppender`] costs `O(n / 64)` word operations per one- or two-qubit gate.
    pub fn append(&mut self, gate: &Self, targets: &[usize]) -> StabilizerResult<()> {
        validate_tableau_targets(gate.len(), self.len(), targets)?;
        local::append(&mut self.rows, gate, targets)
    }

    /// Prepends a local Clifford action on the selected input qubits in place.
    ///
    /// This is semantically equivalent to `*self = gate.embedded(self.len(), targets)?.then(self)?`,
    /// with the same target errors as [`Self::append`]. Only the `2k` generator rows of the `k`
    /// targets change, each becoming a signed product of those rows, so a one- or two-qubit gate
    /// costs `O(n / 64)` word operations and a `k`-qubit gate `O(k^2 n / 64)`.
    #[inline]
    pub fn prepend(&mut self, gate: &Self, targets: &[usize]) -> StabilizerResult<()> {
        validate_tableau_targets(gate.len(), self.len(), targets)?;
        prepend::prepend(&mut self.rows, gate, targets)
    }

    /// Raises this Tableau to a signed integer power by repeated squaring, in `O(log |exponent|)`
    /// compositions.
    pub fn pow(&self, exponent: i64) -> StabilizerResult<Self> {
        let mut power = exponent.unsigned_abs();
        let mut base = if exponent < 0 {
            self.inverse()?
        } else {
            self.clone()
        };
        let mut result: Option<Self> = None;
        while power != 0 {
            if power & 1 != 0 {
                result = Some(match result {
                    None => base.clone(),
                    Some(accumulated) => accumulated.then(&base)?,
                });
            }
            power >>= 1;
            if power != 0 {
                if base.is_identity() {
                    break;
                }
                base = base.then(&base)?;
            }
        }
        match result {
            Some(result) => Ok(result),
            None => Self::identity_unadmitted(self.len()),
        }
    }

    /// Returns the tensor-product action of `self` and `other` on disjoint qubits.
    ///
    /// The result is at most twice the operands' combined size, so it is reserved fallibly
    /// without a separate budget.
    pub fn direct_sum(&self, other: &Self) -> StabilizerResult<Self> {
        let left = self.len();
        let num_qubits =
            left.checked_add(other.len())
                .ok_or(StabilizerError::ResourceSizeOverflow {
                    resource: StabilizerResource::Tableau,
                    item_count: left,
                    repetitions: 2,
                })?;
        let mut result = Self::zeroed(num_qubits)?;
        for (source, offset) in [(self, 0), (other, left)] {
            let width = source.len();
            for index in 0..width {
                for (source_row, target_row) in [
                    (index, offset + index),
                    (width + index, num_qubits + offset + index),
                ] {
                    copy_shifted_row(
                        &source.rows,
                        source_row,
                        &mut result.rows,
                        target_row,
                        offset,
                    )?;
                }
            }
        }
        Ok(result)
    }

    /// Embeds this Tableau into `num_qubits`, acting on `targets` in the supplied order.
    ///
    /// The result is admitted against [`AlgebraMemoryBudget::DEFAULT`]; see
    /// [`Self::embedded_with_budget`].
    pub fn embedded(&self, num_qubits: usize, targets: &[usize]) -> StabilizerResult<Self> {
        self.embedded_with_budget(num_qubits, targets, AlgebraMemoryBudget::DEFAULT)
    }

    /// Embeds this Tableau into `num_qubits`, acting on `targets` in the supplied order.
    ///
    /// The result is admitted against `budget` because `num_qubits` is independent of this
    /// Tableau's size.
    pub fn embedded_with_budget(
        &self,
        num_qubits: usize,
        targets: &[usize],
        budget: AlgebraMemoryBudget,
    ) -> StabilizerResult<Self> {
        validate_tableau_targets(self.len(), num_qubits, targets)?;
        budget.admit(StabilizerResource::Tableau, tableau_bytes(num_qubits))?;
        let mut result = Self::identity_unadmitted(num_qubits)?;
        let width = self.len();
        for (local_index, global_index) in targets.iter().copied().enumerate() {
            for (source_row, target_row) in [
                (local_index, global_index),
                (width + local_index, num_qubits + global_index),
            ] {
                result
                    .rows
                    .set_bits(target_row, global_index, false, false)?;
                for (local_qubit, target) in targets.iter().copied().enumerate() {
                    let (x, z) = self.rows.bits(source_row, local_qubit)?;
                    result.rows.set_bits(target_row, target, x, z)?;
                }
                result
                    .rows
                    .set_sign(target_row, self.rows.sign(source_row)?)?;
            }
        }
        Ok(result)
    }

    /// Returns the inverse operation, signs included, in `O(n^3 / 64)` word operations.
    pub fn inverse(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(true)
    }

    /// Returns the inverse's unsigned generator images by transposition in `O(n^2 / 64)` word
    /// operations, with every sign positive.
    pub fn inverse_skipping_signs(&self) -> StabilizerResult<Self> {
        self.inverse_with_signs(false)
    }

    fn inverse_with_signs(&self, include_signs: bool) -> StabilizerResult<Self> {
        let mut result = Self::zeroed(self.len())?;
        action::transpose_inverse_into(&self.rows, &mut result.rows)?;
        if include_signs {
            action::fix_inverse_signs(&self.rows, &mut result.rows)?;
        }
        Ok(result)
    }

    /// Returns the unique row-reduced generators stabilizing this Tableau's output state.
    pub fn canonical_stabilizers(&self) -> StabilizerResult<Vec<PauliString>> {
        let stabilizers = (0..self.len())
            .map(|index| self.z_output(index))
            .collect::<StabilizerResult<Vec<_>>>()?;
        crate::conversions::canonicalize_stabilizers(&stabilizers)
    }

    pub fn to_pauli_string(&self) -> StabilizerResult<PauliString> {
        if !self.is_pauli_product()? {
            return Err(StabilizerError::NotPauliProduct);
        }
        let num_qubits = self.len();
        let mut result = PauliRows::try_zeroed(1, num_qubits, StabilizerResource::PauliString)?;
        for qubit in 0..num_qubits {
            let x = self.rows.sign(num_qubits + qubit)?;
            let z = self.rows.sign(qubit)?;
            result.set_bits(0, qubit, x, z)?;
        }
        result.to_pauli(0)
    }

    pub fn x_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        self.output_pauli_xyz(self.x_row(input_index)?, output_index)
    }

    pub fn y_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        let x = self.output_bits(self.x_row(input_index)?, output_index)?;
        let z = self.output_bits(self.z_row(input_index)?, output_index)?;
        Ok(pauli_xyz(PauliBasis::from_xz(x.0 ^ z.0, x.1 ^ z.1)))
    }

    pub fn z_output_pauli_xyz(
        &self,
        input_index: usize,
        output_index: usize,
    ) -> StabilizerResult<u8> {
        self.output_pauli_xyz(self.z_row(input_index)?, output_index)
    }

    /// Checks the canonical symplectic commutation relations in `O(n^3 / 64)` word operations.
    pub fn satisfies_invariants(&self) -> StabilizerResult<bool> {
        Ok(self.first_symplectic_error()?.is_none())
    }

    fn first_symplectic_error(&self) -> StabilizerResult<Option<StabilizerError>> {
        let num_qubits = self.len();
        let half_words = self.rows.half_words();
        for left in 0..num_qubits {
            let left_x = self.rows.row(left)?;
            let left_z = self.rows.row(num_qubits + left)?;
            if !rows::rows_anticommute(left_x, left_z, half_words) {
                return Ok(Some(StabilizerError::ConjugatedGeneratorPairCommutes {
                    index: left,
                }));
            }
            for right in left + 1..num_qubits {
                let right_x = self.rows.row(right)?;
                let right_z = self.rows.row(num_qubits + right)?;
                for (left_basis, left_row, right_basis, right_row) in [
                    (PauliBasis::X, left_x, PauliBasis::X, right_x),
                    (PauliBasis::X, left_x, PauliBasis::Z, right_z),
                    (PauliBasis::Z, left_z, PauliBasis::X, right_x),
                    (PauliBasis::Z, left_z, PauliBasis::Z, right_z),
                ] {
                    if rows::rows_anticommute(left_row, right_row, half_words) {
                        return Ok(Some(StabilizerError::ConjugatedGeneratorsAnticommute {
                            left_basis,
                            left_index: left,
                            right_basis,
                            right_index: right,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    fn is_pauli_product(&self) -> StabilizerResult<bool> {
        let num_qubits = self.len();
        for qubit in 0..num_qubits {
            for (row, expected) in [(qubit, (true, false)), (num_qubits + qubit, (false, true))] {
                let words = self.rows.row(row)?;
                let weight = words.iter().map(|word| word.count_ones()).sum::<u32>();
                if weight != 1 || self.rows.bits(row, qubit)? != expected {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn output_bits(&self, row: usize, output_index: usize) -> StabilizerResult<(bool, bool)> {
        if output_index >= self.len() {
            return Err(StabilizerError::TableauIndexOutOfRange {
                index: output_index,
                len: self.len(),
            });
        }
        self.rows.bits(row, output_index)
    }

    fn output_pauli_xyz(&self, row: usize, output_index: usize) -> StabilizerResult<u8> {
        let (x, z) = self.output_bits(row, output_index)?;
        Ok(pauli_xyz(PauliBasis::from_xz(x, z)))
    }
}

impl Display for Tableau {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let num_qubits = self.len();
        f.write_str("+-")?;
        for _ in 0..num_qubits {
            f.write_str("xz-")?;
        }
        f.write_str("\n|")?;
        for index in 0..num_qubits {
            let x = self.rows.sign(index).map_err(|_| std::fmt::Error)?;
            let z = self
                .rows
                .sign(num_qubits + index)
                .map_err(|_| std::fmt::Error)?;
            write!(f, " {}{}", sign_from_bit(x), sign_from_bit(z))?;
        }
        for output_index in 0..num_qubits {
            f.write_str("\n|")?;
            for input_index in 0..num_qubits {
                let basis = |row| {
                    self.rows
                        .bits(row, output_index)
                        .map(|(x, z)| PauliBasis::from_xz(x, z))
                        .map_err(|_| std::fmt::Error)
                };
                let x_basis = basis(input_index)?;
                let z_basis = basis(num_qubits + input_index)?;
                write!(f, " {x_basis}{z_basis}")?;
            }
        }
        Ok(())
    }
}

fn ensure_pauli_len(pauli: &PauliString, expected: usize) -> StabilizerResult<()> {
    if pauli.len() == expected {
        Ok(())
    } else {
        Err(StabilizerError::LengthMismatch {
            left: pauli.len(),
            right: expected,
        })
    }
}

/// Copies `source_row` into `target_row`, shifting its qubits up by `offset`.
fn copy_shifted_row(
    source: &PauliRows,
    source_row: usize,
    target: &mut PauliRows,
    target_row: usize,
    offset: usize,
) -> StabilizerResult<()> {
    let source_half = source.half_words();
    let target_half = target.half_words();
    let words = source.row(source_row)?;
    let (source_x, source_z) = words.split_at(source_half);
    let target_words = target.row_mut(target_row)?;
    let (target_x, target_z) = target_words.split_at_mut(target_half);
    or_shifted(target_x, source_x, offset);
    or_shifted(target_z, source_z, offset);
    let sign = source.sign(source_row)?;
    target.set_sign(target_row, sign)
}

/// ORs `source` into `target` shifted up by `offset` bits, dropping bits past `target`.
fn or_shifted(target: &mut [u64], source: &[u64], offset: usize) {
    let word_offset = offset / 64;
    let bit_offset = offset % 64;
    for (index, word) in source.iter().copied().enumerate() {
        if let Some(slot) = target.get_mut(word_offset + index) {
            *slot |= word << bit_offset;
        }
        if bit_offset != 0
            && let Some(slot) = target.get_mut(word_offset + index + 1)
        {
            *slot |= word >> (64 - bit_offset);
        }
    }
}

#[inline]
fn validate_tableau_targets(
    tableau_qubits: usize,
    num_qubits: usize,
    targets: &[usize],
) -> StabilizerResult<()> {
    if targets.len() != tableau_qubits {
        return Err(StabilizerError::TableauTargetCountMismatch {
            tableau_qubits,
            target_count: targets.len(),
        });
    }
    match targets {
        [] => return Ok(()),
        &[t0] => {
            return if t0 < num_qubits {
                Ok(())
            } else {
                Err(StabilizerError::TableauTargetOutOfRange {
                    target: t0,
                    num_qubits,
                })
            };
        }
        &[t0, t1] => {
            if t0 >= num_qubits {
                return Err(StabilizerError::TableauTargetOutOfRange {
                    target: t0,
                    num_qubits,
                });
            }
            if t1 >= num_qubits {
                return Err(StabilizerError::TableauTargetOutOfRange {
                    target: t1,
                    num_qubits,
                });
            }
            return if t0 != t1 {
                Ok(())
            } else {
                Err(StabilizerError::DuplicateTableauTarget { target: t1 })
            };
        }
        _ => {}
    }
    for (index, target) in targets.iter().copied().enumerate() {
        if target >= num_qubits {
            return Err(StabilizerError::TableauTargetOutOfRange { target, num_qubits });
        }
        let prior_targets =
            targets
                .get(..index)
                .ok_or(StabilizerError::TableauIndexOutOfRange {
                    index,
                    len: targets.len(),
                })?;
        if prior_targets.contains(&target) {
            return Err(StabilizerError::DuplicateTableauTarget { target });
        }
    }
    Ok(())
}

pub(crate) fn pauli_xyz(basis: PauliBasis) -> u8 {
    match basis {
        PauliBasis::I => 0,
        PauliBasis::X => 1,
        PauliBasis::Y => 2,
        PauliBasis::Z => 3,
    }
}

fn sign_from_bit(negative: bool) -> PauliSign {
    if negative {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}
