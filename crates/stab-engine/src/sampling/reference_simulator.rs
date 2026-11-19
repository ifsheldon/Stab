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
//! Each collapse costs one strided bit read per row plus `O(W)` per updated row; a long run of
//! collapses between gates reads its pivot columns from a transposed copy instead (see
//! `pivot_columns`). Random outcomes collapse in Stim's per-instruction order (see the
//! `reference_order` module).
//!
//! The API takes qubit indices and Pauli bases only, so callers can drive it from any resolved
//! circuit representation.
//!
//! The simulator also tallies the work it performs in [`WorkModel`] units, so repeat folding
//! can weigh the cost of a body iteration against the cost of a state snapshot and comparison
//! with numbers measured on the run itself.

use stab_algebra::PauliBasis;
use stab_model::Cancelled;

use crate::detection::frame::PauliTerm;
use crate::work_poll::{WorkPoll, chunk_units};

mod collapse_order;
mod pivot_columns;
mod snapshot;

use pivot_columns::PivotColumns;
#[cfg(test)]
pub(in crate::sampling) use pivot_columns::take_pivot_column_stats;

pub(super) use snapshot::ReferenceStateSnapshot;

/// Tracks simulator work already forwarded to a [`WorkPoll`].
#[derive(Debug)]
pub(in crate::sampling) struct SimulatorWork {
    seen: u64,
}

impl SimulatorWork {
    pub(in crate::sampling) fn new(simulator: &InverseTableauSimulator) -> Self {
        Self {
            seen: simulator.work(),
        }
    }

    #[inline]
    pub(in crate::sampling) fn charge(
        &mut self,
        simulator: &InverseTableauSimulator,
        poll: &mut WorkPoll<'_>,
    ) -> Result<(), Cancelled> {
        let work = simulator.work();
        let done = work.saturating_sub(self.seen);
        self.seen = work;
        poll.charge(usize::try_from(done).unwrap_or(usize::MAX))
    }

    /// Forwards a simulator work tally of `total` units, for charges made while the simulator
    /// itself is borrowed.
    #[inline]
    fn charge_total(&mut self, total: u64, poll: &mut WorkPoll<'_>) -> Result<(), Cancelled> {
        let done = total.saturating_sub(self.seen);
        self.seen = total;
        poll.charge(usize::try_from(done).unwrap_or(usize::MAX))
    }
}

/// Failure while building or driving the reference simulator.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub(super) enum ReferenceSimulatorError {
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
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

/// Relative costs of the simulator's primitive operations, in units of one copied word.
///
/// The weights were calibrated on this simulator by timing gate-, collapse-, and
/// comparison-dominated circuits at 9 to 2449 qubits; see the `reference_fold` module.
#[derive(Clone, Copy, Debug)]
pub(super) struct WorkModel {
    qubit_count: u64,
    row_words: u64,
}

impl WorkModel {
    /// One word copied, compared, swapped, or counted.
    const COPY_WORD: u64 = 1;
    /// One word of a phase-tracking Pauli product.
    const MULTIPLY_WORD: u64 = 3;
    /// Fixed cost of one Pauli product: the call, its phase reduction, and the sign update.
    const MULTIPLY_FIXED: u64 = 48;
    /// One word read from each of many rows a row length apart.
    pub(in crate::sampling) const STRIDED_WORD: u64 = 5;
    /// One `64 x 64` bit-tile transposition, including its scattered row writes.
    const TRANSPOSE_TILE: u64 = 160;
    /// Bookkeeping of one dispatched operation, independent of the state size.
    pub(in crate::sampling) const DISPATCH: u64 = 12;
    /// Fixed cost of checking one snapshot stabilizer: building its terms and two products.
    const STABILIZER_FIXED: u64 = 600;
    /// One word of fresh snapshot storage: reserving, zeroing, and first touching it.
    const ALLOCATED_WORD: u64 = 20;

    fn new(qubit_count: usize, words: usize) -> Self {
        Self {
            qubit_count: qubit_count as u64,
            row_words: (words as u64).saturating_mul(2),
        }
    }

    pub(super) fn qubit_count(self) -> u64 {
        self.qubit_count
    }

    /// Bookkeeping of one operation that touches no rows.
    pub(super) fn dispatch() -> u64 {
        Self::DISPATCH
    }

    pub(super) fn for_qubits(qubit_count: usize) -> Self {
        Self::new(qubit_count, qubit_count.div_ceil(u64::BITS as usize))
    }

    /// Multiplying one tableau row into another, as a Clifford row update does.
    pub(super) fn row_multiply(self) -> u64 {
        self.row_words
            .saturating_mul(Self::MULTIPLY_WORD)
            .saturating_add(Self::MULTIPLY_FIXED)
    }

    /// Copying or swapping one tableau row.
    pub(super) fn row_copy(self) -> u64 {
        self.row_words
            .saturating_mul(Self::COPY_WORD)
            .saturating_add(Self::DISPATCH)
    }

    /// Scanning one bit of every row, as a random-outcome collapse does to find its rows.
    pub(super) fn collapse_scan(self) -> u64 {
        self.qubit_count
            .saturating_mul(2 * Self::STRIDED_WORD)
            .saturating_add(Self::DISPATCH)
    }

    /// Copying, comparing, or counting the whole tableau once.
    pub(super) fn tableau_pass(self) -> u64 {
        self.qubit_count
            .saturating_mul(2)
            .saturating_mul(self.row_words)
            .saturating_mul(Self::COPY_WORD)
    }

    /// Transposing one 64-stabilizer block of a snapshot's X halves.
    fn transpose_block(self) -> u64 {
        let tiles = self
            .qubit_count
            .div_ceil(u64::from(u64::BITS))
            .saturating_mul(2);
        self.qubit_count
            .saturating_mul(2 * Self::STRIDED_WORD)
            .saturating_add(tiles.saturating_mul(Self::TRANSPOSE_TILE))
    }

    /// Building one snapshot stabilizer's terms from its transposed support.
    fn support_scan(self) -> u64 {
        self.row_words
            .saturating_mul(Self::COPY_WORD)
            .saturating_add(Self::STABILIZER_FIXED)
    }

    /// Allocating snapshot storage: the tableau copy and its transposed X halves.
    pub(super) fn snapshot_allocation(self) -> u64 {
        let tableau_words = self
            .qubit_count
            .saturating_mul(2)
            .saturating_mul(self.row_words);
        tableau_words
            .saturating_add(tableau_words / 2)
            .saturating_mul(Self::ALLOCATED_WORD)
    }

    /// Capturing a snapshot: copying the tableau and counting its X halves.
    pub(super) fn capture(self) -> u64 {
        let pass = self.tableau_pass();
        pass.saturating_add(pass / 2)
    }

    /// Comparing a state against a snapshot whose stabilizers all match, given the snapshot's
    /// row products and its comparison data that is not prepared yet.
    pub(super) fn comparison(
        self,
        support_rows: u64,
        untransposed_blocks: u64,
        unphased_rows: u64,
    ) -> u64 {
        let stabilizers = self
            .support_scan()
            .saturating_add(self.row_copy().saturating_mul(2))
            .saturating_mul(self.qubit_count);
        self.tableau_pass()
            .saturating_add(untransposed_blocks.saturating_mul(self.transpose_block()))
            .saturating_add(stabilizers)
            .saturating_add(
                self.row_multiply()
                    .saturating_mul(support_rows.saturating_add(unphased_rows)),
            )
    }

    /// Comparing a state against a fresh capture of a state with `support_rows` row products.
    pub(super) fn fresh_comparison(self, support_rows: u64) -> u64 {
        let blocks = self.qubit_count.div_ceil(u64::from(u64::BITS));
        self.comparison(support_rows, blocks, support_rows)
    }
}

#[derive(Debug)]
pub(super) struct InverseTableauSimulator {
    qubit_count: usize,
    words: usize,
    model: WorkModel,
    /// Work performed since construction, in [`WorkModel`] units.
    work: u64,
    rows: Vec<u64>,
    signs: Vec<bool>,
    product: Vec<u64>,
    multiplier: Vec<u64>,
    local_rows: Vec<u64>,
    local_signs: Vec<bool>,
    /// Rows anticommuting with the collapse generator, gathered by one collapse.
    collapse_rows: Vec<usize>,
    /// Rows of the measured observable's own support, which commute with `m` oppositely.
    collapse_flips: Vec<usize>,
    stabilizer_terms: Vec<PauliTerm>,
    /// Qubit bitsets and per-qubit pair partners for collapsing a fused X- or Y-basis
    /// instruction in Stim's order, sized on first use. Their `n / 4 + 8 n` bytes stay well
    /// inside the `256 n` bytes per qubit that reference work admission reserves beyond the
    /// tableau.
    order_marks: Vec<u64>,
    order_used: Vec<u64>,
    order_partners: Vec<usize>,
    /// A transposed copy of the X halves for wide runs of collapses.
    pivot_columns: PivotColumns,
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
            model: WorkModel::new(qubit_count, words),
            work: 0,
            rows,
            signs,
            product: try_zeroed(row_words, "reference simulator product row")?,
            multiplier: try_zeroed(row_words, "reference simulator multiplier row")?,
            local_rows: Vec::new(),
            local_signs: Vec::new(),
            collapse_rows: Vec::new(),
            collapse_flips: Vec::new(),
            stabilizer_terms: Vec::new(),
            order_marks: Vec::new(),
            order_used: Vec::new(),
            order_partners: Vec::new(),
            pivot_columns: PivotColumns::default(),
        })
    }

    /// Returns to `|0...0>` without reallocating.
    pub(super) fn reset_to_zero_state(&mut self) {
        self.pivot_columns.discard();
        fill_identity(
            &mut self.rows,
            &mut self.signs,
            self.qubit_count,
            self.words,
        );
    }

    pub(super) fn work_model(&self) -> WorkModel {
        self.model
    }

    /// Work performed since construction, in [`WorkModel`] units.
    pub(super) fn work(&self) -> u64 {
        self.work
    }

    fn charge(&mut self, units: u64) {
        self.work = self.work.saturating_add(units);
    }

    pub(super) fn apply_hadamard(&mut self, qubit: usize) -> SimResult<()> {
        let x_row = self.x_row_index(qubit)?;
        let z_row = self.z_row_index(qubit)?;
        let row_words = 2 * self.words;
        self.pivot_columns.discard();
        let (head, tail) = self.rows.split_at_mut(z_row * row_words);
        let x = head
            .get_mut(x_row * row_words..(x_row + 1) * row_words)
            .ok_or(invariant("Hadamard X row is outside the tableau"))?;
        let z = tail
            .get_mut(..row_words)
            .ok_or(invariant("Hadamard Z row is outside the tableau"))?;
        x.swap_with_slice(z);
        self.signs.swap(x_row, z_row);
        self.charge(self.model.row_copy().saturating_mul(2));
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

    /// Applies `CZ`, which maps `X_a` to `X_a Z_b` and `X_b` to `Z_a X_b` and fixes both Z.
    pub(super) fn apply_controlled_z(&mut self, left: usize, right: usize) -> SimResult<()> {
        if left == right {
            return Err(invariant("controlled-Z targets coincide"));
        }
        let left_x = self.x_row_index(left)?;
        let right_x = self.x_row_index(right)?;
        let left_z = self.z_row_index(left)?;
        let right_z = self.z_row_index(right)?;
        self.multiply_row_into_row(left_x, right_z)?;
        self.multiply_row_into_row(right_x, left_z)
    }

    /// Applies `SWAP` by exchanging the two qubits' rows.
    pub(super) fn apply_swap(&mut self, left: usize, right: usize) -> SimResult<()> {
        if left == right {
            return Err(invariant("swap targets coincide"));
        }
        self.pivot_columns.discard();
        for (first, second) in [
            (self.x_row_index(left)?, self.x_row_index(right)?),
            (self.z_row_index(left)?, self.z_row_index(right)?),
        ] {
            let row_words = 2 * self.words;
            let (low, high) = (first.min(second), first.max(second));
            let (head, tail) = self.rows.split_at_mut(high * row_words);
            let low_row = head
                .get_mut(low * row_words..(low + 1) * row_words)
                .ok_or(invariant("swap row is outside the tableau"))?;
            let high_row = tail
                .get_mut(..row_words)
                .ok_or(invariant("swap row is outside the tableau"))?;
            low_row.swap_with_slice(high_row);
            self.signs.swap(low, high);
        }
        self.charge(self.model.row_copy().saturating_mul(4));
        Ok(())
    }

    /// Applies `S`. Since `S^-1 X S = -i X Z` and `S^-1 Z S = Z`, only the X row changes.
    pub(super) fn apply_s(&mut self, qubit: usize) -> SimResult<()> {
        let x_row = self.x_row_index(qubit)?;
        let z_row = self.z_row_index(qubit)?;
        self.multiply_row_into_row_with_phase(x_row, z_row, 3)
    }

    /// Applies `S_DAG`. Since `S X S^-1 = i X Z` and `Z` is fixed, only the X row changes.
    pub(super) fn apply_s_dag(&mut self, qubit: usize) -> SimResult<()> {
        let x_row = self.x_row_index(qubit)?;
        let z_row = self.z_row_index(qubit)?;
        self.multiply_row_into_row_with_phase(x_row, z_row, 1)
    }

    /// Applies a Pauli gate, which flips the signs of the anticommuting input rows.
    pub(super) fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) -> SimResult<()> {
        self.charge(WorkModel::DISPATCH);
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
        targets: &[u32],
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
        self.pivot_columns.discard();
        let local_row_count = targets.len() * 2;
        let local_words = local_row_count * row_words;
        try_resize(&mut self.local_rows, local_words, "local Clifford rows")?;
        try_resize(
            &mut self.local_signs,
            local_row_count,
            "local Clifford signs",
        )?;
        for (index, target) in targets.iter().map(|&target| target as usize).enumerate() {
            for (offset, row) in [self.x_row_index(target)?, self.z_row_index(target)?]
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
        for (index, target) in targets.iter().map(|&target| target as usize).enumerate() {
            for (offset, row) in [self.x_row_index(target)?, self.z_row_index(target)?]
                .into_iter()
                .enumerate()
            {
                let local = 2 * index + offset;
                let image = inverse
                    .images
                    .get(local)
                    .ok_or(invariant("local Clifford image is missing"))?;
                let negative = self.local_image_into_product(image)?;
                let multiplied = image
                    .bases
                    .iter()
                    .map(|basis| u64::from(basis.x_bit()) + u64::from(basis.z_bit()))
                    .sum::<u64>();
                self.charge(
                    self.model
                        .row_multiply()
                        .saturating_mul(multiplied)
                        .saturating_add(self.model.row_copy().saturating_mul(2)),
                );
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
    pub(super) fn measure(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<bool> {
        let qubit = u32::try_from(qubit)
            .map_err(|_| invariant("qubit is outside the reference simulator"))?;
        self.measure_product(&[(qubit, basis)], poll, work)
    }

    /// Measures one qubit and leaves it in the false eigenstate of `basis`.
    pub(super) fn reset(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<()> {
        if self.measure(qubit, basis, poll, work)? {
            self.apply_pauli(qubit, reset_correction(basis))?;
        }
        Ok(())
    }

    /// Reports whether measuring the Hermitian product `terms` has a determined outcome.
    pub(super) fn is_deterministic(
        &mut self,
        terms: &[PauliTerm],
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<bool> {
        self.observable_into_product(terms, poll, work)?;
        let (product_x, _) = self.product.split_at(self.words);
        Ok(first_set_bit(product_x).is_none())
    }

    /// Measures a Hermitian product of single-qubit Paulis on distinct qubits.
    ///
    /// Returns the deterministic outcome, or `false` after collapsing a random outcome.
    pub(super) fn measure_product(
        &mut self,
        terms: &[PauliTerm],
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<bool> {
        let log_i = self.observable_into_product(terms, poll, work)?;
        let row_words = 2 * self.words;
        let (product_x, _) = self.product.split_at(self.words);
        let Some(pivot) = first_set_bit(product_x) else {
            return hermitian_sign(log_i);
        };
        // multiplier = m * Z_k, the inverse image of the anti-Hermitian collapse generator M S.
        self.multiplier.copy_from_slice(&self.product);
        self.charge(self.model.row_copy());
        work.charge(self, poll)?;
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
        if !self.pivot_columns.is_valid() && self.pivot_columns.should_build(self.model, self.words)
        {
            self.pivot_columns.build(
                &self.rows,
                self.qubit_count,
                self.words,
                (&mut self.work, poll, work),
            )?;
        }

        let row_count = 2 * self.qubit_count;
        let mut anticommuting = std::mem::take(&mut self.collapse_rows);
        anticommuting.clear();
        let flip_count = terms.len().saturating_mul(2);
        let listed = row_count.saturating_add(flip_count);
        if anticommuting.capacity() < listed && anticommuting.try_reserve_exact(listed).is_err() {
            self.collapse_rows = anticommuting;
            return Err(ReferenceSimulatorError::Allocation {
                component: "collapse rows",
                items: listed,
            });
        }
        let mut flips = std::mem::take(&mut self.collapse_flips);
        flips.clear();
        if flips.capacity() < flip_count && flips.try_reserve_exact(flip_count).is_err() {
            self.collapse_rows = anticommuting;
            self.collapse_flips = flips;
            return Err(ReferenceSimulatorError::Allocation {
                component: "collapse support rows",
                items: flip_count,
            });
        }
        for (qubit, basis) in terms {
            if basis.z_bit() {
                match self.x_row_index(*qubit as usize) {
                    Ok(row) => flips.push(row),
                    Err(error) => {
                        self.collapse_rows = anticommuting;
                        self.collapse_flips = flips;
                        return Err(error);
                    }
                }
            }
            if basis.x_bit() {
                match self.z_row_index(*qubit as usize) {
                    Ok(row) => flips.push(row),
                    Err(error) => {
                        self.collapse_rows = anticommuting;
                        self.collapse_flips = flips;
                        return Err(error);
                    }
                }
            }
        }
        flips.sort_unstable();
        self.charge(WorkModel::DISPATCH);
        let mut scan_result: SimResult<()> = Ok(());
        let mut merge = ToggleMerge {
            toggles: flips.iter().copied().peekable(),
        };
        if self.pivot_columns.is_valid() {
            // One contiguous column of the transposed copy lists the rows directly.
            let column_words = row_count.div_ceil(u64::BITS as usize);
            self.charge((column_words as u64).saturating_mul(WorkModel::COPY_WORD));
            scan_result = work
                .charge(self, poll)
                .map_err(ReferenceSimulatorError::from)
                .and_then(|()| self.pivot_columns.column(pivot))
                .map(|column| {
                    for (word_index, &word) in column.iter().enumerate() {
                        let mut remaining = word;
                        while remaining != 0 {
                            let bit = remaining.trailing_zeros() as usize;
                            remaining &= remaining - 1;
                            merge.mark(word_index * u64::BITS as usize + bit, &mut anticommuting);
                        }
                    }
                });
        } else {
            // Scan pivot X bits in bounded row chunks so wide collapses observe cancellation.
            let scan_chunk_rows =
                (chunk_units() / usize::try_from(WorkModel::STRIDED_WORD).unwrap_or(5)).max(1);
            let mut row_start = 0;
            while row_start < row_count {
                let row_end = (row_start + scan_chunk_rows).min(row_count);
                let chunk_len = row_end - row_start;
                self.charge(WorkModel::STRIDED_WORD.saturating_mul(chunk_len as u64));
                if let Err(cancelled) = work.charge(self, poll) {
                    scan_result = Err(cancelled.into());
                    break;
                }
                if row_words > 0
                    && let Some(slice) = self.rows.get(row_start * row_words..row_end * row_words)
                {
                    for (offset, row_slice) in slice.chunks_exact(row_words).enumerate() {
                        if row_slice
                            .get(pivot_word)
                            .is_some_and(|word| word & pivot_mask != 0)
                        {
                            merge.mark(row_start + offset, &mut anticommuting);
                        }
                    }
                }
                row_start = row_end;
            }
        }
        if scan_result.is_ok() {
            merge.finish(&mut anticommuting);
        }
        self.collapse_flips = flips;
        if let Err(error) = scan_result {
            self.collapse_rows = anticommuting;
            return Err(error);
        }

        let row_mul_work = self.model.row_multiply();
        let mul_chunk_rows =
            (chunk_units() / usize::try_from(row_mul_work.max(1)).unwrap_or(usize::MAX)).max(1);
        let mut result: SimResult<()> = Ok(());
        for chunk in anticommuting.chunks(mul_chunk_rows) {
            self.charge(row_mul_work.saturating_mul(chunk.len() as u64));
            if let Err(cancelled) = work.charge(self, poll) {
                result = Err(cancelled.into());
                break;
            }
            for &row in chunk {
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
            if result.is_err() {
                break;
            }
        }
        if result.is_ok() {
            let (multiplier_x, _) = self.multiplier.split_at(self.words);
            result = self.pivot_columns.follow_collapse(
                multiplier_x,
                &anticommuting,
                self.model,
                (&mut self.work, poll, work),
            );
        }
        if result.is_err() {
            self.pivot_columns.discard();
        }
        self.collapse_rows = anticommuting;
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
    fn fix_collapse_gauge(&mut self, terms: &[PauliTerm]) -> SimResult<()> {
        let Some((qubit, basis)) = terms.iter().find(|(_, basis)| *basis != PauliBasis::I) else {
            return Ok(());
        };
        let representative = if basis.z_bit() {
            self.x_row_index(*qubit as usize)?
        } else {
            self.z_row_index(*qubit as usize)?
        };
        if !sign(&self.signs, representative)? {
            return Ok(());
        }
        for (qubit, basis) in terms {
            if basis.z_bit() {
                let row = self.x_row_index(*qubit as usize)?;
                flip(&mut self.signs, row)?;
            }
            if basis.x_bit() {
                let row = self.z_row_index(*qubit as usize)?;
                flip(&mut self.signs, row)?;
            }
        }
        Ok(())
    }

    /// Writes `T^-1 M T` for a product of distinct-qubit Paulis into `product`, returning its
    /// phase as a power of `i`.
    fn observable_into_product(
        &mut self,
        terms: &[PauliTerm],
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<u8> {
        self.product.fill(0);
        self.charge(self.model.row_copy());
        let per_term_units = usize::try_from(self.model.row_multiply().saturating_mul(2).max(1))
            .unwrap_or(usize::MAX);
        let chunk_terms = (chunk_units() / per_term_units).max(1);
        let mut log_i = 0_u8;
        for chunk in terms.chunks(chunk_terms) {
            let multiplied = chunk
                .iter()
                .map(|(_, basis)| u64::from(basis.x_bit()) + u64::from(basis.z_bit()))
                .sum::<u64>();
            self.charge(self.model.row_multiply().saturating_mul(multiplied));
            work.charge(self, poll)?;
            let delta = pauli_image_append(
                &self.rows,
                &self.signs,
                self.qubit_count,
                self.words,
                chunk,
                &mut self.product,
            )?;
            log_i = (log_i + delta) & 3;
        }
        Ok(log_i)
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
        self.pivot_columns.discard();
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
        self.work = self.work.saturating_add(self.model.row_multiply());
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

/// Merges the measured observable's support rows, whose commutation with `m Z_k` is the
/// opposite of their pivot bit, into the ascending rows that carry the pivot bit.
struct ToggleMerge<'a> {
    toggles: std::iter::Peekable<std::iter::Copied<std::slice::Iter<'a, usize>>>,
}

impl ToggleMerge<'_> {
    /// The next support row at most `limit`, with whether it occurs an odd number of times.
    fn next_toggle(&mut self, limit: usize) -> Option<(usize, bool)> {
        let toggle = self.toggles.next_if(|&toggle| toggle <= limit)?;
        let mut odd = true;
        while self.toggles.next_if_eq(&toggle).is_some() {
            odd = !odd;
        }
        Some((toggle, odd))
    }

    /// Lists the support rows below `row`, then `row` unless the support toggles it off.
    fn mark(&mut self, row: usize, anticommuting: &mut Vec<usize>) {
        let mut keep = true;
        while let Some((toggle, odd)) = self.next_toggle(row) {
            if toggle == row {
                keep ^= odd;
            } else if odd {
                anticommuting.push(toggle);
            }
        }
        if keep {
            anticommuting.push(row);
        }
    }

    /// Lists the support rows after the last pivot row.
    fn finish(&mut self, anticommuting: &mut Vec<usize>) {
        while let Some((toggle, odd)) = self.next_toggle(usize::MAX) {
            if odd {
                anticommuting.push(toggle);
            }
        }
    }
}

/// Multiplies `T^-1 M_chunk T` into an existing `product` buffer and returns its phase delta.
fn pauli_image_append(
    rows: &[u64],
    signs: &[bool],
    qubit_count: usize,
    words: usize,
    terms: &[PauliTerm],
    product: &mut [u64],
) -> SimResult<u8> {
    let row_words = 2 * words;
    let mut log_i = 0_u8;
    for &(qubit, basis) in terms {
        let qubit = qubit as usize;
        if qubit >= qubit_count {
            return Err(invariant("qubit is outside the reference simulator"));
        }
        if basis == PauliBasis::Y {
            log_i += 1;
        }
        for (row, present) in [(qubit, basis.x_bit()), (qubit_count + qubit, basis.z_bit())] {
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
///
/// Each bit position keeps its own two-bit phase counter, so the words are processed in
/// independent lanes whose counters are summed at the end; this lets the compiler vectorize
/// the loop without changing the result.
#[inline]
fn right_multiply(destination: &mut [u64], source: &[u64], words: usize) -> u8 {
    let (destination_x, destination_z) = destination.split_at_mut(words);
    let (source_x, source_z) = source.split_at(words.min(source.len()));
    let len = destination_x
        .len()
        .min(destination_z.len())
        .min(source_x.len())
        .min(source_z.len());
    let (destination_x, _) = destination_x.split_at_mut(len);
    let (destination_z, _) = destination_z.split_at_mut(len);
    let (source_x, _) = source_x.split_at(len);
    let (source_z, _) = source_z.split_at(len);
    let mut count_1 = [0_u64; MULTIPLY_LANES];
    let mut count_2 = [0_u64; MULTIPLY_LANES];
    let (x1_chunks, x1_tail) = destination_x.as_chunks_mut::<MULTIPLY_LANES>();
    let (z1_chunks, z1_tail) = destination_z.as_chunks_mut::<MULTIPLY_LANES>();
    let (x2_chunks, x2_tail) = source_x.as_chunks::<MULTIPLY_LANES>();
    let (z2_chunks, z2_tail) = source_z.as_chunks::<MULTIPLY_LANES>();
    for (((x1, z1), x2), z2) in x1_chunks
        .iter_mut()
        .zip(z1_chunks.iter_mut())
        .zip(x2_chunks)
        .zip(z2_chunks)
    {
        let [a0, a1, a2, a3, a4, a5, a6, a7] = *x1;
        let [b0, b1, b2, b3, b4, b5, b6, b7] = *z1;
        let [c0, c1, c2, c3, c4, c5, c6, c7] = *x2;
        let [d0, d1, d2, d3, d4, d5, d6, d7] = *z2;
        let [e0, e1, e2, e3, e4, e5, e6, e7] = count_1;
        let [f0, f1, f2, f3, f4, f5, f6, f7] = count_2;
        let lane0 = multiply_word(a0, b0, c0, d0, e0, f0);
        let lane1 = multiply_word(a1, b1, c1, d1, e1, f1);
        let lane2 = multiply_word(a2, b2, c2, d2, e2, f2);
        let lane3 = multiply_word(a3, b3, c3, d3, e3, f3);
        let lane4 = multiply_word(a4, b4, c4, d4, e4, f4);
        let lane5 = multiply_word(a5, b5, c5, d5, e5, f5);
        let lane6 = multiply_word(a6, b6, c6, d6, e6, f6);
        let lane7 = multiply_word(a7, b7, c7, d7, e7, f7);
        *x1 = [
            lane0.0, lane1.0, lane2.0, lane3.0, lane4.0, lane5.0, lane6.0, lane7.0,
        ];
        *z1 = [
            lane0.1, lane1.1, lane2.1, lane3.1, lane4.1, lane5.1, lane6.1, lane7.1,
        ];
        count_1 = [
            lane0.2, lane1.2, lane2.2, lane3.2, lane4.2, lane5.2, lane6.2, lane7.2,
        ];
        count_2 = [
            lane0.3, lane1.3, lane2.3, lane3.3, lane4.3, lane5.3, lane6.3, lane7.3,
        ];
    }
    let [mut tail_1, rest_1 @ ..] = count_1;
    let [mut tail_2, rest_2 @ ..] = count_2;
    for (((x1, z1), x2), z2) in x1_tail
        .iter_mut()
        .zip(z1_tail.iter_mut())
        .zip(x2_tail)
        .zip(z2_tail)
    {
        (*x1, *z1, tail_1, tail_2) = multiply_word(*x1, *z1, *x2, *z2, tail_1, tail_2);
    }
    let ones = |first: u64, rest: [u64; MULTIPLY_LANES - 1]| {
        rest.iter().fold(first.count_ones(), |total, count| {
            total + count.count_ones()
        })
    };
    let phase = (ones(tail_1, rest_1) + 2 * ones(tail_2, rest_2)) & 3;
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the phase is reduced modulo four before narrowing"
    )]
    let phase = phase as u8;
    phase
}

/// Independent word lanes of [`right_multiply`].
const MULTIPLY_LANES: usize = 8;

/// Multiplies one word pair of a packed Pauli, returning the new X and Z words and that bit
/// lane's advanced two-bit phase counter.
#[inline(always)]
fn multiply_word(
    x1: u64,
    z1: u64,
    x2: u64,
    z2: u64,
    count_1: u64,
    count_2: u64,
) -> (u64, u64, u64, u64) {
    let x = x1 ^ x2;
    let z = z1 ^ z2;
    let x1_z2 = x1 & z2;
    let anticommutes = (x2 & z1) ^ x1_z2;
    let count_2 = count_2 ^ ((count_1 ^ x ^ z ^ x1_z2) & anticommutes);
    (x, z, count_1 ^ anticommutes, count_2)
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
