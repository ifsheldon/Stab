use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::num::{NonZeroU32, NonZeroU64};

use statrs::distribution::{Binomial, DiscreteCDF as _};
use thiserror::Error;

use crate::RepoRoot;
use crate::blocker_ledger::BlockerStatisticalPlanSummary;
use crate::fixtures::FixtureStatisticalPlanSummary;
use crate::statistical_contract::AcceptedCountRange;

pub(crate) const MAX_CASE_FAMILYWISE_BOUND: f64 = 1e-6;
pub(crate) const MAX_SELECTED_SUITE_FAMILYWISE_BOUND: f64 = 1e-4;
pub(crate) const MAX_SOAK_ADDITIONAL_SEEDS: usize = 64;

const MAX_PLAN_ID_BYTES: usize = 128;
const MAX_BUCKET_NAME_BYTES: usize = 1_024;
const MAX_BUCKETS_PER_PLAN: usize = 128;
const SOAK_SEED_INCREMENT: u64 = 0x9e37_79b9_7f4a_7c15;
const SOAK_SEED_MIX_1: u64 = 0xbf58_476d_1ce4_e5b9;
const SOAK_SEED_MIX_2: u64 = 0x94d0_49bb_1331_11eb;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StatisticalPlanOrigin {
    OracleFixture,
    BlockerLedger,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StatisticalPlanId(Box<str>);

impl StatisticalPlanId {
    fn try_new(value: String) -> Result<Self, StatisticalPlanError> {
        if value.is_empty()
            || value.len() > MAX_PLAN_ID_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(StatisticalPlanError::InvalidPlanId(value));
        }
        Ok(Self(value.into_boxed_str()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StatisticalPlanId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct StatisticalSeed(u64);

impl StatisticalSeed {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for StatisticalSeed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:016x}", self.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct Probability(f64);

impl Probability {
    fn try_new(
        plan_id: &StatisticalPlanId,
        bucket: &str,
        value: f64,
    ) -> Result<Self, StatisticalPlanError> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(StatisticalPlanError::InvalidProbability {
                plan_id: plan_id.clone(),
                bucket: bucket.to_string(),
                value,
            });
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub(crate) struct FamilywiseBound(f64);

impl FamilywiseBound {
    fn try_new(plan_id: &StatisticalPlanId, value: f64) -> Result<Self, StatisticalPlanError> {
        if !value.is_finite() || value <= 0.0 || value > 1.0 {
            return Err(StatisticalPlanError::InvalidDeclaredFamilywiseBound {
                plan_id: plan_id.clone(),
                value,
            });
        }
        Ok(Self(value))
    }

    pub(crate) const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatisticalBucketSummary {
    name: Box<str>,
    expected_probability: Probability,
    accepted_counts: AcceptedCountRange,
    exact_rejection_probability: f64,
}

impl StatisticalBucketSummary {
    #[cfg(test)]
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    #[cfg(test)]
    pub(crate) const fn expected_probability(&self) -> Probability {
        self.expected_probability
    }

    #[cfg(test)]
    pub(crate) const fn accepted_counts(&self) -> AcceptedCountRange {
        self.accepted_counts
    }

    #[cfg(test)]
    pub(crate) const fn exact_rejection_probability(&self) -> f64 {
        self.exact_rejection_probability
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StatisticalPlanSummary {
    id: StatisticalPlanId,
    origin: StatisticalPlanOrigin,
    shots: NonZeroU64,
    primary_seed: StatisticalSeed,
    buckets: Box<[StatisticalBucketSummary]>,
    declared_familywise_bound: FamilywiseBound,
    exact_familywise_bound: f64,
    shots_per_attempt: NonZeroU64,
    independent_comparisons_per_attempt: NonZeroU32,
    shot_batches_per_attempt: NonZeroU32,
    shot_batches_per_comparison: NonZeroU32,
    seed_override_executable: bool,
}

impl StatisticalPlanSummary {
    pub(crate) fn id(&self) -> &StatisticalPlanId {
        &self.id
    }

    #[cfg(test)]
    pub(crate) const fn origin(&self) -> StatisticalPlanOrigin {
        self.origin
    }

    pub(crate) const fn shots(&self) -> NonZeroU64 {
        self.shots
    }

    pub(crate) const fn shots_per_attempt(&self) -> NonZeroU64 {
        self.shots_per_attempt
    }

    pub(crate) const fn primary_seed(&self) -> StatisticalSeed {
        self.primary_seed
    }

    #[cfg(test)]
    pub(crate) fn buckets(&self) -> &[StatisticalBucketSummary] {
        &self.buckets
    }

    pub(crate) const fn declared_familywise_bound(&self) -> FamilywiseBound {
        self.declared_familywise_bound
    }

    pub(crate) const fn exact_bound_per_attempt(&self) -> f64 {
        self.exact_familywise_bound
    }

    pub(crate) const fn independent_comparisons_per_attempt(&self) -> NonZeroU32 {
        self.independent_comparisons_per_attempt
    }

    pub(crate) const fn shot_batches_per_attempt(&self) -> NonZeroU32 {
        self.shot_batches_per_attempt
    }

    pub(crate) const fn shot_batches_per_comparison(&self) -> NonZeroU32 {
        self.shot_batches_per_comparison
    }

    pub(crate) const fn seed_override_executable(&self) -> bool {
        self.seed_override_executable
    }

    fn try_from_raw(raw: RawStatisticalPlan) -> Result<Self, StatisticalPlanError> {
        let id = StatisticalPlanId::try_new(raw.id)?;
        let Some(shots) = NonZeroU64::new(raw.shots) else {
            return Err(StatisticalPlanError::ZeroShots {
                plan_id: id.clone(),
            });
        };
        let declared_familywise_bound =
            FamilywiseBound::try_new(&id, raw.declared_familywise_bound)?;
        let Some(independent_comparisons_per_attempt) =
            NonZeroU32::new(raw.independent_comparisons_per_attempt)
        else {
            return Err(StatisticalPlanError::ZeroIndependentComparisons {
                plan_id: id.clone(),
            });
        };
        let Some(shot_batches_per_attempt) = NonZeroU32::new(raw.shot_batches_per_attempt) else {
            return Err(StatisticalPlanError::ZeroShotBatches {
                plan_id: id.clone(),
            });
        };
        if !shot_batches_per_attempt
            .get()
            .is_multiple_of(independent_comparisons_per_attempt.get())
        {
            return Err(StatisticalPlanError::UnevenShotBatches {
                plan_id: id.clone(),
                batches: shot_batches_per_attempt.get(),
                comparisons: independent_comparisons_per_attempt.get(),
            });
        }
        let shot_batches_per_comparison = NonZeroU32::new(
            shot_batches_per_attempt.get() / independent_comparisons_per_attempt.get(),
        )
        .ok_or_else(|| StatisticalPlanError::ZeroShotBatches {
            plan_id: id.clone(),
        })?;
        let shots_per_attempt = shots
            .get()
            .checked_mul(u64::from(shot_batches_per_attempt.get()))
            .and_then(NonZeroU64::new)
            .ok_or_else(|| StatisticalPlanError::ShotCountOverflow {
                plan_id: id.clone(),
            })?;
        if raw.buckets.is_empty() {
            return Err(StatisticalPlanError::MissingBuckets {
                plan_id: id.clone(),
            });
        }
        if raw.buckets.len() > MAX_BUCKETS_PER_PLAN {
            return Err(StatisticalPlanError::TooManyBuckets {
                plan_id: id.clone(),
                actual: raw.buckets.len(),
                maximum: MAX_BUCKETS_PER_PLAN,
            });
        }

        let mut names = BTreeSet::new();
        let mut buckets = Vec::with_capacity(raw.buckets.len());
        let mut exact_familywise_bound = 0.0;
        for bucket in raw.buckets {
            if bucket.name.is_empty()
                || bucket.name.len() > MAX_BUCKET_NAME_BYTES
                || bucket.name.chars().any(char::is_control)
            {
                return Err(StatisticalPlanError::InvalidBucketName {
                    plan_id: id.clone(),
                    bucket: bucket.name,
                });
            }
            if !names.insert(bucket.name.clone()) {
                return Err(StatisticalPlanError::DuplicateBucket {
                    plan_id: id.clone(),
                    bucket: bucket.name,
                });
            }
            let probability = Probability::try_new(&id, &bucket.name, bucket.probability)?;
            if !bucket.allowed_delta.is_finite() || bucket.allowed_delta < 0.0 {
                return Err(StatisticalPlanError::InvalidAllowedDelta {
                    plan_id: id.clone(),
                    bucket: bucket.name,
                    value: bucket.allowed_delta,
                });
            }
            let accepted_counts =
                AcceptedCountRange::try_new(shots.get(), probability.get(), bucket.allowed_delta)
                    .ok_or_else(|| StatisticalPlanError::ImpossibleBoundaries {
                    plan_id: id.clone(),
                    bucket: bucket.name.clone(),
                    shots: shots.get(),
                })?;
            let rejection_probability = exact_rejection_probability(
                &id,
                &bucket.name,
                shots.get(),
                probability,
                accepted_counts,
            )?;
            exact_familywise_bound += rejection_probability;
            buckets.push(StatisticalBucketSummary {
                name: bucket.name.into_boxed_str(),
                expected_probability: probability,
                accepted_counts,
                exact_rejection_probability: rejection_probability,
            });
        }

        exact_familywise_bound *= f64::from(independent_comparisons_per_attempt.get());
        if exact_familywise_bound > declared_familywise_bound.get() {
            return Err(StatisticalPlanError::ExactBoundExceedsDeclared {
                plan_id: id.clone(),
                exact: exact_familywise_bound,
                declared: declared_familywise_bound.get(),
            });
        }

        Ok(Self {
            id,
            origin: raw.origin,
            shots,
            primary_seed: StatisticalSeed::new(raw.primary_seed),
            buckets: buckets.into_boxed_slice(),
            declared_familywise_bound,
            exact_familywise_bound,
            shots_per_attempt,
            independent_comparisons_per_attempt,
            shot_batches_per_attempt,
            shot_batches_per_comparison,
            seed_override_executable: raw.seed_override_executable,
        })
    }
}

impl TryFrom<FixtureStatisticalPlanSummary> for StatisticalPlanSummary {
    type Error = StatisticalPlanError;

    fn try_from(value: FixtureStatisticalPlanSummary) -> Result<Self, Self::Error> {
        Self::try_from_raw(RawStatisticalPlan {
            id: value.id,
            origin: StatisticalPlanOrigin::OracleFixture,
            shots: value.shots,
            primary_seed: value.primary_seed,
            buckets: value
                .buckets
                .into_iter()
                .map(|bucket| RawStatisticalBucket {
                    name: bucket.name,
                    probability: bucket.expected_probability,
                    allowed_delta: bucket.allowed_delta,
                })
                .collect(),
            declared_familywise_bound: value.declared_familywise_bound,
            independent_comparisons_per_attempt: value.independent_comparisons_per_attempt,
            shot_batches_per_attempt: value.shot_batches_per_attempt,
            seed_override_executable: value.seed_override_executable,
        })
    }
}

impl TryFrom<BlockerStatisticalPlanSummary> for StatisticalPlanSummary {
    type Error = StatisticalPlanError;

    fn try_from(value: BlockerStatisticalPlanSummary) -> Result<Self, Self::Error> {
        Self::try_from_raw(RawStatisticalPlan {
            id: value.id,
            origin: StatisticalPlanOrigin::BlockerLedger,
            shots: value.shots,
            primary_seed: value.primary_seed,
            buckets: value
                .buckets
                .into_iter()
                .map(|bucket| RawStatisticalBucket {
                    name: bucket.name,
                    probability: bucket.expected_probability,
                    allowed_delta: bucket.allowed_delta,
                })
                .collect(),
            declared_familywise_bound: value.declared_familywise_bound,
            independent_comparisons_per_attempt: value.independent_comparisons_per_attempt,
            shot_batches_per_attempt: value.shot_batches_per_attempt,
            seed_override_executable: value.seed_override_executable,
        })
    }
}

#[derive(Debug)]
struct RawStatisticalPlan {
    id: String,
    origin: StatisticalPlanOrigin,
    shots: u64,
    primary_seed: u64,
    buckets: Vec<RawStatisticalBucket>,
    declared_familywise_bound: f64,
    independent_comparisons_per_attempt: u32,
    shot_batches_per_attempt: u32,
    seed_override_executable: bool,
}

#[derive(Debug)]
struct RawStatisticalBucket {
    name: String,
    probability: f64,
    allowed_delta: f64,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub(crate) enum StatisticalPlanError {
    #[error("invalid statistical plan id {0:?}")]
    InvalidPlanId(String),

    #[error("statistical plan {plan_id} has zero shots")]
    ZeroShots { plan_id: StatisticalPlanId },

    #[error("statistical plan {plan_id} has zero independent comparisons per attempt")]
    ZeroIndependentComparisons { plan_id: StatisticalPlanId },

    #[error("statistical plan {plan_id} has zero shot batches per attempt")]
    ZeroShotBatches { plan_id: StatisticalPlanId },

    #[error(
        "statistical plan {plan_id} has {batches} shot batches that cannot be divided evenly across {comparisons} independent comparisons"
    )]
    UnevenShotBatches {
        plan_id: StatisticalPlanId,
        batches: u32,
        comparisons: u32,
    },

    #[error("statistical plan {plan_id} total shot count per attempt overflows u64")]
    ShotCountOverflow { plan_id: StatisticalPlanId },

    #[error("statistical plan {plan_id} has no named buckets")]
    MissingBuckets { plan_id: StatisticalPlanId },

    #[error("statistical plan {plan_id} has {actual} buckets, exceeding the limit of {maximum}")]
    TooManyBuckets {
        plan_id: StatisticalPlanId,
        actual: usize,
        maximum: usize,
    },

    #[error("statistical plan {plan_id} has invalid bucket name {bucket:?}")]
    InvalidBucketName {
        plan_id: StatisticalPlanId,
        bucket: String,
    },

    #[error("statistical plan {plan_id} repeats bucket {bucket:?}")]
    DuplicateBucket {
        plan_id: StatisticalPlanId,
        bucket: String,
    },

    #[error("statistical plan {plan_id} bucket {bucket:?} probability {value} is outside [0, 1]")]
    InvalidProbability {
        plan_id: StatisticalPlanId,
        bucket: String,
        value: f64,
    },

    #[error(
        "statistical plan {plan_id} bucket {bucket:?} allowed delta {value} must be finite and nonnegative"
    )]
    InvalidAllowedDelta {
        plan_id: StatisticalPlanId,
        bucket: String,
        value: f64,
    },

    #[error("statistical plan {plan_id} declared familywise bound {value} is outside (0, 1]")]
    InvalidDeclaredFamilywiseBound {
        plan_id: StatisticalPlanId,
        value: f64,
    },

    #[error(
        "statistical plan {plan_id} bucket {bucket:?} has no accepted integer count for {shots} shots"
    )]
    ImpossibleBoundaries {
        plan_id: StatisticalPlanId,
        bucket: String,
        shots: u64,
    },

    #[error("statistical plan {plan_id} bucket {bucket:?} has no valid binomial distribution")]
    InvalidBinomialDistribution {
        plan_id: StatisticalPlanId,
        bucket: String,
    },

    #[error(
        "statistical plan {plan_id} exact familywise bound {exact:.6e} exceeds declared bound {declared:.6e}"
    )]
    ExactBoundExceedsDeclared {
        plan_id: StatisticalPlanId,
        exact: f64,
        declared: f64,
    },
}

fn exact_rejection_probability(
    plan_id: &StatisticalPlanId,
    bucket: &str,
    shots: u64,
    probability: Probability,
    accepted: AcceptedCountRange,
) -> Result<f64, StatisticalPlanError> {
    let distribution = Binomial::new(probability.get(), shots).map_err(|_| {
        StatisticalPlanError::InvalidBinomialDistribution {
            plan_id: plan_id.clone(),
            bucket: bucket.to_string(),
        }
    })?;
    let lower_tail = accepted
        .minimum()
        .checked_sub(1)
        .map(|boundary| distribution.cdf(boundary))
        .unwrap_or(0.0);
    let upper_tail = if accepted.maximum() < shots {
        distribution.sf(accepted.maximum())
    } else {
        0.0
    };
    let rejection_probability = lower_tail + upper_tail;
    if rejection_probability.is_finite() && (0.0..=1.0).contains(&rejection_probability) {
        Ok(rejection_probability)
    } else {
        Err(StatisticalPlanError::InvalidBinomialDistribution {
            plan_id: plan_id.clone(),
            bucket: bucket.to_string(),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StatisticalSuiteBudget {
    plan_count: usize,
    declared_familywise_bound: f64,
    exact_familywise_bound: f64,
}

impl StatisticalSuiteBudget {
    pub(crate) const fn plan_count(self) -> usize {
        self.plan_count
    }

    #[cfg(test)]
    pub(crate) const fn declared_familywise_bound(self) -> f64 {
        self.declared_familywise_bound
    }

    #[cfg(test)]
    pub(crate) const fn exact_familywise_bound(self) -> f64 {
        self.exact_familywise_bound
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
#[error("selected statistical suite is invalid:\n{message}")]
pub(crate) struct StatisticalSuiteError {
    message: Box<str>,
}

pub(crate) fn validate_selected_suite<'a>(
    plans: impl IntoIterator<Item = &'a StatisticalPlanSummary>,
) -> Result<StatisticalSuiteBudget, StatisticalSuiteError> {
    let plans = plans.into_iter().collect::<Vec<_>>();
    let mut ids = BTreeMap::<&str, &StatisticalPlanId>::new();
    let mut issues = Vec::new();
    let mut declared_familywise_bound = 0.0;
    let mut exact_familywise_bound = 0.0;

    for plan in &plans {
        if let Some(previous) = ids.insert(plan.id().as_str(), plan.id()) {
            issues.push(format!(
                "statistical plan id {} is selected more than once (first owner {previous})",
                plan.id()
            ));
        }
        let declared = plan.declared_familywise_bound().get();
        if declared > MAX_CASE_FAMILYWISE_BOUND {
            issues.push(format!(
                "statistical plan {} declares familywise bound {declared:.6e}, exceeding the per-case limit {MAX_CASE_FAMILYWISE_BOUND:.6e}",
                plan.id()
            ));
        }
        let exact = plan.exact_bound_per_attempt();
        if exact > declared {
            issues.push(format!(
                "statistical plan {} has per-attempt exact union bound {exact:.6e}, exceeding its declared bound {declared:.6e}",
                plan.id()
            ));
        }
        declared_familywise_bound += declared;
        exact_familywise_bound += exact;
    }
    if declared_familywise_bound > MAX_SELECTED_SUITE_FAMILYWISE_BOUND {
        issues.push(format!(
            "selected statistical suite declares summed familywise bound {declared_familywise_bound:.6e}, exceeding {MAX_SELECTED_SUITE_FAMILYWISE_BOUND:.6e}"
        ));
    }

    if issues.is_empty() {
        Ok(StatisticalSuiteBudget {
            plan_count: plans.len(),
            declared_familywise_bound,
            exact_familywise_bound,
        })
    } else {
        let message = issues.join("\n").into_boxed_str();
        Err(StatisticalSuiteError { message })
    }
}

#[derive(Debug, Error)]
pub(crate) enum StatisticalCatalogError {
    #[error(transparent)]
    Fixture(#[from] crate::fixtures::FixtureError),

    #[error(transparent)]
    BlockerLedger(#[from] crate::blocker_ledger::BlockerLedgerError),

    #[error(transparent)]
    InvalidPlan(#[from] StatisticalPlanError),
}

pub(crate) fn source_plan_summaries(
    root: &RepoRoot,
) -> Result<Vec<StatisticalPlanSummary>, StatisticalCatalogError> {
    let fixture_plans = crate::fixtures::qualification_statistical_plan_summaries(root)?;
    let blocker_plans = crate::blocker_ledger::qualification_statistical_plan_summaries(root)?;
    let mut summaries = Vec::with_capacity(fixture_plans.len() + blocker_plans.len());
    for plan in fixture_plans {
        summaries.push(StatisticalPlanSummary::try_from(plan)?);
    }
    for plan in blocker_plans {
        summaries.push(StatisticalPlanSummary::try_from(plan)?);
    }
    Ok(summaries)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StatisticalSeedPanel {
    primary_seed: StatisticalSeed,
    additional_seeds: Box<[StatisticalSeed]>,
}

impl StatisticalSeedPanel {
    pub(crate) fn try_new(
        primary_seed: StatisticalSeed,
        additional_seeds: Vec<StatisticalSeed>,
    ) -> Result<Self, StatisticalSeedPanelError> {
        if additional_seeds.len() > MAX_SOAK_ADDITIONAL_SEEDS {
            return Err(StatisticalSeedPanelError::TooManySeeds {
                actual: additional_seeds.len(),
                maximum: MAX_SOAK_ADDITIONAL_SEEDS,
            });
        }
        let mut unique = BTreeSet::new();
        unique.insert(primary_seed);
        for seed in &additional_seeds {
            if !unique.insert(*seed) {
                return Err(StatisticalSeedPanelError::DuplicateSeed(*seed));
            }
        }
        Ok(Self {
            primary_seed,
            additional_seeds: additional_seeds.into_boxed_slice(),
        })
    }

    #[cfg(test)]
    pub(crate) const fn primary_seed(&self) -> StatisticalSeed {
        self.primary_seed
    }

    #[cfg(test)]
    pub(crate) fn additional_seeds(&self) -> &[StatisticalSeed] {
        &self.additional_seeds
    }

    pub(crate) fn seeds(&self) -> impl Iterator<Item = StatisticalSeed> + '_ {
        std::iter::once(self.primary_seed).chain(self.additional_seeds.iter().copied())
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(crate) enum StatisticalSeedPanelError {
    #[error("statistical seed panel has {actual} additional seeds, exceeding {maximum}")]
    TooManySeeds { actual: usize, maximum: usize },

    #[error("statistical seed panel repeats seed {0}")]
    DuplicateSeed(StatisticalSeed),

    #[error("statistical plan declared bound cannot fund its primary execution attempt")]
    InsufficientDeclaredBudget,
}

pub(crate) fn expand_soak_seed_panel(
    primary_seed: StatisticalSeed,
    additional_seed_count: usize,
) -> Result<StatisticalSeedPanel, StatisticalSeedPanelError> {
    if additional_seed_count > MAX_SOAK_ADDITIONAL_SEEDS {
        return Err(StatisticalSeedPanelError::TooManySeeds {
            actual: additional_seed_count,
            maximum: MAX_SOAK_ADDITIONAL_SEEDS,
        });
    }
    let mut state = primary_seed.get();
    let mut additional_seeds = Vec::with_capacity(additional_seed_count);
    while additional_seeds.len() < additional_seed_count {
        state = state.wrapping_add(SOAK_SEED_INCREMENT);
        let mut mixed = state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(SOAK_SEED_MIX_1);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(SOAK_SEED_MIX_2);
        let seed = StatisticalSeed::new((mixed ^ (mixed >> 31)) & (i64::MAX as u64));
        if seed != primary_seed {
            additional_seeds.push(seed);
        }
    }
    StatisticalSeedPanel::try_new(primary_seed, additional_seeds)
}

pub(crate) fn expand_budgeted_soak_seed_panel(
    plan: &StatisticalPlanSummary,
    desired_additional_seed_count: usize,
) -> Result<StatisticalSeedPanel, StatisticalSeedPanelError> {
    if desired_additional_seed_count > MAX_SOAK_ADDITIONAL_SEEDS {
        return Err(StatisticalSeedPanelError::TooManySeeds {
            actual: desired_additional_seed_count,
            maximum: MAX_SOAK_ADDITIONAL_SEEDS,
        });
    }
    let exact_per_attempt = plan.exact_bound_per_attempt();
    let declared = plan.declared_familywise_bound().get();
    let additional_seed_count = (0..=desired_additional_seed_count)
        .rev()
        .find(|additional| exact_per_attempt * (*additional as f64 + 1.0) <= declared)
        .ok_or(StatisticalSeedPanelError::InsufficientDeclaredBudget)?;
    expand_soak_seed_panel(plan.primary_seed(), additional_seed_count)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StatisticalAttemptOutcome {
    Passed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StatisticalAttempt {
    seed: StatisticalSeed,
    outcome: StatisticalAttemptOutcome,
}

impl StatisticalAttempt {
    pub(crate) const fn new(seed: StatisticalSeed, outcome: StatisticalAttemptOutcome) -> Self {
        Self { seed, outcome }
    }

    pub(crate) const fn seed(self) -> StatisticalSeed {
        self.seed
    }

    pub(crate) const fn outcome(self) -> StatisticalAttemptOutcome {
        self.outcome
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct StatisticalAttemptHistory {
    attempts: Vec<StatisticalAttempt>,
    failed_attempt: Option<StatisticalAttempt>,
}

impl StatisticalAttemptHistory {
    pub(crate) const fn new() -> Self {
        Self {
            attempts: Vec::new(),
            failed_attempt: None,
        }
    }

    pub(crate) fn record(
        &mut self,
        attempt: StatisticalAttempt,
    ) -> Result<(), StatisticalAttemptError> {
        if let Some(failed_attempt) = self.failed_attempt {
            return Err(StatisticalAttemptError::TerminalFailure { failed_attempt });
        }
        if self
            .attempts
            .iter()
            .any(|existing| existing.seed() == attempt.seed())
        {
            return Err(StatisticalAttemptError::DuplicateSeed(attempt.seed()));
        }
        self.attempts.push(attempt);
        if attempt.outcome() == StatisticalAttemptOutcome::Failed {
            self.failed_attempt = Some(attempt);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn attempts(&self) -> &[StatisticalAttempt] {
        &self.attempts
    }

    #[cfg(test)]
    pub(crate) fn failed_attempt(&self) -> Option<StatisticalAttempt> {
        self.failed_attempt
    }

    #[cfg(test)]
    pub(crate) const fn has_terminal_failure(&self) -> bool {
        self.failed_attempt.is_some()
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub(crate) enum StatisticalAttemptError {
    #[error("statistical seed {0} already has a recorded attempt")]
    DuplicateSeed(StatisticalSeed),

    #[error("statistical attempt history is terminal after a failed attempt")]
    TerminalFailure { failed_attempt: StatisticalAttempt },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_plan(
        id: impl Into<String>,
        seed: u64,
        shots: u64,
        declared_familywise_bound: f64,
        buckets: Vec<(&str, f64, f64)>,
    ) -> RawStatisticalPlan {
        RawStatisticalPlan {
            id: id.into(),
            origin: StatisticalPlanOrigin::OracleFixture,
            shots,
            primary_seed: seed,
            buckets: buckets
                .into_iter()
                .map(|(name, probability, allowed_delta)| RawStatisticalBucket {
                    name: name.to_string(),
                    probability,
                    allowed_delta,
                })
                .collect(),
            declared_familywise_bound,
            independent_comparisons_per_attempt: 2,
            shot_batches_per_attempt: 2,
            seed_override_executable: true,
        }
    }

    fn permissive_plan(id: impl Into<String>, seed: u64, budget: f64) -> StatisticalPlanSummary {
        StatisticalPlanSummary::try_from_raw(raw_plan(
            id,
            seed,
            10,
            budget,
            vec![("hit", 0.5, 1.0)],
        ))
        .expect("permissive plan")
    }

    fn repo_root() -> RepoRoot {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repository root");
        RepoRoot::resolve(root).expect("resolved repository root")
    }

    #[test]
    fn exact_integer_boundaries_match_canonical_predicate_and_binomial_tails() {
        let plan = StatisticalPlanSummary::try_from_raw(raw_plan(
            "exact-boundaries",
            7,
            100_000,
            1e-6,
            vec![("hit", 0.25, 0.01)],
        ))
        .expect("valid exact-tail plan");
        let bucket = plan.buckets().first().expect("statistical bucket");

        assert_eq!(bucket.accepted_counts().minimum(), 24_001);
        assert_eq!(bucket.accepted_counts().maximum(), 25_999);
        assert!(!bucket.accepted_counts().contains(24_000));
        assert!(bucket.accepted_counts().contains(25_000));
        assert!(!bucket.accepted_counts().contains(26_000));
        assert!(bucket.exact_rejection_probability() > 0.0);
        assert!(bucket.exact_rejection_probability() <= plan.declared_familywise_bound().get());
    }

    #[test]
    fn plan_rejects_impossible_integer_boundaries() {
        let error = StatisticalPlanSummary::try_from_raw(raw_plan(
            "impossible",
            7,
            3,
            1.0,
            vec![("hit", 0.5, 0.0)],
        ))
        .expect_err("half of three shots has no exact integer count");

        assert!(matches!(
            error,
            StatisticalPlanError::ImpossibleBoundaries { shots: 3, .. }
        ));
    }

    #[test]
    fn selected_suite_rejects_per_case_and_aggregate_budget_excess() {
        let over_case = permissive_plan("over-case", 0, 1.1e-6);
        let error = validate_selected_suite([&over_case]).expect_err("case budget must fail");
        assert!(error.to_string().contains("per-case limit"));

        let plans = (0_u64..=100)
            .map(|seed| permissive_plan(format!("suite-{seed}"), seed, 1e-6))
            .collect::<Vec<_>>();
        let error = validate_selected_suite(&plans).expect_err("suite budget must fail");
        assert!(error.to_string().contains("summed familywise bound"));
    }

    #[test]
    fn plan_rejects_missing_and_duplicate_buckets() {
        let missing =
            StatisticalPlanSummary::try_from_raw(raw_plan("missing", 1, 10, 1.0, Vec::new()))
                .expect_err("missing buckets must fail");
        assert!(matches!(
            missing,
            StatisticalPlanError::MissingBuckets { .. }
        ));

        let duplicate = StatisticalPlanSummary::try_from_raw(raw_plan(
            "duplicate",
            1,
            10,
            1.0,
            vec![("hit", 0.5, 1.0), ("hit", 0.5, 1.0)],
        ))
        .expect_err("duplicate buckets must fail");
        assert!(matches!(
            duplicate,
            StatisticalPlanError::DuplicateBucket { .. }
        ));
    }

    #[test]
    fn plan_rejects_shot_batches_that_do_not_divide_across_comparisons() {
        let mut plan = raw_plan("uneven-batches", 1, 10, 1.0, vec![("hit", 0.5, 1.0)]);
        plan.independent_comparisons_per_attempt = 3;
        plan.shot_batches_per_attempt = 4;

        let error =
            StatisticalPlanSummary::try_from_raw(plan).expect_err("uneven shot batches must fail");

        assert!(matches!(
            error,
            StatisticalPlanError::UnevenShotBatches {
                batches: 4,
                comparisons: 3,
                ..
            }
        ));
    }

    #[test]
    fn selected_suite_rejects_duplicate_plan_ids() {
        let first = permissive_plan("same-id", 9, 1e-6);
        let duplicate = permissive_plan("same-id", 9, 1e-6);
        let error =
            validate_selected_suite([&first, &duplicate]).expect_err("duplicate ids must fail");

        assert!(error.to_string().contains("selected more than once"));
    }

    #[test]
    fn selected_suite_allows_same_numeric_seed_across_independent_plans() {
        let first = permissive_plan("first", 9, 1e-6);
        let second = permissive_plan("second", 9, 1e-6);

        assert!(validate_selected_suite([&first, &second]).is_ok());
    }

    #[test]
    fn deterministic_soak_panel_is_frozen_and_rejects_duplicate_seeds() {
        let first = expand_soak_seed_panel(StatisticalSeed::new(0), 3).expect("seed panel");
        let second = expand_soak_seed_panel(StatisticalSeed::new(0), 3).expect("seed panel");
        assert_eq!(first, second);
        assert_eq!(first.primary_seed(), StatisticalSeed::new(0));
        assert_eq!(
            first.additional_seeds(),
            &[
                StatisticalSeed::new(0x6220_a839_7b1d_cdaf),
                StatisticalSeed::new(0x6e78_9e6a_a1b9_65f4),
                StatisticalSeed::new(0x06c4_5d18_8009_454f),
            ]
        );
        assert_eq!(first.seeds().count(), 4);

        let error = StatisticalSeedPanel::try_new(
            StatisticalSeed::new(4),
            vec![StatisticalSeed::new(5), StatisticalSeed::new(4)],
        )
        .expect_err("primary seed duplication must fail");
        assert_eq!(
            error,
            StatisticalSeedPanelError::DuplicateSeed(StatisticalSeed::new(4))
        );
    }

    #[test]
    fn failed_attempt_is_retained_and_makes_history_terminal() {
        let mut history = StatisticalAttemptHistory::new();
        history
            .record(StatisticalAttempt::new(
                StatisticalSeed::new(1),
                StatisticalAttemptOutcome::Passed,
            ))
            .expect("passing attempt");
        let failed =
            StatisticalAttempt::new(StatisticalSeed::new(2), StatisticalAttemptOutcome::Failed);
        history.record(failed).expect("failed attempt is recorded");

        let error = history
            .record(StatisticalAttempt::new(
                StatisticalSeed::new(3),
                StatisticalAttemptOutcome::Passed,
            ))
            .expect_err("passing rerun cannot replace failure");
        assert_eq!(
            error,
            StatisticalAttemptError::TerminalFailure {
                failed_attempt: failed
            }
        );
        assert!(history.has_terminal_failure());
        assert_eq!(history.failed_attempt(), Some(failed));
        assert_eq!(history.attempts().len(), 2);
        assert_eq!(history.attempts().get(1), Some(&failed));
    }

    #[test]
    fn source_catalog_normalizes_both_formats_within_suite_budget() {
        let root = repo_root();
        let plans = source_plan_summaries(&root).expect("source statistical plans");
        let fixture = plans
            .iter()
            .find(|plan| plan.id().as_str() == "m8-sample-h-random-statistical")
            .expect("fixture statistical plan");
        assert_eq!(fixture.origin(), StatisticalPlanOrigin::OracleFixture);
        assert_eq!(fixture.shots().get(), 1_200);
        assert_eq!(fixture.independent_comparisons_per_attempt().get(), 2);
        assert_eq!(fixture.shot_batches_per_attempt().get(), 2);
        assert_eq!(fixture.primary_seed(), StatisticalSeed::new(5));
        let fixture_bucket = fixture.buckets().first().expect("fixture bucket");
        assert_eq!(fixture_bucket.name(), "1");
        assert_eq!(fixture_bucket.expected_probability().get(), 0.5);
        assert!(fixture.seed_override_executable());

        let blocker = plans
            .iter()
            .find(|plan| plan.id().as_str() == "pfm3-contract-mpp-stochastic")
            .expect("blocker-ledger statistical plan");
        assert_eq!(blocker.origin(), StatisticalPlanOrigin::BlockerLedger);
        assert_eq!(blocker.shots().get(), 100_000);
        assert_eq!(blocker.independent_comparisons_per_attempt().get(), 3);
        assert_eq!(blocker.shot_batches_per_attempt().get(), 3);
        assert_eq!(
            blocker.buckets().first().expect("blocker bucket").name(),
            "mpp-zero"
        );
        assert!(!blocker.seed_override_executable());

        let budget = validate_selected_suite(&plans).expect("source suite budget");
        assert_eq!(budget.plan_count(), 32);
        assert!((budget.declared_familywise_bound() - 32e-6).abs() < 1e-15);
        assert!(budget.exact_familywise_bound() <= budget.declared_familywise_bound());
    }

    #[test]
    fn source_soak_panels_budget_both_oracle_sides_across_every_seed() {
        let root = repo_root();
        let plans = source_plan_summaries(&root).expect("source statistical plans");
        for plan in &plans {
            let comparison_count = f64::from(plan.independent_comparisons_per_attempt().get());
            let single_comparison_bound = plan
                .buckets()
                .iter()
                .map(StatisticalBucketSummary::exact_rejection_probability)
                .sum::<f64>();
            assert_eq!(
                plan.exact_bound_per_attempt(),
                single_comparison_bound * comparison_count,
                "{} per-attempt union bound",
                plan.id()
            );
            let attempt_count = if plan.seed_override_executable() {
                let panel = expand_budgeted_soak_seed_panel(plan, 3)
                    .expect("source plan has a budgeted soak panel")
                    .seeds()
                    .count() as f64;
                if panel < 4.0 {
                    assert!(
                        plan.exact_bound_per_attempt() * (panel + 1.0)
                            > plan.declared_familywise_bound().get(),
                        "{} did not select the largest permitted soak panel",
                        plan.id()
                    );
                }
                panel
            } else {
                1.0
            };
            assert!(
                plan.exact_bound_per_attempt() * attempt_count
                    <= plan.declared_familywise_bound().get(),
                "{} soak panel exact bound {:.6e} exceeds its declared familywise budget {:.6e}",
                plan.id(),
                plan.exact_bound_per_attempt() * attempt_count,
                plan.declared_familywise_bound().get()
            );
        }
    }
}
