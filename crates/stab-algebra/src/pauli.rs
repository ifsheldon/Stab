use std::fmt::{Display, Formatter};
use std::str::FromStr;

use rand::{Rng, RngExt as _};

use super::{StabilizerError, StabilizerResource, StabilizerResult, TextExcerpt};
use crate::MemoryBudget;
use crate::budget::admit;
use crate::budget::{pauli_bytes, try_reserve_one, try_resize_zeroed_bits, try_zeroed_bits};
use crate::kernels::{PauliPlanes, PauliPlanesMut, pauli_right_multiply_words};
use stab_bits::{BitError, BitVec};

mod scalars;

pub use scalars::{PauliBasis, PauliPhase, PauliSign};

const WORD_BITS: usize = 64;

/// What a Pauli string knows about whether any of its terms is not the identity.
///
/// Clearing a term cannot tell whether it was the last one without a scan, so it leaves the
/// answer unresolved and [`PauliString::has_no_pauli_terms`] scans only when asked. A run of
/// clears, such as undoing a wide reset, then costs one scan instead of one per term.
#[derive(Clone, Copy, Debug)]
enum TermPresence {
    Absent,
    Present,
    Unresolved,
}

impl TermPresence {
    const fn known(has_terms: bool) -> Self {
        if has_terms {
            Self::Present
        } else {
            Self::Absent
        }
    }

    /// The presence after one term was written with `basis`; `old_was_identity` is `None` when
    /// the previous basis is unknown.
    const fn after_write(self, old_was_identity: Option<bool>, basis: PauliBasis) -> Self {
        match basis {
            PauliBasis::X | PauliBasis::Y | PauliBasis::Z => Self::Present,
            PauliBasis::I => match (self, old_was_identity) {
                (Self::Absent, _) | (_, Some(true)) => self,
                _ => Self::Unresolved,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct PauliString {
    sign: PauliSign,
    xs: BitVec,
    zs: BitVec,
    terms: TermPresence,
}

impl PartialEq for PauliString {
    /// Compares sign and terms; the cached term presence is derived from the terms.
    fn eq(&self, rhs: &Self) -> bool {
        self.sign == rhs.sign && self.xs == rhs.xs && self.zs == rhs.zs
    }
}

impl Eq for PauliString {}

impl PauliString {
    /// Creates an all-identity string admitted against [`MemoryBudget::DEFAULT`].
    ///
    /// See [`Self::identity_with_budget`].
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        Self::identity_with_budget(num_qubits, MemoryBudget::DEFAULT)
    }

    /// Creates an all-identity string after admitting its two packed planes against `budget`.
    pub fn identity_with_budget(num_qubits: usize, budget: MemoryBudget) -> StabilizerResult<Self> {
        admit(
            budget,
            StabilizerResource::PauliString,
            pauli_bytes(num_qubits),
        )?;
        Self::try_identity_admitted(num_qubits)
    }

    /// Creates an all-identity string after the caller has admitted its two packed planes,
    /// reserving them fallibly.
    pub(crate) fn try_identity_admitted(num_qubits: usize) -> StabilizerResult<Self> {
        Ok(Self {
            sign: PauliSign::Plus,
            xs: try_zeroed_bits(StabilizerResource::PauliString, num_qubits)?,
            zs: try_zeroed_bits(StabilizerResource::PauliString, num_qubits)?,
            terms: TermPresence::Absent,
        })
    }

    pub(crate) fn identity_unbudgeted(num_qubits: usize) -> Self {
        Self {
            sign: PauliSign::Plus,
            xs: BitVec::zeros(num_qubits),
            zs: BitVec::zeros(num_qubits),
            terms: TermPresence::Absent,
        }
    }

    /// Creates a random Pauli string using the caller-owned RNG.
    ///
    /// Passing a seeded `rand` RNG gives deterministic Stab output. The sign and each Pauli basis
    /// are independently uniform, but the generated stream is not intended to match Stim's C++ RNG
    /// stream. The string is admitted against [`MemoryBudget::DEFAULT`]; see
    /// [`Self::random_with_budget`].
    pub fn random<R>(num_qubits: usize, rng: &mut R) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        Self::random_with_budget(num_qubits, MemoryBudget::DEFAULT, rng)
    }

    /// Creates a random Pauli string after admitting its two packed planes against `budget`,
    /// before consuming randomness.
    pub fn random_with_budget<R>(
        num_qubits: usize,
        budget: MemoryBudget,
        rng: &mut R,
    ) -> StabilizerResult<Self>
    where
        R: Rng + ?Sized,
    {
        let mut result = Self::identity_with_budget(num_qubits, budget)?;
        result.randomize(rng);
        Ok(result)
    }

    /// Collects Pauli bases admitted against [`MemoryBudget::DEFAULT`].
    ///
    /// See [`Self::from_bases_with_budget`].
    pub fn from_bases(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> StabilizerResult<Self> {
        Self::from_bases_with_budget(sign, bases, MemoryBudget::DEFAULT)
    }

    /// Collects Pauli bases, refusing the first item whose collected bases and packed planes
    /// would exceed `budget`.
    pub fn from_bases_with_budget(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let mut collected = Vec::new();
        for basis in bases {
            let count = collected.len().saturating_add(1);
            admit(
                budget,
                StabilizerResource::PauliString,
                collected_bases_bytes(count),
            )?;
            try_reserve_one(StabilizerResource::PauliString, &mut collected)?;
            collected.push(basis);
        }
        let xs = try_zeroed_bits(StabilizerResource::PauliString, collected.len())?;
        let zs = try_zeroed_bits(StabilizerResource::PauliString, collected.len())?;
        Ok(Self::pack_bases(sign, &collected, xs, zs))
    }

    /// Builds a string from `bases` without a budget admission; the caller has already admitted
    /// their two packed planes.
    pub(crate) fn from_bases_unbudgeted(
        sign: PauliSign,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> Self {
        let bases = bases.into_iter().collect::<Vec<_>>();
        let len = bases.len();
        Self::pack_bases(sign, &bases, BitVec::zeros(len), BitVec::zeros(len))
    }

    /// Writes `bases` into the zeroed planes `xs` and `zs`, which hold `bases.len()` bits each.
    fn pack_bases(sign: PauliSign, bases: &[PauliBasis], mut xs: BitVec, mut zs: BitVec) -> Self {
        let mut has_terms = false;
        {
            let mut x_words = xs.words_mut();
            let mut z_words = zs.words_mut();
            for (index, basis) in bases.iter().copied().enumerate() {
                has_terms |= basis != PauliBasis::I;
                let word = index / WORD_BITS;
                let mask = 1_u64 << (index % WORD_BITS);
                if basis.x_bit()
                    && let Some(word) = x_words.get_mut(word)
                {
                    *word |= mask;
                }
                if basis.z_bit()
                    && let Some(word) = z_words.get_mut(word)
                {
                    *word |= mask;
                }
            }
        }
        Self {
            sign,
            xs,
            zs,
            terms: TermPresence::known(has_terms),
        }
    }

    pub fn sign(&self) -> PauliSign {
        self.sign
    }

    /// Returns this Pauli string with the requested real sign.
    pub fn with_sign(&self, sign: PauliSign) -> Self {
        let mut result = self.clone();
        result.sign = sign;
        result
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
        let old_basis = self.get_or_error(index)?;
        self.xs.set(index, basis.x_bit())?;
        self.zs.set(index, basis.z_bit())?;
        self.terms = self
            .terms
            .after_write(Some(old_basis == PauliBasis::I), basis);
        Ok(())
    }

    /// Replaces this value with a random Pauli string of the same length.
    pub fn randomize<R>(&mut self, rng: &mut R)
    where
        R: Rng + ?Sized,
    {
        let len = self.len();
        self.sign = if rng.random_bool(0.5) {
            PauliSign::Minus
        } else {
            PauliSign::Plus
        };
        self.xs = BitVec::from_words_truncated(len, random_words(len, rng));
        self.zs = BitVec::from_words_truncated(len, random_words(len, rng));
        self.terms = TermPresence::known(bits_have_terms(&self.xs, &self.zs));
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

    /// Whether every term is the identity.
    ///
    /// This is constant time unless a term was cleared since the presence was last known, in
    /// which case it scans the planes up to the first non-identity term.
    #[inline]
    pub fn has_no_pauli_terms(&self) -> bool {
        match self.terms {
            TermPresence::Absent => true,
            TermPresence::Present => false,
            TermPresence::Unresolved => !bits_have_terms(&self.xs, &self.zs),
        }
    }

    pub(crate) fn set_in_bounds(&mut self, index: usize, basis: PauliBasis) {
        self.write_basis_in_bounds(index, basis);
        self.terms = self.terms.after_write(None, basis);
    }

    pub(crate) fn clear_known_terms(&mut self, positions: &[usize]) {
        for position in positions.iter().copied() {
            self.write_basis_in_bounds(position, PauliBasis::I);
        }
        self.terms = TermPresence::Absent;
    }

    fn write_basis_in_bounds(&mut self, index: usize, basis: PauliBasis) {
        debug_assert!(index < self.len());
        let word = index / WORD_BITS;
        let bit = index % WORD_BITS;
        let mask = 1_u64 << bit;
        let mut x_words = self.xs.words_mut();
        let mut z_words = self.zs.words_mut();
        let Some(x_word) = x_words.get_mut(word) else {
            return;
        };
        let Some(z_word) = z_words.get_mut(word) else {
            return;
        };
        if basis.x_bit() {
            *x_word |= mask;
        } else {
            *x_word &= !mask;
        }
        if basis.z_bit() {
            *z_word |= mask;
        } else {
            *z_word &= !mask;
        }
    }

    /// Whether some qubit holds a non-identity term in both strings; a shorter string is padded
    /// with identities.
    pub fn intersects(&self, rhs: &Self) -> bool {
        self.xs
            .words()
            .iter()
            .zip(self.zs.words())
            .zip(rhs.xs.words().iter().zip(rhs.zs.words()))
            .any(|((left_x, left_z), (right_x, right_z))| {
                (left_x | left_z) & (right_x | right_z) != 0
            })
    }

    /// Whether the two strings commute; a shorter string is padded with identities.
    pub fn commutes(&self, rhs: &Self) -> bool {
        self.symplectic_product(rhs) == 0
    }

    /// The phase that the termwise products contribute to `self * rhs`, ignoring both signs; a
    /// shorter string is padded with identities.
    pub fn log_i_scalar_byproduct(&self, rhs: &Self) -> PauliPhase {
        (0..self.len().max(rhs.len())).fold(PauliPhase::Plus, |phase, index| {
            let left = self.get_or_identity(index);
            let right = rhs.get_or_identity(index);
            phase.multiply(left.log_i_scalar_byproduct(right))
        })
    }

    pub fn multiply(&self, rhs: &Self) -> StabilizerResult<FlexPauliString> {
        let mut value = if self.len() >= rhs.len() {
            self.clone()
        } else {
            Self {
                sign: self.sign,
                xs: BitVec::from_words_truncated(rhs.len(), self.xs.words().to_vec()),
                zs: BitVec::from_words_truncated(rhs.len(), self.zs.words().to_vec()),
                terms: self.terms,
            }
        };
        let phase = value.right_multiply_in_place_returning_log_i_scalar(rhs)?;
        if phase.is_negative() {
            value.sign = value.sign.flipped();
        }
        Ok(FlexPauliString {
            value,
            imaginary: phase.is_imaginary(),
        })
    }

    pub fn multiply_real(&self, rhs: &Self) -> StabilizerResult<Self> {
        let product = self.multiply(rhs)?;
        product.try_into_real()
    }

    /// Right-multiplies `rhs` into this Pauli string and returns the missing scalar byproduct.
    ///
    /// The X/Z bits stored in this value are updated in place and its sign is kept; the returned
    /// phase, which includes `rhs`'s sign, still needs to be applied by the caller. For example,
    /// [`PauliPhase::PlusI`] means the in-place result must still be multiplied by `i`, and
    /// [`PauliPhase::Minus`] means the caller should apply an additional `-1` scalar.
    #[inline]
    pub fn right_multiply_in_place_returning_log_i_scalar(
        &mut self,
        rhs: &Self,
    ) -> StabilizerResult<PauliPhase> {
        self.ensure_len(rhs.len())?;
        if rhs.has_no_pauli_terms() {
            return Ok(rhs.sign.to_phase());
        }

        let rhs_word_count = rhs.xs.word_count();
        let left_word_count = self.xs.word_count();
        let trailing_has_terms = left_word_count > rhs_word_count
            && self
                .xs
                .words()
                .iter()
                .zip(self.zs.words())
                .skip(rhs_word_count)
                .any(|(x_word, z_word)| (x_word | z_word) != 0);
        let left_len = self.len();
        let right_len = rhs.len();
        let mut left_x_words = self.xs.words_mut();
        let mut left_z_words = self.zs.words_mut();
        let left_x =
            left_x_words
                .get_mut(..rhs_word_count)
                .ok_or(StabilizerError::LengthMismatch {
                    left: left_len,
                    right: right_len,
                })?;
        let left_z =
            left_z_words
                .get_mut(..rhs_word_count)
                .ok_or(StabilizerError::LengthMismatch {
                    left: left_len,
                    right: right_len,
                })?;
        let product = pauli_right_multiply_words(
            PauliPlanesMut {
                x: left_x,
                z: left_z,
            },
            PauliPlanes {
                x: rhs.xs.words(),
                z: rhs.zs.words(),
            },
        )?;
        self.terms = TermPresence::known(trailing_has_terms || product.has_terms);

        Ok(PauliPhase::from_log_i(product.log_i).multiply(rhs.sign.to_phase()))
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

    /// Parses dense Pauli text like [`FromStr`], admitting the string's two packed planes against
    /// `budget` instead of [`MemoryBudget::DEFAULT`].
    pub fn from_str_with_budget(text: &str, budget: MemoryBudget) -> StabilizerResult<Self> {
        Self::parse_dense(text, false, budget)
    }

    fn parse_dense(
        text: &str,
        allow_lowercase: bool,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let (sign, body) = parse_real_prefix(text);
        let mut result = Self::parse_dense_body(body, allow_lowercase, budget)?;
        result.sign = sign;
        Ok(result)
    }

    /// Parses a dense Pauli body whose sign prefix was already consumed, so a
    /// second sign character rejects like pinned Stim's `+-X` and `i-X`.
    fn parse_dense_body(
        body: &str,
        allow_lowercase: bool,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let mut result = Self::identity_with_budget(body.chars().count(), budget)?;
        for (offset, character) in body.chars().enumerate() {
            result.set(
                offset,
                PauliBasis::from_dense_char(character, offset, allow_lowercase)?,
            )?;
        }
        Ok(result)
    }

    pub(crate) fn from_parts(sign: PauliSign, xs: BitVec, zs: BitVec) -> StabilizerResult<Self> {
        if xs.len() != zs.len() {
            return Err(StabilizerError::LengthMismatch {
                left: xs.len(),
                right: zs.len(),
            });
        }
        let terms = TermPresence::known(bits_have_terms(&xs, &zs));
        Ok(Self {
            sign,
            xs,
            zs,
            terms,
        })
    }

    #[inline]
    fn ensure_len(&mut self, len: usize) -> StabilizerResult<()> {
        if len <= self.len() {
            return Ok(());
        }
        self.grow_len(len)
    }

    /// Pads both planes with identities up to `len` qubits, reserving the new words fallibly; on
    /// failure the string may have its X plane grown but keeps its old length.
    #[cold]
    fn grow_len(&mut self, len: usize) -> StabilizerResult<()> {
        let old_len = self.len();
        try_resize_zeroed_bits(StabilizerResource::PauliString, &mut self.xs, len)?;
        if let Err(error) =
            try_resize_zeroed_bits(StabilizerResource::PauliString, &mut self.zs, len)
        {
            self.xs.resize_zeros(old_len);
            return Err(error);
        }
        Ok(())
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
        Self::from_str_with_budget(text, MemoryBudget::DEFAULT)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FlexPauliString {
    value: PauliString,
    imaginary: bool,
}

impl FlexPauliString {
    /// Creates an all-identity value admitted like [`PauliString::identity`].
    pub fn identity(num_qubits: usize) -> StabilizerResult<Self> {
        Self::identity_with_budget(num_qubits, MemoryBudget::DEFAULT)
    }

    /// Creates an all-identity value admitted like [`PauliString::identity_with_budget`].
    pub fn identity_with_budget(num_qubits: usize, budget: MemoryBudget) -> StabilizerResult<Self> {
        Ok(Self {
            value: PauliString::identity_with_budget(num_qubits, budget)?,
            imaginary: false,
        })
    }

    pub fn value(&self) -> &PauliString {
        &self.value
    }

    /// Moves the signed Pauli string out, dropping the imaginary flag.
    pub(crate) fn into_value(self) -> PauliString {
        self.value
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

    /// Collects Pauli bases admitted like [`PauliString::from_bases`].
    pub fn from_phase_and_bases(
        phase: PauliPhase,
        bases: impl IntoIterator<Item = PauliBasis>,
    ) -> StabilizerResult<Self> {
        Self::from_phase_and_bases_with_budget(phase, bases, MemoryBudget::DEFAULT)
    }

    /// Collects Pauli bases admitted like [`PauliString::from_bases_with_budget`].
    pub fn from_phase_and_bases_with_budget(
        phase: PauliPhase,
        bases: impl IntoIterator<Item = PauliBasis>,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let imaginary = phase.is_imaginary();
        let value = PauliString::from_bases_with_budget(phase.sign(), bases, budget)?;
        Ok(Self { value, imaginary })
    }

    fn from_phase_and_bits(phase: PauliPhase, xs: BitVec, zs: BitVec) -> StabilizerResult<Self> {
        let value = PauliString::from_parts(phase.sign(), xs, zs)?;
        Ok(Self {
            value,
            imaginary: phase.is_imaginary(),
        })
    }

    pub(crate) fn multiply_phase(self, phase: PauliPhase) -> StabilizerResult<Self> {
        let phase = self.phase().multiply(phase);
        Self::from_phase_and_bits(phase, self.value.xs, self.value.zs)
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
        self.value.ensure_len(index.saturating_add(1))?;
        let old_basis = self.value.get_or_error(index)?;
        let (new_basis, phase) = old_basis.multiply(basis);
        let phase = self.phase().multiply(phase);
        self.imaginary = phase.is_imaginary();
        self.value.sign = phase.sign();
        self.value.set(index, new_basis)
    }

    /// Parses dense or sparse Pauli text like [`FromStr`], admitting the value's two packed
    /// planes against `budget` instead of [`MemoryBudget::DEFAULT`].
    ///
    /// A sparse text's width is its largest qubit index plus one, so a short text can request a
    /// wide value; the budget is checked before that storage is allocated.
    pub fn from_str_with_budget(text: &str, budget: MemoryBudget) -> StabilizerResult<Self> {
        let (phase, body) = parse_flex_prefix(text);
        let sparse_size = sparse_size_if_sparse(body)?;
        if sparse_size > 0 {
            Self::parse_sparse_body(phase, body, sparse_size, text, budget)
        } else {
            // The flex prefix already consumed the phase, so the dense body
            // must not carry a second sign (Stim rejects `+-X` and friends).
            let mut value = PauliString::parse_dense_body(body, true, budget)?;
            value.sign = phase.sign();
            Ok(Self {
                value,
                imaginary: phase.is_imaginary(),
            })
        }
    }

    fn parse_sparse_body(
        phase: PauliPhase,
        body: &str,
        num_qubits: usize,
        original_text: &str,
        budget: MemoryBudget,
    ) -> StabilizerResult<Self> {
        let mut value = PauliString::identity_with_budget(num_qubits, budget)?;
        value.sign = phase.sign();
        let mut result = Self {
            value,
            imaginary: phase.is_imaginary(),
        };
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
                            excerpt: TextExcerpt::new(original_text),
                        });
                    }
                    current_basis = Some(PauliBasis::from_dense_char(character, 0, true)?);
                }
                '0'..='9' => {
                    if current_basis.is_none() {
                        return Err(StabilizerError::InvalidSparsePauliString {
                            excerpt: TextExcerpt::new(original_text),
                        });
                    }
                    let digit = character.to_digit(10).ok_or_else(|| {
                        StabilizerError::InvalidSparsePauliString {
                            excerpt: TextExcerpt::new(original_text),
                        }
                    })?;
                    let next = current_index
                        .unwrap_or(0_usize)
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(digit as usize))
                        .ok_or_else(|| StabilizerError::InvalidSparsePauliString {
                            excerpt: TextExcerpt::new(original_text),
                        })?;
                    current_index = Some(next);
                }
                _ => {
                    return Err(StabilizerError::InvalidSparsePauliString {
                        excerpt: TextExcerpt::new(original_text),
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
        Self::from_str_with_budget(text, MemoryBudget::DEFAULT)
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
                    excerpt: TextExcerpt::new(text),
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
            excerpt: TextExcerpt::new(original_text),
        });
    };
    let Some(index) = index.take() else {
        return Err(StabilizerError::InvalidSparsePauliString {
            excerpt: TextExcerpt::new(original_text),
        });
    };
    if index >= result.len() {
        return Err(StabilizerError::InvalidSparsePauliString {
            excerpt: TextExcerpt::new(original_text),
        });
    }
    result.right_mul_basis(index, basis)
}

/// Bytes held while collecting `count` bases: one byte per collected basis plus the packed planes.
fn collected_bases_bytes(count: usize) -> u128 {
    count as u128 * size_of::<PauliBasis>() as u128 + pauli_bytes(count)
}

fn bits_have_terms(xs: &BitVec, zs: &BitVec) -> bool {
    xs.words()
        .iter()
        .zip(zs.words())
        .any(|(x_word, z_word)| (x_word | z_word) != 0)
}

fn random_words<R>(bit_len: usize, rng: &mut R) -> Vec<u64>
where
    R: Rng + ?Sized,
{
    (0..bit_len.div_ceil(WORD_BITS))
        .map(|_| rng.random::<u64>())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "stabilizer algebra tests use compact literal Pauli fixtures"
    )]

    use super::*;

    #[test]
    fn right_multiply_in_place_returns_missing_i_scalar() {
        let mut left = PauliString::from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::X, PauliBasis::Y, PauliBasis::Z, PauliBasis::I],
        );
        let right = PauliString::from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::Y, PauliBasis::Z, PauliBasis::X, PauliBasis::Z],
        );

        let log_i = left
            .right_multiply_in_place_returning_log_i_scalar(&right)
            .unwrap();

        assert_eq!(log_i, PauliPhase::MinusI);
        assert_eq!(left.to_string(), "+ZXYZ");
    }

    #[test]
    fn multiply_uses_in_place_core_without_losing_phase() {
        let left = PauliString::from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::X, PauliBasis::Y, PauliBasis::Z, PauliBasis::I],
        );
        let right = PauliString::from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::Y, PauliBasis::Z, PauliBasis::X, PauliBasis::Z],
        );

        let product = left.multiply(&right).unwrap();

        assert_eq!(product.to_string(), "-iZXYZ");
    }

    #[test]
    fn negative_identity_contributes_minus_one_scalar() {
        let mut left = PauliString::identity_unbudgeted(4);
        let right = PauliString::from_bases_unbudgeted(
            PauliSign::Minus,
            std::iter::repeat_n(PauliBasis::I, 4),
        );

        let log_i = left
            .right_multiply_in_place_returning_log_i_scalar(&right)
            .unwrap();

        assert_eq!(log_i, PauliPhase::Minus);
        assert_eq!(left.to_string(), "+____");
    }

    #[test]
    fn right_multiply_in_place_extends_for_longer_identity_rhs() {
        let mut left = PauliString::from_bases_unbudgeted(PauliSign::Plus, [PauliBasis::X]);
        let right = PauliString::identity_unbudgeted(3);

        let log_i = left
            .right_multiply_in_place_returning_log_i_scalar(&right)
            .unwrap();

        assert_eq!(log_i, PauliPhase::Plus);
        assert_eq!(left.to_string(), "+X__");
    }

    #[test]
    fn clearing_terms_keeps_identity_detection_exact() {
        let width = 1_000;
        let mut value = PauliString::from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::X, PauliBasis::Y, PauliBasis::Z]
                .into_iter()
                .cycle()
                .take(width),
        );
        // Clear from both ends so the last surviving term sits in a middle word.
        for index in (0..width / 2).chain((width / 2 + 1..width).rev()) {
            value.set(index, PauliBasis::I).unwrap();
            assert!(!value.has_no_pauli_terms(), "index={index}");
        }
        value.set(width / 2, PauliBasis::I).unwrap();
        assert!(value.has_no_pauli_terms());
        assert_eq!(value, PauliString::identity_unbudgeted(width));

        // An identity right operand reached by clearing still takes the identity fast path.
        let mut left = PauliString::from_bases_unbudgeted(PauliSign::Plus, [PauliBasis::Y; 3]);
        assert_eq!(
            left.right_multiply_in_place_returning_log_i_scalar(&value)
                .unwrap(),
            PauliPhase::Plus
        );
        assert_eq!(left.to_string(), format!("+YYY{}", "_".repeat(width - 3)));

        value.set(7, PauliBasis::Z).unwrap();
        assert!(!value.has_no_pauli_terms());
        value.set_in_bounds(7, PauliBasis::I);
        assert!(value.has_no_pauli_terms());
    }
}
