//! State snapshots and the stabilizer-state comparison that lets repeat folding recognize a
//! recurring state.
//!
//! A snapshot keeps an exact copy of the tableau. Comparing a later state against it needs each
//! snapshot stabilizer `S_k = T_s Z_k T_s^-1`, whose X and Z components are bit `k` of the X
//! halves of the snapshot rows `T_s^-1 Z_j T_s` and `T_s^-1 X_j T_s`. Those bits form columns of
//! the rows, so the snapshot transposes its X halves one 64-stabilizer block at a time, which
//! takes `O(n / 64)` word operations per stabilizer instead of `2n` strided bit reads. The
//! transposed supports and the phase of each snapshot stabilizer depend on the snapshot only,
//! so they are prepared on first use and reused by every later comparison against the same
//! capture.

use stab_algebra::PauliBasis;
use stab_bits::BitMatrix;

use crate::detection::frame::PauliTerm;
use crate::work_poll::{WorkPoll, chunk_units};

use super::{
    InverseTableauSimulator, ReferenceSimulatorError, SimResult, SimulatorWork, WorkModel,
    first_set_bit, invariant, pauli_image_append, try_resize, try_zeroed,
};

const BLOCK_BITS: usize = u64::BITS as usize;

/// Exact copy of a simulator state, with lazily prepared comparison data.
#[derive(Debug)]
pub(in crate::sampling) struct ReferenceStateSnapshot {
    qubit_count: usize,
    rows: Vec<u64>,
    signs: Vec<bool>,
    /// Row `k` holds the Z components of `S_k`: bit `j` is X bit `k` of `T_s^-1 X_j T_s`.
    support_z: BitMatrix,
    /// Row `k` holds the X components of `S_k`: bit `j` is X bit `k` of `T_s^-1 Z_j T_s`.
    support_x: BitMatrix,
    /// One X word per row, gathered for one block transposition.
    gather: Vec<u64>,
    /// Stabilizer blocks of 64 whose supports are transposed for the current capture.
    transposed_blocks: usize,
    /// Phase, as a power of `i`, of `T_s^-1 P_k T_s` for the unsigned support `P_k` of `S_k`.
    phases: Vec<u8>,
    /// Stabilizers whose phase is prepared for the current capture.
    phased: usize,
    /// Row products of one full comparison: the X bits in the snapshot's X halves.
    support_rows: u64,
}

impl ReferenceStateSnapshot {
    /// Allocates snapshot storage for `qubit_count` qubits with fallible reservations.
    pub(in crate::sampling) fn try_new(qubit_count: usize) -> SimResult<Self> {
        let words = qubit_count.div_ceil(BLOCK_BITS);
        let row_count = qubit_count
            .checked_mul(2)
            .ok_or(ReferenceSimulatorError::Allocation {
                component: "reference snapshot rows",
                items: usize::MAX,
            })?;
        let word_count = row_count
            .checked_mul(words)
            .and_then(|count| count.checked_mul(2));
        let word_count = word_count.ok_or(ReferenceSimulatorError::Allocation {
            component: "reference snapshot row words",
            items: usize::MAX,
        })?;
        let rows = try_zeroed(word_count, "reference snapshot row words")?;
        let mut signs = Vec::new();
        try_resize(&mut signs, row_count, "reference snapshot row signs")?;
        let support = || {
            BitMatrix::zeros(qubit_count, qubit_count).map_err(|_| {
                ReferenceSimulatorError::Allocation {
                    component: "reference snapshot stabilizer supports",
                    items: qubit_count.saturating_mul(qubit_count),
                }
            })
        };
        let support_z = support()?;
        let support_x = support()?;
        let gather = try_zeroed(qubit_count, "reference snapshot transposition rows")?;
        let mut phases = Vec::new();
        try_resize(
            &mut phases,
            qubit_count,
            "reference snapshot stabilizer phases",
        )?;
        Ok(Self {
            qubit_count,
            rows,
            signs,
            support_z,
            support_x,
            gather,
            transposed_blocks: 0,
            phases,
            phased: 0,
            support_rows: 0,
        })
    }

    /// Estimated work of comparing a state against this capture when every stabilizer matches,
    /// counting only the comparison data not prepared yet.
    pub(in crate::sampling) fn full_comparison_work(&self, model: WorkModel) -> u64 {
        let blocks = self.qubit_count.div_ceil(BLOCK_BITS);
        let untransposed = blocks.saturating_sub(self.transposed_blocks) as u64;
        // Phases are prepared in stabilizer order; charge the unprepared share of the rows.
        let unphased = self.qubit_count.saturating_sub(self.phased) as u128;
        let unphased_rows = u128::from(self.support_rows)
            .saturating_mul(unphased)
            .checked_div(self.qubit_count as u128)
            .unwrap_or(0);
        model.comparison(
            self.support_rows,
            untransposed,
            u64::try_from(unphased_rows).unwrap_or(u64::MAX),
        )
    }

    /// Transposes stabilizer block `block` of the X halves, if the capture has not yet.
    ///
    /// Blocks are prepared in order, so `block` is at most one past the prepared blocks.
    fn ensure_transposed(&mut self, block: usize, words: usize) -> SimResult<bool> {
        if block < self.transposed_blocks {
            return Ok(false);
        }
        if block != self.transposed_blocks {
            return Err(invariant("snapshot supports were transposed out of order"));
        }
        let row_words = 2 * words;
        let qubits = self.qubit_count;
        let rows = self
            .rows
            .get(..2 * qubits * row_words)
            .ok_or(invariant("snapshot rows are shorter than the tableau"))?;
        let (input_x, input_z) = rows.split_at(qubits * row_words);
        for (matrix, source) in [
            (&mut self.support_z, input_x),
            (&mut self.support_x, input_z),
        ] {
            for (slot, row) in self.gather.iter_mut().zip(source.chunks_exact(row_words)) {
                *slot = row
                    .get(block)
                    .copied()
                    .ok_or(invariant("snapshot support block is outside its row"))?;
            }
            matrix
                .copy_transpose_from_word_rows_at(block * BLOCK_BITS, &self.gather)
                .map_err(|_| invariant("snapshot support transposition failed"))?;
        }
        self.transposed_blocks += 1;
        Ok(true)
    }

    /// Writes the unsigned support of stabilizer `stabilizer` into `terms` in qubit order.
    fn support_terms(&self, stabilizer: usize, terms: &mut Vec<PauliTerm>) -> SimResult<()> {
        let support_error = |_| invariant("snapshot stabilizer is outside its supports");
        let z_words = self
            .support_z
            .row(stabilizer)
            .map_err(support_error)?
            .words();
        let x_words = self
            .support_x
            .row(stabilizer)
            .map_err(support_error)?
            .words();
        terms.clear();
        for (word_index, (&z_word, &x_word)) in z_words.iter().zip(x_words).enumerate() {
            let mut remaining = z_word | x_word;
            while remaining != 0 {
                let bit = remaining.trailing_zeros();
                remaining &= remaining - 1;
                let mask = 1_u64 << bit;
                let qubit =
                    u32::try_from(word_index * BLOCK_BITS + bit as usize).map_err(|_| {
                        invariant("snapshot stabilizer support exceeds the qubit range")
                    })?;
                terms.push((
                    qubit,
                    PauliBasis::from_xz(x_word & mask != 0, z_word & mask != 0),
                ));
            }
        }
        Ok(())
    }
}

impl InverseTableauSimulator {
    /// Copies the current state into `snapshot`, discarding its prepared comparison data.
    ///
    /// Returns false, leaving the snapshot unusable for comparison, when it was sized for a
    /// different qubit count.
    pub(in crate::sampling) fn capture(&mut self, snapshot: &mut ReferenceStateSnapshot) -> bool {
        if snapshot.qubit_count != self.qubit_count
            || snapshot.rows.len() != self.rows.len()
            || snapshot.signs.len() != self.signs.len()
        {
            return false;
        }
        snapshot.rows.copy_from_slice(&self.rows);
        snapshot.signs.copy_from_slice(&self.signs);
        snapshot.transposed_blocks = 0;
        snapshot.phased = 0;
        snapshot.support_rows = self.support_rows();
        self.charge(self.model.capture());
        true
    }

    /// Number of row products a comparison against a capture of the current state performs.
    pub(in crate::sampling) fn current_support_rows(&mut self) -> u64 {
        self.charge(self.model.tableau_pass() / 2);
        self.support_rows()
    }

    fn support_rows(&self) -> u64 {
        let row_words = 2 * self.words;
        if row_words == 0 {
            return 0;
        }
        self.rows
            .chunks_exact(row_words)
            .flat_map(|row| row.iter().take(self.words))
            .map(|word| u64::from(word.count_ones()))
            .sum()
    }

    /// Reports whether the current state equals the snapshot's state.
    ///
    /// Identical tableaux are accepted directly. Otherwise each stabilizer generator
    /// `S_k = T_s Z_k T_s^-1` of the snapshot state is rebuilt from the snapshot's transposed
    /// X halves, signed by `T_s^-1 S_k T_s = +Z_k`, and checked to be a stabilizer of the current
    /// state with the same sign, stopping at the first that is not. This recognizes states that
    /// recur although the tableau that prepares them does not, as in every round of a memory
    /// experiment.
    pub(in crate::sampling) fn same_state_as(
        &mut self,
        snapshot: &mut ReferenceStateSnapshot,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<bool> {
        let row_words = (2 * self.words).max(1);
        if snapshot.signs == self.signs {
            let row_copy_work = self.model.row_copy();
            let chunk_rows = (chunk_units()
                / usize::try_from(row_copy_work.max(1)).unwrap_or(usize::MAX))
            .max(1);
            let total_rows = 2 * self.qubit_count;
            let mut row_start = 0;
            let mut found_mismatch = false;
            while row_start < total_rows {
                let row_end = (row_start + chunk_rows).min(total_rows);
                let byte_range = row_start * row_words..row_end * row_words;
                let mismatch = match (
                    snapshot.rows.get(byte_range.clone()),
                    self.rows.get(byte_range),
                ) {
                    (Some(snap_slice), Some(cur_slice)) => snap_slice
                        .chunks(row_words)
                        .zip(cur_slice.chunks(row_words))
                        .position(|(snapshot_row, row)| snapshot_row != row),
                    _ => Some(0),
                };
                if let Some(offset) = mismatch {
                    self.charge(row_copy_work.saturating_mul((offset + 1) as u64));
                    work.charge(self, poll)?;
                    found_mismatch = true;
                    break;
                }
                self.charge(row_copy_work.saturating_mul((row_end - row_start) as u64));
                work.charge(self, poll)?;
                row_start = row_end;
            }
            if !found_mismatch && snapshot.rows.len() == self.rows.len() {
                return Ok(true);
            }
        }
        if snapshot.qubit_count != self.qubit_count
            || snapshot.rows.len() != self.rows.len()
            || snapshot.signs.len() != self.signs.len()
        {
            return Ok(false);
        }
        let mut terms = std::mem::take(&mut self.stabilizer_terms);
        if terms.capacity() < self.qubit_count
            && terms
                .try_reserve_exact(self.qubit_count - terms.len())
                .is_err()
        {
            self.stabilizer_terms = terms;
            return Err(ReferenceSimulatorError::Allocation {
                component: "reference state comparison terms",
                items: self.qubit_count,
            });
        }
        let mut result = Ok(true);
        for stabilizer in 0..self.qubit_count {
            match self.stabilizer_recurs(snapshot, stabilizer, &mut terms, poll, work) {
                Ok(true) => {}
                Ok(false) => {
                    result = Ok(false);
                    break;
                }
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        terms.clear();
        self.stabilizer_terms = terms;
        result
    }

    /// Whether snapshot stabilizer `stabilizer` stabilizes the current state with its sign.
    fn stabilizer_recurs(
        &mut self,
        snapshot: &mut ReferenceStateSnapshot,
        stabilizer: usize,
        terms: &mut Vec<PauliTerm>,
        poll: &mut WorkPoll<'_>,
        work: &mut SimulatorWork,
    ) -> SimResult<bool> {
        if snapshot.ensure_transposed(stabilizer / BLOCK_BITS, self.words)? {
            self.charge(self.model.transpose_block());
            work.charge(self, poll)?;
        }
        snapshot.support_terms(stabilizer, terms)?;
        self.charge(self.model.support_scan());
        work.charge(self, poll)?;
        let snapshot_log_i = if stabilizer < snapshot.phased {
            snapshot
                .phases
                .get(stabilizer)
                .copied()
                .ok_or(invariant("snapshot stabilizer phase is missing"))?
        } else if stabilizer == snapshot.phased {
            // x_j(S_k) = <S_k, Z_j> and z_j(S_k) = <S_k, X_j>, read through T_s^-1.
            self.product.fill(0);
            self.charge(self.model.row_copy());
            let per_term_units =
                usize::try_from(self.model.row_multiply().saturating_mul(2).max(1))
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
                    &snapshot.rows,
                    &snapshot.signs,
                    self.qubit_count,
                    self.words,
                    chunk,
                    &mut self.product,
                )?;
                log_i = (log_i + delta) & 3;
            }
            let (image_x, _) = self.product.split_at(self.words);
            if first_set_bit(image_x).is_some() {
                return Err(invariant("snapshot stabilizer did not map back to Z"));
            }
            *snapshot
                .phases
                .get_mut(stabilizer)
                .ok_or(invariant("snapshot stabilizer phase is missing"))? = log_i;
            snapshot.phased += 1;
            log_i
        } else {
            return Err(invariant(
                "snapshot stabilizer phases were prepared out of order",
            ));
        };
        let current_log_i = self.observable_into_product(terms, poll, work)?;
        let (image_x, _) = self.product.split_at(self.words);
        Ok(first_set_bit(image_x).is_none() && current_log_i == snapshot_log_i)
    }
}
