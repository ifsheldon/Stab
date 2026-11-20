//! Slice-level SIMD word kernels for Stab, built on `fearless_simd`.
//!
//! Each public kernel checks its slice lengths once, selects the instruction set once with
//! [`Level::new`], and then runs a body that is generic over [`Simd`], so one source serves
//! 128-bit, 256-bit, and 512-bit hardware. Main loops use a fixed minimum block (`u64x8` for XOR,
//! `u64x4` for Pauli and Clifford products on 128-bit levels) so a block's loads are issued before
//! its stores; remainders go through the native-width `S::u64s` vector and then scalar words. On
//! 128-bit levels a Pauli product of at least 64 words instead runs six-word blocks that give two
//! words to the scalar units next to one `u64x4`, because the vector units alone bound that loop.
//! The kernels own chunking, scalar tails, and phase/non-identity counting.

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

/// Borrowed packed X and Z planes of a Pauli string, one bit per qubit in each plane.
#[derive(Clone, Copy, Debug)]
pub struct PauliPlanes<'a> {
    pub x: &'a [u64],
    pub z: &'a [u64],
}

/// Mutably borrowed packed X and Z planes of a Pauli string.
#[derive(Debug)]
pub struct PauliPlanesMut<'a> {
    pub x: &'a mut [u64],
    pub z: &'a mut [u64],
}

/// Phase exponent and non-identity status of an in-place Pauli word product.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PauliWordProduct {
    /// The exponent `k` of the product's scalar `i^k`, in `0..4`.
    pub log_i: u8,
    pub has_terms: bool,
}

/// Right-multiplies `right` into `left` qubit by qubit.
///
/// Returns the base-`i` exponent of the product's scalar and whether any product qubit is not
/// identity.
///
/// # Errors
///
/// Returns [`WordLengthMismatch`] without modifying `left` when any of the four planes differs
/// in length from `left.x`.
#[inline]
pub fn pauli_right_multiply_words(
    left: PauliPlanesMut<'_>,
    right: PauliPlanes<'_>,
) -> Result<PauliWordProduct, WordLengthMismatch> {
    pauli_right_multiply::<true>(left, right)
}

/// Right-multiplies `right` into `left` qubit by qubit and returns only the base-`i` exponent of
/// the product's scalar, in `0..4`.
///
/// This is [`pauli_right_multiply_words`] without the non-identity tracking, for callers such as
/// tableau row products that do not need it.
///
/// # Errors
///
/// Returns [`WordLengthMismatch`] without modifying `left` when any of the four planes differs
/// in length from `left.x`.
#[inline]
pub fn pauli_right_multiply_words_log_i(
    left: PauliPlanesMut<'_>,
    right: PauliPlanes<'_>,
) -> Result<u8, WordLengthMismatch> {
    pauli_right_multiply::<false>(left, right).map(|product| product.log_i)
}

/// Checks the plane lengths, multiplies empty and single-word planes inline, and dispatches longer
/// planes.
///
/// `has_terms` is only computed when `TRACK_NONZERO` is set and is `false` otherwise.
#[inline(always)]
fn pauli_right_multiply<const TRACK_NONZERO: bool>(
    left: PauliPlanesMut<'_>,
    right: PauliPlanes<'_>,
) -> Result<PauliWordProduct, WordLengthMismatch> {
    let PauliPlanesMut {
        x: left_x,
        z: left_z,
    } = left;
    let PauliPlanes {
        x: right_x,
        z: right_z,
    } = right;
    let expected = left_x.len();
    for actual in [left_z.len(), right_x.len(), right_z.len()] {
        ensure_len(expected, actual)?;
    }
    if expected == 0 {
        return Ok(PhaseCounters::ZERO.product());
    }
    if let ([lx], [lz], [rx], [rz]) = (&mut *left_x, &mut *left_z, right_x, right_z) {
        let mut counters = PhaseCounters::ZERO;
        (*lx, *lz) = counters.multiply::<TRACK_NONZERO>(*lx, *lz, *rx, *rz);
        return Ok(counters.product());
    }
    let level = Level::new();
    Ok(dispatch!(
        level,
        simd => pauli_right_multiply_words_simd::<_, TRACK_NONZERO>(
            simd, left_x, left_z, right_x, right_z
        )
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

/// The four planes of an in-place Pauli product; callers guarantee equal lengths.
struct PauliProductPlanes<'l, 'r> {
    left_x: &'l mut [u64],
    left_z: &'l mut [u64],
    right_x: &'r [u64],
    right_z: &'r [u64],
}

impl<'l, 'r> PauliProductPlanes<'l, 'r> {
    /// Calls `body` on each aligned group of `chunk` words of all four planes and returns the
    /// planes' unprocessed remainders.
    #[inline(always)]
    fn for_each_chunk(
        self,
        chunk: usize,
        mut body: impl FnMut(&mut [u64], &mut [u64], &[u64], &[u64]),
    ) -> Self {
        let Self {
            left_x,
            left_z,
            right_x,
            right_z,
        } = self;
        for (((lx, lz), rx), rz) in left_x
            .chunks_exact_mut(chunk)
            .zip(left_z.chunks_exact_mut(chunk))
            .zip(right_x.chunks_exact(chunk))
            .zip(right_z.chunks_exact(chunk))
        {
            body(lx, lz, rx, rz);
        }
        Self {
            left_x: left_x.chunks_exact_mut(chunk).into_remainder(),
            left_z: left_z.chunks_exact_mut(chunk).into_remainder(),
            right_x: right_x.chunks_exact(chunk).remainder(),
            right_z: right_z.chunks_exact(chunk).remainder(),
        }
    }
}

/// Per-bit running mod-4 count of the product's `i` factors, plus the OR of every product word
/// when non-identity tracking is on. One formula serves vectors and scalar words.
#[derive(Clone, Copy)]
struct PhaseCounters<T> {
    bit_1: T,
    bit_2: T,
    nonzero: T,
}

impl PhaseCounters<u64> {
    const ZERO: Self = Self {
        bit_1: 0,
        bit_2: 0,
        nonzero: 0,
    };

    /// Multiplies the planes one word at a time.
    #[inline(always)]
    fn words<const TRACK_NONZERO: bool>(&mut self, planes: PauliProductPlanes<'_, '_>) {
        let PauliProductPlanes {
            left_x,
            left_z,
            right_x,
            right_z,
        } = planes;
        for (((lx, lz), rx), rz) in left_x.iter_mut().zip(left_z).zip(right_x).zip(right_z) {
            (*lx, *lz) = self.multiply::<TRACK_NONZERO>(*lx, *lz, *rx, *rz);
        }
    }

    /// Multiplies every full `V` vector of the planes and returns the unprocessed remainders.
    #[inline(always)]
    fn vectors<'l, 'r, S, V, const TRACK_NONZERO: bool>(
        &mut self,
        simd: S,
        planes: PauliProductPlanes<'l, 'r>,
    ) -> PauliProductPlanes<'l, 'r>
    where
        S: Simd,
        V: SimdBase<S, Element = u64> + BitAnd<Output = V> + BitOr<Output = V> + BitXor<Output = V>,
    {
        if planes.left_x.len() < V::LEN {
            return planes;
        }
        let mut vector = PhaseCounters::<V>::splat(simd, 0);
        let rest = planes.for_each_chunk(V::LEN, |lx, lz, rx, rz| {
            vector.multiply_slices::<S, TRACK_NONZERO>(simd, lx, lz, rx, rz);
        });
        self.absorb_lanes(&vector);
        rest
    }

    /// Adds every lane of vector counters into these scalar counters.
    #[inline(always)]
    fn absorb_lanes<S: Simd, V: SimdBase<S, Element = u64>>(&mut self, vector: &PhaseCounters<V>) {
        let lanes = vector.bit_1.as_slice().iter().zip(vector.bit_2.as_slice());
        for ((&bit_1, &bit_2), &nonzero) in lanes.zip(vector.nonzero.as_slice()) {
            let carry = self.bit_1 & bit_1;
            self.bit_1 ^= bit_1;
            self.bit_2 ^= bit_2 ^ carry;
            self.nonzero |= nonzero;
        }
    }

    #[inline(always)]
    const fn product(self) -> PauliWordProduct {
        PauliWordProduct {
            log_i: mod4_from_counters(self.bit_1, self.bit_2),
            has_terms: self.nonzero != 0,
        }
    }
}

impl<V: Copy + BitAnd<Output = V> + BitOr<Output = V> + BitXor<Output = V>> PhaseCounters<V> {
    #[inline(always)]
    fn splat<S: Simd>(simd: S, value: u64) -> Self
    where
        V: SimdBase<S, Element = u64>,
    {
        let value = V::splat(simd, value);
        Self {
            bit_1: value,
            bit_2: value,
            nonzero: value,
        }
    }

    /// Multiplies one vector of words of each plane in place, counting its phase.
    #[inline(always)]
    fn multiply_slices<S: Simd, const TRACK_NONZERO: bool>(
        &mut self,
        simd: S,
        left_x: &mut [u64],
        left_z: &mut [u64],
        right_x: &[u64],
        right_z: &[u64],
    ) where
        V: SimdBase<S, Element = u64>,
    {
        let (new_x, new_z) = self.multiply::<TRACK_NONZERO>(
            V::from_slice(simd, left_x),
            V::from_slice(simd, left_z),
            V::from_slice(simd, right_x),
            V::from_slice(simd, right_z),
        );
        new_x.store_slice(left_x);
        new_z.store_slice(left_z);
    }
}

impl<T> PhaseCounters<T>
where
    T: Copy + BitAnd<Output = T> + BitOr<Output = T> + BitXor<Output = T>,
{
    /// Returns the product planes of one word group and counts its phase.
    #[inline(always)]
    fn multiply<const TRACK_NONZERO: bool>(
        &mut self,
        left_x: T,
        left_z: T,
        right_x: T,
        right_z: T,
    ) -> (T, T) {
        let new_x = left_x ^ right_x;
        let new_z = left_z ^ right_z;
        let old_x_new_z = left_x & right_z;
        let anti_commutes = (right_x & left_z) ^ old_x_new_z;
        self.bit_2 = self.bit_2 ^ ((self.bit_1 ^ new_x ^ new_z ^ old_x_new_z) & anti_commutes);
        self.bit_1 = self.bit_1 ^ anti_commutes;
        if TRACK_NONZERO {
            self.nonzero = self.nonzero | new_x | new_z;
        }
        (new_x, new_z)
    }
}

/// The vectorized body of [`pauli_right_multiply`] for planes of at least two words.
///
/// On 128-bit levels planes of at least [`MIXED_MIN_WORDS`] words go to
/// [`pauli_mixed_blocks_simd`], and shorter planes use `u64x4` (two native registers per plane) so
/// a block's loads are issued before its stores. Wider levels use the native vector directly.
/// Native vectors and then scalar words finish the remainder.
#[simd]
fn pauli_right_multiply_words_simd<S: Simd, const TRACK_NONZERO: bool>(
    simd: S,
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> PauliWordProduct {
    if S::u64s::LEN < u64x4::<S>::LEN && left_x.len() >= MIXED_MIN_WORDS {
        return pauli_mixed_blocks_simd::<S, TRACK_NONZERO>(simd, left_x, left_z, right_x, right_z);
    }
    let planes = PauliProductPlanes {
        left_x,
        left_z,
        right_x,
        right_z,
    };
    let mut counters = PhaseCounters::ZERO;
    let planes = if S::u64s::LEN < u64x4::<S>::LEN {
        let planes = counters.vectors::<S, u64x4<S>, TRACK_NONZERO>(simd, planes);
        counters.vectors::<S, S::u64s, TRACK_NONZERO>(simd, planes)
    } else {
        counters.vectors::<S, S::u64s, TRACK_NONZERO>(simd, planes)
    };
    counters.words::<TRACK_NONZERO>(planes);
    counters.product()
}

/// Planes of at least this many words take [`pauli_mixed_blocks_simd`] on 128-bit levels; below
/// it, the call and the larger loop cost more than the blocks save.
const MIXED_MIN_WORDS: usize = 64;
/// Words per block of [`pauli_mixed_blocks_simd`]: one `u64x4` and [`MIXED_SCALAR_WORDS`] scalar
/// words.
const MIXED_BLOCK_WORDS: usize = 4 + MIXED_SCALAR_WORDS;
/// Scalar words per block of [`pauli_mixed_blocks_simd`].
const MIXED_SCALAR_WORDS: usize = 2;

/// Multiplies the planes in blocks of one `u64x4` and two scalar words, then finishes with one
/// `u64x4` and scalar words.
///
/// On 128-bit levels the vector units alone bound the Pauli product loop, so giving a third of
/// each block to the scalar units raises throughput. The loop needs many registers, so it stays
/// out of line, taking each plane as its own argument, and shorter products do not pay for saving
/// them.
#[simd]
#[inline(never)]
fn pauli_mixed_blocks_simd<S: Simd, const TRACK_NONZERO: bool>(
    simd: S,
    left_x: &mut [u64],
    left_z: &mut [u64],
    right_x: &[u64],
    right_z: &[u64],
) -> PauliWordProduct {
    let planes = PauliProductPlanes {
        left_x,
        left_z,
        right_x,
        right_z,
    };
    let mut counters = PhaseCounters::ZERO;
    let mut vector = PhaseCounters::<u64x4<S>>::splat(simd, 0);
    let planes = planes.for_each_chunk(MIXED_BLOCK_WORDS, |lx, lz, rx, rz| {
        let (lx_v, lx_s) = lx.split_at_mut(u64x4::<S>::LEN);
        let (lz_v, lz_s) = lz.split_at_mut(u64x4::<S>::LEN);
        let (rx_v, rx_s) = rx.split_at(u64x4::<S>::LEN);
        let (rz_v, rz_s) = rz.split_at(u64x4::<S>::LEN);
        vector.multiply_slices::<S, TRACK_NONZERO>(simd, lx_v, lz_v, rx_v, rz_v);
        counters.words::<TRACK_NONZERO>(PauliProductPlanes {
            left_x: lx_s,
            left_z: lz_s,
            right_x: rx_s,
            right_z: rz_s,
        });
    });
    let planes = planes.for_each_chunk(u64x4::<S>::LEN, |lx, lz, rx, rz| {
        vector.multiply_slices::<S, TRACK_NONZERO>(simd, lx, lz, rx, rz);
    });
    counters.absorb_lanes(&vector);
    counters.words::<TRACK_NONZERO>(planes);
    counters.product()
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
