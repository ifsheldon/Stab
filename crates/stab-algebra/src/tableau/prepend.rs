//! Prepending a local Clifford: rewriting only the generator rows of its targets.
//!
//! Prepending `G` on targets `t` gives `T' = T . G`, so `T'(g) = T(G(g))` for every generator
//! `g`. A generator off the targets is fixed by `G`, so its row is unchanged. A target generator
//! `g` has the local image `(-1)^s prod_j B_j` with `B_j` acting on `t_j`, so its new row is the
//! signed product of the old rows of the target generators that `G(g)` selects, with `Y` read as
//! `iXZ`. A `k`-qubit gate therefore rewrites at most `2k` rows with at most `2k` row products
//! each, `O(k^2 n / 64)` word operations. Rows whose old value another rewritten row still needs
//! are copied first, and a row whose product includes itself is updated in place.

use super::rows::multiply_row_into;
use super::small_gate::MAX_SMALL_ARITY;
use super::{PauliRows, Tableau};
use crate::budget::{StabilizerResource, try_filled, try_with_capacity, try_zeroed_words};
use crate::{PauliPhase, StabilizerError, StabilizerResult};

/// Saved rows that fit this many words use a stack buffer instead of a heap allocation.
const STACK_SCRATCH_WORDS: usize = 64;

/// The new value of one target generator row.
///
/// Generator `b` is local qubit `b / 2` in plane `b % 2`, where plane 0 is X and plane 1 is Z.
/// `factors` lists the selected old generators in ascending order, which is the order
/// `X_0, Z_0, X_1, Z_1, ...` of the Hermitian product with `Y = iXZ`.
struct Output<'a> {
    factors: &'a [usize],
    negative: bool,
    y_terms: usize,
}

pub(super) fn prepend(
    rows: &mut PauliRows,
    gate: &Tableau,
    targets: &[usize],
) -> StabilizerResult<()> {
    if gate.len() <= MAX_SMALL_ARITY {
        prepend_small(rows, gate, targets)
    } else {
        prepend_general(rows, gate, targets)
    }
}

/// Prepends a gate of at most two qubits from its one-word generator rows.
///
/// Generator sets are bit masks over the at most four target generators, and saved rows live on
/// the stack unless they are wider than [`STACK_SCRATCH_WORDS`].
fn prepend_small(rows: &mut PauliRows, gate: &Tableau, targets: &[usize]) -> StabilizerResult<()> {
    const GENERATORS: usize = 2 * MAX_SMALL_ARITY;
    let arity = gate.len();
    let generators = 2 * arity;
    let local = gate.rows();
    let num_qubits = rows.num_qubits();
    let mut global = [0_usize; GENERATORS];
    let mut factors = [0_u8; GENERATORS];
    let mut y_terms = [0_u32; GENERATORS];
    let mut negative = 0_u8;
    let mut old_signs = 0_u8;
    for generator in 0..generators {
        // Local rows are ordered X_0.., Z_0..; generators are ordered X_0, Z_0, X_1, Z_1.
        let row = (generator % 2) * arity + generator / 2;
        let [x, z] = local.row(row)? else {
            return Err(out_of_range(row, local.row_count()));
        };
        let mut mask = 0_u8;
        for qubit in 0..arity {
            mask |= u8::from(x >> qubit & 1 != 0) << (2 * qubit);
            mask |= u8::from(z >> qubit & 1 != 0) << (2 * qubit + 1);
        }
        let target = targets
            .get(generator / 2)
            .ok_or_else(|| out_of_range(generator, generators))?;
        let destination = (generator % 2) * num_qubits + target;
        let (Some(global_slot), Some(factor_slot), Some(y_slot)) = (
            global.get_mut(generator),
            factors.get_mut(generator),
            y_terms.get_mut(generator),
        ) else {
            return Err(out_of_range(generator, GENERATORS));
        };
        *global_slot = destination;
        *factor_slot = mask;
        *y_slot = (x & z).count_ones();
        negative |= u8::from(local.sign(row)?) << generator;
        old_signs |= u8::from(rows.sign(destination)?) << generator;
    }
    let mut changed = 0_u8;
    let mut needed = 0_u8;
    for (generator, mask) in factors.iter().copied().enumerate().take(generators) {
        let own = 1_u8 << generator;
        negative ^= u8::from((mask & old_signs).count_ones() % 2 == 1) << generator;
        if mask != own {
            changed |= own;
            needed |= mask & !own;
        }
    }
    let saved = needed & changed;
    let row_words = rows.row_words();
    let scratch_words = saved.count_ones() as usize * row_words;
    let mut stack: [u64; STACK_SCRATCH_WORDS];
    let mut heap: Vec<u64>;
    let scratch: &mut [u64] = if scratch_words == 0 {
        &mut []
    } else if scratch_words <= STACK_SCRATCH_WORDS {
        stack = [0; STACK_SCRATCH_WORDS];
        stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, STACK_SCRATCH_WORDS))?
    } else {
        heap = try_zeroed_words(StabilizerResource::Tableau, scratch_words)?;
        &mut heap
    };
    let mut slots = [0_usize; GENERATORS];
    let mut next_slot = 0;
    for (generator, (slot, row)) in slots.iter_mut().zip(global).enumerate() {
        if saved >> generator & 1 != 0 {
            *slot = next_slot;
            scratch_row_mut(scratch, next_slot, row_words)?.copy_from_slice(rows.row(row)?);
            next_slot += 1;
        }
    }
    let half_words = rows.half_words();
    for generator in (0..generators).filter(|generator| changed >> generator & 1 != 0) {
        let (Some(&destination), Some(&mask), Some(&y)) = (
            global.get(generator),
            factors.get(generator),
            y_terms.get(generator),
        ) else {
            return Err(out_of_range(generator, GENERATORS));
        };
        let in_place = mask >> generator & 1 != 0;
        let mut log_i = y;
        // Moving Z_j in front of X_j of the same qubit reverses an anticommuting pair.
        if in_place && generator % 2 == 1 && mask >> (generator - 1) & 1 != 0 {
            log_i += 2;
        }
        let mut first = !in_place;
        for factor in
            (0..generators).filter(|factor| *factor != generator && mask >> factor & 1 != 0)
        {
            let quarter_turns = if saved >> factor & 1 != 0 {
                let slot = *slots
                    .get(factor)
                    .ok_or_else(|| out_of_range(factor, GENERATORS))?;
                let source = scratch_row(scratch, slot, row_words)?;
                combine(rows.row_mut(destination)?, source, half_words, first)
            } else {
                let source_row = *global
                    .get(factor)
                    .ok_or_else(|| out_of_range(factor, GENERATORS))?;
                let (target, source) = rows.row_pair_mut(destination, source_row)?;
                combine(target, source, half_words, first)
            };
            log_i += u32::from(quarter_turns);
            first = false;
        }
        negative ^= u8::from(real_phase_is_negative(log_i)?) << generator;
    }
    for (generator, destination) in global.iter().copied().enumerate().take(generators) {
        rows.set_sign(destination, negative >> generator & 1 != 0)?;
    }
    Ok(())
}

/// Whether the base-`i` exponent `log_i` is `-1`, refusing an imaginary phase.
fn real_phase_is_negative(log_i: u32) -> StabilizerResult<bool> {
    match log_i % 4 {
        0 => Ok(false),
        2 => Ok(true),
        1 => Err(StabilizerError::ImaginaryProduct {
            phase: PauliPhase::PlusI,
        }),
        _ => Err(StabilizerError::ImaginaryProduct {
            phase: PauliPhase::MinusI,
        }),
    }
}

fn prepend_general(
    rows: &mut PauliRows,
    gate: &Tableau,
    targets: &[usize],
) -> StabilizerResult<()> {
    let arity = gate.len();
    let generators = 2 * arity;
    let local = gate.rows();
    let mut factor_lists: Vec<Vec<usize>> =
        try_with_capacity(StabilizerResource::Tableau, generators)?;
    let mut signs = try_filled(StabilizerResource::Tableau, generators, (false, 0_usize))?;
    for (generator, sign) in signs.iter_mut().enumerate() {
        // Local rows are ordered X_0.., Z_0..; generators are ordered X_0, Z_0, X_1, Z_1, ...
        let row = (generator % 2) * arity + generator / 2;
        let mut factors = try_with_capacity(StabilizerResource::Tableau, generators)?;
        let mut y_terms = 0;
        for qubit in 0..arity {
            let (x, z) = local.bits(row, qubit)?;
            if x {
                factors.push(2 * qubit);
            }
            if z {
                factors.push(2 * qubit + 1);
            }
            y_terms += usize::from(x && z);
        }
        factor_lists.push(factors);
        *sign = (local.sign(row)?, y_terms);
    }
    let mut outputs: Vec<Output<'_>> = try_with_capacity(StabilizerResource::Tableau, generators)?;
    for (factors, (negative, y_terms)) in factor_lists.iter().zip(signs) {
        outputs.push(Output {
            factors,
            negative,
            y_terms,
        });
    }
    let mut slots = try_filled(StabilizerResource::Tableau, generators, None)?;
    apply_outputs(rows, targets, &mut outputs, &mut slots)
}

/// The global row of local generator `generator`.
fn global_row(rows: &PauliRows, targets: &[usize], generator: usize) -> StabilizerResult<usize> {
    let target = targets
        .get(generator / 2)
        .ok_or(StabilizerError::TableauIndexOutOfRange {
            index: generator,
            len: 2 * targets.len(),
        })?;
    Ok((generator % 2) * rows.num_qubits() + target)
}

/// Whether output `generator` changes any row word.
fn rewrites(output: &Output<'_>, generator: usize) -> bool {
    output.factors != [generator]
}

fn apply_outputs(
    rows: &mut PauliRows,
    targets: &[usize],
    outputs: &mut [Output<'_>],
    slots: &mut [Option<usize>],
) -> StabilizerResult<()> {
    // Fold the old signs of every selected row into each output before any row changes.
    for output in outputs.iter_mut() {
        for factor in output.factors {
            output.negative ^= rows.sign(global_row(rows, targets, *factor)?)?;
        }
    }
    let mut saved = 0;
    for (generator, output) in outputs.iter().enumerate() {
        if !rewrites(output, generator) {
            continue;
        }
        for factor in output.factors.iter().copied() {
            let needed = factor != generator
                && outputs
                    .get(factor)
                    .is_some_and(|source| rewrites(source, factor));
            if let Some(slot) = slots.get_mut(factor)
                && needed
                && slot.is_none()
            {
                *slot = Some(saved);
                saved += 1;
            }
        }
    }
    let row_words = rows.row_words();
    let scratch_words = saved * row_words;
    let mut stack = [0_u64; STACK_SCRATCH_WORDS];
    let mut heap: Vec<u64>;
    let scratch = if scratch_words <= STACK_SCRATCH_WORDS {
        stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, STACK_SCRATCH_WORDS))?
    } else {
        heap = try_zeroed_words(StabilizerResource::Tableau, scratch_words)?;
        heap.as_mut_slice()
    };
    for (generator, slot) in slots.iter().enumerate() {
        if let Some(slot) = slot {
            let source = rows.row(global_row(rows, targets, generator)?)?;
            scratch_row_mut(scratch, *slot, row_words)?.copy_from_slice(source);
        }
    }
    let half_words = rows.half_words();
    for (generator, output) in outputs.iter_mut().enumerate() {
        let destination = global_row(rows, targets, generator)?;
        if rewrites(output, generator) {
            let in_place = output.factors.contains(&generator);
            let mut log_i = output.y_terms;
            // Moving Z_j in front of X_j of the same qubit reverses an anticommuting pair.
            if in_place && generator % 2 == 1 && output.factors.contains(&(generator - 1)) {
                log_i += 2;
            }
            let mut first = !in_place;
            for factor in output.factors.iter().copied() {
                if factor == generator {
                    continue;
                }
                let source_row = global_row(rows, targets, factor)?;
                let saved_slot = slots.get(factor).copied().flatten();
                let quarter_turns = match saved_slot {
                    Some(slot) => {
                        let source = scratch_row(scratch, slot, row_words)?;
                        combine(rows.row_mut(destination)?, source, half_words, first)
                    }
                    None => {
                        let (target, source) = rows.row_pair_mut(destination, source_row)?;
                        combine(target, source, half_words, first)
                    }
                };
                log_i += usize::from(quarter_turns);
                first = false;
            }
            output.negative ^= real_phase_is_negative(
                u32::try_from(log_i % 4).map_err(|_| out_of_range(log_i, 4))?,
            )?;
        }
        rows.set_sign(destination, output.negative)?;
    }
    Ok(())
}

const fn out_of_range(index: usize, len: usize) -> StabilizerError {
    StabilizerError::TableauIndexOutOfRange { index, len }
}

/// Copies `source` into `target` when `first`, else right-multiplies it, and returns the product's
/// extra base-`i` phase exponent.
fn combine(target: &mut [u64], source: &[u64], half_words: usize, first: bool) -> u8 {
    if first {
        target.copy_from_slice(source);
        0
    } else {
        multiply_row_into(target, source, half_words)
    }
}

fn scratch_row(scratch: &[u64], slot: usize, row_words: usize) -> StabilizerResult<&[u64]> {
    scratch.get(slot * row_words..(slot + 1) * row_words).ok_or(
        StabilizerError::TableauIndexOutOfRange {
            index: slot,
            len: scratch.len() / row_words.max(1),
        },
    )
}

fn scratch_row_mut(
    scratch: &mut [u64],
    slot: usize,
    row_words: usize,
) -> StabilizerResult<&mut [u64]> {
    let len = scratch.len() / row_words.max(1);
    scratch
        .get_mut(slot * row_words..(slot + 1) * row_words)
        .ok_or(StabilizerError::TableauIndexOutOfRange { index: slot, len })
}
