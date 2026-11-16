#![allow(
    clippy::cast_possible_truncation,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "differential fixtures index bounded test vectors and derive small deterministic seeds"
)]

use super::*;

/// Covers empty input, every tail length, and at least four full vectors at 512 bits (8 words).
const MAX_WORDS: usize = 40;

type Planes = [Vec<u64>; 6];

/// Runs every differential check against one instruction-set level.
fn check_level<S: Simd>(simd: S) {
    for word_count in 0..=MAX_WORDS {
        check_xor(simd, word_count);
        check_clifford(simd, word_count);
        check_pauli_multiply(simd, word_count);
    }
}

fn check_xor<S: Simd>(simd: S, word_count: usize) {
    let left = words(word_count, 1);
    let right = words(word_count, 9_001);
    let mut actual = left.clone();
    xor_assign_words_simd(simd, &mut actual, &right);
    let expected = (0..word_count)
        .map(|word| left[word] ^ right[word])
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        expected,
        "level={:?} words={word_count}",
        simd.level()
    );
}

fn check_clifford<S: Simd>(simd: S, word_count: usize) {
    let left = planes(word_count, 3);
    let right = planes(word_count, 41);
    let (expected, expected_counts) = reference_clifford(&left, &right);
    let mut actual = left;
    let counts =
        clifford_right_multiply_words_simd(simd, planes_mut(&mut actual), planes_ref(&right));
    let context = format!("level={:?} words={word_count}", simd.level());
    assert_eq!(actual, expected, "{context}");
    assert_eq!(counts, expected_counts, "{context}");
}

fn check_pauli_multiply<S: Simd>(simd: S, word_count: usize) {
    let left_x = words(word_count, 17);
    let left_z = words(word_count, 29);
    let right_x = words(word_count, 53);
    let right_z = words(word_count, 71);
    assert_pauli_case(simd, &left_x, &left_z, &right_x, &right_z);
    // Cancelling equal operands produces all-zero output (`has_terms == false`, `log_i == 0`).
    assert_pauli_case(simd, &left_x, &left_z, &left_x, &left_z);
    if word_count > 0 {
        let mut sparse_rx = left_x.clone();
        sparse_rx[word_count - 1] ^= 1_u64 << 63;
        assert_pauli_case(simd, &left_x, &left_z, &sparse_rx, &left_z);
    }
}

fn assert_pauli_case<S: Simd>(
    simd: S,
    left_x: &[u64],
    left_z: &[u64],
    right_x: &[u64],
    right_z: &[u64],
) {
    let (expected_x, expected_z, expected_product) =
        reference_pauli_multiply(left_x, left_z, right_x, right_z);
    let mut actual_x = left_x.to_vec();
    let mut actual_z = left_z.to_vec();
    let product =
        pauli_right_multiply_words_simd(simd, &mut actual_x, &mut actual_z, right_x, right_z);
    let context = format!("level={:?} words={}", simd.level(), left_x.len());
    assert_eq!(actual_x, expected_x, "{context}");
    assert_eq!(actual_z, expected_z, "{context}");
    assert_eq!(product, expected_product, "{context}");
}

/// Independent bit-by-bit Pauli multiplication reference using the 4x4 single-qubit product table.
fn reference_pauli_multiply(
    left_x: &[u64],
    left_z: &[u64],
    right_x: &[u64],
    right_z: &[u64],
) -> (Vec<u64>, Vec<u64>, PauliWordProduct) {
    let word_count = left_x.len();
    let mut out_x = vec![0_u64; word_count];
    let mut out_z = vec![0_u64; word_count];
    let mut log_i = 0_u8;
    let mut has_terms = false;
    for word in 0..word_count {
        for bit in 0..64 {
            let lx = (left_x[word] >> bit) & 1;
            let lz = (left_z[word] >> bit) & 1;
            let rx = (right_x[word] >> bit) & 1;
            let rz = (right_z[word] >> bit) & 1;
            let nx = lx ^ rx;
            let nz = lz ^ rz;
            out_x[word] |= nx << bit;
            out_z[word] |= nz << bit;
            has_terms |= (nx | nz) != 0;
            // Encode I=0, X=1, Z=2, Y=3.
            let l_code = lx | (lz << 1);
            let r_code = rx | (rz << 1);
            let delta = match (l_code, r_code) {
                (1, 3) | (3, 2) | (2, 1) => 1, // X*Y=+iZ, Y*Z=+iX, Z*X=+iY
                (3, 1) | (2, 3) | (1, 2) => 3, // Y*X=-iZ, Z*Y=-iX, X*Z=-iY
                _ => 0,
            };
            log_i = (log_i + delta) & 3;
        }
    }
    (out_x, out_z, PauliWordProduct { log_i, has_terms })
}

/// An independent reference: a separately written copy of the plane formula applied one `u64`
/// at a time, with non-identity counts taken bit by bit, sharing no chunking, tail, vector, or
/// counting code with the kernel.
fn reference_clifford(left: &Planes, right: &Planes) -> (Planes, CliffordNonIdentityCounts) {
    let word_count = left[0].len();
    let mut result: Planes = std::array::from_fn(|_| vec![0; word_count]);
    let mut counts = CliffordNonIdentityCounts::default();
    for word in 0..word_count {
        let [lz, lx, lixx, lxz, lzx, lizz] = std::array::from_fn(|plane| left[plane][word]);
        let [rz, rx, rixx, rxz, rzx, rizz] = std::array::from_fn(|plane| right[plane][word]);
        let dy = (lxz & lzx) ^ lixx ^ lzx ^ lxz ^ lizz;
        let product = [
            rz ^ (rzx & lx) ^ (!rizz & rzx & dy) ^ (!rizz & lz),
            rx ^ (!rixx & lx) ^ (!rixx & rxz & dy) ^ (rxz & lz),
            (lixx | rixx) ^ (lzx & rxz),
            (!rixx & lxz) ^ (!lizz & rxz),
            (!lixx & rzx) ^ (!rizz & lzx),
            (lxz & rzx) ^ (lizz | rizz),
        ];
        for bit in 0..64 {
            let is_set = |values: [u64; 6]| values.iter().any(|value| (value >> bit) & 1 == 1);
            counts.before += usize::from(is_set([lz, lx, lixx, lxz, lzx, lizz]));
            counts.after += usize::from(is_set(product));
        }
        for (values, value) in result.iter_mut().zip(product) {
            values[word] = value;
        }
    }
    (result, counts)
}

fn planes(word_count: usize, seed: u64) -> Planes {
    std::array::from_fn(|plane| words(word_count, seed + 1_000 * plane as u64))
}

fn planes_mut(planes: &mut Planes) -> [&mut [u64]; 6] {
    planes.each_mut().map(Vec::as_mut_slice)
}

fn planes_ref(planes: &Planes) -> [&[u64]; 6] {
    planes.each_ref().map(Vec::as_slice)
}

fn words(word_count: usize, seed: u64) -> Vec<u64> {
    (0..word_count as u64)
        .map(|word| splitmix64(seed + word))
        .collect()
}

fn splitmix64(index: u64) -> u64 {
    let mut value = index.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[test]
fn scalar_fallback_matches_reference() {
    check_level(fearless_simd::Fallback::new());
}

#[cfg(target_arch = "aarch64")]
#[test]
fn neon_matches_reference() {
    let neon = Level::new().as_neon().unwrap();
    check_level(neon);
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[test]
fn every_available_x86_level_matches_reference() {
    let level = Level::new();
    let sse2 = level.as_sse2().unwrap();
    check_level(sse2);
    if let Some(sse4_2) = level.as_sse4_2() {
        check_level(sse4_2);
    }
    if let Some(avx2) = level.as_avx2() {
        check_level(avx2);
    }
    if let Some(avx512) = level.as_avx512() {
        check_level(avx512);
    }
}

#[test]
fn public_kernels_match_reference_at_the_detected_level() {
    for word_count in 0..=MAX_WORDS {
        let left = words(word_count, 5);
        let right = words(word_count, 77);
        let mut actual = left.clone();
        xor_assign_words(&mut actual, &right).unwrap();
        let expected = (0..word_count)
            .map(|word| left[word] ^ right[word])
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "words={word_count}");

        let left = planes(word_count, 7);
        let right = planes(word_count, 99);
        let (expected, expected_counts) = reference_clifford(&left, &right);
        let mut actual = left;
        let counts =
            clifford_right_multiply_words(public_mut(&mut actual), public_ref(&right)).unwrap();
        assert_eq!(actual, expected, "words={word_count}");
        assert_eq!(counts, expected_counts, "words={word_count}");

        let lx = words(word_count, 19);
        let lz = words(word_count, 37);
        let rx = words(word_count, 61);
        let rz = words(word_count, 83);
        let (exp_x, exp_z, exp_prod) = reference_pauli_multiply(&lx, &lz, &rx, &rz);
        let mut act_x = lx;
        let mut act_z = lz;
        let prod = pauli_right_multiply_words(&mut act_x, &mut act_z, &rx, &rz).unwrap();
        assert_eq!(act_x, exp_x, "words={word_count}");
        assert_eq!(act_z, exp_z, "words={word_count}");
        assert_eq!(prod, exp_prod, "words={word_count}");
    }
}

#[test]
fn pauli_length_mismatch_in_any_plane_is_rejected_without_writing() {
    for short_plane in 0..4 {
        let mut planes = [words(5, 1), words(5, 2), words(5, 3), words(5, 4)];
        planes[short_plane].pop();
        let [mut lx, mut lz, rx, rz] = planes;
        let orig_x = lx.clone();
        let orig_z = lz.clone();
        let error = pauli_right_multiply_words(&mut lx, &mut lz, &rx, &rz).unwrap_err();
        let expected = if short_plane == 0 {
            WordLengthMismatch {
                expected: 4,
                actual: 5,
            }
        } else {
            WordLengthMismatch {
                expected: 5,
                actual: 4,
            }
        };
        assert_eq!(error, expected, "plane={short_plane}");
        assert_eq!(lx, orig_x, "plane={short_plane}");
        assert_eq!(lz, orig_z, "plane={short_plane}");
    }
}

#[test]
fn xor_length_mismatch_is_rejected_without_writing() {
    let mut lhs = vec![1_u64, 2, 3];
    let error = xor_assign_words(&mut lhs, &[7, 7]).unwrap_err();
    assert_eq!(
        error,
        WordLengthMismatch {
            expected: 3,
            actual: 2
        }
    );
    assert_eq!(lhs, [1, 2, 3]);
}

#[test]
fn clifford_length_mismatch_in_any_plane_is_rejected_without_writing() {
    for side in [Side::Left, Side::Right] {
        for short_plane in 0..6 {
            let mut left = planes(5, 11);
            let mut right = planes(5, 13);
            let target = match side {
                Side::Left => &mut left[short_plane],
                Side::Right => &mut right[short_plane],
            };
            target.pop();
            let original = left.clone();
            let error = clifford_right_multiply_words(public_mut(&mut left), public_ref(&right))
                .unwrap_err();
            let expected = if side == Side::Left && short_plane == 0 {
                WordLengthMismatch {
                    expected: 4,
                    actual: 5,
                }
            } else {
                WordLengthMismatch {
                    expected: 5,
                    actual: 4,
                }
            };
            assert_eq!(error, expected, "side={side:?} plane={short_plane}");
            assert_eq!(left, original, "side={side:?} plane={short_plane}");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Left,
    Right,
}

fn public_mut(planes: &mut Planes) -> CliffordPlanesMut<'_> {
    let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes_mut(planes);
    CliffordPlanesMut {
        z_signs,
        x_signs,
        inv_x2x,
        x2z,
        z2x,
        inv_z2z,
    }
}

fn public_ref(planes: &Planes) -> CliffordPlanes<'_> {
    let [z_signs, x_signs, inv_x2x, x2z, z2x, inv_z2z] = planes_ref(planes);
    CliffordPlanes {
        z_signs,
        x_signs,
        inv_x2x,
        x2z,
        z2x,
        inv_z2z,
    }
}
