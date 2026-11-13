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
}
