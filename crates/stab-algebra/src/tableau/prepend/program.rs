//! Shortest in-place row programs for every two-qubit symplectic action, built at compile time.
//!
//! Prepending a two-qubit gate replaces its four target generator rows `R_g` by the products of
//! old rows that the gate's key selects. Every invertible `4 x 4` matrix over `GF(2)` is a product
//! of the in-place steps `R_i <- R_i R_j` and `R_i <-> R_j`, and a swap moves past a product step
//! by renaming its rows, so a shortest program is a shortest run of product steps followed by the
//! fewest swaps that put the rows in order. A breadth-first search from the identity over the 12
//! product steps, then a choice among the 24 final row orders, gives each of the 720 symplectic
//! keys such a program, at most [`MAX_OPS`] steps long, that rewrites the four rows without saved
//! copies.
//!
//! Each step multiplies whole signed rows, so the runtime keeps every row's exact base-`i` phase
//! exponent, and the program records the phase `c_g` the same steps give the identity tableau's
//! local generators. A Clifford tableau `T` maps products to products, so after the program row
//! `g` holds `i^c_g T(P_g)` where `P_g` is the Hermitian local Pauli the key selects, and the new
//! sign follows from the tracked phase minus `c_g` plus the gate's own sign.

use super::super::PauliRows;
use super::{swap_rows_words, write_changed_signs};
use crate::tableau::rows::multiply_row_into;
use crate::{PauliPhase, StabilizerError, StabilizerResult};

/// Longest shortest program over the 720 two-qubit symplectic keys.
const MAX_OPS: usize = 7;
/// Number of two-qubit symplectic keys, `|Sp(4, 2)|`.
const SYMPLECTIC_KEYS: usize = 720;
/// Number of invertible `4 x 4` matrices over `GF(2)`, the states the search can reach.
const INVERTIBLE_KEYS: usize = 20_160;
/// The key of the identity, each generator selecting only itself.
const IDENTITY_KEY: u16 = 0x8421;
/// Step flag for `R_i <-> R_j`; without it a step is `R_i <- R_i R_j`.
const SWAP_STEP: u8 = 0x10;
/// Marks an unreached key in the search.
const UNREACHED: u8 = 0xFF;
/// Marks an index slot whose first three masks start no symplectic key.
const NO_PROGRAM: u16 = u16::MAX;

/// An in-place program for one symplectic key.
///
/// Step `s` is `ops[s]`, with destination row `i = ops[s] >> 2 & 3`, source row `j = ops[s] & 3`,
/// and [`SWAP_STEP`] selecting a swap. Bits `2g..2g + 2` of `phases` hold `c_g`. `z1_mask` is the
/// key's last mask, which tells apart the two keys that share their first three masks.
#[derive(Clone, Copy, Debug)]
pub(super) struct TwoQubitProgram {
    ops: [u8; MAX_OPS],
    len: u8,
    phases: u8,
    z1_mask: u8,
}

impl TwoQubitProgram {
    const EMPTY: Self = Self {
        ops: [0; MAX_OPS],
        len: 0,
        phases: 0,
        z1_mask: 0,
    };

    /// The program of a two-qubit symplectic `key`, or `None` when `key` is not symplectic.
    #[inline]
    pub(super) fn of(key: u16) -> Option<&'static Self> {
        let tables = &TABLES;
        let base = *tables.index.get(usize::from(key & 0x0FFF))?;
        if base == NO_PROGRAM {
            return None;
        }
        let first = tables.programs.get(usize::from(base))?;
        let [_, high] = key.to_le_bytes();
        let z1_mask = high >> 4;
        if first.z1_mask == z1_mask {
            return Some(first);
        }
        let second = tables.programs.get(usize::from(base) + 1)?;
        (second.z1_mask == z1_mask).then_some(second)
    }

    /// Runs the program on the generator rows `global`, in `X_0, Z_0, X_1, Z_1` order, whose
    /// current signs are `old_signs`, and writes the new signs flipped by `gate_signs`.
    #[inline]
    pub(super) fn apply(
        &self,
        rows: &mut PauliRows,
        global: [usize; 4],
        old_signs: u8,
        gate_signs: u8,
    ) -> StabilizerResult<()> {
        let half_words = rows.half_words();
        let ops = self.ops.get(..usize::from(self.len)).ok_or(
            StabilizerError::TableauIndexOutOfRange {
                index: usize::from(self.len),
                len: MAX_OPS,
            },
        )?;
        // Bits `2g..2g + 2` hold row `g`'s base-`i` phase exponent, starting at twice its sign.
        let mut phases = spread_signs(old_signs) << 1;
        let mut working = rows.disjoint_rows_mut(global)?;
        for &op in ops {
            let destination = usize::from(op >> 2 & 3);
            let source = usize::from(op & 3);
            let [target, source_row] =
                working
                    .get_disjoint_mut([destination, source])
                    .map_err(|_| StabilizerError::TableauIndexOutOfRange {
                        index: destination,
                        len: 4,
                    })?;
            let (to, from) = (2 * destination, 2 * source);
            if op & SWAP_STEP != 0 {
                swap_rows_words(target, source_row);
                let difference = ((phases >> to) ^ (phases >> from)) & 3;
                phases ^= (difference << to) | (difference << from);
            } else {
                let quarter_turns = multiply_row_into(target, source_row, half_words)?;
                let sum = ((phases >> to) & 3) + ((phases >> from) & 3) + quarter_turns;
                phases = (phases & !(3 << to)) | ((sum & 3) << to);
            }
        }
        // Row `g` ends as `i^(e_g - c_g)` times its Hermitian image, which must be real: equal low
        // bits leave no borrow, so each sign is the XOR of the high bits.
        let difference = phases ^ self.phases;
        if difference & 0x55 != 0 {
            return Err(imaginary_row(phases, self.phases));
        }
        let new_signs = gate_signs ^ compact_signs(difference >> 1);
        write_changed_signs(rows, global, old_signs, new_signs)
    }
}

/// The error for the first row whose phase exponent minus its program phase is odd, which a
/// Clifford tableau never produces; the phase is `+i` unless that row's difference is three.
#[cold]
fn imaginary_row(phases: u8, program_phases: u8) -> StabilizerError {
    let first_odd = (0..4)
        .map(|generator| {
            (phases >> (2 * generator)).wrapping_sub(program_phases >> (2 * generator)) & 3
        })
        .find(|log_i| log_i % 2 == 1);
    let phase = if first_odd == Some(3) {
        PauliPhase::MinusI
    } else {
        PauliPhase::PlusI
    };
    StabilizerError::ImaginaryProduct { phase }
}

/// Moves bit `2g` of `bits` to bit `g`, for `g < 4`, the inverse of [`spread_signs`].
#[inline]
const fn compact_signs(bits: u8) -> u8 {
    (bits & 1) | ((bits >> 1) & 2) | ((bits >> 2) & 4) | ((bits >> 3) & 8)
}

/// Moves bit `g` of `bits` to bit `2g`, for `g < 4`.
#[inline]
const fn spread_signs(bits: u8) -> u8 {
    (bits & 1) | ((bits & 2) << 1) | ((bits & 4) << 2) | ((bits & 8) << 3)
}

struct Tables {
    /// The first of the at most two programs whose key has these low twelve bits.
    index: [u16; 1 << 12],
    programs: [TwoQubitProgram; SYMPLECTIC_KEYS],
}

/// The product-step distance from the identity to every invertible key, and the step that first
/// reached it; [`UNREACHED`] marks the other keys. Only [`TABLES`] reads it during compilation.
struct Search {
    distance: [u8; 1 << 16],
    last_step: [u8; 1 << 16],
}

// A separate item keeps each compile-time evaluation below rustc's long-running lint threshold.
#[expect(
    clippy::large_const_arrays,
    reason = "only the TABLES initializer reads it during compilation, so it is never materialized \
              at runtime"
)]
const SEARCH: Search = search();

static TABLES: Tables = build_tables(&SEARCH);

/// Whether two generator masks anticommute under the symplectic form pairing bits `2q`, `2q + 1`.
const fn anticommute(left: u8, right: u8) -> bool {
    let crossed = ((left & 0b0101) << 1) | ((left & 0b1010) >> 1);
    (crossed & right).count_ones() % 2 == 1
}

const fn mask_of(key: u16, generator: usize) -> u8 {
    let [low, _] = ((key >> (4 * generator)) & 0xF).to_le_bytes();
    low
}

/// Applies the product step `R_destination <- R_destination R_source` to the masks in `key`; it
/// is its own inverse.
const fn product_step(key: u16, op: u8) -> u16 {
    let destination = (op >> 2) & 3;
    let source = op & 3;
    key ^ (((key >> (4 * source)) & 0xF) << (4 * destination))
}

/// The base-`i` exponent of `P_left P_right / P_(left ^ right)` for Hermitian two-qubit Paulis
/// coded with X of qubit `q` at bit `2q` and Z at bit `2q + 1`.
const fn local_product_log_i(left: u8, right: u8) -> u8 {
    let mut log_i = 0_u8;
    let mut qubit = 0;
    while qubit < 2 {
        let x1 = (left >> (2 * qubit)) & 1;
        let z1 = (left >> (2 * qubit + 1)) & 1;
        let x2 = (right >> (2 * qubit)) & 1;
        let z2 = (right >> (2 * qubit + 1)) & 1;
        // Aaronson and Gottesman's g, plus four to stay non-negative.
        log_i += match (x1, z1) {
            (0, 0) => 4,
            (1, 1) => 4 + z2 - x2,
            (1, 0) => 4 + 2 * z2 * x2 - z2,
            _ => 4 + x2 - 2 * x2 * z2,
        };
        qubit += 1;
    }
    log_i & 3
}

#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build; every \
              index is a 16-bit key or bounded by INVERTIBLE_KEYS"
)]
const fn search() -> Search {
    const PRODUCT_STEPS: [u8; 12] = [1, 2, 3, 4, 6, 7, 8, 9, 11, 12, 13, 14];
    let mut search = Search {
        distance: [UNREACHED; 1 << 16],
        last_step: [UNREACHED; 1 << 16],
    };
    let mut queue = [0_u16; INVERTIBLE_KEYS];
    search.distance[IDENTITY_KEY as usize] = 0;
    queue[0] = IDENTITY_KEY;
    let (mut head, mut tail) = (0, 1);
    while head < tail {
        let key = queue[head];
        head += 1;
        let distance = search.distance[key as usize] + 1;
        let mut index = 0;
        while index < PRODUCT_STEPS.len() {
            let op = PRODUCT_STEPS[index];
            let next = product_step(key, op);
            if search.distance[next as usize] == UNREACHED {
                search.distance[next as usize] = distance;
                search.last_step[next as usize] = op;
                queue[tail] = next;
                tail += 1;
            }
            index += 1;
        }
    }
    assert!(
        tail == INVERTIBLE_KEYS,
        "the product steps reach every invertible matrix"
    );
    search
}

/// The 24 orders of four rows.
#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build"
)]
const fn row_orders() -> [[usize; 4]; 24] {
    let mut orders = [[0; 4]; 24];
    let mut count = 0;
    let mut code = 0;
    while code < 256 {
        let order = [code & 3, (code >> 2) & 3, (code >> 4) & 3, (code >> 6) & 3];
        let seen = (1_u8 << order[0]) | (1 << order[1]) | (1 << order[2]) | (1 << order[3]);
        if seen == 0xF {
            orders[count] = order;
            count += 1;
        }
        code += 1;
    }
    orders
}

#[expect(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "evaluated only at compile time, where an out-of-range index or a bad cast fails the \
              build; every count is bounded by the table sizes"
)]
const fn build_tables(search: &Search) -> Tables {
    let orders = row_orders();
    let mut tables = Tables {
        index: [NO_PROGRAM; 1 << 12],
        programs: [TwoQubitProgram::EMPTY; SYMPLECTIC_KEYS],
    };
    let mut count = 0;
    // The last mask varies fastest, so keys sharing their first three masks are adjacent.
    let mut x0 = 1_u8;
    while x0 < 16 {
        let mut z0 = 1_u8;
        while z0 < 16 {
            let mut x1 = 1_u8;
            while anticommute(x0, z0) && x1 < 16 {
                let mut z1 = 1_u8;
                while !anticommute(x0, x1) && !anticommute(z0, x1) && z1 < 16 {
                    if anticommute(x1, z1) && !anticommute(x0, z1) && !anticommute(z0, z1) {
                        let masks = [x0, z0, x1, z1];
                        let low = masks_key(masks) & 0x0FFF;
                        if tables.index[low as usize] == NO_PROGRAM {
                            tables.index[low as usize] = count as u16;
                        }
                        tables.programs[count] = program_for(masks, search, &orders);
                        count += 1;
                    }
                    z1 += 1;
                }
                x1 += 1;
            }
            z0 += 1;
        }
        x0 += 1;
    }
    assert!(count == SYMPLECTIC_KEYS, "Sp(4, 2) has 720 elements");
    tables
}

const fn masks_key(masks: [u8; 4]) -> u16 {
    let [x0, z0, x1, z1] = masks;
    (x0 as u16) | ((z0 as u16) << 4) | ((x1 as u16) << 8) | ((z1 as u16) << 12)
}

/// A shortest program for the symplectic `masks`: the product steps reaching the row order that
/// needs the fewest further steps, then the swaps that sort it.
#[expect(
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    reason = "evaluated only at compile time, where an out-of-range index or a bad cast fails the \
              build"
)]
const fn program_for(
    masks: [u8; 4],
    search: &Search,
    orders: &[[usize; 4]; 24],
) -> TwoQubitProgram {
    // Row order `order` ends the product steps with row `i` holding `masks[order[i]]`, and
    // `4 - cycles(order)` swaps then sort the rows.
    let mut best_cost = usize::MAX;
    let mut best_key = 0_u16;
    let mut index = 0;
    while index < orders.len() {
        let order = orders[index];
        let key = masks_key([
            masks[order[0]],
            masks[order[1]],
            masks[order[2]],
            masks[order[3]],
        ]);
        let cost = search.distance[key as usize] as usize + 4 - cycles(order);
        if cost < best_cost {
            best_cost = cost;
            best_key = key;
        }
        index += 1;
    }
    assert!(
        best_cost <= MAX_OPS,
        "every symplectic key is at most MAX_OPS steps away"
    );
    let mut ops = [0_u8; MAX_OPS];
    let products = search.distance[best_key as usize] as usize;
    let mut current = best_key;
    let mut position = products;
    while position > 0 {
        position -= 1;
        let op = search.last_step[current as usize];
        ops[position] = op;
        current = product_step(current, op);
    }
    assert!(
        current == IDENTITY_KEY,
        "the search path starts at the identity"
    );
    // Selection-sort the rows into place with swaps; the masks are distinct.
    let mut rows = [
        mask_of(best_key, 0),
        mask_of(best_key, 1),
        mask_of(best_key, 2),
        mask_of(best_key, 3),
    ];
    let mut len = products;
    let mut slot = 0;
    while slot < 4 {
        if rows[slot] != masks[slot] {
            let mut other = slot + 1;
            while rows[other] != masks[slot] {
                other += 1;
            }
            rows[other] = rows[slot];
            rows[slot] = masks[slot];
            ops[len] = SWAP_STEP | ((slot as u8) << 2) | other as u8;
            len += 1;
        }
        slot += 1;
    }
    assert!(len == best_cost, "the swaps match the cycle count");
    TwoQubitProgram {
        ops,
        len: len as u8,
        phases: local_phases(&ops, len, masks),
        z1_mask: masks[3],
    }
}

/// The number of cycles of a row order.
#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build"
)]
const fn cycles(order: [usize; 4]) -> usize {
    let mut seen = [false; 4];
    let mut cycles = 0;
    let mut start = 0;
    while start < 4 {
        if !seen[start] {
            cycles += 1;
            let mut row = start;
            while !seen[row] {
                seen[row] = true;
                row = order[row];
            }
        }
        start += 1;
    }
    cycles
}

/// Runs `ops[..len]` on the identity's local generators and packs each row's phase `c_g`.
#[expect(
    clippy::indexing_slicing,
    reason = "evaluated only at compile time, where an out-of-range index fails the build"
)]
const fn local_phases(ops: &[u8; MAX_OPS], len: usize, masks: [u8; 4]) -> u8 {
    let mut codes = [1_u8, 2, 4, 8];
    let mut phases = [0_u8; 4];
    let mut index = 0;
    while index < len {
        let op = ops[index];
        let destination = ((op >> 2) & 3) as usize;
        let source = (op & 3) as usize;
        if op & SWAP_STEP != 0 {
            let (code, phase) = (codes[destination], phases[destination]);
            codes[destination] = codes[source];
            phases[destination] = phases[source];
            codes[source] = code;
            phases[source] = phase;
        } else {
            let log_i = local_product_log_i(codes[destination], codes[source]);
            phases[destination] = (phases[destination] + phases[source] + log_i) & 3;
            codes[destination] ^= codes[source];
        }
        index += 1;
    }
    let mut packed = 0_u8;
    let mut generator = 0;
    while generator < 4 {
        assert!(
            codes[generator] == masks[generator],
            "the program realizes its key"
        );
        packed |= phases[generator] << (2 * generator);
        generator += 1;
    }
    packed
}
