//! Slice-level SIMD word kernels for Stab, built on `fearless_simd`.
//!
//! Each public kernel checks its slice lengths once, selects the instruction set once with
//! [`Level::new`], and then runs a body that is generic over [`Simd`], so one source serves
//! 128-bit, 256-bit, and 512-bit hardware. Main loops use a fixed minimum block (`u64x8` for XOR,
//! `u64x4` for Pauli and Clifford products on 128-bit levels) so a block's loads are issued before
//! its stores; remainders go through the native-width `S::u64s` vector and then scalar words. The
//! kernels own chunking, scalar tails, and phase/non-identity counting.

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::ops::{BitAnd, BitOr, BitXor, Not};

use fearless_simd::{Level, Simd, SimdBase, dispatch, u64x4, u64x8};
use fearless_simd_macros::simd;

/// Two word slices that a kernel requires to have equal lengths do not.
///
/// `expected` is the length of the first (destination) slice and `actual` is the length of the
/// first slice that disagrees with it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WordLengthMismatch {
    pub expected: usize,
    pub actual: usize,
}

impl Display for WordLengthMismatch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "word slice length mismatch: expected {} words, got {}",
            self.expected, self.actual
        )
    }
}

impl Error for WordLengthMismatch {}

/// XORs `rhs` into `lhs` word by word.
///
/// # Errors
///
/// Returns [`WordLengthMismatch`] without modifying `lhs` when the slices differ in length.
pub fn xor_assign_words(lhs: &mut [u64], rhs: &[u64]) -> Result<(), WordLengthMismatch> {
    ensure_len(lhs.len(), rhs.len())?;
    let level = Level::new();
    dispatch!(level, simd => xor_assign_words_simd(simd, lhs, rhs));
    Ok(())
}

/// Borrowed packed planes of a single-qubit Clifford string, one bit per qubit in each plane.
#[derive(Clone, Copy, Debug)]
pub struct CliffordPlanes<'a> {
    pub z_signs: &'a [u64],
    pub x_signs: &'a [u64],
    pub inv_x2x: &'a [u64],
    pub x2z: &'a [u64],
    pub z2x: &'a [u64],
    pub inv_z2z: &'a [u64],
}

/// Mutably borrowed packed planes of a single-qubit Clifford string.
#[derive(Debug)]
pub struct CliffordPlanesMut<'a> {
    pub z_signs: &'a mut [u64],
    pub x_signs: &'a mut [u64],
    pub inv_x2x: &'a mut [u64],
    pub x2z: &'a mut [u64],
    pub z2x: &'a mut [u64],
    pub inv_z2z: &'a mut [u64],
}

/// Number of non-identity qubits in the multiplied range before and after multiplication.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CliffordNonIdentityCounts {
    pub before: usize,
    pub after: usize,
}

/// Right-multiplies `right` into `left`, qubit by qubit.
///
/// # Errors
///
/// Returns [`WordLengthMismatch`] without modifying `left` when any of the twelve planes differs
/// in length from `left.z_signs`.
#[inline]
pub fn clifford_right_multiply_words(
    left: CliffordPlanesMut<'_>,
    right: CliffordPlanes<'_>,
) -> Result<CliffordNonIdentityCounts, WordLengthMismatch> {
    let expected = left.z_signs.len();
    let other_lengths = [
        left.x_signs.len(),
        left.inv_x2x.len(),
        left.x2z.len(),
        left.z2x.len(),
        left.inv_z2z.len(),
        right.z_signs.len(),
        right.x_signs.len(),
        right.inv_x2x.len(),
        right.x2z.len(),
        right.z2x.len(),
        right.inv_z2z.len(),
    ];
    if let Some(actual) = other_lengths.into_iter().find(|&len| len != expected) {
        return Err(WordLengthMismatch { expected, actual });
    }
    let left = [
        left.z_signs,
        left.x_signs,
        left.inv_x2x,
        left.x2z,
        left.z2x,
        left.inv_z2z,
    ];
    let right = [
        right.z_signs,
        right.x_signs,
        right.inv_x2x,
        right.x2z,
        right.z2x,
        right.inv_z2z,
    ];
    let level = Level::new();
    Ok(dispatch!(level, simd => clifford_right_multiply_words_simd(simd, left, right)))
}

/// Phase exponent and non-identity status of an in-place Pauli word product.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PauliWordProduct {
    pub log_i: u8,
    pub has_terms: bool,
}

/// Right-multiplies `(right_x, right_z)` into `(left_x, left_z)` qubit by qubit.
///
/// # Errors
///
/// Returns [`WordLengthMismatch`] without modifying `left_x` or `left_z` when any of the four
/// planes differs in length from `left_x`.
#[inline]
pub fn pauli_right_multiply_words(
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> Result<PauliWordProduct, WordLengthMismatch> {
    let expected = left_x.len();
    for actual in [left_z.len(), right_x.len(), right_z.len()] {
        ensure_len(expected, actual)?;
    }
    if expected == 0 {
        return Ok(PauliWordProduct {
            log_i: 0,
            has_terms: false,
        });
    }
    if let ([lx], [lz], [rx], [rz]) = (&mut *left_x, &mut *left_z, right_x, right_z) {
        let old_lx = *lx;
        let old_lz = *lz;
        let rx_w = *rx;
        let rz_w = *rz;
        let new_lx = old_lx ^ rx_w;
        let new_lz = old_lz ^ rz_w;
        *lx = new_lx;
        *lz = new_lz;
        let old_x_new_z = old_lx & rz_w;
        let anti_commutes = (rx_w & old_lz) ^ old_x_new_z;
        let count_bit_2 = (new_lx ^ new_lz ^ old_x_new_z) & anti_commutes;
        return Ok(PauliWordProduct {
            log_i: mod4_from_counters(anti_commutes, count_bit_2),
            has_terms: (new_lx | new_lz) != 0,
        });
    }
    let level = Level::new();
    Ok(dispatch!(
        level,
        simd => pauli_right_multiply_words_simd(simd, left_x, left_z, right_x, right_z)
    ))
}

fn ensure_len(expected: usize, actual: usize) -> Result<(), WordLengthMismatch> {
    if expected == actual {
        Ok(())
    } else {
        Err(WordLengthMismatch { expected, actual })
    }
}

/// The vectorized body of [`xor_assign_words`]; callers guarantee equal lengths.
///
/// The main loop uses 512-bit `u64x8` blocks, so each block's loads are issued before its store on
/// every level; the remainder goes through native vectors and then scalar words.
#[simd]
fn xor_assign_words_simd<S: Simd>(simd: S, lhs: &mut [u64], rhs: &[u64]) {
    let (lhs, rhs) = xor_assign_vectors::<S, u64x8<S>>(simd, lhs, rhs);
    let (lhs, rhs) = xor_assign_vectors::<S, S::u64s>(simd, lhs, rhs);
    for (left, right) in lhs.iter_mut().zip(rhs) {
        *left ^= *right;
    }
}

/// XORs every full `V` vector and returns the unprocessed remainders.
#[inline(always)]
fn xor_assign_vectors<'l, 'r, S, V>(
    simd: S,
    lhs: &'l mut [u64],
    rhs: &'r [u64],
) -> (&'l mut [u64], &'r [u64])
where
    S: Simd,
    V: SimdBase<S, Element = u64> + BitXor<Output = V>,
{
    for (left, right) in lhs.chunks_exact_mut(V::LEN).zip(rhs.chunks_exact(V::LEN)) {
        (V::from_slice(simd, left) ^ V::from_slice(simd, right)).store_slice(left);
    }
    (
        lhs.chunks_exact_mut(V::LEN).into_remainder(),
        rhs.chunks_exact(V::LEN).remainder(),
    )
}

/// The vectorized body of [`clifford_right_multiply_words`]; callers guarantee equal lengths.
///
/// On 128-bit levels the main loop uses `u64x4` (two native registers per plane) so every load
/// of a block is issued before any store to it; wider levels use the native vector directly. The
/// remainder goes through native vectors and then scalar words.
#[simd]
fn clifford_right_multiply_words_simd<S: Simd>(
    simd: S,
    left: [&mut [u64]; 6],
    right: [&[u64]; 6],
) -> CliffordNonIdentityCounts {
    let mut counts = CliffordNonIdentityCounts::default();
    let (left, right) = if S::u64s::LEN < u64x4::<S>::LEN {
        let (left, right) = clifford_vectors::<S, u64x4<S>>(simd, left, right, &mut counts);
        clifford_vectors::<S, S::u64s>(simd, left, right, &mut counts)
    } else {
        clifford_vectors::<S, S::u64s>(simd, left, right, &mut counts)
    };

    let [l0, l1, l2, l3, l4, l5] = left;
    let [r0, r1, r2, r3, r4, r5] = right;
    let left_tail = l0.iter_mut().zip(l1).zip(l2).zip(l3).zip(l4).zip(l5);
    let right_tail = r0.iter().zip(r1).zip(r2).zip(r3).zip(r4).zip(r5);
    for (left, right) in left_tail.zip(right_tail) {
        let (((((lz, lx), lixx), lxz), lzx), lizz) = left;
        let (((((rz, rx), rixx), rxz), rzx), rizz) = right;
        let before = [*lz, *lx, *lixx, *lxz, *lzx, *lizz];
        let after = clifford_product(before, [*rz, *rx, *rixx, *rxz, *rzx, *rizz]);
        counts.before += count_ones(&[non_identity(before)]);
        counts.after += count_ones(&[non_identity(after)]);
        [*lz, *lx, *lixx, *lxz, *lzx, *lizz] = after;
    }
    counts
}

/// Multiplies every full `V` vector of the planes and returns the unprocessed remainders.
#[inline(always)]
fn clifford_vectors<'l, 'r, S, V>(
    simd: S,
    mut left: [&'l mut [u64]; 6],
    right: [&'r [u64]; 6],
    counts: &mut CliffordNonIdentityCounts,
) -> ([&'l mut [u64]; 6], [&'r [u64]; 6])
where
    S: Simd,
    V: SimdBase<S, Element = u64>
        + BitAnd<Output = V>
        + BitOr<Output = V>
        + BitXor<Output = V>
        + Not<Output = V>,
{
    for_each_plane_chunk(&mut left, right, V::LEN, |left, right| {
        let before = left.each_ref().map(|plane| V::from_slice(simd, plane));
        let after = clifford_product(before, right.map(|plane| V::from_slice(simd, plane)));
        counts.before += count_ones(non_identity(before).as_slice());
        counts.after += count_ones(non_identity(after).as_slice());
        for (vector, plane) in after.into_iter().zip(left) {
            vector.store_slice(plane);
        }
    });
    (
        left.map(|plane| plane.chunks_exact_mut(V::LEN).into_remainder()),
        right.map(|plane| plane.chunks_exact(V::LEN).remainder()),
    )
}

/// Calls `body` on each aligned group of `chunk` words across all twelve planes.
///
/// Chunk iterators are zipped by value so the standard library drives the loop from one trip
/// count; callers re-derive the remainders afterwards from fresh chunk iterators in constant time.
#[inline(always)]
fn for_each_plane_chunk(
    left: &mut [&mut [u64]; 6],
    right: [&[u64]; 6],
    chunk: usize,
    mut body: impl FnMut([&mut [u64]; 6], [&[u64]; 6]),
) {
    let [l0, l1, l2, l3, l4, l5] = left.each_mut().map(|plane| plane.chunks_exact_mut(chunk));
    let [r0, r1, r2, r3, r4, r5] = right.map(|plane| plane.chunks_exact(chunk));
    let left_chunks = l0.zip(l1).zip(l2).zip(l3).zip(l4).zip(l5);
    let right_chunks = r0.zip(r1).zip(r2).zip(r3).zip(r4).zip(r5);
    for (left, right) in left_chunks.zip(right_chunks) {
        let (((((lz, lx), lixx), lxz), lzx), lizz) = left;
        let (((((rz, rx), rixx), rxz), rzx), rizz) = right;
        body(
            [lz, lx, lixx, lxz, lzx, lizz],
            [rz, rx, rixx, rxz, rzx, rizz],
        );
    }
}

/// Right-multiplies packed single-qubit Cliffords plane-wise.
///
/// Planes are ordered `z_signs`, `x_signs`, `inv_x2x`, `x2z`, `z2x`, `inv_z2z`. This one formula
/// serves both the vector body (`T = S::u64s`) and the scalar tail (`T = u64`).
#[inline(always)]
fn clifford_product<T>(left: [T; 6], right: [T; 6]) -> [T; 6]
where
    T: Copy + BitAnd<Output = T> + BitOr<Output = T> + BitXor<Output = T> + Not<Output = T>,
{
    let [
        left_z_signs,
        left_x_signs,
        left_inv_x2x,
        left_x2z,
        left_z2x,
        left_inv_z2z,
    ] = left;
    let [
        right_z_signs,
        right_x_signs,
        right_inv_x2x,
        right_x2z,
        right_z2x,
        right_inv_z2z,
    ] = right;
    let inv_x2x = (left_inv_x2x | right_inv_x2x) ^ (left_z2x & right_x2z);
    let x2z = (!right_inv_x2x & left_x2z) ^ (!left_inv_z2z & right_x2z);
    let z2x = (!left_inv_x2x & right_z2x) ^ (!right_inv_z2z & left_z2x);
    let inv_z2z = (left_x2z & right_z2x) ^ (left_inv_z2z | right_inv_z2z);
    let right_x2y = !right_inv_x2x & right_x2z;
    let right_z2y = !right_inv_z2z & right_z2x;
    let dy = (left_x2z & left_z2x) ^ left_inv_x2x ^ left_z2x ^ left_x2z ^ left_inv_z2z;
    let x_signs = right_x_signs
        ^ (!right_inv_x2x & left_x_signs)
        ^ (right_x2y & dy)
        ^ (right_x2z & left_z_signs);
    let z_signs = right_z_signs
        ^ (right_z2x & left_x_signs)
        ^ (right_z2y & dy)
        ^ (!right_inv_z2z & left_z_signs);
    [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z]
}

/// Bits set where any of the six planes is set, i.e. where the qubit's Clifford is not identity.
#[inline(always)]
fn non_identity<T>(planes: [T; 6]) -> T
where
    T: Copy + BitOr<Output = T>,
{
    let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes;
    z_signs | x_signs | inv_x2x | x2z | z2x | inv_z2z
}

#[inline(always)]
fn count_ones(words: &[u64]) -> usize {
    words.iter().map(|word| word.count_ones() as usize).sum()
}

/// The vectorized body of [`pauli_right_multiply_words`]; callers guarantee equal lengths.
///
/// On 128-bit levels the main loop uses `u64x4` (two native registers per plane); wider levels use
/// the native vector directly. The remainder goes through native vectors and then scalar words.
#[simd]
fn pauli_right_multiply_words_simd<S: Simd>(
    simd: S,
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> PauliWordProduct {
    let mut count_bit_1 = 0_u64;
    let mut count_bit_2 = 0_u64;
    let mut nonzero_or = 0_u64;
    let (left_x, left_z, right_x, right_z) = if S::u64s::LEN < u64x4::<S>::LEN {
        let (left_x, left_z, right_x, right_z) = pauli_vectors::<S, u64x4<S>>(
            simd,
            left_x,
            left_z,
            right_x,
            right_z,
            &mut count_bit_1,
            &mut count_bit_2,
            &mut nonzero_or,
        );
        pauli_vectors::<S, S::u64s>(
            simd,
            left_x,
            left_z,
            right_x,
            right_z,
            &mut count_bit_1,
            &mut count_bit_2,
            &mut nonzero_or,
        )
    } else {
        pauli_vectors::<S, S::u64s>(
            simd,
            left_x,
            left_z,
            right_x,
            right_z,
            &mut count_bit_1,
            &mut count_bit_2,
            &mut nonzero_or,
        )
    };

    if !left_x.is_empty() {
        for (((lx, lz), rx), rz) in left_x.iter_mut().zip(left_z).zip(right_x).zip(right_z) {
            let old_lx = *lx;
            let old_lz = *lz;
            let rx_w = *rx;
            let rz_w = *rz;
            let new_lx = old_lx ^ rx_w;
            let new_lz = old_lz ^ rz_w;
            *lx = new_lx;
            *lz = new_lz;

            let old_x_new_z = old_lx & rz_w;
            let anti_commutes = (rx_w & old_lz) ^ old_x_new_z;
            count_bit_2 ^= (count_bit_1 ^ new_lx ^ new_lz ^ old_x_new_z) & anti_commutes;
            count_bit_1 ^= anti_commutes;
            nonzero_or |= new_lx | new_lz;
        }
    }

    PauliWordProduct {
        log_i: mod4_from_counters(count_bit_1, count_bit_2),
        has_terms: nonzero_or != 0,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "borrows the four Pauli planes and three scalar accumulators without wrapper structs"
)]
#[inline(always)]
fn pauli_vectors<'lx, 'lz, 'rx, 'rz, S, V>(
    simd: S,
    left_x: &'lx mut [u64],
    left_z: &'lz mut [u64],
    right_x: &'rx [u64],
    right_z: &'rz [u64],
    count_bit_1: &mut u64,
    count_bit_2: &mut u64,
    nonzero_or: &mut u64,
) -> (&'lx mut [u64], &'lz mut [u64], &'rx [u64], &'rz [u64])
where
    S: Simd,
    V: SimdBase<S, Element = u64> + BitAnd<Output = V> + BitOr<Output = V> + BitXor<Output = V>,
{
    if left_x.len() < V::LEN {
        return (left_x, left_z, right_x, right_z);
    }
    let mut cnt1 = V::splat(simd, 0);
    let mut cnt2 = V::splat(simd, 0);
    let mut nonzero = V::splat(simd, 0);
    for (((lx, lz), rx), rz) in left_x
        .chunks_exact_mut(V::LEN)
        .zip(left_z.chunks_exact_mut(V::LEN))
        .zip(right_x.chunks_exact(V::LEN))
        .zip(right_z.chunks_exact(V::LEN))
    {
        let old_lx = V::from_slice(simd, lx);
        let old_lz = V::from_slice(simd, lz);
        let rx_v = V::from_slice(simd, rx);
        let rz_v = V::from_slice(simd, rz);
        let new_lx = old_lx ^ rx_v;
        let new_lz = old_lz ^ rz_v;
        new_lx.store_slice(lx);
        new_lz.store_slice(lz);

        let old_x_new_z = old_lx & rz_v;
        let anti_commutes = (rx_v & old_lz) ^ old_x_new_z;
        cnt2 = cnt2 ^ ((cnt1 ^ new_lx ^ new_lz ^ old_x_new_z) & anti_commutes);
        cnt1 = cnt1 ^ anti_commutes;
        nonzero = nonzero | new_lx | new_lz;
    }
    for ((&lane1, &lane2), &nz) in cnt1
        .as_slice()
        .iter()
        .zip(cnt2.as_slice())
        .zip(nonzero.as_slice())
    {
        add_mod4_counters(count_bit_1, count_bit_2, lane1, lane2);
        *nonzero_or |= nz;
    }
    (
        left_x.chunks_exact_mut(V::LEN).into_remainder(),
        left_z.chunks_exact_mut(V::LEN).into_remainder(),
        right_x.chunks_exact(V::LEN).remainder(),
        right_z.chunks_exact(V::LEN).remainder(),
    )
}

#[inline(always)]
fn add_mod4_counters(c1: &mut u64, c2: &mut u64, rhs1: u64, rhs2: u64) {
    let carry = *c1 & rhs1;
    *c1 ^= rhs1;
    *c2 ^= rhs2 ^ carry;
}

#[inline(always)]
const fn mod4_from_counters(count_bit_1: u64, count_bit_2: u64) -> u8 {
    match (count_bit_1.count_ones() ^ (count_bit_2.count_ones() << 1)) & 3 {
        0 => 0,
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests;
