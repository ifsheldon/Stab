use std::collections::BTreeSet;

use super::super::protocol::EvidenceMode;
use super::super::run::{QualificationReport, TimingAttempt, TimingAttemptKind};
use super::super::statistics::{PairedSample, StatisticsSummary, summarize};
use super::{
    EXPECTED_WARMUPS, ReportError, approximately_equal, expected_pair_count,
    validate_pair_execution,
};

const EXPECTED_THRESHOLD: f64 = 1.25;

pub(super) fn validate(report: &QualificationReport) -> Result<(), ReportError> {
    validate_attempt_policy(&report.timing_attempts)?;
    for attempt in &report.timing_attempts {
        validate_attempt(report, attempt)?;
    }
    Ok(())
}

pub(super) fn validate_attempt_policy(attempts: &[TimingAttempt]) -> Result<(), ReportError> {
    let initial = attempts.first().ok_or(ReportError::TimingAttemptCount(0))?;
    let expected_attempts = if initial.requires_noisy_rerun() { 2 } else { 1 };
    if attempts.len() != expected_attempts {
        return Err(ReportError::TimingAttemptCount(attempts.len()));
    }
    for (attempt_index, attempt) in attempts.iter().enumerate() {
        let expected_kind = if attempt_index == 0 {
            TimingAttemptKind::Initial
        } else {
            TimingAttemptKind::PairedRatioNoiseRerun
        };
        if attempt.attempt_index != attempt_index || attempt.kind != expected_kind {
            return Err(ReportError::TimingAttemptIdentity);
        }
    }
    Ok(())
}

fn validate_attempt(
    report: &QualificationReport,
    attempt: &TimingAttempt,
) -> Result<(), ReportError> {
    if attempt.warmups.len() != EXPECTED_WARMUPS {
        return Err(ReportError::WarmupCount(attempt.warmups.len()));
    }
    for (index, warmup) in attempt.warmups.iter().enumerate() {
        if warmup.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        validate_pair_execution(warmup, EvidenceMode::Timing)?;
    }
    if attempt.samples.len() != expected_pair_count(report.tier) {
        return Err(ReportError::SampleCount(attempt.samples.len()));
    }
    let mut reconstructed = Vec::new();
    for (index, sample) in attempt.samples.iter().enumerate() {
        if sample.pair_index != index {
            return Err(ReportError::PairIndex);
        }
        reconstructed.extend(validate_pair_execution(sample, EvidenceMode::Timing)?);
    }
    if !paired_samples_equivalent(&reconstructed, &attempt.paired_samples) {
        return Err(ReportError::PairedSamples);
    }
    let measurement_ids = attempt
        .paired_samples
        .iter()
        .map(|sample| sample.measurement_id.clone())
        .collect::<BTreeSet<_>>();
    if measurement_ids.len() != attempt.statistics.len() || measurement_ids.is_empty() {
        return Err(ReportError::StatisticsSet);
    }
    let mut reconstructed_statistics = Vec::new();
    for measurement_id in measurement_ids {
        let selected = attempt
            .paired_samples
            .iter()
            .filter(|sample| sample.measurement_id == measurement_id)
            .cloned()
            .collect::<Vec<PairedSample>>();
        reconstructed_statistics.push(summarize(measurement_id, &selected, EXPECTED_THRESHOLD)?);
    }
    if !statistics_equivalent(&reconstructed_statistics, &attempt.statistics) {
        return Err(ReportError::StatisticsMismatch {
            expected: format!("{reconstructed_statistics:?}"),
            actual: format!("{:?}", attempt.statistics),
        });
    }
    let worst = reconstructed_statistics
        .iter()
        .map(|summary| summary.confidence_interval_upper)
        .reduce(f64::max)
        .ok_or(ReportError::StatisticsSet)?;
    if !approximately_equal(worst, attempt.worst_confidence_interval_upper) {
        return Err(ReportError::WorstUpperBound);
    }
    Ok(())
}

fn paired_samples_equivalent(expected: &[PairedSample], actual: &[PairedSample]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(expected, actual)| {
            expected.pair_index == actual.pair_index
                && expected.order == actual.order
                && expected.measurement_id == actual.measurement_id
                && expected.work_count == actual.work_count
                && approximately_equal(expected.stim_elapsed_seconds, actual.stim_elapsed_seconds)
                && approximately_equal(expected.stab_elapsed_seconds, actual.stab_elapsed_seconds)
                && approximately_equal(expected.stim_work_per_second, actual.stim_work_per_second)
                && approximately_equal(expected.stab_work_per_second, actual.stab_work_per_second)
                && approximately_equal(expected.ratio, actual.ratio)
        })
}

fn statistics_equivalent(expected: &[StatisticsSummary], actual: &[StatisticsSummary]) -> bool {
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(|(expected, actual)| {
            expected.measurement_id == actual.measurement_id
                && expected.pair_count == actual.pair_count
                && expected.outcome == actual.outcome
                && approximately_equal(expected.median_ratio, actual.median_ratio)
                && approximately_equal(
                    expected.confidence_interval_lower,
                    actual.confidence_interval_lower,
                )
                && approximately_equal(
                    expected.confidence_interval_upper,
                    actual.confidence_interval_upper,
                )
                && approximately_equal(expected.stim_relative_mad, actual.stim_relative_mad)
                && approximately_equal(expected.stab_relative_mad, actual.stab_relative_mad)
                && approximately_equal(expected.ratio_relative_mad, actual.ratio_relative_mad)
                && approximately_equal(expected.threshold, actual.threshold)
        })
}
