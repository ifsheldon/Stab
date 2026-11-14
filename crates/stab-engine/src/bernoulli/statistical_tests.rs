//! Statistical contract for the Bernoulli kernel.
//!
//! Every assertion is a two-sided binomial bound at [`SIGMA`] standard deviations with no
//! absolute floor, so sparse probabilities are held to the same relative power as dense ones.
//! Each probability performs 1 total-count, 64 bit-position, and 1 adjacent-pair comparison on
//! each of the two random sources: 1320 comparisons in all. At 5 sigma the two-sided normal tail
//! is about 5.7e-7, so re-seeding trips a false failure with probability below 1e-3; the seeds
//! below are fixed, so a failure is a deterministic regression.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "statistical kernel tests index fixed-size bit-position counters"
)]

use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use super::sample_words;
use super::{INDEXED_BLOCK_SHOTS, INDEXED_BLOCK_WORDS, RandSource, sample_indexed_range_into};

/// 4,194,304 Bernoulli trials per probability and source.
const WORDS: usize = 1 << 16;
const SIGMA: f64 = 5.0;
/// Spans both kernel boundaries (`p < 0.02` sparse, `p == 0.5`, `p > 0.5` complement) and the
/// numerically extreme tails.
const PROBABILITIES: [f64; 10] = [
    1e-12,
    1e-4,
    0.0199,
    0.02,
    0.1,
    0.37,
    0.5,
    0.63,
    0.9,
    1.0 - 1e-9,
];
/// Production callers request one 256-lane block and longer fills; cycling request sizes,
/// including single words, puts every per-call restart of the sparse gap walk under test.
const RAND_SOURCE_CHUNK_WORDS: [usize; 4] = [1, 4, 7, 64];

#[derive(Debug)]
struct BitStatistics {
    ones: u64,
    position_ones: [u64; 64],
    adjacent_pairs: u64,
    bits: u64,
    words: u64,
}

impl BitStatistics {
    fn collect(words: &[u64]) -> Self {
        let mut statistics = Self {
            ones: 0,
            position_ones: [0; 64],
            adjacent_pairs: 0,
            bits: words.len() as u64 * u64::from(u64::BITS),
            words: words.len() as u64,
        };
        let mut previous_high_bit = 0_u64;
        for &word in words {
            let ones = word.count_ones();
            statistics.ones += u64::from(ones);
            statistics.adjacent_pairs +=
                u64::from((word & (word >> 1)).count_ones()) + (previous_high_bit & word & 1);
            previous_high_bit = word >> 63;
            // Visit the minority bit value so dense and sparse words cost the same.
            if ones <= 32 {
                let mut remaining = word;
                while remaining != 0 {
                    statistics.position_ones[remaining.trailing_zeros() as usize] += 1;
                    remaining &= remaining - 1;
                }
            } else {
                for count in &mut statistics.position_ones {
                    *count += 1;
                }
                let mut remaining = !word;
                while remaining != 0 {
                    statistics.position_ones[remaining.trailing_zeros() as usize] -= 1;
                    remaining &= remaining - 1;
                }
            }
        }
        statistics
    }

    fn assert_matches(&self, source: &str, probability: f64) {
        let label = format!("{source} p={probability:e}");
        assert_binomial(
            &format!("{label} total ones"),
            self.ones,
            self.bits,
            probability,
            probability * (1.0 - probability),
        );
        for (position, &ones) in self.position_ones.iter().enumerate() {
            assert_binomial(
                &format!("{label} bit {position}"),
                ones,
                self.words,
                probability,
                probability * (1.0 - probability),
            );
        }
        // Overlapping adjacent pairs share one bit, so each pair's indicator covaries with its two
        // neighbours: Var = n[p^2(1-p^2) + 2(p^3 - p^4)].
        let squared = probability * probability;
        assert_binomial(
            &format!("{label} adjacent pairs"),
            self.adjacent_pairs,
            self.bits - 1,
            squared,
            squared * (1.0 - squared) + 2.0 * (squared * probability - squared * squared),
        );
    }
}

fn assert_binomial(label: &str, successes: u64, trials: u64, probability: f64, variance: f64) {
    let expected = trials as f64 * probability;
    let tolerance = SIGMA * (trials as f64 * variance).sqrt();
    let observed = successes as f64;
    assert!(
        (observed - expected).abs() <= tolerance,
        "{label}: observed {successes} of {trials}, expected {expected:.3} +/- {tolerance:.3}"
    );
}

fn rand_source_words(probability: f64, seed: u64) -> Vec<u64> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut words = vec![0_u64; WORDS];
    let mut start = 0;
    for chunk in RAND_SOURCE_CHUNK_WORDS.iter().cycle() {
        if start == WORDS {
            break;
        }
        let end = (start + chunk).min(WORDS);
        sample_words(
            probability,
            &mut words[start..end],
            &mut RandSource(&mut rng),
        );
        start = end;
    }
    words
}

fn indexed_words(probability: f64, seed: u64) -> Vec<u64> {
    let mut words = vec![0_u64; WORDS];
    for (block, output) in words
        .as_chunks_mut::<INDEXED_BLOCK_WORDS>()
        .0
        .iter_mut()
        .enumerate()
    {
        sample_indexed_range_into(
            probability,
            seed,
            3,
            block as u64 * INDEXED_BLOCK_SHOTS as u64,
            INDEXED_BLOCK_SHOTS,
            output,
        )
        .expect("aligned indexed block");
    }
    words
}

#[test]
fn rand_source_bits_are_binomial_position_uniform_and_pairwise_independent() {
    for (index, probability) in PROBABILITIES.into_iter().enumerate() {
        let words = rand_source_words(probability, 0x5EED_B001 + index as u64);
        BitStatistics::collect(&words).assert_matches("RandSource", probability);
    }
}

#[test]
fn indexed_splitmix_bits_are_binomial_position_uniform_and_pairwise_independent() {
    for (index, probability) in PROBABILITIES.into_iter().enumerate() {
        let words = indexed_words(probability, 0x5EED_B101 + index as u64);
        BitStatistics::collect(&words).assert_matches("indexed SplitMix", probability);
    }
}
