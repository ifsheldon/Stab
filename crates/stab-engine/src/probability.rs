use rand::Rng;
use stab_model::Probability;

use crate::bernoulli::fill_words;

pub fn biased_randomize_bits<R>(probability: Probability, words: &mut [u64], rng: &mut R)
where
    R: Rng,
{
    fill_words(probability.get(), words, rng);
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "probability utility tests use valid literal probabilities"
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
        let mut low = [0_u64; 128];
        let mut high = [0_u64; 128];

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
        let mut repeated = [0_u64; 128];
        biased_randomize_bits(
            Probability::try_new(0.01).unwrap(),
            &mut repeated,
            &mut repeated_rng,
        );
        assert_eq!(low, repeated);
    }
}
