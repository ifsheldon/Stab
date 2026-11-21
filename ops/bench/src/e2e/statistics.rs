use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use serde::{Deserialize, Serialize};

use super::error::{Implementation, MedianPart, QuantileSide, StatisticsError};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum PairOrder {
    StimThenStab,
    StabThenStim,
}

impl PairOrder {
    pub(super) const fn for_index(index: usize) -> Self {
        if index.is_multiple_of(2) {
            Self::StimThenStab
        } else {
            Self::StabThenStim
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PairedTiming {
    pub(super) index: usize,
    pub(super) order: PairOrder,
    pub(super) stim_seconds: f64,
    pub(super) stab_seconds: f64,
    pub(super) stim_work: u64,
    pub(super) stab_work: u64,
    pub(super) stim_peak_rss_bytes: u64,
    pub(super) stab_peak_rss_bytes: u64,
    pub(super) stim_output_bytes: u64,
    pub(super) stab_output_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StabTiming {
    pub(super) index: usize,
    pub(super) seconds: f64,
    pub(super) work: u64,
    pub(super) peak_rss_bytes: u64,
    pub(super) output_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DistributionSummary {
    pub(super) sample_count: usize,
    pub(super) median: f64,
    pub(super) confidence_lower: f64,
    pub(super) confidence_upper: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TimingSummary {
    pub(super) parity_ratio: Option<DistributionSummary>,
    pub(super) stab_seconds_per_work: DistributionSummary,
    pub(super) stim_throughput: Option<f64>,
    pub(super) stab_throughput: f64,
    pub(super) stim_peak_rss_bytes: Option<u64>,
    pub(super) stab_peak_rss_bytes: u64,
    pub(super) stim_output_bytes: Option<u64>,
    pub(super) stab_output_bytes: u64,
}

pub(super) fn summarize_paired(
    samples: &[PairedTiming],
    seed: u64,
    resamples: usize,
    confidence: f64,
) -> Result<TimingSummary, StatisticsError> {
    if samples.is_empty() {
        return Err(StatisticsError::NoPairedSamples);
    }
    let mut ratios = Vec::with_capacity(samples.len());
    let mut stab_seconds_per_work = Vec::with_capacity(samples.len());
    let mut stim_seconds_per_work = Vec::with_capacity(samples.len());
    for (position, sample) in samples.iter().enumerate() {
        if sample.index != position || sample.order != PairOrder::for_index(position) {
            return Err(StatisticsError::PairIdentity { position });
        }
        validate_sample(
            sample.stim_seconds,
            sample.stim_work,
            sample.stim_peak_rss_bytes,
        )?;
        validate_sample(
            sample.stab_seconds,
            sample.stab_work,
            sample.stab_peak_rss_bytes,
        )?;
        if sample.stim_work != sample.stab_work {
            return Err(StatisticsError::UnequalWork {
                position,
                stim: sample.stim_work,
                stab: sample.stab_work,
            });
        }
        let stim = sample.stim_seconds / sample.stim_work as f64;
        let stab = sample.stab_seconds / sample.stab_work as f64;
        stim_seconds_per_work.push(stim);
        stab_seconds_per_work.push(stab);
        ratios.push(stab / stim);
    }
    let parity_ratio = distribution(&ratios, seed, resamples, confidence)?;
    let stab_distribution = distribution(
        &stab_seconds_per_work,
        seed ^ 0x5354_4142,
        resamples,
        confidence,
    )?;
    let stim_median = median(&stim_seconds_per_work)?;
    Ok(TimingSummary {
        parity_ratio: Some(parity_ratio),
        stab_seconds_per_work: stab_distribution,
        stim_throughput: Some(1.0 / stim_median),
        stab_throughput: 1.0 / median(&stab_seconds_per_work)?,
        stim_peak_rss_bytes: samples
            .iter()
            .map(|sample| sample.stim_peak_rss_bytes)
            .max(),
        stab_peak_rss_bytes: samples
            .iter()
            .map(|sample| sample.stab_peak_rss_bytes)
            .max()
            .ok_or(StatisticsError::NoPairedRss)?,
        stim_output_bytes: constant_output(
            samples.iter().map(|sample| sample.stim_output_bytes),
            Implementation::Stim,
        )?,
        stab_output_bytes: constant_output(
            samples.iter().map(|sample| sample.stab_output_bytes),
            Implementation::Stab,
        )?
        .ok_or(StatisticsError::NoPairedOutput)?,
    })
}

pub(super) fn summarize_stab(
    samples: &[StabTiming],
    seed: u64,
    resamples: usize,
    confidence: f64,
) -> Result<TimingSummary, StatisticsError> {
    if samples.is_empty() {
        return Err(StatisticsError::NoStabSamples);
    }
    let mut seconds_per_work = Vec::with_capacity(samples.len());
    for (position, sample) in samples.iter().enumerate() {
        if sample.index != position {
            return Err(StatisticsError::StabIndex { position });
        }
        validate_sample(sample.seconds, sample.work, sample.peak_rss_bytes)?;
        seconds_per_work.push(sample.seconds / sample.work as f64);
    }
    let distribution = distribution(&seconds_per_work, seed, resamples, confidence)?;
    Ok(TimingSummary {
        parity_ratio: None,
        stab_seconds_per_work: distribution,
        stim_throughput: None,
        stab_throughput: 1.0 / median(&seconds_per_work)?,
        stim_peak_rss_bytes: None,
        stab_peak_rss_bytes: samples
            .iter()
            .map(|sample| sample.peak_rss_bytes)
            .max()
            .ok_or(StatisticsError::NoStabRss)?,
        stim_output_bytes: None,
        stab_output_bytes: constant_output(
            samples.iter().map(|sample| sample.output_bytes),
            Implementation::Stab,
        )?
        .ok_or(StatisticsError::NoStabOutput)?,
    })
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "validated quantile probabilities keep rounded indices within 0..=last"
)]
fn distribution(
    values: &[f64],
    seed: u64,
    resamples: usize,
    confidence: f64,
) -> Result<DistributionSummary, StatisticsError> {
    validate_values(values)?;
    if resamples == 0 {
        return Err(StatisticsError::ZeroResamples);
    }
    if !confidence.is_finite() || confidence <= 0.0 || confidence >= 1.0 {
        return Err(StatisticsError::Confidence);
    }
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut medians = Vec::with_capacity(resamples);
    let mut resample = Vec::with_capacity(values.len());
    for _ in 0..resamples {
        resample.clear();
        for _ in 0..values.len() {
            let index = rng.random_range(0..values.len());
            resample.push(
                values
                    .get(index)
                    .copied()
                    .ok_or(StatisticsError::BootstrapIndex)?,
            );
        }
        medians.push(median(&resample)?);
    }
    medians.sort_by(f64::total_cmp);
    let tail = (1.0 - confidence) / 2.0;
    let last = medians.len().saturating_sub(1);
    let lower = ((last as f64) * tail).floor() as usize;
    let upper = ((last as f64) * (1.0 - tail)).ceil() as usize;
    Ok(DistributionSummary {
        sample_count: values.len(),
        median: median(values)?,
        confidence_lower: *medians
            .get(lower.min(last))
            .ok_or(StatisticsError::MissingQuantile(QuantileSide::Lower))?,
        confidence_upper: *medians
            .get(upper.min(last))
            .ok_or(StatisticsError::MissingQuantile(QuantileSide::Upper))?,
    })
}

fn validate_sample(seconds: f64, work: u64, peak_rss_bytes: u64) -> Result<(), StatisticsError> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(StatisticsError::Seconds(seconds));
    }
    if work == 0 {
        return Err(StatisticsError::ZeroWork);
    }
    if peak_rss_bytes == 0 {
        return Err(StatisticsError::ZeroRss);
    }
    Ok(())
}

fn validate_values(values: &[f64]) -> Result<(), StatisticsError> {
    if values.is_empty() {
        return Err(StatisticsError::NoValues);
    }
    if values
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(StatisticsError::InvalidValue);
    }
    Ok(())
}

fn median(values: &[f64]) -> Result<f64, StatisticsError> {
    validate_values(values)?;
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        let lower = sorted
            .get(middle.saturating_sub(1))
            .ok_or(StatisticsError::MissingMedian(MedianPart::Lower))?;
        let upper = sorted
            .get(middle)
            .ok_or(StatisticsError::MissingMedian(MedianPart::Upper))?;
        Ok((lower + upper) / 2.0)
    } else {
        sorted
            .get(middle)
            .copied()
            .ok_or(StatisticsError::MissingMedian(MedianPart::Middle))
    }
}

fn constant_output(
    mut values: impl Iterator<Item = u64>,
    implementation: Implementation,
) -> Result<Option<u64>, StatisticsError> {
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.any(|value| value != first) {
        return Err(StatisticsError::OutputDrift(implementation));
    }
    Ok(Some(first))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(index: usize, ratio: f64) -> PairedTiming {
        PairedTiming {
            index,
            order: PairOrder::for_index(index),
            stim_seconds: 1.0,
            stab_seconds: ratio,
            stim_work: 100,
            stab_work: 100,
            stim_peak_rss_bytes: 10,
            stab_peak_rss_bytes: 20,
            stim_output_bytes: 30,
            stab_output_bytes: 30,
        }
    }

    #[test]
    fn paired_summary_is_deterministic_and_uses_semantic_work() {
        let samples = [pair(0, 1.0), pair(1, 1.2), pair(2, 1.1)];
        let first = summarize_paired(&samples, 7, 1_000, 0.95).expect("summary");
        let second = summarize_paired(&samples, 7, 1_000, 0.95).expect("replay");
        assert_eq!(first, second);
        assert_eq!(first.parity_ratio.as_ref().expect("parity").median, 1.1);
        assert_eq!(first.stab_peak_rss_bytes, 20);
    }

    #[test]
    fn pairing_rejects_wrong_order_unequal_work_and_variable_output_size() {
        let mut samples = vec![pair(0, 1.0), pair(1, 1.1)];
        samples[1].order = PairOrder::StimThenStab;
        assert!(summarize_paired(&samples, 7, 100, 0.95).is_err());
        samples[1].order = PairOrder::StabThenStim;
        samples[1].stab_work = 99;
        assert!(summarize_paired(&samples, 7, 100, 0.95).is_err());
        samples[1].stab_work = 100;
        samples[1].stab_output_bytes = 31;
        assert!(summarize_paired(&samples, 7, 100, 0.95).is_err());
    }
}
