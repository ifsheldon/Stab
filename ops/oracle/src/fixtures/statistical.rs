use std::collections::{BTreeMap, BTreeSet};

use super::row::CliArgv;
use crate::statistical_contract::{CountTolerance, two_sided_normal_tail_bound};

const PROBABILITY_SUM_TOLERANCE: f64 = 1e-9;
/// Every one-sample row checks Stim's output and Stab's output against the same analytic
/// distribution, so its Bonferroni bound counts each checked rate twice.
const ONE_SAMPLE_CHECKED_OUTPUTS: usize = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StatisticalSource {
    Stdout,
    FixtureOutput,
}

impl StatisticalPlan {
    pub(super) fn parse(plan: &str) -> Result<Self, String> {
        parse_statistical_plan(plan)
    }

    pub(super) const fn source(&self) -> StatisticalSource {
        self.source
    }

    /// Checks one output against the plan, returning a failure reason.
    pub(super) fn compare(&self, bytes: &[u8]) -> Option<String> {
        let text = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => return Some(format!("statistical output is not UTF-8: {error}")),
        };
        match &self.tolerance {
            StatisticalTolerance::Binomial {
                expected_probability,
                sigma,
            } => compare_binomial_samples(text, self.sample_count, *expected_probability, *sigma),
            StatisticalTolerance::Buckets { expected, sigma } => {
                compare_bucket_samples(text, self.sample_count, expected, *sigma)
            }
        }
    }

    /// Checks that the argv requests the plan's sample count and seed.
    pub(super) fn validate_argv(&self, argv: &CliArgv) -> Option<String> {
        validate_shots_and_seed(self.sample_count, self.fixed_seed, argv)
    }
}

/// Checks that a plan's declared sample count and seed are the ones the argv requests.
pub(super) fn validate_shots_and_seed(
    sample_count: usize,
    fixed_seed: u64,
    argv: &CliArgv,
) -> Option<String> {
    let shots = match argv.option_value("--shots") {
        Some(value) => value,
        None => return Some("statistical fixture argv does not declare --shots".to_string()),
    };
    let shots = match shots.parse::<usize>() {
        Ok(shots) => shots,
        Err(error) => return Some(format!("invalid --shots value {shots:?}: {error}")),
    };
    if shots != sample_count {
        return Some(format!(
            "statistical plan sample_count={sample_count} does not match --shots {shots}"
        ));
    }
    let seed = match argv.option_value("--seed") {
        Some(value) => value,
        None => return Some("statistical fixture argv does not declare --seed".to_string()),
    };
    let seed = match seed.parse::<u64>() {
        Ok(seed) => seed,
        Err(error) => return Some(format!("invalid --seed value {seed:?}: {error}")),
    };
    if seed != fixed_seed {
        return Some(format!(
            "statistical plan fixed_seed={fixed_seed} does not match --seed {seed}"
        ));
    }
    None
}

fn compare_binomial_samples(
    text: &str,
    sample_count: usize,
    expected_probability: f64,
    sigma: f64,
) -> Option<String> {
    let mut total = 0usize;
    let mut hits = 0usize;
    for line in text.lines() {
        match line {
            "0" => total += 1,
            "1" => {
                total += 1;
                hits += 1;
            }
            _ => {
                return Some(format!(
                    "statistical comparator expected one 0/1 bit per shot, got {line:?}"
                ));
            }
        }
    }
    if total == 0 {
        return Some("statistical comparator received no samples".to_string());
    }
    if total != sample_count {
        return Some(format!(
            "statistical comparator expected {sample_count} samples, got {total}",
        ));
    }
    let standard_deviation = binomial_standard_deviation(expected_probability, total);
    let allowed_delta = sigma * standard_deviation;
    let Ok(total_u64) = u64::try_from(total) else {
        return Some("statistical sample count exceeds the supported u64 range".to_string());
    };
    let Some(accepted) = CountTolerance::try_new(total_u64, expected_probability, allowed_delta)
    else {
        return Some(
            "statistical comparator could not derive an accepted integer count range".to_string(),
        );
    };
    let Ok(hits) = u64::try_from(hits) else {
        return Some("statistical hit count exceeds the supported u64 range".to_string());
    };
    (!accepted.accepts(hits)).then(|| {
        format!(
            "actual hit count {hits}/{total} is outside the {}",
            accepted.describe()
        )
    })
}

fn compare_bucket_samples(
    text: &str,
    sample_count: usize,
    expected: &[(String, f64)],
    sigma: f64,
) -> Option<String> {
    let mut counts = expected
        .iter()
        .map(|(bucket, _)| (bucket.as_str(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut total = 0usize;
    for line in text.lines() {
        let Some(count) = counts.get_mut(line) else {
            return Some(format!(
                "statistical comparator saw unplanned bucket {line:?}"
            ));
        };
        *count += 1;
        total += 1;
    }
    if total == 0 {
        return Some("statistical comparator received no samples".to_string());
    }
    if total != sample_count {
        return Some(format!(
            "statistical comparator expected {sample_count} samples, got {total}",
        ));
    }
    for (bucket, expected_probability) in expected {
        let count = counts.get(bucket.as_str()).copied().unwrap_or_default();
        let standard_deviation = binomial_standard_deviation(*expected_probability, total);
        let allowed_delta = sigma * standard_deviation;
        let Ok(total_u64) = u64::try_from(total) else {
            return Some("statistical sample count exceeds the supported u64 range".to_string());
        };
        let Some(accepted) =
            CountTolerance::try_new(total_u64, *expected_probability, allowed_delta)
        else {
            return Some(format!(
                "statistical comparator could not derive an accepted integer count range for bucket {bucket:?}"
            ));
        };
        let Ok(count) = u64::try_from(count) else {
            return Some("statistical bucket count exceeds the supported u64 range".to_string());
        };
        if !accepted.accepts(count) {
            return Some(format!(
                "actual bucket {bucket:?} count {count}/{total} is outside the {}",
                accepted.describe()
            ));
        }
    }
    None
}

fn binomial_standard_deviation(probability: f64, sample_count: usize) -> f64 {
    (probability * (1.0 - probability) / sample_count as f64).sqrt()
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct StatisticalPlan {
    sample_count: usize,
    fixed_seed: u64,
    false_positive_rate: f64,
    source: StatisticalSource,
    tolerance: StatisticalTolerance,
}

#[derive(Clone, Debug, PartialEq)]
enum StatisticalTolerance {
    Binomial {
        expected_probability: f64,
        sigma: f64,
    },
    Buckets {
        expected: Vec<(String, f64)>,
        sigma: f64,
    },
}

fn parse_statistical_plan(plan: &str) -> Result<StatisticalPlan, String> {
    let mut source = None;
    let mut tolerance = None;
    let mut common = CommonPlanFields::default();
    for token in plan.split(';').map(str::trim) {
        if token.is_empty() || common.parse_token(token)? {
            continue;
        }
        if let Some(value) = token.strip_prefix("source=") {
            let parsed = match value {
                "stdout" => StatisticalSource::Stdout,
                "fixture_output" => StatisticalSource::FixtureOutput,
                _ => return Err(format!("unknown statistical source {value:?}")),
            };
            if source.replace(parsed).is_some() {
                return Err("statistical plan declares source more than once".to_string());
            }
            continue;
        }
        if let Some(rest) = token.strip_prefix("tolerate binomial p=") {
            let (probability, sigma) = parse_probability_and_sigma(token, rest, "binomial")?;
            let parsed = StatisticalTolerance::Binomial {
                expected_probability: probability,
                sigma,
            };
            if tolerance.replace(parsed).is_some() {
                return Err("statistical plan declares its tolerance more than once".to_string());
            }
            continue;
        }
        if let Some(rest) = token.strip_prefix("tolerate buckets ") {
            let (bucket_text, sigma) = parse_bucket_text_and_sigma(token, rest)?;
            let parsed = StatisticalTolerance::Buckets {
                expected: parse_bucket_probabilities(bucket_text)?,
                sigma,
            };
            if tolerance.replace(parsed).is_some() {
                return Err("statistical plan declares its tolerance more than once".to_string());
            }
            continue;
        }
        return Err(format!("unknown statistical plan token {token:?}"));
    }
    let (sample_count, fixed_seed, false_positive_rate) = common.finish()?;
    let tolerance = tolerance
        .ok_or_else(|| "statistical plan does not contain a statistical tolerance".to_string())?;
    validate_false_positive_budget(false_positive_rate, &tolerance)?;
    Ok(StatisticalPlan {
        sample_count,
        fixed_seed,
        false_positive_rate,
        source: source.unwrap_or(StatisticalSource::Stdout),
        tolerance,
    })
}

/// Plan tokens shared by the one-sample and two-sample statistical grammars.
#[derive(Default)]
pub(super) struct CommonPlanFields {
    sample_count: Option<usize>,
    fixed_seed: Option<u64>,
    false_positive_rate: Option<f64>,
}

impl CommonPlanFields {
    /// Consumes `token` when it is a shared plan token, returning whether it was consumed.
    pub(super) fn parse_token(&mut self, token: &str) -> Result<bool, String> {
        if let Some(value) = token.strip_prefix("sample_count=") {
            let parsed = value
                .parse::<usize>()
                .map_err(|error| format!("invalid sample_count {value:?}: {error}"))?;
            if parsed == 0 {
                return Err("sample_count must be positive".to_string());
            }
            if self.sample_count.replace(parsed).is_some() {
                return Err("statistical plan declares sample_count more than once".to_string());
            }
            return Ok(true);
        }
        if let Some(value) = token.strip_prefix("fixed_seed=") {
            let parsed = value
                .parse::<u64>()
                .map_err(|error| format!("invalid fixed_seed {value:?}: {error}"))?;
            if self.fixed_seed.replace(parsed).is_some() {
                return Err("statistical plan declares fixed_seed more than once".to_string());
            }
            return Ok(true);
        }
        if let Some(value) = token.strip_prefix("false_positive_rate<=") {
            let parsed = value
                .parse::<f64>()
                .map_err(|error| format!("invalid false_positive_rate {value:?}: {error}"))?;
            if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) || parsed == 0.0 {
                return Err(format!(
                    "false_positive_rate {parsed} must be positive and at most 1"
                ));
            }
            if self.false_positive_rate.replace(parsed).is_some() {
                return Err(
                    "statistical plan declares false_positive_rate more than once".to_string(),
                );
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Returns `(sample_count, fixed_seed, false_positive_rate)` once all are declared.
    pub(super) fn finish(self) -> Result<(usize, u64, f64), String> {
        let false_positive_rate = self
            .false_positive_rate
            .ok_or_else(|| "statistical plan does not contain false_positive_rate".to_string())?;
        let sample_count = self
            .sample_count
            .ok_or_else(|| "statistical plan does not contain sample_count".to_string())?;
        let fixed_seed = self
            .fixed_seed
            .ok_or_else(|| "statistical plan does not contain fixed_seed".to_string())?;
        Ok((sample_count, fixed_seed, false_positive_rate))
    }
}

fn validate_false_positive_budget(
    false_positive_rate: f64,
    tolerance: &StatisticalTolerance,
) -> Result<(), String> {
    let (sigma, rates) = match tolerance {
        StatisticalTolerance::Binomial { sigma, .. } => (*sigma, 1usize),
        StatisticalTolerance::Buckets { expected, sigma } => (*sigma, expected.len()),
    };
    let checked_rates = rates * ONE_SAMPLE_CHECKED_OUTPUTS;
    let estimated_bound = (checked_rates as f64 * two_sided_normal_tail_bound(sigma)).min(1.0);
    if estimated_bound > false_positive_rate {
        return Err(format!(
            "false_positive_rate<={false_positive_rate} is tighter than the estimated {estimated_bound:.6e} false-positive bound from {sigma} sigma over {checked_rates} checked rate(s) ({rates} per output, Stim and Stab)"
        ));
    }
    Ok(())
}

fn parse_probability_and_sigma(token: &str, rest: &str, kind: &str) -> Result<(f64, f64), String> {
    let Some((probability, sigma_text)) = rest.split_once(" within ") else {
        return Err(format!("invalid {kind} statistical plan token {token:?}"));
    };
    let sigma = parse_sigma(token, sigma_text, kind)?;
    let probability = parse_probability(probability, kind)?;
    Ok((probability, sigma))
}

fn parse_bucket_text_and_sigma<'a>(token: &str, rest: &'a str) -> Result<(&'a str, f64), String> {
    let Some((bucket_text, sigma_text)) = rest.split_once(" within ") else {
        return Err(format!("invalid bucket statistical plan token {token:?}"));
    };
    let sigma = parse_sigma(token, sigma_text, "bucket")?;
    Ok((bucket_text, sigma))
}

pub(super) fn parse_sigma(token: &str, sigma_text: &str, kind: &str) -> Result<f64, String> {
    let Some(sigma_text) = sigma_text.strip_suffix(" sigma") else {
        return Err(format!("invalid {kind} statistical sigma token {token:?}"));
    };
    let sigma = sigma_text
        .parse::<f64>()
        .map_err(|error| format!("invalid {kind} sigma {sigma_text:?}: {error}"))?;
    if !sigma.is_finite() || sigma <= 0.0 {
        return Err(format!("{kind} sigma {sigma} must be positive and finite"));
    }
    Ok(sigma)
}

fn parse_probability(text: &str, kind: &str) -> Result<f64, String> {
    let probability = text
        .parse::<f64>()
        .map_err(|error| format!("invalid {kind} probability {text:?}: {error}"))?;
    if !(0.0..=1.0).contains(&probability) {
        return Err(format!(
            "{kind} probability {probability} is outside the closed unit interval"
        ));
    }
    Ok(probability)
}

fn parse_bucket_probabilities(text: &str) -> Result<Vec<(String, f64)>, String> {
    let mut seen = BTreeSet::new();
    let mut buckets = Vec::new();
    for entry in text.split(',').map(str::trim) {
        let Some((bucket, probability)) = entry.split_once('=') else {
            return Err(format!("invalid bucket probability entry {entry:?}"));
        };
        if bucket.is_empty() || !bucket.bytes().all(|byte| matches!(byte, b'0' | b'1')) {
            return Err(format!("invalid statistical bucket name {bucket:?}"));
        }
        if !seen.insert(bucket.to_string()) {
            return Err(format!("duplicate statistical bucket {bucket:?}"));
        }
        buckets.push((
            bucket.to_string(),
            parse_probability(probability, "bucket")?,
        ));
    }
    if buckets.is_empty() {
        return Err("bucket statistical plan has no buckets".to_string());
    }
    let bucket_width = buckets.first().map_or_default(|(bucket, _)| bucket.len());
    if buckets
        .iter()
        .any(|(bucket, _)| bucket.len() != bucket_width)
    {
        return Err("bucket statistical plan mixes bucket widths".to_string());
    }
    let total_probability = buckets
        .iter()
        .map(|(_, probability)| probability)
        .sum::<f64>();
    if (total_probability - 1.0).abs() > PROBABILITY_SUM_TOLERANCE {
        return Err(format!(
            "bucket probabilities must sum to 1, got {total_probability}"
        ));
    }
    Ok(buckets)
}
