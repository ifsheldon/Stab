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

/// Narrow saved-row stack buffer (one cache line) before falling back to [`STACK_SCRATCH_WORDS`].
const NARROW_SCRATCH_WORDS: usize = 8;
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

#[inline]
pub(super) fn prepend(
    rows: &mut PauliRows,
    gate: &Tableau,
    targets: &[usize],
) -> StabilizerResult<()> {
    match targets {
        [] => Ok(()),
        &[q] => prepend_one_qubit(rows, gate, q),
        &[q0, q1] => prepend_two_qubit(rows, gate, q0, q1),
        _ if gate.len() <= MAX_SMALL_ARITY => prepend_small(rows, gate, targets),
        _ => prepend_general(rows, gate, targets),
    }
}

/// Prepends any one-qubit Clifford in place without scratch storage.
///
/// Every invertible `2 x 2` symplectic matrix over `GF(2)` reduces to at most one slice swap and
/// one in-place row multiplication; degenerate non-invertible 1-qubit tableaux fall back to
/// [`prepend_small`].
fn prepend_one_qubit(rows: &mut PauliRows, gate: &Tableau, qubit: usize) -> StabilizerResult<()> {
    let (&[x_x, x_z, z_x, z_z], &[gate_sign]) = gate.rows().words_and_signs() else {
        return prepend_small(rows, gate, &[qubit]);
    };
    let pattern = u8::from(x_x & 1 != 0)
        | (u8::from(x_z & 1 != 0) << 1)
        | (u8::from(z_x & 1 != 0) << 2)
        | (u8::from(z_z & 1 != 0) << 3);
    let row_x = qubit;
    let row_z = rows.num_qubits() + qubit;
    let old_sx = rows.sign(row_x)?;
    let old_sz = rows.sign(row_z)?;
    let gx_neg = gate_sign & 1 != 0;
    let gz_neg = (gate_sign >> 1) & 1 != 0;
    let half_words = rows.half_words();

    let (new_sx, new_sz) = match pattern {
        // X -> ±X, Z -> ±Z (I, X, Y, Z)
        0b1001 => (gx_neg ^ old_sx, gz_neg ^ old_sz),
        // X -> ±Z, Z -> ±X (H family)
        0b0110 => {
            let (rx, rz) = rows.two_rows_mut(row_x, row_z)?;
            rx.swap_with_slice(rz);
            (gx_neg ^ old_sz, gz_neg ^ old_sx)
        }
        // X -> ±Y = ±(i X Z), Z -> ±Z (S family)
        0b1011 => {
            let (rx, rz) = rows.row_pair_mut(row_x, row_z)?;
            let q = multiply_row_into(rx, rz, half_words)?;
            let neg = real_phase_is_negative(1 + u32::from(q))?;
            (gx_neg ^ old_sx ^ old_sz ^ neg, gz_neg ^ old_sz)
        }
        // X -> ±X, Z -> ±Y = ±(i X Z) (SQRT_X / H_YZ family)
        0b1101 => {
            let (rz, rx) = rows.row_pair_mut(row_z, row_x)?;
            let q = multiply_row_into(rz, rx, half_words)?;
            let neg = real_phase_is_negative(1 + u32::from((4 - q) & 3))?;
            (gx_neg ^ old_sx, gz_neg ^ old_sx ^ old_sz ^ neg)
        }
        // X -> ±Z, Z -> ±Y = ±(i X Z) (C_XYZ family)
        0b1110 => {
            let (rx, rz) = rows.two_rows_mut(row_x, row_z)?;
            rx.swap_with_slice(rz);
            let q = multiply_row_into(rz, rx, half_words)?;
            let neg = real_phase_is_negative(1 + u32::from(q))?;
            (gx_neg ^ old_sz, gz_neg ^ old_sx ^ old_sz ^ neg)
        }
        // X -> ±Y = ±(i X Z), Z -> ±X (C_ZYX family)
        0b0111 => {
            let (rx, rz) = rows.two_rows_mut(row_x, row_z)?;
            rx.swap_with_slice(rz);
            let q = multiply_row_into(rx, rz, half_words)?;
            let neg = real_phase_is_negative(1 + u32::from((4 - q) & 3))?;
            (gx_neg ^ old_sx ^ old_sz ^ neg, gz_neg ^ old_sx)
        }
        _ => return prepend_small(rows, gate, &[qubit]),
    };

    if new_sx != old_sx {
        rows.set_sign(row_x, new_sx)?;
    }
    if new_sz != old_sz {
        rows.set_sign(row_z, new_sz)?;
    }
    Ok(())
}

#[inline]
fn pack_two_qubit_mask(x: u64, z: u64) -> u8 {
    u8::from(x & 1 != 0)
        | (u8::from(z & 1 != 0) << 1)
        | (u8::from(x & 2 != 0) << 2)
        | (u8::from(z & 2 != 0) << 3)
}

/// Prepends any two-qubit gate directly from its eight packed local generator words.
fn prepend_two_qubit(
    rows: &mut PauliRows,
    gate: &Tableau,
    q0: usize,
    q1: usize,
) -> StabilizerResult<()> {
    let (&[x0_x, x0_z, x1_x, x1_z, z0_x, z0_z, z1_x, z1_z], &[local_sign_word]) =
        gate.rows().words_and_signs()
    else {
        return prepend_small(rows, gate, &[q0, q1]);
    };

    let m0 = pack_two_qubit_mask(x0_x, x0_z);
    let m1 = pack_two_qubit_mask(z0_x, z0_z);
    let m2 = pack_two_qubit_mask(x1_x, x1_z);
    let m3 = pack_two_qubit_mask(z1_x, z1_z);
    let mut negative = u8::from(local_sign_word & 1 != 0)
        | (u8::from(local_sign_word & 4 != 0) << 1)
        | (u8::from(local_sign_word & 2 != 0) << 2)
        | (u8::from(local_sign_word & 8 != 0) << 3);

    let num_qubits = rows.num_qubits();
    let r_x0 = q0;
    let r_z0 = num_qubits + q0;
    let r_x1 = q1;
    let r_z1 = num_qubits + q1;
    let sx0 = rows.sign(r_x0)?;
    let sz0 = rows.sign(r_z0)?;
    let sx1 = rows.sign(r_x1)?;
    let sz1 = rows.sign(r_z1)?;
    let half_words = rows.half_words();

    match [m0, m1, m2, m3] {
        // CX(q0, q1) + local Pauli signs: X_0 -> ±X_0 X_1, Z_0 -> ±Z_0, X_1 -> ±X_1, Z_1 -> ±Z_0 Z_1
        [0b0101, 0b0010, 0b0100, 0b1010] => {
            let (rx0, rx1) = rows.row_pair_mut(r_x0, r_x1)?;
            let q_x = multiply_row_into(rx0, rx1, half_words)?;
            let nx0 = (negative & 1 != 0) ^ sx0 ^ sx1 ^ real_phase_is_negative(u32::from(q_x))?;
            let nz0 = (negative & 2 != 0) ^ sz0;
            let nx1 = (negative & 4 != 0) ^ sx1;
            let (rz1, rz0) = rows.row_pair_mut(r_z1, r_z0)?;
            let q_z = multiply_row_into(rz1, rz0, half_words)?;
            let nz1 =
                (negative & 8 != 0) ^ sz0 ^ sz1 ^ real_phase_is_negative(u32::from((4 - q_z) & 3))?;
            return write_four_signs(
                rows,
                [
                    (r_x0, sx0, nx0),
                    (r_z0, sz0, nz0),
                    (r_x1, sx1, nx1),
                    (r_z1, sz1, nz1),
                ],
            );
        }
        // XCZ(q0, q1) / CX(q1, q0) + local Pauli signs
        [0b0001, 0b1010, 0b0101, 0b1000] => {
            let nx0 = (negative & 1 != 0) ^ sx0;
            let (rz0, rz1) = rows.row_pair_mut(r_z0, r_z1)?;
            let q_z = multiply_row_into(rz0, rz1, half_words)?;
            let nz0 = (negative & 2 != 0) ^ sz0 ^ sz1 ^ real_phase_is_negative(u32::from(q_z))?;
            let (rx1, rx0) = rows.row_pair_mut(r_x1, r_x0)?;
            let q_x = multiply_row_into(rx1, rx0, half_words)?;
            let nx1 =
                (negative & 4 != 0) ^ sx0 ^ sx1 ^ real_phase_is_negative(u32::from((4 - q_x) & 3))?;
            let nz1 = (negative & 8 != 0) ^ sz1;
            return write_four_signs(
                rows,
                [
                    (r_x0, sx0, nx0),
                    (r_z0, sz0, nz0),
                    (r_x1, sx1, nx1),
                    (r_z1, sz1, nz1),
                ],
            );
        }
        // CZ(q0, q1) + local Pauli signs: X_0 -> ±X_0 Z_1, Z_0 -> ±Z_0, X_1 -> ±Z_0 X_1, Z_1 -> ±Z_1
        [0b1001, 0b0010, 0b0110, 0b1000] => {
            let (rx0, rz1) = rows.row_pair_mut(r_x0, r_z1)?;
            let q0_exp = multiply_row_into(rx0, rz1, half_words)?;
            let nx0 = (negative & 1 != 0) ^ sx0 ^ sz1 ^ real_phase_is_negative(u32::from(q0_exp))?;
            let nz0 = (negative & 2 != 0) ^ sz0;
            let (rx1, rz0) = rows.row_pair_mut(r_x1, r_z0)?;
            let q1_exp = multiply_row_into(rx1, rz0, half_words)?;
            let nx1 = (negative & 4 != 0)
                ^ sz0
                ^ sx1
                ^ real_phase_is_negative(u32::from((4 - q1_exp) & 3))?;
            let nz1 = (negative & 8 != 0) ^ sz1;
            return write_four_signs(
                rows,
                [
                    (r_x0, sx0, nx0),
                    (r_z0, sz0, nz0),
                    (r_x1, sx1, nx1),
                    (r_z1, sz1, nz1),
                ],
            );
        }
        // SWAP(q0, q1) + local Pauli signs
        [0b0100, 0b1000, 0b0001, 0b0010] => {
            let (rx0, rx1) = rows.two_rows_mut(r_x0, r_x1)?;
            rx0.swap_with_slice(rx1);
            let (rz0, rz1) = rows.two_rows_mut(r_z0, r_z1)?;
            rz0.swap_with_slice(rz1);
            let nx0 = (negative & 1 != 0) ^ sx1;
            let nz0 = (negative & 2 != 0) ^ sz1;
            let nx1 = (negative & 4 != 0) ^ sx0;
            let nz1 = (negative & 8 != 0) ^ sz0;
            return write_four_signs(
                rows,
                [
                    (r_x0, sx0, nx0),
                    (r_z0, sz0, nz0),
                    (r_x1, sx1, nx1),
                    (r_z1, sz1, nz1),
                ],
            );
        }
        _ => {}
    }

    let global = [r_x0, r_z0, r_x1, r_z1];
    let factors = [m0, m1, m2, m3];
    let y_terms = [
        (x0_x & x0_z).count_ones(),
        (z0_x & z0_z).count_ones(),
        (x1_x & x1_z).count_ones(),
        (z1_x & z1_z).count_ones(),
    ];
    let old_signs =
        u8::from(sx0) | (u8::from(sz0) << 1) | (u8::from(sx1) << 2) | (u8::from(sz1) << 3);

    apply_small_masks(rows, 4, global, factors, y_terms, &mut negative, old_signs)
}

#[inline]
fn write_four_signs(
    rows: &mut PauliRows,
    updates: [(usize, bool, bool); 4],
) -> StabilizerResult<()> {
    for (row, old_sign, new_sign) in updates {
        if new_sign != old_sign {
            rows.set_sign(row, new_sign)?;
        }
    }
    Ok(())
}

/// Prepends a gate of at most two qubits from its one-word generator rows.
///
/// Generator sets are bit masks over the at most four target generators, and saved rows live in an
/// unzeroed stack `ArrayVec` unless they are wider than [`STACK_SCRATCH_WORDS`].
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
    apply_small_masks(
        rows,
        generators,
        global,
        factors,
        y_terms,
        &mut negative,
        old_signs,
    )
}

fn apply_small_masks(
    rows: &mut PauliRows,
    generators: usize,
    global: [usize; 2 * MAX_SMALL_ARITY],
    factors: [u8; 2 * MAX_SMALL_ARITY],
    y_terms: [u32; 2 * MAX_SMALL_ARITY],
    negative: &mut u8,
    old_signs: u8,
) -> StabilizerResult<()> {
    const GENERATORS: usize = 2 * MAX_SMALL_ARITY;
    let mut changed = 0_u8;
    let mut needed = 0_u8;
    for (generator, mask) in factors.iter().copied().enumerate().take(generators) {
        let own = 1_u8 << generator;
        *negative ^= u8::from((mask & old_signs).count_ones() % 2 == 1) << generator;
        if mask != own {
            changed |= own;
            needed |= mask & !own;
        }
    }
    let saved = needed & changed;
    let row_words = rows.row_words();
    let scratch_words = saved.count_ones() as usize * row_words;
    let mut narrow_stack: [u64; NARROW_SCRATCH_WORDS];
    let mut wide_stack: [u64; STACK_SCRATCH_WORDS];
    let mut heap: Vec<u64>;
    let scratch: &mut [u64] = if scratch_words == 0 {
        &mut []
    } else if scratch_words <= NARROW_SCRATCH_WORDS {
        narrow_stack = [0; NARROW_SCRATCH_WORDS];
        narrow_stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, NARROW_SCRATCH_WORDS))?
    } else if scratch_words <= STACK_SCRATCH_WORDS {
        wide_stack = [0; STACK_SCRATCH_WORDS];
        wide_stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, STACK_SCRATCH_WORDS))?
    } else {
        heap = try_zeroed_words(StabilizerResource::Tableau, scratch_words)?;
        &mut heap
    };
    let mut slots = [0_usize; GENERATORS];
    if scratch_words > 0 {
        let mut next_slot = 0;
        for (generator, (slot, row)) in slots.iter_mut().zip(global).enumerate() {
            if saved >> generator & 1 != 0 {
                *slot = next_slot;
                scratch_row_mut(scratch, next_slot, row_words)?.copy_from_slice(rows.row(row)?);
                next_slot += 1;
            }
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
                combine(rows.row_mut(destination)?, source, half_words, first)?
            } else {
                let source_row = *global
                    .get(factor)
                    .ok_or_else(|| out_of_range(factor, GENERATORS))?;
                let (target, source) = rows.row_pair_mut(destination, source_row)?;
                combine(target, source, half_words, first)?
            };
            log_i += u32::from(quarter_turns);
            first = false;
        }
        *negative ^= u8::from(real_phase_is_negative(log_i)?) << generator;
    }
    for (generator, destination) in global.iter().copied().enumerate().take(generators) {
        let new_sign = *negative >> generator & 1 != 0;
        let old_sign = old_signs >> generator & 1 != 0;
        if new_sign != old_sign {
            rows.set_sign(destination, new_sign)?;
        }
    }
    Ok(())
}

/// Whether the base-`i` exponent `log_i` is `-1`, refusing an imaginary phase.
#[inline]
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
                *slot = Some(0);
                saved += 1;
            }
        }
    }
    let row_words = rows.row_words();
    let scratch_words = saved * row_words;
    let mut narrow_stack: [u64; NARROW_SCRATCH_WORDS];
    let mut wide_stack: [u64; STACK_SCRATCH_WORDS];
    let mut heap: Vec<u64>;
    let scratch: &mut [u64] = if scratch_words == 0 {
        &mut []
    } else if scratch_words <= NARROW_SCRATCH_WORDS {
        narrow_stack = [0; NARROW_SCRATCH_WORDS];
        narrow_stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, NARROW_SCRATCH_WORDS))?
    } else if scratch_words <= STACK_SCRATCH_WORDS {
        wide_stack = [0; STACK_SCRATCH_WORDS];
        wide_stack
            .get_mut(..scratch_words)
            .ok_or_else(|| out_of_range(scratch_words, STACK_SCRATCH_WORDS))?
    } else {
        heap = try_zeroed_words(StabilizerResource::Tableau, scratch_words)?;
        heap.as_mut_slice()
    };
    let mut next_slot = 0;
    for (generator, slot) in slots.iter_mut().enumerate() {
        if let Some(slot_index) = slot {
            *slot_index = next_slot;
            let source = rows.row(global_row(rows, targets, generator)?)?;
            scratch_row_mut(scratch, next_slot, row_words)?.copy_from_slice(source);
            next_slot += 1;
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
                        combine(rows.row_mut(destination)?, source, half_words, first)?
                    }
                    None => {
                        let (target, source) = rows.row_pair_mut(destination, source_row)?;
                        combine(target, source, half_words, first)?
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
#[inline]
fn combine(
    target: &mut [u64],
    source: &[u64],
    half_words: usize,
    first: bool,
) -> StabilizerResult<u8> {
    if first {
        target.copy_from_slice(source);
        Ok(0)
    } else {
        multiply_row_into(target, source, half_words)
    }
}

#[inline]
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
