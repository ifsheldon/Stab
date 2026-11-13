#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GateContractStatisticalBucket {
    pub name: &'static str,
    pub expected_probability: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GateContractStatisticalPlan {
    pub case_id: &'static str,
    pub shots: u64,
    pub seed: u64,
    pub sigma_multiplier: f64,
    pub independent_comparisons_per_attempt: u32,
    pub shot_batches_per_attempt: u32,
    pub buckets: &'static [GateContractStatisticalBucket],
}

/// Every bucket is a two-sided binomial bound at `STATISTICAL_SIGMA` standard deviations with no
/// absolute floor.
///
/// False-positive budget: one full run makes about 220 bucket comparisons. The 6-sigma two-sided
/// normal tail is about 2e-9, so re-seeding every plan trips a false failure with probability
/// below 1e-6; the seeds are fixed, so any failure is deterministic. Power: 150,000 shots make
/// 6 sigma narrower than a 10% relative bias for every bucket at or above p=0.025, the smallest
/// declared bucket, which `a_ten_percent_relative_bias_is_rejected_in_every_bucket` enforces.
const STATISTICAL_SHOTS: u64 = 150_000;
const STATISTICAL_SIGMA: f64 = 6.0;

const MPP_BUCKETS: &[GateContractStatisticalBucket] =
    &[bucket("mpp-zero", 0.75), bucket("mpp-one", 0.25)];
const MPAD_BUCKETS: &[GateContractStatisticalBucket] =
    &[bucket("mpad-zero", 0.75), bucket("mpad-one", 0.25)];
/// Two products whose noiseless results are `0, 1`, each flipped independently with p=0.25.
const INDEPENDENT_PAIR_FLIP_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("pair-00", 0.1875),
    bucket("pair-01", 0.5625),
    bucket("pair-10", 0.0625),
    bucket("pair-11", 0.1875),
];
const PAULI_NOISE_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("identity", 0.25),
    bucket("x", 0.25),
    bucket("y", 0.25),
    bucket("z", 0.25),
];
/// `PAULI_CHANNEL_2` arguments in Stim order (IX, IY, ..., ZZ). Every entry is distinct and
/// each neighbour pair differs by at least 11% relative, so any permutation of the channel table
/// moves some bucket by more than the 10% relative bias the plan is powered to reject.
pub(crate) const PAULI_CHANNEL_2_PROBABILITIES: [f64; 15] = [
    0.025, 0.028, 0.032, 0.036, 0.04, 0.045, 0.05, 0.056, 0.063, 0.07, 0.078, 0.087, 0.097, 0.108,
    0.12,
];
const PAULI_CHANNEL_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("pc1-i", 0.4),
    bucket("pc1-x", 0.1),
    bucket("pc1-y", 0.2),
    bucket("pc1-z", 0.3),
    bucket("pc2-ii", 0.065),
    bucket("pc2-ix", PAULI_CHANNEL_2_PROBABILITIES[0]),
    bucket("pc2-iy", PAULI_CHANNEL_2_PROBABILITIES[1]),
    bucket("pc2-iz", PAULI_CHANNEL_2_PROBABILITIES[2]),
    bucket("pc2-xi", PAULI_CHANNEL_2_PROBABILITIES[3]),
    bucket("pc2-xx", PAULI_CHANNEL_2_PROBABILITIES[4]),
    bucket("pc2-xy", PAULI_CHANNEL_2_PROBABILITIES[5]),
    bucket("pc2-xz", PAULI_CHANNEL_2_PROBABILITIES[6]),
    bucket("pc2-yi", PAULI_CHANNEL_2_PROBABILITIES[7]),
    bucket("pc2-yx", PAULI_CHANNEL_2_PROBABILITIES[8]),
    bucket("pc2-yy", PAULI_CHANNEL_2_PROBABILITIES[9]),
    bucket("pc2-yz", PAULI_CHANNEL_2_PROBABILITIES[10]),
    bucket("pc2-zi", PAULI_CHANNEL_2_PROBABILITIES[11]),
    bucket("pc2-zx", PAULI_CHANNEL_2_PROBABILITIES[12]),
    bucket("pc2-zy", PAULI_CHANNEL_2_PROBABILITIES[13]),
    bucket("pc2-zz", PAULI_CHANNEL_2_PROBABILITIES[14]),
];
const DEPOLARIZATION_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("depol1-i", 0.4),
    bucket("depol1-x", 0.2),
    bucket("depol1-y", 0.2),
    bucket("depol1-z", 0.2),
    bucket("depol2-ii", 0.25),
    bucket("depol2-ix", 0.05),
    bucket("depol2-iy", 0.05),
    bucket("depol2-iz", 0.05),
    bucket("depol2-xi", 0.05),
    bucket("depol2-xx", 0.05),
    bucket("depol2-xy", 0.05),
    bucket("depol2-xz", 0.05),
    bucket("depol2-yi", 0.05),
    bucket("depol2-yx", 0.05),
    bucket("depol2-yy", 0.05),
    bucket("depol2-yz", 0.05),
    bucket("depol2-zi", 0.05),
    bucket("depol2-zx", 0.05),
    bucket("depol2-zy", 0.05),
    bucket("depol2-zz", 0.05),
];
const CORRELATED_ERROR_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("no-error", 0.3),
    bucket("first-branch", 0.2),
    bucket("else-branch-one", 0.2),
    bucket("else-branch-two", 0.3),
];
const MEASURE_RESET_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("measurement-zero", 0.95),
    bucket("measurement-one", 0.05),
];
const HERALDED_ERASE_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("erase-no-herald", 0.9),
    bucket("erase-i", 0.025),
    bucket("erase-x", 0.025),
    bucket("erase-y", 0.025),
    bucket("erase-z", 0.025),
];
const HERALDED_CHANNEL_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("no-herald", 0.45),
    bucket("herald-no-data-error", 0.05),
    bucket("herald-x", 0.1),
    bucket("herald-y", 0.15),
    bucket("herald-z", 0.25),
];

const GATE_CONTRACT_STATISTICAL_PLANS: &[GateContractStatisticalPlan] = &[
    statistical_plan(
        "pfm3-contract-mpp-stochastic",
        12_648_431,
        MPP_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-mpad-stochastic",
        12_648_432,
        MPAD_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-mpp-independent-products",
        12_648_443,
        INDEPENDENT_PAIR_FLIP_BUCKETS,
        1,
        1,
    ),
    statistical_plan(
        "pfm3-contract-mpad-independent-targets",
        12_648_444,
        INDEPENDENT_PAIR_FLIP_BUCKETS,
        1,
        1,
    ),
    statistical_plan(
        "pfm3-contract-pauli-noise",
        12_648_432,
        PAULI_NOISE_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-pauli-channels",
        12_648_433,
        PAULI_CHANNEL_BUCKETS,
        3,
        6,
    ),
    statistical_plan(
        "pfm3-contract-depolarization",
        12_648_434,
        DEPOLARIZATION_BUCKETS,
        3,
        6,
    ),
    statistical_plan(
        "pfm3-contract-correlated-errors",
        12_648_435,
        CORRELATED_ERROR_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-heralded-noise",
        12_648_436,
        HERALDED_ERASE_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-heralded-channel",
        12_648_437,
        HERALDED_CHANNEL_BUCKETS,
        3,
        3,
    ),
    statistical_plan(
        "pfm3-contract-heralded-erase-offset",
        12_648_438,
        HERALDED_ERASE_BUCKETS,
        1,
        1,
    ),
    statistical_plan(
        "pfm3-contract-heralded-channel-offset",
        12_648_439,
        HERALDED_CHANNEL_BUCKETS,
        1,
        1,
    ),
    statistical_plan(
        "pfm3-contract-measure-reset-x",
        12_648_440,
        MEASURE_RESET_BUCKETS,
        2,
        2,
    ),
    statistical_plan(
        "pfm3-contract-measure-reset-y",
        12_648_441,
        MEASURE_RESET_BUCKETS,
        2,
        2,
    ),
    statistical_plan(
        "pfm3-contract-measure-reset-z",
        12_648_442,
        MEASURE_RESET_BUCKETS,
        2,
        2,
    ),
];

const fn bucket(name: &'static str, expected_probability: f64) -> GateContractStatisticalBucket {
    GateContractStatisticalBucket {
        name,
        expected_probability,
    }
}

const fn statistical_plan(
    case_id: &'static str,
    seed: u64,
    buckets: &'static [GateContractStatisticalBucket],
    independent_comparisons_per_attempt: u32,
    shot_batches_per_attempt: u32,
) -> GateContractStatisticalPlan {
    GateContractStatisticalPlan {
        case_id,
        shots: STATISTICAL_SHOTS,
        seed,
        sigma_multiplier: STATISTICAL_SIGMA,
        independent_comparisons_per_attempt,
        shot_batches_per_attempt,
        buckets,
    }
}

#[cfg(test)]
pub(crate) fn gate_contract_statistical_plan(
    case_id: &str,
) -> Option<&'static GateContractStatisticalPlan> {
    GATE_CONTRACT_STATISTICAL_PLANS
        .iter()
        .find(|plan| plan.case_id == case_id)
}

/// Two-sided half-width, in probability units, accepted for one bucket of `plan`.
pub(crate) fn gate_contract_statistical_tolerance(
    plan: &GateContractStatisticalPlan,
    expected_probability: f64,
) -> f64 {
    plan.sigma_multiplier
        * (expected_probability * (1.0 - expected_probability) / plan.shots as f64).sqrt()
}

pub(crate) fn gate_contract_statistical_count_is_accepted(
    count: u64,
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
) -> bool {
    if shots == 0 || count > shots {
        return false;
    }
    let observed = count as f64 / shots as f64;
    (observed - expected_probability).abs() <= allowed_delta
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bound must have the power to reject a 10% relative bias in every declared bucket at
    /// the plan's own shot count, in both directions, while accepting the exact expectation. The
    /// `PAULI_CHANNEL_2` table is spaced wider than that bias, so any permutation of it fails.
    #[test]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded expected counts are nonnegative and bounded by the plan's shot count"
    )]
    fn sigma_bounds_reject_a_ten_percent_relative_bias_in_every_bucket() {
        for plan in GATE_CONTRACT_STATISTICAL_PLANS {
            for bucket in plan.buckets {
                let tolerance =
                    gate_contract_statistical_tolerance(plan, bucket.expected_probability);
                let expected = plan.shots as f64 * bucket.expected_probability;
                assert!(
                    gate_contract_statistical_count_is_accepted(
                        expected.round() as u64,
                        plan.shots,
                        bucket.expected_probability,
                        tolerance,
                    ),
                    "{} bucket {} rejects its own expectation",
                    plan.case_id,
                    bucket.name
                );
                for biased in [expected * 1.1, expected * 0.9] {
                    assert!(
                        !gate_contract_statistical_count_is_accepted(
                            biased.round() as u64,
                            plan.shots,
                            bucket.expected_probability,
                            tolerance,
                        ),
                        "{} bucket {} (p={}) accepts a 10% relative bias at {} shots: \
                         tolerance {tolerance:.6}",
                        plan.case_id,
                        bucket.name,
                        bucket.expected_probability,
                        plan.shots
                    );
                }
            }
        }
        for pair in PAULI_CHANNEL_2_PROBABILITIES.windows(2) {
            let &[smaller, larger] = pair else {
                panic!("windows(2) yields pairs");
            };
            assert!(
                larger > smaller * 1.1,
                "{smaller} and {larger} are too close for a permuted table to fail"
            );
        }
    }
}
