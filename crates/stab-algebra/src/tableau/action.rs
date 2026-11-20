//! Word-parallel Tableau actions: conjugating packed Pauli rows, composition, and inversion.
//!
//! A Tableau `T` maps a Hermitian Pauli `P = (-1)^s i^{y(a,b)} X^a Z^b`, where `y` counts the
//! qubits carrying `Y`, to `(-1)^s i^{y(a,b)} prod_j G_j` over the selected generator images
//! `G_j = T(X_k)` or `T(Z_k)`, taken in the order `X_0..X_{n-1}, Z_0..Z_{n-1}`. Writing each
//! image as `(-1)^{s_j} i^{y_j} X^{x_j} Z^{z_j}`, the ordered product of the `X^{x_j} Z^{z_j}`
//! factors is `(-1)^Q X^{sum x_j} Z^{sum z_j}` with `Q = sum_{j<l} z_j . x_l`. The accumulated
//! Z plane therefore gives `Q` one AND-parity per selected generator, and the result's phase
//! exponent `2s + y_in + sum (2 s_j + y_j) + 2Q - y_out` must be even, its half being the sign.
//! A single action costs one fused pass over `2 n / 64` words per selected generator. Batches of
//! at least 256 rows instead tabulate the ordered products of each eight consecutive generators
//! once per block of 1024 rows, so a row pays one pass per selection byte; composing or inverting
//! an `n`-qubit Tableau stays `O(n^3 / 64)` word operations with an eight-fold smaller constant.

use stab_bits::BitMatrix;

use super::PauliRows;
use super::rows::y_count_mod_4;
use crate::budget::{StabilizerResource, try_filled, try_zeroed_words};
use crate::{PauliPhase, StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;

/// Per-generator phase constants `2 s_j + y_j (mod 4)` of a Tableau's rows.
pub(super) fn generator_phases(table: &PauliRows) -> StabilizerResult<Vec<u8>> {
    let mut phases = try_filled(StabilizerResource::Tableau, table.row_count(), 0_u8)?;
    for (row, phase) in phases.iter_mut().enumerate() {
        let words = table.row(row)?;
        *phase = (2 * u8::from(table.sign(row)?) + y_count_mod_4(words, table.half_words())) & 3;
    }
    Ok(phases)
}

/// Writes the packed image of the Pauli `(in_x, in_z, in_negative)` under `table` into `output`
/// and returns whether the image is negative.
///
/// `phases` caches [`generator_phases`] for batches; a single action computes the phase of each
/// selected generator on the fly so its cost stays proportional to the input weight.
pub(super) fn apply_words(
    table: &PauliRows,
    phases: Option<&[u8]>,
    in_x: &[u64],
    in_z: &[u64],
    in_negative: bool,
    output: &mut [u64],
) -> StabilizerResult<bool> {
    let num_qubits = table.num_qubits();
    let half_words = table.half_words();
    output.fill(0);
    let (out_x, out_z) = output.split_at_mut(half_words);
    let mut exponent = 2 * u32::from(in_negative)
        + in_x
            .iter()
            .zip(in_z)
            .map(|(x, z)| (x & z).count_ones())
            .fold(0_u32, u32::wrapping_add);
    let mut parity = 0_u64;
    for (plane, generator_offset) in [(in_x, 0), (in_z, num_qubits)] {
        for (word_index, word) in plane.iter().copied().enumerate() {
            let mut bits = word;
            while let Some(bit) = bits.lowest_one() {
                let qubit = word_index * WORD_BITS + bit as usize;
                bits &= bits - 1;
                if qubit >= num_qubits {
                    return Err(StabilizerError::LengthMismatch {
                        left: qubit + 1,
                        right: num_qubits,
                    });
                }
                let generator = generator_offset + qubit;
                let row = table.row(generator)?;
                let phase = match phases {
                    Some(phases) => {
                        *phases
                            .get(generator)
                            .ok_or(StabilizerError::TableauIndexOutOfRange {
                                index: generator,
                                len: phases.len(),
                            })?
                    }
                    None => 2 * u8::from(table.sign(generator)?) + y_count_mod_4(row, half_words),
                };
                exponent = exponent.wrapping_add(u32::from(phase));
                let (row_x, row_z) = row.split_at(half_words);
                parity ^= crate::kernels::pauli_accumulate_words(out_x, out_z, row_x, row_z);
            }
        }
    }
    exponent = exponent.wrapping_add(2 * (parity.count_ones() & 1));
    let output_y = out_x
        .iter()
        .zip(out_z.iter())
        .map(|(x, z)| (x & z).count_ones())
        .fold(0_u32, u32::wrapping_add);
    exponent = exponent.wrapping_sub(output_y) & 3;
    if exponent & 1 != 0 {
        return Err(StabilizerError::ImaginaryProduct {
            phase: PauliPhase::from_log_i((exponent & 3) as u8),
        });
    }
    Ok(exponent == 2)
}

/// Input rows that share each generator lookup table in [`BlockAction::apply`].
const BLOCK_ROWS: usize = 1024;
/// Below this many rows the operands stay in cache and a lookup table would cost more than it
/// saves, so rows are conjugated one at a time.
const BLOCKED_MIN_ROWS: usize = 256;
/// Generators combined per lookup table: one byte of an input row's packed selection.
const TABLE_GENERATORS: usize = 8;
const TABLE_ENTRIES: usize = 1 << TABLE_GENERATORS;

/// Conjugates blocks of rows with the method of four Russians: for every eight consecutive
/// generators, the 256 ordered products of their images are tabulated once per block, with each
/// product's internal commutation parity and phase, and every row of the block then accumulates
/// one table entry per selection byte instead of up to eight generator rows.
pub(super) struct BlockAction {
    phases: Vec<u8>,
    table_words: Vec<u64>,
    table_parities: [u64; TABLE_ENTRIES],
    table_phases: [u32; TABLE_ENTRIES],
    exponents: Vec<u32>,
    parities: Vec<u64>,
}

impl BlockAction {
    pub(super) fn new(table: &PauliRows) -> StabilizerResult<Self> {
        let row_words = 2 * table.half_words();
        Ok(Self {
            phases: generator_phases(table)?,
            table_words: try_zeroed_words(StabilizerResource::Tableau, TABLE_ENTRIES * row_words)?,
            table_parities: [0; TABLE_ENTRIES],
            table_phases: [0; TABLE_ENTRIES],
            exponents: crate::budget::try_filled(StabilizerResource::Tableau, BLOCK_ROWS, 0_u32)?,
            parities: try_zeroed_words(StabilizerResource::Tableau, BLOCK_ROWS)?,
        })
    }

    /// Conjugates input rows `first..first + count` (at most [`BLOCK_ROWS`]) by `table`, writing
    /// the packed images into `output` (`count` consecutive rows) and the negative images into
    /// `negative`.
    fn apply(
        &mut self,
        table: &PauliRows,
        input: &PauliRows,
        first: usize,
        count: usize,
        output: &mut [u64],
        negative: &mut [bool],
    ) -> StabilizerResult<()> {
        let num_qubits = table.num_qubits();
        let half_words = table.half_words();
        let row_words = 2 * half_words;
        let rows = input
            .rows(first, count)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: first + count,
                len: input.row_count(),
            })?;
        let Self {
            phases,
            table_words,
            table_parities,
            table_phases,
            exponents,
            parities,
        } = self;
        let exponents = exponents
            .get_mut(..count)
            .ok_or(StabilizerError::InvalidTableauInverse)?;
        let parities = parities
            .get_mut(..count)
            .ok_or(StabilizerError::InvalidTableauInverse)?;
        for ((exponent, parity), (offset, row)) in exponents
            .iter_mut()
            .zip(parities.iter_mut())
            .zip(rows.chunks_exact(row_words).enumerate())
        {
            let (in_x, in_z) = row.split_at(half_words);
            *exponent = 2 * u32::from(input.sign(first + offset)?) + y_count(in_x, in_z);
            *parity = 0;
        }
        output.fill(0);
        for word_index in 0..row_words {
            let (plane_offset, plane_word) = if word_index < half_words {
                (0, word_index)
            } else {
                (num_qubits, word_index - half_words)
            };
            let used = rows.chunks_exact(row_words).fold(0_u64, |used, row| {
                used | row.get(word_index).copied().unwrap_or(0)
            });
            for chunk in 0..WORD_BITS / TABLE_GENERATORS {
                let shift = chunk * TABLE_GENERATORS;
                if (used >> shift) & 0xff == 0 {
                    continue;
                }
                let first_qubit = plane_word * WORD_BITS + shift;
                build_table(
                    table,
                    phases,
                    plane_offset,
                    first_qubit,
                    table_words,
                    table_parities,
                    table_phases,
                )?;
                for (((offset, row), target), (exponent, parity)) in rows
                    .chunks_exact(row_words)
                    .enumerate()
                    .zip(output.chunks_exact_mut(row_words))
                    .zip(exponents.iter_mut().zip(parities.iter_mut()))
                {
                    let selected = usize::from(
                        ((row.get(word_index).copied().unwrap_or(0) >> shift) & 0xff) as u8,
                    );
                    if selected == 0 {
                        continue;
                    }
                    let entry = table_words
                        .get(selected * row_words..(selected + 1) * row_words)
                        .ok_or(StabilizerError::TableauIndexOutOfRange {
                            index: first + offset,
                            len: input.row_count(),
                        })?;
                    let (entry_x, entry_z) = entry.split_at(half_words);
                    let (out_x, out_z) = target.split_at_mut(half_words);
                    *parity ^=
                        crate::kernels::pauli_accumulate_words(out_x, out_z, entry_x, entry_z)
                            ^ table_parities.get(selected).copied().unwrap_or(0);
                    *exponent =
                        exponent.wrapping_add(table_phases.get(selected).copied().unwrap_or(0));
                }
            }
        }
        for (((target, exponent), parity), negative) in output
            .chunks_exact(row_words)
            .zip(exponents.iter())
            .zip(parities.iter())
            .zip(negative.iter_mut())
        {
            let (out_x, out_z) = target.split_at(half_words);
            let exponent = exponent
                .wrapping_add(2 * (parity.count_ones() & 1))
                .wrapping_sub(y_count(out_x, out_z))
                & 3;
            if exponent & 1 != 0 {
                return Err(StabilizerError::ImaginaryProduct {
                    phase: PauliPhase::from_log_i((exponent & 3) as u8),
                });
            }
            *negative = exponent == 2;
        }
        Ok(())
    }
}

/// Tabulates the ordered products of the images of qubits `first_qubit..first_qubit + 8` of
/// one plane, whose generator rows start at `plane_offset`; qubits past the width contribute
/// identity.
fn build_table(
    table: &PauliRows,
    phases: &[u8],
    plane_offset: usize,
    first_qubit: usize,
    table_words: &mut [u64],
    table_parities: &mut [u64; TABLE_ENTRIES],
    table_phases: &mut [u32; TABLE_ENTRIES],
) -> StabilizerResult<()> {
    let half_words = table.half_words();
    let row_words = 2 * half_words;
    if let Some(identity) = table_words.get_mut(..row_words) {
        identity.fill(0);
    }
    for bit in 0..TABLE_GENERATORS {
        let qubit = first_qubit + bit;
        let low = 1_usize << bit;
        let (generator_row, phase) =
            if qubit < table.num_qubits() {
                let generator = plane_offset + qubit;
                let phase = phases.get(generator).copied().ok_or(
                    StabilizerError::TableauIndexOutOfRange {
                        index: generator,
                        len: phases.len(),
                    },
                )?;
                (Some(table.row(generator)?), u32::from(phase))
            } else {
                (None, 0)
            };
        // Entries `low..2 low` add this generator after the products of the lower ones.
        let (lower, upper) = table_words.split_at_mut(low * row_words);
        for (offset, (source, target)) in lower
            .chunks_exact(row_words)
            .zip(upper.chunks_exact_mut(row_words))
            .take(low)
            .enumerate()
        {
            target.copy_from_slice(source);
            let parity = match generator_row {
                Some(generator) => {
                    let (target_x, target_z) = target.split_at_mut(half_words);
                    let (generator_x, generator_z) = generator.split_at(half_words);
                    crate::kernels::pauli_accumulate_words(
                        target_x,
                        target_z,
                        generator_x,
                        generator_z,
                    )
                }
                None => 0,
            };
            let lower_parity = table_parities.get(offset).copied().unwrap_or(0);
            let lower_phase = table_phases.get(offset).copied().unwrap_or(0);
            if let (Some(entry_parity), Some(entry_phase)) = (
                table_parities.get_mut(low + offset),
                table_phases.get_mut(low + offset),
            ) {
                *entry_parity = u64::from((lower_parity ^ parity).count_ones() & 1);
                *entry_phase = lower_phase.wrapping_add(phase);
            }
        }
    }
    Ok(())
}

fn y_count(xs: &[u64], zs: &[u64]) -> u32 {
    xs.iter()
        .zip(zs)
        .map(|(x, z)| (x & z).count_ones())
        .fold(0_u32, u32::wrapping_add)
}

/// Writes `table(input_row)` for every row of `input` into `output`, keeping input signs.
pub(super) fn apply_rows(
    table: &PauliRows,
    input: &PauliRows,
    output: &mut PauliRows,
) -> StabilizerResult<()> {
    if input.row_count() < BLOCKED_MIN_ROWS {
        let phases = generator_phases(table)?;
        let half_words = input.half_words();
        for row in 0..input.row_count() {
            let (in_x, in_z) = input.row(row)?.split_at(half_words);
            let negative = apply_words(
                table,
                Some(&phases),
                in_x,
                in_z,
                input.sign(row)?,
                output.row_mut(row)?,
            )?;
            output.set_sign(row, negative)?;
        }
        return Ok(());
    }
    let mut action = BlockAction::new(table)?;
    let mut negative = crate::budget::try_filled(StabilizerResource::Tableau, BLOCK_ROWS, false)?;
    for first in (0..input.row_count()).step_by(BLOCK_ROWS) {
        let count = BLOCK_ROWS.min(input.row_count() - first);
        let flags = negative
            .get_mut(..count)
            .ok_or(StabilizerError::InvalidTableauInverse)?;
        action.apply(
            table,
            input,
            first,
            count,
            output.rows_mut(first, count)?,
            flags,
        )?;
        for (offset, flag) in flags.iter().enumerate() {
            output.set_sign(first + offset, *flag)?;
        }
    }
    Ok(())
}

/// Writes the unsigned inverse of `table` into the zeroed `output` by transposing quadrants.
///
/// With `XX`, `XZ`, `ZX`, and `ZZ` the quadrants of generator rows by output bit planes, the
/// inverse's quadrants are `ZZ^T`, `XZ^T`, `ZX^T`, and `XX^T`, as in Stim's `Tableau::inverse`.
pub(super) fn transpose_inverse_into(
    table: &PauliRows,
    output: &mut PauliRows,
) -> StabilizerResult<()> {
    let num_qubits = table.num_qubits();
    if num_qubits == 0 {
        return Ok(());
    }
    let mut scratch = BitMatrix::zeros(num_qubits, num_qubits)?;
    // (source row offset, source half, target row offset, target half)
    for (source_rows, source_half, target_rows, target_half) in [
        (num_qubits, 1, 0, 0),
        (0, 1, 0, 1),
        (num_qubits, 0, num_qubits, 0),
        (0, 0, num_qubits, 1),
    ] {
        copy_quadrant_into(table, source_rows, source_half, &mut scratch)?;
        scratch.transpose_square_in_place()?;
        copy_quadrant_from(&scratch, output, target_rows, target_half)?;
    }
    Ok(())
}

fn copy_quadrant_into(
    table: &PauliRows,
    row_offset: usize,
    half: usize,
    scratch: &mut BitMatrix,
) -> StabilizerResult<()> {
    let half_words = table.half_words();
    for row in 0..table.num_qubits() {
        let words = table.row(row_offset + row)?;
        let source = words
            .get(half * half_words..(half + 1) * half_words)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: row_offset + row,
                len: table.row_count(),
            })?;
        let mut target = scratch.row_words_mut(row)?;
        if target.len() != source.len() {
            return Err(StabilizerError::LengthMismatch {
                left: source.len(),
                right: target.len(),
            });
        }
        target.copy_from_slice(source);
    }
    Ok(())
}

fn copy_quadrant_from(
    scratch: &BitMatrix,
    table: &mut PauliRows,
    row_offset: usize,
    half: usize,
) -> StabilizerResult<()> {
    let half_words = table.half_words();
    let row_count = table.row_count();
    for row in 0..table.num_qubits() {
        let source = scratch.row(row)?;
        let words = table.row_mut(row_offset + row)?;
        let target = words
            .get_mut(half * half_words..(half + 1) * half_words)
            .ok_or(StabilizerError::TableauIndexOutOfRange {
                index: row_offset + row,
                len: row_count,
            })?;
        if target.len() != source.words().len() {
            return Err(StabilizerError::LengthMismatch {
                left: source.words().len(),
                right: target.len(),
            });
        }
        target.copy_from_slice(source.words());
    }
    Ok(())
}

/// Signs the transposed inverse rows of `table`: `inverse` row `r` must map back to the `r`-th
/// canonical generator, and the sign of that round trip is the inverse row's sign.
pub(super) fn fix_inverse_signs(
    table: &PauliRows,
    inverse: &mut PauliRows,
) -> StabilizerResult<()> {
    let num_qubits = table.num_qubits();
    let half_words = table.half_words();
    let row_words = 2 * half_words;
    let rows = inverse.row_count();
    let block_rows = if rows < BLOCKED_MIN_ROWS {
        1
    } else {
        BLOCK_ROWS.min(rows)
    };
    let mut scratch = try_zeroed_words(StabilizerResource::Tableau, block_rows * row_words)?;
    let mut negative = crate::budget::try_filled(StabilizerResource::Tableau, block_rows, false)?;
    let mut action = if block_rows > 1 {
        Some(BlockAction::new(table)?)
    } else {
        None
    };
    let phases = generator_phases(table)?;
    for first in (0..rows).step_by(block_rows) {
        let count = block_rows.min(rows - first);
        let images = scratch
            .get_mut(..count * row_words)
            .ok_or(StabilizerError::InvalidTableauInverse)?;
        let flags = negative
            .get_mut(..count)
            .ok_or(StabilizerError::InvalidTableauInverse)?;
        match action.as_mut() {
            Some(action) => action.apply(table, inverse, first, count, images, flags)?,
            None => {
                let (in_x, in_z) = inverse.row(first)?.split_at(half_words);
                let flag = flags
                    .first_mut()
                    .ok_or(StabilizerError::InvalidTableauInverse)?;
                *flag = apply_words(table, Some(&phases), in_x, in_z, false, images)?;
            }
        }
        for (offset, (image, flag)) in images.chunks_exact(row_words).zip(flags.iter()).enumerate()
        {
            let row = first + offset;
            let (qubit, plane) = if row < num_qubits {
                (row, 0)
            } else {
                (row - num_qubits, 1)
            };
            let expected_word = plane * half_words + qubit / WORD_BITS;
            let expected_bit = 1_u64 << (qubit % WORD_BITS);
            let round_trips = image.iter().enumerate().all(|(index, word)| {
                *word
                    == if index == expected_word {
                        expected_bit
                    } else {
                        0
                    }
            });
            if !round_trips {
                return Err(StabilizerError::InvalidTableauInverse);
            }
            inverse.set_sign(row, *flag)?;
        }
    }
    Ok(())
}
