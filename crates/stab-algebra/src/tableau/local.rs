//! Appending a local Clifford to selected output qubits of every generator row.
//!
//! Appending `G` on targets `t` replaces each row's local Pauli on `t` by its image under `G`
//! and multiplies the row sign by the image sign, because Paulis on disjoint qubits commute and
//! the row's other qubits are unchanged. One- and two-qubit gates are compiled once into a
//! sixteen-entry signed image table; each row then reads its target bits from precomputed word
//! offsets, looks up its image, writes the new bits, and collects sign flips 64 rows per sign
//! word. Wider gates conjugate each gathered local Pauli through the gate's own rows. Either way
//! a gate costs `O(n)` row visits.

use super::action::{apply_words, generator_phases};
use super::small_gate::{LocalImage, SmallGate};
use super::{PauliRows, Tableau};
use crate::budget::{StabilizerResource, try_zeroed_words};
use crate::{StabilizerError, StabilizerResult};

const WORD_BITS: usize = u64::BITS as usize;

pub(super) fn append(
    rows: &mut PauliRows,
    gate: &Tableau,
    targets: &[usize],
) -> StabilizerResult<()> {
    match SmallGate::compile(gate)? {
        Some(small) => append_small(rows, &small, targets),
        None => append_general(rows, gate, targets),
    }
}

/// Bit positions of the local code bits within one packed row.
struct TargetBits<const BITS: usize> {
    words: [usize; BITS],
    shifts: [u32; BITS],
}

impl<const BITS: usize> TargetBits<BITS> {
    /// Locates local bit `2i` (X) and `2i + 1` (Z) of each target `i` in a row of `half` X words
    /// followed by `half` Z words.
    fn new(rows: &PauliRows, targets: &[usize]) -> StabilizerResult<Self> {
        let half = rows.half_words();
        let mut words = [0_usize; BITS];
        let mut shifts = [0_u32; BITS];
        if targets.len() * 2 != BITS {
            return Err(StabilizerError::TableauTargetCountMismatch {
                tableau_qubits: BITS / 2,
                target_count: targets.len(),
            });
        }
        for (local, target) in targets.iter().copied().enumerate() {
            if target >= rows.num_qubits() {
                return Err(StabilizerError::TableauTargetOutOfRange {
                    target,
                    num_qubits: rows.num_qubits(),
                });
            }
            let word = target / WORD_BITS;
            let shift = u32::try_from(target % WORD_BITS).map_err(|_| {
                StabilizerError::TableauTargetOutOfRange {
                    target,
                    num_qubits: rows.num_qubits(),
                }
            })?;
            for (plane, offset) in [(0, 0), (1, half)] {
                let bit = 2 * local + plane;
                if let (Some(word_slot), Some(shift_slot)) =
                    (words.get_mut(bit), shifts.get_mut(bit))
                {
                    *word_slot = offset + word;
                    *shift_slot = shift;
                }
            }
        }
        Ok(Self { words, shifts })
    }
}

fn append_small(rows: &mut PauliRows, gate: &SmallGate, targets: &[usize]) -> StabilizerResult<()> {
    match gate.arity() {
        0 => Ok(()),
        1 => append_table::<2>(rows, gate.images(), &TargetBits::new(rows, targets)?),
        _ => append_table::<4>(rows, gate.images(), &TargetBits::new(rows, targets)?),
    }
}

fn append_table<const BITS: usize>(
    rows: &mut PauliRows,
    images: &[LocalImage; 16],
    bits: &TargetBits<BITS>,
) -> StabilizerResult<()> {
    let row_words = rows.row_words();
    let row_count = rows.row_count();
    if row_words == 0 {
        return Ok(());
    }
    let (words, signs) = rows.words_and_signs_mut();
    let Some(words) = words.get_mut(..row_count * row_words) else {
        return Err(StabilizerError::TableauIndexOutOfRange {
            index: row_count,
            len: words.len() / row_words.max(1),
        });
    };
    let out_of_row = |index: usize| StabilizerError::TableauIndexOutOfRange {
        index,
        len: row_words,
    };
    for (block, sign_word) in words
        .chunks_mut(WORD_BITS * row_words)
        .zip(signs.iter_mut())
    {
        let mut flips = 0_u64;
        for (row_bit, row) in block.chunks_exact_mut(row_words).enumerate() {
            let mut code = 0_usize;
            for (bit, (word, shift)) in bits.words.iter().zip(bits.shifts).enumerate() {
                let value = *row.get(*word).ok_or_else(|| out_of_row(*word))?;
                code |= ((value >> shift & 1) as usize) << bit;
            }
            let Some(image) = images.get(code) else {
                return Err(StabilizerError::TableauIndexOutOfRange {
                    index: code,
                    len: images.len(),
                });
            };
            for (bit, (word, shift)) in bits.words.iter().zip(bits.shifts).enumerate() {
                let slot = row.get_mut(*word).ok_or_else(|| out_of_row(*word))?;
                let value = u64::from(image.code >> bit & 1);
                *slot = (*slot & !(1 << shift)) | (value << shift);
            }
            flips |= u64::from(image.negative) << row_bit;
        }
        *sign_word ^= flips;
    }
    Ok(())
}

fn append_general(rows: &mut PauliRows, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
    let local_rows = gate.rows();
    let local_half = local_rows.half_words();
    let phases = generator_phases(local_rows)?;
    let mut input = try_zeroed_words(StabilizerResource::Tableau, 2 * local_half)?;
    let mut output = try_zeroed_words(StabilizerResource::Tableau, 2 * local_half)?;
    for row in 0..rows.row_count() {
        input.fill(0);
        {
            let (in_x, in_z) = input.split_at_mut(local_half);
            for (local_index, target) in targets.iter().copied().enumerate() {
                let (x, z) = rows.bits(row, target)?;
                set_bit(in_x, local_index, x)?;
                set_bit(in_z, local_index, z)?;
            }
        }
        let (in_x, in_z) = input.split_at(local_half);
        let negative = apply_words(local_rows, Some(&phases), in_x, in_z, false, &mut output)?;
        let (out_x, out_z) = output.split_at(local_half);
        for (local_index, target) in targets.iter().copied().enumerate() {
            rows.set_bits(
                row,
                target,
                get_bit(out_x, local_index)?,
                get_bit(out_z, local_index)?,
            )?;
        }
        if negative {
            rows.flip_sign(row)?;
        }
    }
    Ok(())
}

fn get_bit(words: &[u64], index: usize) -> StabilizerResult<bool> {
    let word = words
        .get(index / WORD_BITS)
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index,
            len: words.len() * WORD_BITS,
        })?;
    Ok(word >> (index % WORD_BITS) & 1 != 0)
}

fn set_bit(words: &mut [u64], index: usize, value: bool) -> StabilizerResult<()> {
    let len = words.len() * WORD_BITS;
    let word = words
        .get_mut(index / WORD_BITS)
        .ok_or(StabilizerError::TableauIndexOutOfRange { index, len })?;
    if value {
        *word |= 1_u64 << (index % WORD_BITS);
    }
    Ok(())
}
