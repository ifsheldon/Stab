use rand::{Rng, RngExt as _};

use crate::Probability;

const WORD_BITS: usize = u64::BITS as usize;
const COIN_FLIPS: usize = 8;
const BUCKET_COUNT: u32 = 1 << COIN_FLIPS;
const BUCKETS: f64 = 256.0;
const RARE_PROBABILITY_THRESHOLD: f64 = 0.02;

pub fn biased_randomize_bits<R>(probability: Probability, words: &mut [u64], rng: &mut R)
where
    R: Rng,
{
    biased_randomize_bits_with_raw_probability(probability.get(), words, rng);
}

fn biased_randomize_bits_with_raw_probability<R>(probability: f64, words: &mut [u64], rng: &mut R)
where
    R: Rng,
{
    if probability == 0.0 {
        words.fill(0);
    } else if probability == 1.0 {
        words.fill(u64::MAX);
    } else if probability > 0.5 {
        biased_randomize_bits_with_raw_probability(1.0 - probability, words, rng);
        for word in words {
            *word ^= u64::MAX;
        }
    } else if probability == 0.5 {
        for word in words {
            *word = rng.random();
        }
    } else if probability < RARE_PROBABILITY_THRESHOLD {
        words.fill(0);
        sample_rare_hits(probability, words, rng);
    } else {
        biased_randomize_bits_bucketed(probability, words, rng);
    }
}

fn biased_randomize_bits_bucketed<R>(probability: f64, words: &mut [u64], rng: &mut R)
where
    R: Rng,
{
    let raised = probability * BUCKETS;
    let (p_top_bits, raised_floor) = bucket_floor(raised);
    let raised_leftover = raised - raised_floor;
    let p_truncated = raised_floor / BUCKETS;
    let p_leftover = raised_leftover / BUCKETS;

    for word in words.iter_mut() {
        let mut alive: u64 = rng.random();
        let mut result = 0u64;
        for bit_index in (0..(COIN_FLIPS - 1)).rev() {
            let shoot: u64 = rng.random();
            let mask = 0u64.wrapping_sub((p_top_bits >> bit_index) & 1);
            result ^= shoot & alive & mask;
            alive &= !shoot;
        }
        *word = result;
    }

    let correction_probability = p_leftover / (1.0 - p_truncated);
    if correction_probability > 0.0 {
        sample_rare_hits(correction_probability, words, rng);
    }
}

fn bucket_floor(raised: f64) -> (u64, f64) {
    let mut bits = 0u64;
    let mut floor = 0.0;
    for candidate in 1..=BUCKET_COUNT {
        let candidate_floor = f64::from(candidate);
        if candidate_floor > raised {
            break;
        }
        bits = u64::from(candidate);
        floor = candidate_floor;
    }
    (bits, floor)
}

fn sample_rare_hits<R>(probability: f64, words: &mut [u64], rng: &mut R)
where
    R: Rng,
{
    let bit_count = words.len().saturating_mul(WORD_BITS);
    let mut candidate = 0usize;
    let inverse_log_survival = if probability == 0.0 || probability == 1.0 {
        0.0
    } else {
        1.0 / (-probability).ln_1p()
    };
    while candidate < bit_count {
        let remaining_bits = bit_count - candidate;
        let Some(skip) = next_rare_skip(probability, inverse_log_survival, remaining_bits, rng)
        else {
            break;
        };
        let hit = candidate + skip;
        if let Some(word) = words.get_mut(hit / WORD_BITS) {
            *word |= 1u64 << (hit % WORD_BITS);
        }
        candidate = hit + 1;
    }
}

fn next_rare_skip<R>(
    probability: f64,
    inverse_log_survival: f64,
    remaining_bits: usize,
    rng: &mut R,
) -> Option<usize>
where
    R: Rng,
{
    if probability == 0.0 {
        return None;
    }
    if probability == 1.0 {
        return Some(0);
    }
    let uniform = (1.0 - rng.random::<f64>()).max(f64::MIN_POSITIVE);
    let skip = uniform.ln() * inverse_log_survival;
    finite_floor_below_bound(skip, remaining_bits)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "geometric inverse-CDF skips are finite, non-negative, and bounded before conversion"
)]
fn finite_floor_below_bound(value: f64, upper_bound: usize) -> Option<usize> {
    if !(value.is_finite() && value >= 0.0 && value < upper_bound as f64) {
        return None;
    }
    Some(value.floor() as usize)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        reason = "probability utility tests use compact direct fixture assertions"
    )]

    use rand::SeedableRng as _;
    use rand::rngs::SmallRng;

    use super::*;

    #[test]
    fn biased_randomize_bits_handles_deterministic_probabilities() {
        let mut rng = SmallRng::seed_from_u64(5);
        let mut words = [123, 456];
        biased_randomize_bits(Probability::try_new(0.0).unwrap(), &mut words, &mut rng);
        assert_eq!(words, [0, 0]);

        biased_randomize_bits(Probability::try_new(1.0).unwrap(), &mut words, &mut rng);
        assert_eq!(words, [u64::MAX, u64::MAX]);
    }

    #[test]
    fn biased_randomize_bits_is_seed_deterministic_and_probability_sensitive() {
        let mut low_rng = SmallRng::seed_from_u64(11);
        let mut high_rng = SmallRng::seed_from_u64(11);
        let mut low = [0u64; 128];
        let mut high = [0u64; 128];

        biased_randomize_bits(Probability::try_new(0.01).unwrap(), &mut low, &mut low_rng);
        biased_randomize_bits(
            Probability::try_new(0.99).unwrap(),
            &mut high,
            &mut high_rng,
        );

        let low_popcount = low.iter().map(|word| word.count_ones()).sum::<u32>();
        let high_popcount = high.iter().map(|word| word.count_ones()).sum::<u32>();
        assert!(low_popcount < 256, "low_popcount={low_popcount}");
        assert!(high_popcount > 7936, "high_popcount={high_popcount}");

        let mut repeated_rng = SmallRng::seed_from_u64(11);
        let mut repeated = [0u64; 128];
        biased_randomize_bits(
            Probability::try_new(0.01).unwrap(),
            &mut repeated,
            &mut repeated_rng,
        );
        assert_eq!(low, repeated);
    }
}
