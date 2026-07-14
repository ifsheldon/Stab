#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    not(feature = "ops-contracts"),
    allow(
        unreachable_pub,
        reason = "the type is exported only by the ops-contracts feature"
    )
)]
pub struct GateContractStatisticalBucket {
    pub name: &'static str,
    pub expected_probability: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(
    not(feature = "ops-contracts"),
    allow(
        unreachable_pub,
        reason = "the type is exported only by the ops-contracts feature"
    )
)]
pub struct GateContractStatisticalPlan {
    pub case_id: &'static str,
    pub shots: u64,
    pub seed: u64,
    pub sigma_multiplier: f64,
    pub absolute_probability_floor: f64,
    pub familywise_false_positive_budget: f64,
    pub independent_comparisons_per_attempt: u32,
    pub shot_batches_per_attempt: u32,
    pub buckets: &'static [GateContractStatisticalBucket],
}

const STATISTICAL_SHOTS: u64 = 100_000;
const STATISTICAL_SIGMA: f64 = 6.0;
const STATISTICAL_ABSOLUTE_FLOOR: f64 = 0.01;
const STATISTICAL_FAMILYWISE_BUDGET: f64 = 0.000_001;

const MPP_BUCKETS: &[GateContractStatisticalBucket] =
    &[bucket("mpp-zero", 0.75), bucket("mpp-one", 0.25)];
const MPAD_BUCKETS: &[GateContractStatisticalBucket] =
    &[bucket("mpad-zero", 0.75), bucket("mpad-one", 0.25)];
const PAULI_NOISE_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("identity", 0.25),
    bucket("x", 0.25),
    bucket("y", 0.25),
    bucket("z", 0.25),
];
const PAULI_CHANNEL_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("pc1-i", 0.4),
    bucket("pc1-x", 0.1),
    bucket("pc1-y", 0.2),
    bucket("pc1-z", 0.3),
    bucket("pc2-ii", 0.4),
    bucket("pc2-ix", 0.04),
    bucket("pc2-iy", 0.04),
    bucket("pc2-iz", 0.04),
    bucket("pc2-xi", 0.04),
    bucket("pc2-xx", 0.04),
    bucket("pc2-xy", 0.04),
    bucket("pc2-xz", 0.04),
    bucket("pc2-yi", 0.04),
    bucket("pc2-yx", 0.04),
    bucket("pc2-yy", 0.04),
    bucket("pc2-yz", 0.04),
    bucket("pc2-zi", 0.04),
    bucket("pc2-zx", 0.04),
    bucket("pc2-zy", 0.04),
    bucket("pc2-zz", 0.04),
];
const DEPOLARIZATION_BUCKETS: &[GateContractStatisticalBucket] = &[
    bucket("depol1-i", 0.4),
    bucket("depol1-x", 0.2),
    bucket("depol1-y", 0.2),
    bucket("depol1-z", 0.2),
    bucket("depol2-ii", 0.25),
    bucket("depol2-nonidentity", 0.75),
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
        absolute_probability_floor: STATISTICAL_ABSOLUTE_FLOOR,
        familywise_false_positive_budget: STATISTICAL_FAMILYWISE_BUDGET,
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

#[cfg(feature = "ops-contracts")]
pub(crate) fn gate_contract_statistical_plans() -> &'static [GateContractStatisticalPlan] {
    GATE_CONTRACT_STATISTICAL_PLANS
}

#[cfg(any(test, feature = "ops-contracts"))]
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

#[cfg(any(test, feature = "ops-contracts"))]
pub(crate) fn gate_contract_statistical_rejection_boundaries(
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
) -> (Option<u64>, Option<u64>) {
    if shots == 0
        || !expected_probability.is_finite()
        || !allowed_delta.is_finite()
        || allowed_delta < 0.0
    {
        return (None, None);
    }
    let rejected = |count| {
        !gate_contract_statistical_count_is_accepted(
            count,
            shots,
            expected_probability,
            allowed_delta,
        )
    };

    let lower_max = rejected(0).then(|| {
        let mut low = 0;
        let mut high = shots;
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            if rejected(middle) && middle as f64 / shots as f64 <= expected_probability {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        low
    });
    let upper_min = rejected(shots).then(|| {
        let mut low = 0;
        let mut high = shots;
        while low < high {
            let middle = low + (high - low) / 2;
            if rejected(middle) && middle as f64 / shots as f64 >= expected_probability {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    });
    (lower_max, upper_min)
}

#[cfg(test)]
mod tests {
    use super::{
        gate_contract_statistical_count_is_accepted, gate_contract_statistical_rejection_boundaries,
    };

    #[test]
    fn rejection_boundaries_match_the_executable_floating_predicate() {
        let shots = 100_000;
        let expected_probability = 0.25;
        let allowed_delta = 0.01;

        assert!(!gate_contract_statistical_count_is_accepted(
            24_000,
            shots,
            expected_probability,
            allowed_delta
        ));
        assert_eq!(
            gate_contract_statistical_rejection_boundaries(
                shots,
                expected_probability,
                allowed_delta
            ),
            (Some(24_000), Some(26_000))
        );
    }
}
