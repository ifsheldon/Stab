//! A transposed copy of the tableau's X bits that turns a collapse's pivot scan into one
//! contiguous column read.
//!
//! A random collapse must find every row with an X bit on the pivot qubit. Row-major storage
//! makes that a strided read of one word from each of the `2n` rows, which dominates a reference
//! run that collapses thousands of qubits between gates, such as the first round of a wide
//! surface code or a layer of random measurements. Like Stim's `TableauTransposedRaii` around a
//! batch of collapses, the simulator transposes the X halves once and reads column `k` as `2n / 64`
//! contiguous words.
//!
//! The copy stays exact while only collapses change rows: a collapse multiplies its
//! anticommuting rows by `m Z_k`, which toggles each such row's X bits on the support of `m`, so
//! the copy toggles the same bits. Sign-only updates (Pauli gates, reset corrections, the
//! collapse gauge) leave it untouched, and every other row update discards it.
//!
//! Building the copy costs about as much as a few dozen strided scans, so it is built ski-rental
//! style: only once the strided scans of consecutive collapses since the last discard have cost
//! as much as a build, which keeps any run within about twice the scan work it would have done
//! without the copy. A collapse whose bookkeeping would cost more than the strided scan it saves
//! discards the copy instead. Many-round circuits whose later rounds are deterministic never
//! scan, so they never build it.
//!
//! Storage is `n * 2n` bits plus `GATHER_WORDS * 2n` gather words and one `2n`-bit row mask,
//! which stays inside the `4 n^2 + 256 n` bytes that reference work admission reserves together
//! with the `0.5 n^2`-byte tableau.

use stab_bits::BitMatrix;

use super::{SimResult, SimulatorWork, WorkModel, invariant, try_resize};
use crate::work_poll::{WorkPoll, chunk_units};

/// Tableau words of each row gathered per pass over the rows while transposing. Gathering
/// several adjacent words per row reads each row's cache line once for that many tiles.
const GATHER_WORDS: usize = 4;

/// Work of toggling one scattered bit of the copy, in [`WorkModel`] units.
const BIT_TOGGLE: u64 = 4;

/// How often this thread built the copy and followed a collapse by row bits or by row mask.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::sampling) struct PivotColumnStats {
    pub(in crate::sampling) builds: u64,
    pub(in crate::sampling) bit_follows: u64,
    pub(in crate::sampling) mask_follows: u64,
}

#[cfg(test)]
thread_local! {
    static PIVOT_COLUMN_STATS: std::cell::Cell<PivotColumnStats> =
        const { std::cell::Cell::new(PivotColumnStats { builds: 0, bit_follows: 0, mask_follows: 0 }) };
}

#[cfg(test)]
pub(in crate::sampling) fn take_pivot_column_stats() -> PivotColumnStats {
    PIVOT_COLUMN_STATS.replace(PivotColumnStats::default())
}

#[cfg(test)]
fn record(update: impl FnOnce(&mut PivotColumnStats)) {
    PIVOT_COLUMN_STATS.with(|cell| {
        let mut stats = cell.get();
        update(&mut stats);
        cell.set(stats);
    });
}

#[derive(Debug, Default)]
pub(super) struct PivotColumns {
    /// Row `q` has bit `r` set when tableau row `r` has an X bit on qubit `q`; allocated on
    /// first build.
    columns: Option<BitMatrix>,
    gather: Vec<u64>,
    mask: Vec<u64>,
    /// Whether `columns` matches the tableau.
    valid: bool,
    /// Strided scan work spent since the copy was last discarded.
    debt: u64,
}

impl PivotColumns {
    /// Discards the copy after a row update it cannot follow.
    #[inline]
    pub(super) fn discard(&mut self) {
        self.valid = false;
        self.debt = 0;
    }

    pub(super) fn is_valid(&self) -> bool {
        self.valid
    }

    /// Records the strided scan of one collapse and reports whether a build has paid off.
    pub(super) fn should_build(&mut self, model: WorkModel, words: usize) -> bool {
        self.debt = self.debt.saturating_add(model.collapse_scan());
        self.debt >= build_work(model, words)
    }

    /// Column `qubit` of the copy, as words over the `2n` tableau rows.
    pub(super) fn column(&self, qubit: usize) -> SimResult<&[u64]> {
        let columns = self
            .columns
            .as_ref()
            .filter(|_| self.valid)
            .ok_or(invariant("pivot columns were read while stale"))?;
        columns
            .row(qubit)
            .map(|column| column.words())
            .map_err(|_| invariant("pivot column is outside the transposed tableau"))
    }

    /// Rebuilds the copy from the tableau's X halves, polling between row chunks.
    pub(super) fn build(
        &mut self,
        rows: &[u64],
        qubit_count: usize,
        words: usize,
        (total, poll, work): Charge<'_, '_, '_>,
    ) -> SimResult<()> {
        self.discard();
        let row_count = 2 * qubit_count;
        let row_words = 2 * words;
        if self.columns.is_none() {
            self.columns = Some(BitMatrix::zeros(qubit_count, row_count).map_err(|_| {
                super::ReferenceSimulatorError::Allocation {
                    component: "reference pivot columns",
                    items: qubit_count.saturating_mul(row_count),
                }
            })?);
        }
        try_resize(
            &mut self.gather,
            GATHER_WORDS.saturating_mul(row_count),
            "reference pivot column gather words",
        )?;
        try_resize(
            &mut self.mask,
            row_count.div_ceil(u64::BITS as usize),
            "reference pivot column row mask",
        )?;
        let columns = self
            .columns
            .as_mut()
            .ok_or(invariant("pivot columns were not allocated"))?;
        let chunk_rows =
            (chunk_units() / usize::try_from(WorkModel::STRIDED_WORD).unwrap_or(5)).max(1);
        let tile_rows = row_count.div_ceil(u64::BITS as usize) as u64;
        for first_word in (0..words).step_by(GATHER_WORDS) {
            let gathered = GATHER_WORDS.min(words - first_word);
            for row_start in (0..row_count).step_by(chunk_rows) {
                let row_end = (row_start + chunk_rows).min(row_count);
                *total = total.saturating_add(
                    WorkModel::STRIDED_WORD.saturating_mul((row_end - row_start) as u64),
                );
                work.charge_total(*total, poll)?;
                for row in row_start..row_end {
                    let source = rows
                        .get(row * row_words + first_word..row * row_words + first_word + gathered)
                        .ok_or(invariant("pivot column gather left the tableau"))?;
                    for (offset, &word) in source.iter().enumerate() {
                        *self
                            .gather
                            .get_mut(offset * row_count + row)
                            .ok_or(invariant("pivot column gather left its buffer"))? = word;
                    }
                }
            }
            for offset in 0..gathered {
                *total = total.saturating_add(tile_rows.saturating_mul(WorkModel::TRANSPOSE_TILE));
                work.charge_total(*total, poll)?;
                let source = self
                    .gather
                    .get(offset * row_count..(offset + 1) * row_count)
                    .ok_or(invariant("pivot column gather left its buffer"))?;
                columns
                    .copy_transpose_from_word_rows_at((first_word + offset) * 64, source)
                    .map_err(|_| invariant("pivot column transposition failed"))?;
            }
        }
        self.valid = true;
        #[cfg(test)]
        record(|stats| stats.builds += 1);
        Ok(())
    }

    /// Toggles, on every column of `multiplier_x`'s support, the bits of `rows`, which a collapse
    /// has just multiplied by a Pauli with that X part.
    ///
    /// Discards the copy instead when that would cost more than the strided scan it replaces.
    pub(super) fn follow_collapse(
        &mut self,
        multiplier_x: &[u64],
        rows: &[usize],
        model: WorkModel,
        (total, poll, work): Charge<'_, '_, '_>,
    ) -> SimResult<()> {
        if !self.valid {
            return Ok(());
        }
        // Stale until every toggle lands, so a cancelled update leaves nothing to trust.
        self.valid = false;
        let support = multiplier_x
            .iter()
            .map(|word| u64::from(word.count_ones()))
            .sum::<u64>();
        let mask_words = self.mask.len() as u64;
        let by_bits = (rows.len() as u64).saturating_mul(BIT_TOGGLE);
        let per_column = by_bits.min(mask_words.saturating_mul(2)).max(1);
        if support.saturating_mul(per_column) > model.collapse_scan() {
            self.debt = 0;
            return Ok(());
        }
        let use_mask = per_column < by_bits;
        #[cfg(test)]
        record(|stats| {
            if use_mask {
                stats.mask_follows += 1;
            } else {
                stats.bit_follows += 1;
            }
        });
        if use_mask {
            self.mask.fill(0);
            for &row in rows {
                let word = self
                    .mask
                    .get_mut(row / u64::BITS as usize)
                    .ok_or(invariant("collapse row is outside the pivot column mask"))?;
                *word ^= 1 << (row % u64::BITS as usize);
            }
        }
        let columns = self
            .columns
            .as_mut()
            .ok_or(invariant("pivot columns were not allocated"))?;
        // Charge and poll per bounded chunk of columns, as the strided scan does per row chunk.
        let chunk_columns = (chunk_units() as u64 / per_column).max(1);
        let mut charged_columns = 0;
        for (word_index, &word) in multiplier_x.iter().enumerate() {
            let mut remaining = word;
            while remaining != 0 {
                let qubit = word_index * u64::BITS as usize + remaining.trailing_zeros() as usize;
                remaining &= remaining - 1;
                if charged_columns == 0 {
                    charged_columns = chunk_columns;
                    *total =
                        total.saturating_add(chunk_columns.min(support).saturating_mul(per_column));
                    work.charge_total(*total, poll)?;
                }
                charged_columns -= 1;
                let mut column = columns
                    .row_words_mut(qubit)
                    .map_err(|_| invariant("collapse support is outside the pivot columns"))?;
                if use_mask {
                    for (target, &source) in column.iter_mut().zip(&self.mask) {
                        *target ^= source;
                    }
                } else {
                    for &row in rows {
                        let target = column
                            .get_mut(row / u64::BITS as usize)
                            .ok_or(invariant("collapse row is outside the pivot columns"))?;
                        *target ^= 1 << (row % u64::BITS as usize);
                    }
                }
            }
        }
        self.valid = true;
        Ok(())
    }
}

/// The simulator's work tally, the poll it forwards to, and the work already forwarded.
pub(super) type Charge<'t, 'p, 'a> = (&'t mut u64, &'p mut WorkPoll<'a>, &'t mut SimulatorWork);

/// Estimated work of one build: a strided gather pass per `GATHER_WORDS` row words and one
/// 64-bit tile transposition per 64 rows and row word.
fn build_work(model: WorkModel, words: usize) -> u64 {
    let rows = model.qubit_count().saturating_mul(2);
    let passes = words.div_ceil(GATHER_WORDS) as u64;
    let tiles = (words as u64).saturating_mul(rows.div_ceil(u64::from(u64::BITS)));
    passes
        .saturating_mul(rows.saturating_mul(WorkModel::STRIDED_WORD))
        .saturating_add(tiles.saturating_mul(WorkModel::TRANSPOSE_TILE))
}
