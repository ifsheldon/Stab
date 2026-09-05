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
    use super::gate_contract_statistical_count_is_accepted;

    #[test]
    fn statistical_counts_preserve_floating_acceptance_boundaries() {
        for (count, accepted) in [
            (24_000, false),
            (24_001, true),
            (25_999, true),
            (26_000, false),
        ] {
            assert_eq!(
                gate_contract_statistical_count_is_accepted(count, 100_000, 0.25, 0.01),
                accepted,
                "count={count}"
            );
        }
    }
}
