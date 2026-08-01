#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "exact-ML unit tests use bounded generated tables"
)]

use super::{
    ExactDyadicDistribution, ExactProbability, Mechanism, exact_compare, exact_shift_left,
    interval_joint_distribution,
};

const PROBABILITIES: [f64; 4] = [0.0, 0.25, 0.5, 1.0];

#[test]
fn directed_intervals_and_exact_dyadics_match_exhaustive_subset_enumeration() {
    for first_effect in 0..8 {
        for first_probability in PROBABILITIES {
            for second_effect in 0..8 {
                for second_probability in PROBABILITIES {
                    let mechanisms = [
                        Mechanism {
                            probability: first_probability,
                            effect: first_effect,
                        },
                        Mechanism {
                            probability: second_probability,
                            effect: second_effect,
                        },
                    ];
                    let intervals = interval_joint_distribution(8, &mechanisms).expect("intervals");
                    let exact =
                        ExactDyadicDistribution::try_compute(8, &mechanisms).expect("exact");
                    let denominator_exponent = mechanisms
                        .iter()
                        .filter(|mechanism| mechanism.effect != 0 && mechanism.probability != 0.0)
                        .map(|mechanism| {
                            ExactProbability::from_f64(mechanism.probability)
                                .expect("exact probability")
                                .denominator_exponent
                        })
                        .sum::<usize>();
                    let expected = direct_distribution(8, &mechanisms);
                    let expected_numerators =
                        direct_numerator_distribution(8, &mechanisms, denominator_exponent);
                    for state in 0..8 {
                        let expected_probability = expected[state];
                        assert!(
                            intervals[state].lower <= expected_probability
                                && expected_probability <= intervals[state].upper,
                            "effects=({first_effect},{second_effect}) probabilities=({first_probability},{second_probability}) state={state}: interval={:?} expected={expected_probability}",
                            intervals[state]
                        );
                        let actual_numerator = exact
                            .state(state)
                            .expect("exact state")
                            .iter()
                            .copied()
                            .enumerate()
                            .fold(0_u128, |value, (index, limb)| {
                                value | ((limb as u128) << (index * 64))
                            });
                        assert_eq!(actual_numerator, expected_numerators[state]);
                    }
                }
            }
        }
    }
}

#[test]
fn directed_intervals_enclose_exact_probability_boundaries() {
    let probabilities = [
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::from_bits(0.5_f64.to_bits() - 1),
        f64::from_bits(0.5_f64.to_bits() + 1),
        f64::from_bits(1.0_f64.to_bits() - 1),
    ];
    for probability in probabilities {
        let mechanisms = [Mechanism {
            probability,
            effect: 1,
        }];
        let intervals = interval_joint_distribution(2, &mechanisms).expect("intervals");
        let exact = ExactDyadicDistribution::try_compute(2, &mechanisms).expect("exact");
        let exponent = ExactProbability::from_f64(probability)
            .expect("exact probability")
            .denominator_exponent;
        for (state, interval) in intervals.iter().copied().enumerate() {
            let exact_state = exact.state(state).expect("exact state");
            assert!(
                compare_exact_state_to_f64(exact_state, exponent, interval.lower).is_ge(),
                "p={probability:?} state={state} lower={:?}",
                interval
            );
            assert!(
                compare_exact_state_to_f64(exact_state, exponent, interval.upper).is_le(),
                "p={probability:?} state={state} upper={:?}",
                interval
            );
        }
    }
}

fn compare_exact_state_to_f64(
    state: &[u64],
    state_exponent: usize,
    value: f64,
) -> std::cmp::Ordering {
    let value = ExactProbability::from_f64(value).expect("exact f64 bound");
    let common_exponent = state_exponent.max(value.denominator_exponent);
    let limbs = (common_exponent + 65) / 64;
    let mut scaled_state = vec![0_u64; limbs];
    scaled_state[..state.len()].copy_from_slice(state);
    exact_shift_left(&mut scaled_state, common_exponent - state_exponent)
        .expect("scale exact state");
    let mut scaled_value = vec![0_u64; limbs];
    scaled_value[0] = value.numerator;
    exact_shift_left(
        &mut scaled_value,
        common_exponent - value.denominator_exponent,
    )
    .expect("scale exact f64 bound");
    exact_compare(&scaled_state, &scaled_value)
}

fn direct_distribution(state_count: usize, mechanisms: &[Mechanism]) -> Vec<f64> {
    let mut result = vec![0.0; state_count];
    let subset_count = 1_usize << mechanisms.len();
    for subset in 0..subset_count {
        let mut state = 0_usize;
        let mut probability = 1.0;
        for (index, mechanism) in mechanisms.iter().enumerate() {
            let occurs = subset & (1_usize << index) != 0;
            if occurs {
                state ^= mechanism.effect;
                probability *= mechanism.probability;
            } else {
                probability *= 1.0 - mechanism.probability;
            }
        }
        result[state] += probability;
    }
    result
}

fn direct_numerator_distribution(
    state_count: usize,
    mechanisms: &[Mechanism],
    denominator_exponent: usize,
) -> Vec<u128> {
    let active = mechanisms
        .iter()
        .filter(|mechanism| mechanism.effect != 0 && mechanism.probability != 0.0)
        .copied()
        .collect::<Vec<_>>();
    let mut result = vec![0_u128; state_count];
    for subset in 0..(1_usize << active.len()) {
        let mut state = 0_usize;
        let mut numerator = 1_u128;
        for (index, mechanism) in active.iter().enumerate() {
            let probability =
                ExactProbability::from_f64(mechanism.probability).expect("exact probability");
            let denominator = 1_u128 << probability.denominator_exponent;
            if subset & (1_usize << index) != 0 {
                state ^= mechanism.effect;
                numerator *= u128::from(probability.numerator);
            } else {
                numerator *= denominator - u128::from(probability.numerator);
            }
        }
        result[state] += numerator;
    }
    let active_exponent = active
        .iter()
        .map(|mechanism| {
            ExactProbability::from_f64(mechanism.probability)
                .expect("exact probability")
                .denominator_exponent
        })
        .sum::<usize>();
    assert_eq!(active_exponent, denominator_exponent);
    result
}
