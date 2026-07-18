use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::protocol::{EvidenceMode, Implementation, ProtocolId, WorkerMeasurement};
use crate::qualification::model::TimingBatchPolicy;

pub(crate) const BOOTSTRAP_RESAMPLES: usize = 10_000;
const BOOTSTRAP_SEED: u64 = 0x5354_4142_5051_3031;
const NOISY_RELATIVE_MAD: f64 = 0.10;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PairOrder {
    StimThenStab,
    StabThenStim,
}

impl PairOrder {
    pub(super) const fn for_pair(pair_index: usize) -> Self {
        if pair_index.is_multiple_of(2) {
            Self::StimThenStab
        } else {
            Self::StabThenStim
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PairedSample {
    pub(super) pair_index: usize,
    pub(super) order: PairOrder,
    pub(super) measurement_id: ProtocolId,
    pub(super) stim_elapsed_seconds: f64,
    pub(super) stab_elapsed_seconds: f64,
    pub(super) stim_work_count: u64,
    pub(super) stab_work_count: u64,
    pub(super) stim_work_per_second: f64,
    pub(super) stab_work_per_second: f64,
    pub(super) ratio: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum GateOutcome {
    Passed,
    Failed,
    Noisy,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct StatisticsSummary {
    pub(super) measurement_id: ProtocolId,
    pub(super) pair_count: usize,
    pub(super) median_ratio: f64,
    pub(super) confidence_interval_lower: f64,
    pub(super) confidence_interval_upper: f64,
    pub(super) stim_relative_mad: f64,
    pub(super) stab_relative_mad: f64,
    pub(super) ratio_relative_mad: f64,
    pub(super) threshold: f64,
    pub(super) outcome: GateOutcome,
}

pub(crate) fn pair_measurements(
    pair_index: usize,
    order: PairOrder,
    stim: &[WorkerMeasurement],
    stab: &[WorkerMeasurement],
) -> Result<Vec<PairedSample>, StatisticsError> {
    pair_measurements_with_policy(
        pair_index,
        order,
        stim,
        stab,
        TimingBatchPolicy::CommonIterations,
    )
}

pub(crate) fn pair_measurements_with_policy(
    pair_index: usize,
    order: PairOrder,
    stim: &[WorkerMeasurement],
    stab: &[WorkerMeasurement],
    batch_policy: TimingBatchPolicy,
) -> Result<Vec<PairedSample>, StatisticsError> {
    let stim = by_measurement(Implementation::Stim, stim)?;
    let stab = by_measurement(Implementation::Stab, stab)?;
    if stim.keys().collect::<BTreeSet<_>>() != stab.keys().collect::<BTreeSet<_>>() {
        return Err(StatisticsError::MeasurementSetMismatch);
    }
    let mut pairs = Vec::with_capacity(stim.len());
    for (measurement_id, stim) in stim {
        let stab = stab
            .get(measurement_id)
            .ok_or(StatisticsError::MeasurementSetMismatch)?;
        if stim.evidence_mode != EvidenceMode::Timing || stab.evidence_mode != EvidenceMode::Timing
        {
            return Err(StatisticsError::MemoryEvidenceInTiming {
                measurement: measurement_id.clone(),
            });
        }
        let shared_identity_matches =
            stim.workload_id == stab.workload_id && stim.stim_commit == stab.stim_commit;
        let common_semantics_match = stim.iteration_count == stab.iteration_count
            && stim.work_count == stab.work_count
            && stim.output_digest == stab.output_digest;
        let independent_work_units_match = stim.iteration_count > 0
            && stab.iteration_count > 0
            && u128::from(stim.work_count) * u128::from(stab.iteration_count)
                == u128::from(stab.work_count) * u128::from(stim.iteration_count)
            && (stim.iteration_count != stab.iteration_count
                || stim.output_digest == stab.output_digest);
        if !shared_identity_matches
            || match batch_policy {
                TimingBatchPolicy::CommonIterations => !common_semantics_match,
                TimingBatchPolicy::IndependentThroughput => !independent_work_units_match,
            }
        {
            return Err(StatisticsError::SemanticMismatch {
                measurement: measurement_id.clone(),
            });
        }
        if stim.work_count == 0 || stab.work_count == 0 {
            return Err(StatisticsError::MissingWork {
                measurement: measurement_id.clone(),
            });
        }
        let stim_work_per_second = stim.work_count as f64 / stim.elapsed_seconds;
        let stab_work_per_second = stab.work_count as f64 / stab.elapsed_seconds;
        let ratio = (stab.elapsed_seconds / stab.work_count as f64)
            / (stim.elapsed_seconds / stim.work_count as f64);
        if [stim_work_per_second, stab_work_per_second, ratio]
            .into_iter()
            .any(|value| !value.is_finite() || value <= 0.0)
        {
            return Err(StatisticsError::InvalidRatio {
                measurement: measurement_id.clone(),
            });
        }
        pairs.push(PairedSample {
            pair_index,
            order,
            measurement_id: measurement_id.clone(),
            stim_elapsed_seconds: stim.elapsed_seconds,
            stab_elapsed_seconds: stab.elapsed_seconds,
            stim_work_count: stim.work_count,
            stab_work_count: stab.work_count,
            stim_work_per_second,
            stab_work_per_second,
            ratio,
        });
    }
    Ok(pairs)
}

fn by_measurement(
    implementation: Implementation,
    rows: &[WorkerMeasurement],
) -> Result<BTreeMap<&ProtocolId, &WorkerMeasurement>, StatisticsError> {
    let mut result = BTreeMap::new();
    for row in rows {
        if row.implementation != implementation {
            return Err(StatisticsError::WrongImplementation {
                expected: implementation,
                actual: row.implementation,
            });
        }
        if result.insert(&row.measurement_id, row).is_some() {
            return Err(StatisticsError::DuplicateMeasurement(
                row.measurement_id.clone(),
            ));
        }
    }
    Ok(result)
}

pub(crate) fn summarize(
    measurement_id: ProtocolId,
    samples: &[PairedSample],
    threshold: f64,
) -> Result<StatisticsSummary, StatisticsError> {
    if samples.is_empty() {
        return Err(StatisticsError::MissingSamples);
    }
    if !threshold.is_finite() || threshold <= 0.0 {
        return Err(StatisticsError::InvalidThreshold(threshold));
    }
    let mut indices = BTreeSet::new();
    for sample in samples {
        if sample.measurement_id != measurement_id {
            return Err(StatisticsError::MixedMeasurements);
        }
        if !indices.insert(sample.pair_index) {
            return Err(StatisticsError::DuplicatePair(sample.pair_index));
        }
        if sample.order != PairOrder::for_pair(sample.pair_index) {
            return Err(StatisticsError::WrongPairOrder(sample.pair_index));
        }
    }
    let stim = samples
        .iter()
        .map(|sample| sample.stim_elapsed_seconds / sample.stim_work_count as f64)
        .collect::<Vec<_>>();
    let stab = samples
        .iter()
        .map(|sample| sample.stab_elapsed_seconds / sample.stab_work_count as f64)
        .collect::<Vec<_>>();
    let ratios = samples
        .iter()
        .map(|sample| sample.ratio)
        .collect::<Vec<_>>();
    validate_positive_finite(&stim)?;
    validate_positive_finite(&stab)?;
    validate_positive_finite(&ratios)?;
    let median_ratio = median(&ratios)?;
    let (confidence_interval_lower, confidence_interval_upper) = bootstrap_interval(&ratios)?;
    let stim_relative_mad = relative_mad(&stim)?;
    let stab_relative_mad = relative_mad(&stab)?;
    let ratio_relative_mad = relative_mad(&ratios)?;
    let noisy = ratio_relative_mad > NOISY_RELATIVE_MAD;
    let outcome = if noisy {
        GateOutcome::Noisy
    } else if median_ratio <= threshold && confidence_interval_upper <= threshold {
        GateOutcome::Passed
    } else {
        GateOutcome::Failed
    };
    Ok(StatisticsSummary {
        measurement_id,
        pair_count: samples.len(),
        median_ratio,
        confidence_interval_lower,
        confidence_interval_upper,
        stim_relative_mad,
        stab_relative_mad,
        ratio_relative_mad,
        threshold,
        outcome,
    })
}

fn validate_positive_finite(values: &[f64]) -> Result<(), StatisticsError> {
    if values
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        Err(StatisticsError::InvalidSample)
    } else {
        Ok(())
    }
}

fn median(values: &[f64]) -> Result<f64, StatisticsError> {
    if values.is_empty() {
        return Err(StatisticsError::MissingSamples);
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        let left = sorted
            .get(middle.saturating_sub(1))
            .copied()
            .ok_or(StatisticsError::InternalIndex)?;
        let right = sorted
            .get(middle)
            .copied()
            .ok_or(StatisticsError::InternalIndex)?;
        Ok((left + right) / 2.0)
    } else {
        sorted
            .get(middle)
            .copied()
            .ok_or(StatisticsError::InternalIndex)
    }
}

fn relative_mad(values: &[f64]) -> Result<f64, StatisticsError> {
    let center = median(values)?;
    if center <= 0.0 {
        return Err(StatisticsError::InvalidSample);
    }
    let deviations = values
        .iter()
        .map(|value| (value - center).abs())
        .collect::<Vec<_>>();
    Ok(median(&deviations)? / center)
}

fn bootstrap_interval(values: &[f64]) -> Result<(f64, f64), StatisticsError> {
    if values.is_empty() {
        return Err(StatisticsError::MissingSamples);
    }
    let mut rng = SmallRng::seed_from_u64(BOOTSTRAP_SEED);
    let mut medians = Vec::with_capacity(BOOTSTRAP_RESAMPLES);
    let mut sample = Vec::with_capacity(values.len());
    for _ in 0..BOOTSTRAP_RESAMPLES {
        sample.clear();
        for _ in 0..values.len() {
            let index = rng.random_range(0..values.len());
            sample.push(
                values
                    .get(index)
                    .copied()
                    .ok_or(StatisticsError::InternalIndex)?,
            );
        }
        medians.push(median(&sample)?);
    }
    medians.sort_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal));
    let last = medians.len() - 1;
    let lower = last * 25 / 1000;
    let upper = (last * 975).div_ceil(1000);
    Ok((
        medians
            .get(lower)
            .copied()
            .ok_or(StatisticsError::InternalIndex)?,
        medians
            .get(upper)
            .copied()
            .ok_or(StatisticsError::InternalIndex)?,
    ))
}

#[derive(Debug, Error)]
pub(crate) enum StatisticsError {
    #[error("worker row implementation is {actual}, expected {expected}")]
    WrongImplementation {
        expected: Implementation,
        actual: Implementation,
    },
    #[error("worker repeats measurement {0}")]
    DuplicateMeasurement(ProtocolId),
    #[error("Stim and Stab measurement id sets differ")]
    MeasurementSetMismatch,
    #[error("measurement {measurement} uses memory evidence in a timing comparison")]
    MemoryEvidenceInTiming { measurement: ProtocolId },
    #[error(
        "measurement {measurement} has mismatched workload, iterations, work, digest, or Stim commit"
    )]
    SemanticMismatch { measurement: ProtocolId },
    #[error("measurement {measurement} has no semantic work")]
    MissingWork { measurement: ProtocolId },
    #[error("measurement {measurement} produced an invalid normalized ratio")]
    InvalidRatio { measurement: ProtocolId },
    #[error("statistics require at least one paired sample")]
    MissingSamples,
    #[error("statistics threshold {0} must be finite and positive")]
    InvalidThreshold(f64),
    #[error("statistics contain unlike measurement ids")]
    MixedMeasurements,
    #[error("statistics repeat pair index {0}")]
    DuplicatePair(usize),
    #[error("pair index {0} does not use the deterministic alternating order")]
    WrongPairOrder(usize),
    #[error("statistics contain a non-finite or non-positive sample")]
    InvalidSample,
    #[error("statistics internal index invariant failed")]
    InternalIndex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::qualification::runtime::protocol::{
        GitCommit, InputDigest, PROTOCOL_SCHEMA_VERSION, SemanticDigest, Sha256Digest,
    };

    fn row(implementation: Implementation, elapsed_seconds: f64) -> WorkerMeasurement {
        WorkerMeasurement {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            implementation,
            evidence_mode: EvidenceMode::Timing,
            workload_id: ProtocolId::try_new("synthetic").expect("workload id"),
            measurement_id: ProtocolId::try_new("main").expect("measurement id"),
            iteration_count: 1,
            elapsed_seconds,
            work_count: 100,
            input_bytes: 0,
            input_digest: InputDigest::try_new(
                "6a09e667f3bcc908bb67ae8584caa73b3c6ef372fe94f82ba54ff53a5f1d36f1",
            )
            .expect("empty input digest"),
            output_digest: SemanticDigest::try_new("a".repeat(64)).expect("output digest"),
            setup_rss_bytes: None,
            peak_rss_bytes: None,
            affinity_cpu: None,
            stim_commit: GitCommit::try_new("e2fc1eca7fd21684d433aa5f10f4504ea4860d07")
                .expect("commit"),
            source_digest: Sha256Digest::try_new("b".repeat(64)).expect("source digest"),
            build_fingerprint: Sha256Digest::try_new("c".repeat(64)).expect("build fingerprint"),
        }
    }

    fn constant_samples(ratio: f64, count: usize) -> Vec<PairedSample> {
        (0..count)
            .flat_map(|pair_index| {
                pair_measurements(
                    pair_index,
                    PairOrder::for_pair(pair_index),
                    &[row(Implementation::Stim, 1.0)],
                    &[row(Implementation::Stab, ratio)],
                )
                .expect("pair measurements")
            })
            .collect()
    }

    #[test]
    fn equal_speed_confidence_interval_contains_one() {
        let summary = summarize(
            ProtocolId::try_new("main").expect("measurement id"),
            &constant_samples(1.0, 9),
            1.25,
        )
        .expect("statistics");
        assert!(summary.confidence_interval_lower <= 1.0);
        assert!(summary.confidence_interval_upper >= 1.0);
        assert_eq!(summary.confidence_interval_lower, 1.0);
        assert_eq!(summary.confidence_interval_upper, 1.0);
        assert_eq!(summary.outcome, GateOutcome::Passed);
    }

    #[test]
    fn thirty_percent_slowdown_fails_the_gate() {
        let summary = summarize(
            ProtocolId::try_new("main").expect("measurement id"),
            &constant_samples(1.30, 9),
            1.25,
        )
        .expect("statistics");
        assert_eq!(summary.median_ratio, 1.30);
        assert_eq!(summary.outcome, GateOutcome::Failed);
    }

    #[test]
    fn pairing_rejects_work_digest_and_memory_mode_mismatches() {
        let stim = row(Implementation::Stim, 1.0);
        let mut stab = row(Implementation::Stab, 1.0);
        stab.work_count = 99;
        assert!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                &[stab]
            )
            .is_err()
        );

        let mut stab = row(Implementation::Stab, 1.0);
        stab.output_digest = SemanticDigest::try_new("d".repeat(64)).expect("different digest");
        assert!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                &[stab]
            )
            .is_err()
        );

        let mut stab = row(Implementation::Stab, 1.0);
        stab.evidence_mode = EvidenceMode::Memory;
        assert!(pair_measurements(0, PairOrder::StimThenStab, &[stim], &[stab]).is_err());
    }

    #[test]
    fn independent_throughput_normalizes_unequal_iteration_counts() {
        let stim = row(Implementation::Stim, 1.0);
        let mut stab = row(Implementation::Stab, 5.0);
        stab.iteration_count = 10;
        stab.work_count = 1_000;
        stab.output_digest =
            SemanticDigest::try_new("d".repeat(64)).expect("count-specific output digest");

        assert!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                std::slice::from_ref(&stab),
            )
            .is_err(),
            "common-iteration comparisons must reject unequal work"
        );
        let [pair] = pair_measurements_with_policy(
            0,
            PairOrder::StimThenStab,
            std::slice::from_ref(&stim),
            std::slice::from_ref(&stab),
            TimingBatchPolicy::IndependentThroughput,
        )
        .expect("independent throughput comparison")
        .try_into()
        .expect("one measurement");
        assert_eq!(pair.stim_work_count, 100);
        assert_eq!(pair.stab_work_count, 1_000);
        assert_eq!(pair.stim_work_per_second, 100.0);
        assert_eq!(pair.stab_work_per_second, 200.0);
        assert_eq!(pair.ratio, 0.5);

        let mut wrong_unit = stab.clone();
        wrong_unit.work_count = 999;
        assert!(
            pair_measurements_with_policy(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                &[wrong_unit],
                TimingBatchPolicy::IndependentThroughput,
            )
            .is_err()
        );

        let mut same_count_wrong_output = stab;
        same_count_wrong_output.iteration_count = 1;
        same_count_wrong_output.work_count = 100;
        assert!(
            pair_measurements_with_policy(
                0,
                PairOrder::StimThenStab,
                &[stim],
                &[same_count_wrong_output],
                TimingBatchPolicy::IndependentThroughput,
            )
            .is_err(),
            "equal-count independent samples still require exact output equality"
        );
    }

    #[test]
    fn pairing_rejects_stale_duplicate_missing_and_heterogeneous_rows() {
        let stim = row(Implementation::Stim, 1.0);
        let stab = row(Implementation::Stab, 1.0);

        let duplicate = [stim.clone(), stim.clone()];
        assert!(matches!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                &duplicate,
                std::slice::from_ref(&stab)
            ),
            Err(StatisticsError::DuplicateMeasurement(_))
        ));

        let mut stale = stab.clone();
        stale.measurement_id = ProtocolId::try_new("stale").expect("stale id");
        assert!(matches!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                &[stale]
            ),
            Err(StatisticsError::MeasurementSetMismatch)
        ));

        let mut missing_work_stim = stim.clone();
        let mut missing_work_stab = stab.clone();
        missing_work_stim.work_count = 0;
        missing_work_stab.work_count = 0;
        assert!(matches!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                &[missing_work_stim],
                &[missing_work_stab]
            ),
            Err(StatisticsError::MissingWork { .. })
        ));

        let mut wrong_iteration = stab;
        wrong_iteration.iteration_count = 2;
        assert!(matches!(
            pair_measurements(
                0,
                PairOrder::StimThenStab,
                std::slice::from_ref(&stim),
                &[wrong_iteration]
            ),
            Err(StatisticsError::SemanticMismatch { .. })
        ));
    }

    #[test]
    fn pair_order_alternates_and_noisy_data_is_not_promoted() {
        assert_eq!(PairOrder::for_pair(0), PairOrder::StimThenStab);
        assert_eq!(PairOrder::for_pair(1), PairOrder::StabThenStim);
        let mut samples = constant_samples(1.0, 9);
        for (sample, ratio) in samples
            .iter_mut()
            .zip([0.5, 0.6, 0.7, 0.8, 1.0, 1.2, 1.3, 1.4, 1.5])
        {
            sample.stab_elapsed_seconds = ratio;
            sample.ratio = ratio;
        }
        let summary = summarize(
            ProtocolId::try_new("main").expect("measurement id"),
            &samples,
            1.25,
        )
        .expect("statistics");
        assert_eq!(summary.outcome, GateOutcome::Noisy);
    }

    #[test]
    fn common_mode_rate_variation_does_not_make_paired_ratios_noisy() {
        let samples = [0.5, 0.6, 0.8, 1.0, 1.3, 1.7, 2.0, 2.5, 3.0]
            .into_iter()
            .enumerate()
            .flat_map(|(pair_index, elapsed)| {
                pair_measurements(
                    pair_index,
                    PairOrder::for_pair(pair_index),
                    &[row(Implementation::Stim, elapsed)],
                    &[row(Implementation::Stab, elapsed)],
                )
                .expect("paired common-mode measurements")
            })
            .collect::<Vec<_>>();
        let summary = summarize(
            ProtocolId::try_new("main").expect("measurement id"),
            &samples,
            1.25,
        )
        .expect("common-mode statistics");

        assert!(summary.stim_relative_mad > NOISY_RELATIVE_MAD);
        assert!(summary.stab_relative_mad > NOISY_RELATIVE_MAD);
        assert_eq!(summary.ratio_relative_mad, 0.0);
        assert_eq!(summary.outcome, GateOutcome::Passed);
    }

    #[test]
    fn median_mad_threshold_and_aggregate_contracts_are_exact() {
        let measurement_id = ProtocolId::try_new("main").expect("measurement id");
        let boundary = summarize(measurement_id.clone(), &constant_samples(1.25, 9), 1.25)
            .expect("boundary statistics");
        assert_eq!(boundary.median_ratio, 1.25);
        assert_eq!(boundary.confidence_interval_upper, 1.25);
        assert_eq!(boundary.outcome, GateOutcome::Passed);

        let mut samples = constant_samples(1.0, 5);
        for (sample, ratio) in samples.iter_mut().zip([0.8, 0.9, 1.0, 1.1, 1.2]) {
            sample.stab_elapsed_seconds = ratio;
            sample.ratio = ratio;
        }
        let summary = summarize(measurement_id.clone(), &samples, 2.0).expect("MAD statistics");
        assert!((summary.median_ratio - 1.0).abs() < f64::EPSILON);
        assert!((summary.ratio_relative_mad - 0.1).abs() < 16.0 * f64::EPSILON);

        let mut duplicate = constant_samples(1.0, 2);
        let duplicate_second = duplicate.get_mut(1).expect("second duplicate sample");
        duplicate_second.pair_index = 0;
        duplicate_second.order = PairOrder::StimThenStab;
        assert!(matches!(
            summarize(measurement_id.clone(), &duplicate, 1.25),
            Err(StatisticsError::DuplicatePair(0))
        ));

        let mut mixed = constant_samples(1.0, 2);
        mixed
            .get_mut(1)
            .expect("second mixed sample")
            .measurement_id = ProtocolId::try_new("other").expect("other measurement");
        assert!(matches!(
            summarize(measurement_id, &mixed, 1.25),
            Err(StatisticsError::MixedMeasurements)
        ));
    }
}
