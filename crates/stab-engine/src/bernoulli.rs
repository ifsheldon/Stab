use rand::{Rng, RngExt as _};

pub(crate) const INDEXED_BLOCK_SHOTS: usize = 1024;
pub(crate) const INDEXED_BLOCK_WORDS: usize = INDEXED_BLOCK_SHOTS / u64::BITS as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum IndexedRangeError {
    TooManyShots,
    ShotRangeOverflow,
    OutputWidthMismatch,
}

pub(crate) fn sample_mask(probability: f64, lanes: u64, rng: &mut impl Rng) -> u64 {
    if lanes == 0 {
        return 0;
    }
    let mut word = [0_u64; 1];
    fill_words(probability, &mut word, rng);
    word[0] & lanes
}

pub(crate) fn fill_words(probability: f64, output: &mut [u64], rng: &mut impl Rng) {
    sample_words(probability, output, &mut RandSource(rng));
}

/// Samples globally indexed Bernoulli trials into shot-major low-to-high words.
///
/// Every aligned 1024-shot block has an independent deterministic substream. Splitting one seeded
/// shot range across calls therefore preserves the sampled records exactly.
pub(crate) fn sample_indexed_range_into(
    probability: f64,
    seed: u64,
    stream_index: u64,
    first_shot: u64,
    shot_count: usize,
    output: &mut [u64],
) -> Result<(), IndexedRangeError> {
    if shot_count > INDEXED_BLOCK_SHOTS {
        return Err(IndexedRangeError::TooManyShots);
    }
    let expected_words = shot_count.div_ceil(u64::BITS as usize);
    if output.len() != expected_words {
        return Err(IndexedRangeError::OutputWidthMismatch);
    }
    let shot_count_u64 = u64::try_from(shot_count).map_err(|_| IndexedRangeError::TooManyShots)?;
    first_shot
        .checked_add(shot_count_u64)
        .ok_or(IndexedRangeError::ShotRangeOverflow)?;
    output.fill(0);

    let mut sampled_block = [0_u64; INDEXED_BLOCK_WORDS];
    let mut produced = 0_usize;
    while produced < shot_count {
        let produced_u64 = u64::try_from(produced).map_err(|_| IndexedRangeError::TooManyShots)?;
        let global_shot = first_shot
            .checked_add(produced_u64)
            .ok_or(IndexedRangeError::ShotRangeOverflow)?;
        let block = global_shot / INDEXED_BLOCK_SHOTS as u64;
        let block_offset = usize::try_from(global_shot % INDEXED_BLOCK_SHOTS as u64)
            .map_err(|_| IndexedRangeError::TooManyShots)?;
        let take = (INDEXED_BLOCK_SHOTS - block_offset).min(shot_count - produced);
        let mut rng = SplitMixSource::new(indexed_seed(seed, stream_index, block));
        sample_words(probability, &mut sampled_block, &mut rng);
        copy_bit_range(&sampled_block, block_offset, output, produced, take);
        produced += take;
    }
    Ok(())
}

trait RandomSource {
    fn next_word(&mut self) -> u64;

    #[allow(
        clippy::cast_precision_loss,
        reason = "the high 53 random bits are intentionally mapped onto the exactly representable f64 mantissa"
    )]
    fn next_unit_f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / ((1_u64 << 53) as f64);
        ((self.next_word() >> 11) as f64) * SCALE
    }
}

struct RandSource<'a, R>(&'a mut R);

impl<R: Rng> RandomSource for RandSource<'_, R> {
    fn next_word(&mut self) -> u64 {
        self.0.random()
    }

    fn next_unit_f64(&mut self) -> f64 {
        self.0.random()
    }
}

struct SplitMixSource {
    state: u64,
}

impl SplitMixSource {
    const fn new(seed: u64) -> Self {
        Self { state: seed }
    }
}

impl RandomSource for SplitMixSource {
    #[inline(always)]
    fn next_word(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        splitmix64_round(self.state)
    }
}

fn sample_words(probability: f64, output: &mut [u64], rng: &mut impl RandomSource) {
    if probability <= 0.0 || output.is_empty() {
        output.fill(0);
        return;
    }
    if probability >= 1.0 {
        output.fill(u64::MAX);
        return;
    }
    if probability == 0.5 {
        output.fill_with(|| rng.next_word());
        return;
    }
    if probability > 0.5 {
        sample_words(1.0 - probability, output, rng);
        for word in output {
            *word = !*word;
        }
        return;
    }
    if probability < 0.02 {
        output.fill(0);
        or_sparse_hits(probability, output, rng);
    } else {
        sample_dense_words(probability, output, rng);
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "the finite nonnegative geometric gap is bounded by the remaining bit count before conversion"
)]
fn or_sparse_hits(probability: f64, output: &mut [u64], rng: &mut impl RandomSource) {
    let bit_count = output.len().saturating_mul(u64::BITS as usize);
    let log_failure = (-probability).ln_1p();
    let mut position = 0_usize;
    while position < bit_count {
        let uniform = 1.0 - rng.next_unit_f64();
        let gap = (uniform.ln() / log_failure).floor();
        if !gap.is_finite() || gap >= bit_count.saturating_sub(position) as f64 {
            break;
        }
        position += gap as usize;
        output[position / u64::BITS as usize] |= 1_u64 << (position % u64::BITS as usize);
        position += 1;
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "a probability below one-half scaled by 256 is a nonnegative integer below 128"
)]
fn sample_dense_words(probability: f64, output: &mut [u64], rng: &mut impl RandomSource) {
    const BUCKETS: f64 = 256.0;
    let raised = probability * BUCKETS;
    let raised_floor = raised.floor();
    let p_truncated = raised_floor / BUCKETS;
    let p_leftover = (raised - raised_floor) / BUCKETS;
    let top_bits = raised_floor as u64;

    for word in output.iter_mut() {
        let mut alive = rng.next_word();
        let mut result = 0_u64;
        for probability_bit in (0..7).rev() {
            let shoot = rng.next_word();
            if top_bits & (1_u64 << probability_bit) != 0 {
                result ^= shoot & alive;
            }
            alive &= !shoot;
        }
        *word = result;
    }

    let correction_probability = p_leftover / (1.0 - p_truncated);
    if correction_probability > 0.0 {
        or_sparse_hits(correction_probability, output, rng);
    }
}

#[allow(
    clippy::indexing_slicing,
    reason = "source and target offsets are bounded by the validated shot range and exact output width"
)]
fn copy_bit_range(
    source: &[u64],
    source_offset: usize,
    target: &mut [u64],
    target_offset: usize,
    bit_count: usize,
) {
    let mut copied = 0_usize;
    while copied < bit_count {
        let source_position = source_offset + copied;
        let target_position = target_offset + copied;
        let source_word = source_position / u64::BITS as usize;
        let target_word = target_position / u64::BITS as usize;
        let source_bit = source_position % u64::BITS as usize;
        let target_bit = target_position % u64::BITS as usize;
        let chunk = (bit_count - copied)
            .min(u64::BITS as usize - source_bit)
            .min(u64::BITS as usize - target_bit);
        let bits = (source[source_word] >> source_bit) & low_bits_mask(chunk);
        target[target_word] |= bits << target_bit;
        copied += chunk;
    }
}

fn indexed_seed(seed: u64, stream_index: u64, block: u64) -> u64 {
    splitmix64(
        seed ^ splitmix64(stream_index.wrapping_add(0xA076_1D64_78BD_642F))
            ^ splitmix64(block.wrapping_add(0xE703_7ED1_A0B4_28DB)),
    )
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9E37_79B9_7F4A_7C15);
    splitmix64_round(value)
}

fn splitmix64_round(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

const fn low_bits_mask(bits: usize) -> u64 {
    if bits >= u64::BITS as usize {
        u64::MAX
    } else if bits == 0 {
        0
    } else {
        (1_u64 << bits) - 1
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::indexing_slicing,
        reason = "focused kernel tests use valid fixed-size ranges and fail immediately on fixture errors"
    )]

    use super::*;
    use rand::SeedableRng as _;
    use rand::rngs::SmallRng;

    fn indexed_range(
        probability: f64,
        seed: u64,
        stream_index: u64,
        first_shot: u64,
        shot_count: usize,
    ) -> Vec<u64> {
        let mut output = vec![0; shot_count.div_ceil(u64::BITS as usize)];
        sample_indexed_range_into(
            probability,
            seed,
            stream_index,
            first_shot,
            shot_count,
            &mut output,
        )
        .expect("valid indexed range");
        output
    }

    #[test]
    fn indexed_ranges_are_independent_of_request_partitioning() {
        let whole = indexed_range(0.001, 17, 9, 0, 64)[0];
        let prefix = indexed_range(0.001, 17, 9, 0, 17)[0];
        let suffix = indexed_range(0.001, 17, 9, 17, 47)[0];
        assert_eq!(whole, prefix | (suffix << 17));

        let crossing = indexed_range(0.25, 23, 4, 1000, 64)[0];
        let first = indexed_range(0.25, 23, 4, 1000, 24)[0];
        let rest = indexed_range(0.25, 23, 4, 1024, 40)[0];
        assert_eq!(crossing, first | (rest << 24));
    }

    #[test]
    fn indexed_ranges_preserve_probability_boundaries_and_stream_identity() {
        assert_eq!(indexed_range(0.0, 1, 2, 3, 64), [0]);
        assert_eq!(indexed_range(1.0, 1, 2, 3, 64), [u64::MAX]);
        assert_ne!(
            indexed_range(0.5, 1, 2, 3, 64),
            indexed_range(0.5, 1, 3, 3, 64)
        );
    }

    #[test]
    fn indexed_ranges_reject_invalid_storage_and_overflow() {
        assert_eq!(
            sample_indexed_range_into(0.1, 1, 2, 0, 65, &mut [0]),
            Err(IndexedRangeError::OutputWidthMismatch)
        );
        assert_eq!(
            sample_indexed_range_into(0.1, 1, 2, u64::MAX, 1, &mut [0]),
            Err(IndexedRangeError::ShotRangeOverflow)
        );
    }

    #[test]
    fn sparse_and_dense_sampling_have_expected_frequencies() {
        let mut rng = SmallRng::seed_from_u64(91);
        for probability in [0.001, 0.25, 0.75] {
            let mut ones = 0_u32;
            let mut words = [0_u64; INDEXED_BLOCK_WORDS];
            for _ in 0..128 {
                sample_words(probability, &mut words, &mut RandSource(&mut rng));
                ones += words.iter().map(|word| word.count_ones()).sum::<u32>();
            }
            let trials = (words.len() * u64::BITS as usize * 128) as f64;
            let observed = f64::from(ones) / trials;
            assert!(
                (observed - probability).abs() < 0.01,
                "p={probability}, observed={observed}"
            );
        }
    }
}
