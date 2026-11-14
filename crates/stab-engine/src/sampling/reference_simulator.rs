//! General inverse-tableau stabilizer simulator used for every deterministic reference sample.
//!
//! The simulator tracks the inverse `T^-1` of the Clifford `T` that prepares the current state
//! from `|0...0>`. Row `r` stores `T^-1 P T` for the input Pauli `P = X_r` (`r < n`) or
//! `P = Z_{r-n}` (`r >= n`), packed as `W = ceil(n / 64)` X words followed by `W` Z words, plus
//! one sign bit. Appending a gate `G` to the circuit replaces each row `T^-1 P T` with
//! `T^-1 (G^-1 P G) T`, so a gate on `k` qubits only rewrites the `2k` rows of its targets and
//! costs `O(k * W)` word operations regardless of how entangled the state is.
//!
//! Measuring a Hermitian Pauli `M` multiplies the rows of `M`'s support into `m = T^-1 M T`.
//! When `m` has no X bits the outcome is deterministic and equals `m`'s sign. Otherwise the
//! state is collapsed onto the `+1` eigenspace of `M`, which is the fixed false outcome Stim's
//! reference sample selects: with `S = T Z_k T^-1` a stabilizer anticommuting with `M`, the
//! Clifford `U = (1 + M S) / sqrt(2)` maps the state onto `(1 + M) |psi> / sqrt(2)`, and
//! appending `U` multiplies every row anticommuting with `M S` by `T^-1 M S T = m Z_k`.
//! Each collapse costs one strided bit read per row plus `O(W)` per updated row.
//!
//! The API takes qubit indices and Pauli bases only, so callers can drive it from any resolved
//! circuit representation.

use stab_algebra::PauliBasis;

/// Failure while building or driving the reference simulator.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum ReferenceSimulatorError {
    #[error("could not allocate {component} with {items} items")]
    Allocation {
        component: &'static str,
        items: usize,
    },
    #[error("reference simulator invariant failed: {0}")]
    Invariant(&'static str),
}

type SimResult<T> = Result<T, ReferenceSimulatorError>;

/// `G^-1 P G` for each target `X_j` and `Z_j` of a local Clifford `G`, as signed local Paulis.
///
/// Entry `2 * j` is the image of `X_j` and entry `2 * j + 1` the image of `Z_j`, where `j`
/// indexes the gate's target list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LocalCliffordInverse {
    images: Vec<SignedLocalPauli>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SignedLocalPauli {
    pub(super) negative: bool,
    pub(super) bases: Vec<PauliBasis>,
}

impl LocalCliffordInverse {
    pub(super) fn new(images: Vec<SignedLocalPauli>) -> Self {
        Self { images }
    }

    fn target_count(&self) -> usize {
        self.images.len() / 2
    }
}

/// Exact copy of a simulator state, used to detect that a repeat iteration left it unchanged.
#[derive(Debug)]
pub(super) struct ReferenceStateSnapshot {
    rows: Vec<u64>,
    signs: Vec<bool>,
}

impl ReferenceStateSnapshot {
    pub(super) fn try_new(qubit_count: usize) -> SimResult<Self> {
        let (rows, signs) = try_identity_storage(qubit_count)?;
        Ok(Self { rows, signs })
    }
}

#[derive(Debug)]
pub(super) struct InverseTableauSimulator {
    qubit_count: usize,
    words: usize,
    rows: Vec<u64>,
    signs: Vec<bool>,
    product: Vec<u64>,
    multiplier: Vec<u64>,
    local_rows: Vec<u64>,
    local_signs: Vec<bool>,
    collapse_flags: Vec<bool>,
    stabilizer_terms: Vec<(usize, PauliBasis)>,
}

impl InverseTableauSimulator {
    /// Allocates the `|0...0>` state with fallible storage.
    pub(super) fn try_new(qubit_count: usize) -> SimResult<Self> {
        let words = qubit_count.div_ceil(u64::BITS as usize);
        let (rows, signs) = try_identity_storage(qubit_count)?;
        let row_words = words
            .checked_mul(2)
            .ok_or(ReferenceSimulatorError::Allocation {
                component: "reference simulator scratch row",
                items: usize::MAX,
            })?;
        Ok(Self {
            qubit_count,
            words,
            rows,
            signs,
            product: try_zeroed(row_words, "reference simulator product row")?,
            multiplier: try_zeroed(row_words, "reference simulator multiplier row")?,
            local_rows: Vec::new(),
            local_signs: Vec::new(),
            collapse_flags: Vec::new(),
            stabilizer_terms: Vec::new(),
        })
    }

    pub(super) fn qubit_count(&self) -> usize {
        self.qubit_count
    }

    /// Returns to `|0...0>` without reallocating.
    pub(super) fn reset_to_zero_state(&mut self) {
        fill_identity(
            &mut self.rows,
            &mut self.signs,
            self.qubit_count,
            self.words,
        );
    }

    pub(super) fn capture(&self, snapshot: &mut ReferenceStateSnapshot) -> bool {
        if snapshot.rows.len() != self.rows.len() || snapshot.signs.len() != self.signs.len() {
            return false;
        }
        snapshot.rows.copy_from_slice(&self.rows);
        snapshot.signs.copy_from_slice(&self.signs);
        true
    }

    /// Reports whether the current state equals the snapshot's state.
    ///
    /// Identical tableaux are accepted directly. Otherwise each stabilizer generator
    /// `S_k = T_s Z_k T_s^-1` of the snapshot state is rebuilt from the snapshot's X-bit
    /// columns, signed by `T_s^-1 S_k T_s = +Z_k`, and checked to be a stabilizer of the current
    /// state with the same sign. This recognizes states that recur although the tableau that
    /// prepares them does not, as in every round of a memory experiment.
    pub(super) fn same_state_as(&mut self, snapshot: &ReferenceStateSnapshot) -> SimResult<bool> {
        if snapshot.signs == self.signs && snapshot.rows == self.rows {
            return Ok(true);
        }
        if snapshot.rows.len() != self.rows.len() || snapshot.signs.len() != self.signs.len() {
            return Ok(false);
        }
        let row_words = 2 * self.words;
        let mut terms = std::mem::take(&mut self.stabilizer_terms);
        if terms.capacity() < self.qubit_count {
            terms
                .try_reserve_exact(self.qubit_count - terms.len())
                .map_err(|_| ReferenceSimulatorError::Allocation {
                    component: "reference state comparison terms",
                    items: self.qubit_count,
                })?;
        }
        let mut result = Ok(true);
        for column in 0..self.qubit_count {
            let word = column / u64::BITS as usize;
            let mask = 1_u64 << (column % u64::BITS as usize);
            let x_bit = |row: usize| {
                snapshot
                    .rows
                    .get(row * row_words + word)
                    .is_some_and(|bits| bits & mask != 0)
            };
            terms.clear();
            for qubit in 0..self.qubit_count {
                // x_j(S_k) = <S_k, Z_j> and z_j(S_k) = <S_k, X_j>, read through T_s^-1.
                let basis = PauliBasis::from_xz(x_bit(self.qubit_count + qubit), x_bit(qubit));
                if basis != PauliBasis::I {
                    terms.push((qubit, basis));
                }
            }
            let snapshot_log_i = match pauli_image_into(
                &snapshot.rows,
                &snapshot.signs,
                self.qubit_count,
                self.words,
                &terms,
                &mut self.product,
            ) {
                Ok(log_i) => log_i,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            };
            let (image_x, _) = self.product.split_at(self.words);
            if first_set_bit(image_x).is_some() {
                result = Err(invariant("snapshot stabilizer did not map back to Z"));
                break;
            }
            let current_log_i = match self.observable_into_product(&terms) {
                Ok(log_i) => log_i,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            };
            let (image_x, _) = self.product.split_at(self.words);
            if first_set_bit(image_x).is_some() || current_log_i != snapshot_log_i {
                result = Ok(false);
                break;
            }
        }
        terms.clear();
        self.stabilizer_terms = terms;
        result
    }

    pub(super) fn apply_hadamard(&mut self, qubit: usize) -> SimResult<()> {
        let x_row = self.x_row_index(qubit)?;
        let z_row = self.z_row_index(qubit)?;
        let row_words = 2 * self.words;
        let (head, tail) = self.rows.split_at_mut(z_row * row_words);
        let x = head
            .get_mut(x_row * row_words..(x_row + 1) * row_words)
            .ok_or(invariant("Hadamard X row is outside the tableau"))?;
        let z = tail
            .get_mut(..row_words)
            .ok_or(invariant("Hadamard Z row is outside the tableau"))?;
        x.swap_with_slice(z);
        self.signs.swap(x_row, z_row);
        Ok(())
    }

    pub(super) fn apply_controlled_x(&mut self, control: usize, target: usize) -> SimResult<()> {
        if control == target {
            return Err(invariant("controlled-X control and target coincide"));
        }
        let control_x = self.x_row_index(control)?;
        let target_x = self.x_row_index(target)?;
        let control_z = self.z_row_index(control)?;
        let target_z = self.z_row_index(target)?;
        // CX^-1 X_c CX = X_c X_t and CX^-1 Z_t CX = Z_c Z_t.
        self.multiply_row_into_row(control_x, target_x)?;
        self.multiply_row_into_row(target_z, control_z)
    }

    /// Applies `S`. Since `S^-1 X S = -i X Z` and `S^-1 Z S = Z`, only the X row changes.
    pub(super) fn apply_s(&mut self, qubit: usize) -> SimResult<()> {
        let x_row = self.x_row_index(qubit)?;
        let z_row = self.z_row_index(qubit)?;
        self.multiply_row_into_row_with_phase(x_row, z_row, 3)
    }

    /// Applies a Pauli gate, which flips the signs of the anticommuting input rows.
    pub(super) fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) -> SimResult<()> {
        if basis.z_bit() {
            let row = self.x_row_index(qubit)?;
            flip(&mut self.signs, row)?;
        }
        if basis.x_bit() {
            let row = self.z_row_index(qubit)?;
            flip(&mut self.signs, row)?;
        }
        Ok(())
    }

    /// Applies a local Clifford `G` given the images `G^-1 P G` of its targets' X and Z.
    pub(super) fn apply_local_clifford(
        &mut self,
        targets: &[usize],
        inverse: &LocalCliffordInverse,
    ) -> SimResult<()> {
        if targets.len() != inverse.target_count() {
            return Err(invariant(
                "local Clifford target count disagrees with its images",
            ));
        }
        for (index, target) in targets.iter().enumerate() {
            if targets.get(..index).unwrap_or_default().contains(target) {
                return Err(invariant("local Clifford targets repeat a qubit"));
            }
        }
        let row_words = 2 * self.words;
        let local_row_count = targets.len() * 2;
        let local_words = local_row_count * row_words;
        try_resize(&mut self.local_rows, local_words, "local Clifford rows")?;
        try_resize(
            &mut self.local_signs,
            local_row_count,
            "local Clifford signs",
        )?;
        for (index, target) in targets.iter().enumerate() {
            for (offset, row) in [self.x_row_index(*target)?, self.z_row_index(*target)?]
                .into_iter()
                .enumerate()
            {
                let local = 2 * index + offset;
                let source = row_slice(&self.rows, row, row_words)?;
                row_slice_mut(&mut self.local_rows, local, row_words)?.copy_from_slice(source);
                *self
                    .local_signs
                    .get_mut(local)
                    .ok_or(invariant("local Clifford sign is outside scratch"))? =
                    sign(&self.signs, row)?;
            }
        }
        for (index, target) in targets.iter().enumerate() {
            for (offset, row) in [self.x_row_index(*target)?, self.z_row_index(*target)?]
                .into_iter()
                .enumerate()
            {
                let local = 2 * index + offset;
                let image = inverse
                    .images
                    .get(local)
                    .ok_or(invariant("local Clifford image is missing"))?;
                let negative = self.local_image_into_product(image)?;
                let row_storage = row_slice_mut(&mut self.rows, row, row_words)?;
                row_storage.copy_from_slice(&self.product);
                *self
                    .signs
                    .get_mut(row)
                    .ok_or(invariant("local Clifford row sign is outside the tableau"))? = negative;
            }
        }
        Ok(())
    }

    /// Measures one qubit in `basis`, collapsing a random outcome onto the false branch.
    pub(super) fn measure(&mut self, qubit: usize, basis: PauliBasis) -> SimResult<bool> {
        self.measure_product(&[(qubit, basis)])
    }

    /// Measures one qubit and leaves it in the false eigenstate of `basis`.
    pub(super) fn reset(&mut self, qubit: usize, basis: PauliBasis) -> SimResult<()> {
        if self.measure(qubit, basis)? {
            self.apply_pauli(qubit, reset_correction(basis))?;
        }
        Ok(())
    }

    /// Reports whether measuring the Hermitian product `terms` has a determined outcome.
    pub(super) fn is_deterministic(&mut self, terms: &[(usize, PauliBasis)]) -> SimResult<bool> {
        self.observable_into_product(terms)?;
        let (product_x, _) = self.product.split_at(self.words);
        Ok(first_set_bit(product_x).is_none())
    }

    /// Measures a Hermitian product of single-qubit Paulis on distinct qubits.
    ///
    /// Returns the deterministic outcome, or `false` after collapsing a random outcome.
    pub(super) fn measure_product(&mut self, terms: &[(usize, PauliBasis)]) -> SimResult<bool> {
        let log_i = self.observable_into_product(terms)?;
        let row_words = 2 * self.words;
        let (product_x, _) = self.product.split_at(self.words);
        let Some(pivot) = first_set_bit(product_x) else {
            return hermitian_sign(log_i);
        };
        // multiplier = m * Z_k, the inverse image of the anti-Hermitian collapse generator M S.
        self.multiplier.copy_from_slice(&self.product);
        let pivot_word = pivot / u64::BITS as usize;
        let pivot_mask = 1_u64 << (pivot % u64::BITS as usize);
        let pivot_z = self
            .multiplier
            .get(self.words + pivot_word)
            .is_some_and(|word| word & pivot_mask != 0);
        // X * Z = -iY and Y * Z = iX.
        let multiplier_log_i = (log_i + if pivot_z { 1 } else { 3 }) & 3;
        if let Some(word) = self.multiplier.get_mut(self.words + pivot_word) {
            *word ^= pivot_mask;
        }

        let row_count = 2 * self.qubit_count;
        let mut anticommuting = std::mem::take(&mut self.collapse_flags);
        try_resize(&mut anticommuting, row_count, "collapse row flags")?;
        for (row, flag) in anticommuting.iter_mut().enumerate() {
            *flag = self
                .rows
                .get(row * row_words + pivot_word)
                .is_some_and(|word| word & pivot_mask != 0);
        }
        for (qubit, basis) in terms {
            if basis.z_bit() {
                flip(&mut anticommuting, self.x_row_index(*qubit)?)?;
            }
            if basis.x_bit() {
                flip(&mut anticommuting, self.z_row_index(*qubit)?)?;
            }
        }
        let mut result = Ok(());
        for (row, flag) in anticommuting.iter().enumerate() {
            if !*flag {
                continue;
            }
            let row_storage = match row_slice_mut(&mut self.rows, row, row_words) {
                Ok(storage) => storage,
                Err(error) => {
                    result = Err(error);
                    break;
                }
            };
            let delta = right_multiply(row_storage, &self.multiplier, self.words);
            let Some(row_sign) = self.signs.get_mut(row) else {
                result = Err(invariant("collapse row sign is outside the tableau"));
                break;
            };
            match (u8::from(*row_sign) * 2 + multiplier_log_i + delta) & 3 {
                0 => *row_sign = false,
                2 => *row_sign = true,
                _ => {
                    result = Err(invariant("collapse produced a non-Hermitian row"));
                    break;
                }
            }
        }
        self.collapse_flags = anticommuting;
        result?;
        self.fix_collapse_gauge(terms)?;
        Ok(false)
    }

    /// Chooses the positive sign for the first row anticommuting with the measured observable.
    ///
    /// After a collapse onto `M`, exactly the rows anticommuting with `M` carry an X bit on the
    /// pivot column, so flipping all their signs is the gauge `T -> T Z_k`, which leaves the
    /// state unchanged. Fixing it makes equal states after equal histories produce equal
    /// tableaux, which is what lets repeat folding recognize recurring iterations.
    fn fix_collapse_gauge(&mut self, terms: &[(usize, PauliBasis)]) -> SimResult<()> {
        let Some((qubit, basis)) = terms.iter().find(|(_, basis)| *basis != PauliBasis::I) else {
            return Ok(());
        };
        let representative = if basis.z_bit() {
            self.x_row_index(*qubit)?
        } else {
            self.z_row_index(*qubit)?
        };
        if !sign(&self.signs, representative)? {
            return Ok(());
        }
        for (qubit, basis) in terms {
            if basis.z_bit() {
                let row = self.x_row_index(*qubit)?;
                flip(&mut self.signs, row)?;
            }
            if basis.x_bit() {
                let row = self.z_row_index(*qubit)?;
                flip(&mut self.signs, row)?;
            }
        }
        Ok(())
    }

    /// Writes `T^-1 M T` for a product of distinct-qubit Paulis into `product`, returning its
    /// phase as a power of `i`.
    fn observable_into_product(&mut self, terms: &[(usize, PauliBasis)]) -> SimResult<u8> {
        pauli_image_into(
            &self.rows,
            &self.signs,
            self.qubit_count,
            self.words,
            terms,
            &mut self.product,
        )
    }

    /// Writes the product of saved local rows named by `image` into `product`, returning
    /// whether the resulting Hermitian row is negative.
    fn local_image_into_product(&mut self, image: &SignedLocalPauli) -> SimResult<bool> {
        self.product.fill(0);
        let row_words = 2 * self.words;
        let mut log_i = u8::from(image.negative) * 2;
        for (index, basis) in image.bases.iter().enumerate() {
            if *basis == PauliBasis::Y {
                log_i += 1;
            }
            for (offset, present) in [(0, basis.x_bit()), (1, basis.z_bit())] {
                if !present {
                    continue;
                }
                let local = 2 * index + offset;
                log_i += u8::from(sign(&self.local_signs, local)?) * 2
                    + right_multiply(
                        &mut self.product,
                        row_slice(&self.local_rows, local, row_words)?,
                        self.words,
                    );
            }
            log_i &= 3;
        }
        hermitian_sign(log_i)
    }

    /// Replaces row `destination` with `destination * source` for commuting rows.
    fn multiply_row_into_row(&mut self, destination: usize, source: usize) -> SimResult<()> {
        self.multiply_row_into_row_with_phase(destination, source, 0)
    }

    /// Replaces row `destination` with `i^extra_log_i * destination * source`, which must be
    /// Hermitian.
    fn multiply_row_into_row_with_phase(
        &mut self,
        destination: usize,
        source: usize,
        extra_log_i: u8,
    ) -> SimResult<()> {
        let row_words = 2 * self.words;
        let (destination_row, source_row) = if destination < source {
            let (head, tail) = self.rows.split_at_mut(source * row_words);
            (
                head.get_mut(destination * row_words..(destination + 1) * row_words),
                tail.get(..row_words),
            )
        } else {
            let (head, tail) = self.rows.split_at_mut(destination * row_words);
            (
                tail.get_mut(..row_words),
                head.get(source * row_words..(source + 1) * row_words),
            )
        };
        let (Some(destination_row), Some(source_row)) = (destination_row, source_row) else {
            return Err(invariant("row multiplication is outside the tableau"));
        };
        let delta = right_multiply(destination_row, source_row, self.words);
        let source_sign = sign(&self.signs, source)?;
        let destination_sign = self
            .signs
            .get_mut(destination)
            .ok_or(invariant("row multiplication sign is outside the tableau"))?;
        *destination_sign = hermitian_sign(
            (u8::from(*destination_sign) * 2 + u8::from(source_sign) * 2 + delta + extra_log_i) & 3,
        )?;
        Ok(())
    }

    fn x_row_index(&self, qubit: usize) -> SimResult<usize> {
        if qubit < self.qubit_count {
            Ok(qubit)
        } else {
            Err(invariant("qubit is outside the reference simulator"))
        }
    }

    fn z_row_index(&self, qubit: usize) -> SimResult<usize> {
        if qubit < self.qubit_count {
            Ok(self.qubit_count + qubit)
        } else {
            Err(invariant("qubit is outside the reference simulator"))
        }
    }
}

/// Writes `T^-1 M T` for a product `M` of distinct-qubit Paulis into `product`, returning its
/// phase as a power of `i`.
fn pauli_image_into(
    rows: &[u64],
    signs: &[bool],
    qubit_count: usize,
    words: usize,
    terms: &[(usize, PauliBasis)],
    product: &mut [u64],
) -> SimResult<u8> {
    product.fill(0);
    let row_words = 2 * words;
    let mut log_i = 0_u8;
    for (qubit, basis) in terms {
        if *qubit >= qubit_count {
            return Err(invariant("qubit is outside the reference simulator"));
        }
        if *basis == PauliBasis::Y {
            log_i += 1;
        }
        for (row, present) in [
            (*qubit, basis.x_bit()),
            (qubit_count + *qubit, basis.z_bit()),
        ] {
            if present {
                log_i += u8::from(sign(signs, row)?) * 2
                    + right_multiply(product, row_slice(rows, row, row_words)?, words);
            }
        }
        log_i &= 3;
    }
    Ok(log_i)
}

pub(super) fn reset_correction(basis: PauliBasis) -> PauliBasis {
    match basis {
        PauliBasis::I | PauliBasis::Z => PauliBasis::X,
        PauliBasis::X | PauliBasis::Y => PauliBasis::Z,
    }
}

/// Right-multiplies the packed Pauli `destination` by `source`, returning the scalar phase of
/// the product as a power of `i` (signs excluded).
#[inline]
fn right_multiply(destination: &mut [u64], source: &[u64], words: usize) -> u8 {
    let (destination_x, destination_z) = destination.split_at_mut(words);
    let (source_x, source_z) = source.split_at(words.min(source.len()));
    let mut count_1 = 0_u64;
    let mut count_2 = 0_u64;
    for (((x1, z1), x2), z2) in destination_x
        .iter_mut()
        .zip(destination_z.iter_mut())
        .zip(source_x)
        .zip(source_z)
    {
        let old_x1 = *x1;
        let old_z1 = *z1;
        *x1 ^= *x2;
        *z1 ^= *z2;
        let x1_z2 = old_x1 & *z2;
        let anticommutes = (*x2 & old_z1) ^ x1_z2;
        count_2 ^= (count_1 ^ *x1 ^ *z1 ^ x1_z2) & anticommutes;
        count_1 ^= anticommutes;
    }
    let phase = (count_1.count_ones() + 2 * count_2.count_ones()) & 3;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the phase is reduced modulo four before narrowing"
    )]
    let phase = phase as u8;
    phase
}

fn hermitian_sign(log_i: u8) -> SimResult<bool> {
    match log_i & 3 {
        0 => Ok(false),
        2 => Ok(true),
        _ => Err(invariant(
            "a Hermitian Pauli product acquired an imaginary phase",
        )),
    }
}

fn first_set_bit(words: &[u64]) -> Option<usize> {
    words.iter().enumerate().find_map(|(index, word)| {
        word.lowest_one()
            .map(|bit| index * u64::BITS as usize + bit as usize)
    })
}

fn sign(signs: &[bool], row: usize) -> SimResult<bool> {
    signs
        .get(row)
        .copied()
        .ok_or(invariant("row sign is outside the tableau"))
}

fn flip(flags: &mut [bool], row: usize) -> SimResult<()> {
    let flag = flags
        .get_mut(row)
        .ok_or(invariant("row flag is outside the tableau"))?;
    *flag = !*flag;
    Ok(())
}

fn row_slice(rows: &[u64], row: usize, row_words: usize) -> SimResult<&[u64]> {
    rows.get(row * row_words..(row + 1) * row_words)
        .ok_or(invariant("row is outside the tableau"))
}

fn row_slice_mut(rows: &mut [u64], row: usize, row_words: usize) -> SimResult<&mut [u64]> {
    rows.get_mut(row * row_words..(row + 1) * row_words)
        .ok_or(invariant("row is outside the tableau"))
}

const fn invariant(message: &'static str) -> ReferenceSimulatorError {
    ReferenceSimulatorError::Invariant(message)
}

fn try_zeroed(len: usize, component: &'static str) -> SimResult<Vec<u64>> {
    let mut words = Vec::new();
    try_resize(&mut words, len, component)?;
    Ok(words)
}

fn try_resize<T: Clone + Default>(
    values: &mut Vec<T>,
    len: usize,
    component: &'static str,
) -> SimResult<()> {
    if let Some(additional) = len.checked_sub(values.len())
        && additional > 0
    {
        values
            .try_reserve_exact(additional)
            .map_err(|_| ReferenceSimulatorError::Allocation {
                component,
                items: len,
            })?;
    }
    values.resize(len, T::default());
    Ok(())
}

fn try_identity_storage(qubit_count: usize) -> SimResult<(Vec<u64>, Vec<bool>)> {
    let words = qubit_count.div_ceil(u64::BITS as usize);
    let row_count = qubit_count
        .checked_mul(2)
        .ok_or(ReferenceSimulatorError::Allocation {
            component: "reference simulator rows",
            items: usize::MAX,
        })?;
    let word_count = row_count
        .checked_mul(words)
        .and_then(|count| count.checked_mul(2))
        .ok_or(ReferenceSimulatorError::Allocation {
            component: "reference simulator row words",
            items: usize::MAX,
        })?;
    let mut rows = try_zeroed(word_count, "reference simulator row words")?;
    let mut signs = Vec::new();
    try_resize(&mut signs, row_count, "reference simulator row signs")?;
    fill_identity(&mut rows, &mut signs, qubit_count, words);
    Ok((rows, signs))
}

fn fill_identity(rows: &mut [u64], signs: &mut [bool], qubit_count: usize, words: usize) {
    rows.fill(0);
    signs.fill(false);
    let row_words = 2 * words;
    for qubit in 0..qubit_count {
        let word = qubit / u64::BITS as usize;
        let mask = 1_u64 << (qubit % u64::BITS as usize);
        if let Some(x) = rows.get_mut(qubit * row_words + word) {
            *x |= mask;
        }
        if let Some(z) = rows.get_mut((qubit_count + qubit) * row_words + words + word) {
            *z |= mask;
        }
    }
}
