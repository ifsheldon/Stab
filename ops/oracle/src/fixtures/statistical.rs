use std::collections::{BTreeMap, BTreeSet};

use crate::statistical_contract::AcceptedCountRange;

const PROBABILITY_SUM_TOLERANCE: f64 = 1e-9;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StatisticalSource {
    Stdout,
    FixtureOutput,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FixtureStatisticalPlanSummary {
    pub(crate) id: String,
    pub(crate) shots: u64,
    pub(crate) primary_seed: u64,
    pub(crate) buckets: Vec<FixtureStatisticalBucketSummary>,
    pub(crate) declared_familywise_bound: f64,
    pub(crate) independent_comparisons_per_attempt: u32,
    pub(crate) shot_batches_per_attempt: u32,
    pub(crate) seed_override_executable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FixtureStatisticalBucketSummary {
    pub(crate) name: String,
    pub(crate) expected_probability: f64,
    pub(crate) allowed_delta: f64,
}

pub(super) fn source_for_plan(plan: &str) -> Result<StatisticalSource, String> {
    parse_statistical_plan(plan).map(|plan| plan.source)
}

pub(super) fn compare_statistical_plan(plan: &str, bytes: &[u8]) -> Option<String> {
    evaluate_statistical_plan(plan, bytes).reason
}

pub(super) fn completed_shots(plan: &str, bytes: &[u8]) -> Option<u64> {
    evaluate_statistical_plan(plan, bytes).completed_shots
}

pub(super) fn argv_with_seed(argv: &str, seed: u64) -> Option<String> {
    let mut tokens = argv.split('|').map(ToOwned::to_owned).collect::<Vec<_>>();
    let mut index = 0;
    while index < tokens.len() {
        if tokens.get(index).is_some_and(|token| token == "--seed") {
            *tokens.get_mut(index.checked_add(1)?)? = seed.to_string();
            return Some(tokens.join("|"));
        }
        if tokens
            .get(index)
            .is_some_and(|token| token.starts_with("--seed="))
        {
            *tokens.get_mut(index)? = format!("--seed={seed}");
            return Some(tokens.join("|"));
        }
        index += 1;
    }
    None
}

struct StatisticalEvaluation {
    completed_shots: Option<u64>,
    reason: Option<String>,
}

impl StatisticalEvaluation {
    fn incomplete(reason: impl Into<String>) -> Self {
        Self {
            completed_shots: None,
            reason: Some(reason.into()),
        }
    }

    fn complete(sample_count: usize, reason: Option<String>) -> Self {
        Self {
            completed_shots: u64::try_from(sample_count).ok(),
            reason,
        }
    }
}

fn evaluate_statistical_plan(plan: &str, bytes: &[u8]) -> StatisticalEvaluation {
    let plan = match parse_statistical_plan(plan) {
        Ok(plan) => plan,
        Err(reason) => return StatisticalEvaluation::incomplete(reason),
    };
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            return StatisticalEvaluation::incomplete(format!(
                "statistical output is not UTF-8: {error}"
            ));
        }
    };
    match plan.tolerance {
        StatisticalTolerance::Binomial {
            expected_probability,
            sigma,
        } => compare_binomial_samples(text, plan.sample_count, expected_probability, sigma),
        StatisticalTolerance::Buckets { expected, sigma } => {
            compare_bucket_samples(text, plan.sample_count, &expected, sigma)
        }
    }
}

pub(super) fn validate_statistical_plan(plan: &str, argv_tokens: &[String]) -> Option<String> {
    let plan = match parse_statistical_plan(plan) {
        Ok(plan) => plan,
        Err(reason) => return Some(reason),
    };
    let shots = match option_value(argv_tokens, "--shots") {
        Some(value) => value,
        None => return Some("statistical fixture argv does not declare --shots".to_string()),
    };
    let shots = match shots.parse::<usize>() {
        Ok(shots) => shots,
        Err(error) => return Some(format!("invalid --shots value {shots:?}: {error}")),
    };
    if shots != plan.sample_count {
        return Some(format!(
            "statistical plan sample_count={} does not match --shots {shots}",
            plan.sample_count
        ));
    }
    let seed = match option_value(argv_tokens, "--seed") {
        Some(value) => value,
        None => return Some("statistical fixture argv does not declare --seed".to_string()),
    };
    let seed = match seed.parse::<u64>() {
        Ok(seed) => seed,
        Err(error) => return Some(format!("invalid --seed value {seed:?}: {error}")),
    };
    if seed != plan.fixed_seed {
        return Some(format!(
            "statistical plan fixed_seed={} does not match --seed {seed}",
            plan.fixed_seed
        ));
    }
    None
}

pub(super) fn qualification_plan_summary(
    id: &str,
    plan: &str,
    argv_tokens: &[String],
) -> Result<FixtureStatisticalPlanSummary, String> {
    if let Some(reason) = validate_statistical_plan(plan, argv_tokens) {
        return Err(reason);
    }
    let plan = parse_statistical_plan(plan)?;
    let shots = u64::try_from(plan.sample_count).map_err(|_| {
        format!(
            "statistical plan sample_count={} exceeds u64",
            plan.sample_count
        )
    })?;
    let buckets = match plan.tolerance {
        StatisticalTolerance::Binomial {
            expected_probability,
            sigma,
        } => vec![FixtureStatisticalBucketSummary {
            name: "1".to_string(),
            expected_probability,
            allowed_delta: sigma
                * binomial_standard_deviation(expected_probability, plan.sample_count),
        }],
        StatisticalTolerance::Buckets { expected, sigma } => expected
            .into_iter()
            .map(
                |(name, expected_probability)| FixtureStatisticalBucketSummary {
                    name,
                    expected_probability,
                    allowed_delta: sigma
                        * binomial_standard_deviation(expected_probability, plan.sample_count),
                },
            )
            .collect(),
    };
    Ok(FixtureStatisticalPlanSummary {
        id: id.to_string(),
        shots,
        primary_seed: plan.fixed_seed,
        buckets,
        declared_familywise_bound: plan.false_positive_rate,
        independent_comparisons_per_attempt: 2,
        shot_batches_per_attempt: 2,
        seed_override_executable: option_value(argv_tokens, "--seed").is_some(),
    })
}

fn compare_binomial_samples(
    text: &str,
    sample_count: usize,
    expected_probability: f64,
    sigma: f64,
) -> StatisticalEvaluation {
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
                return StatisticalEvaluation::incomplete(format!(
                    "statistical comparator expected one 0/1 bit per shot, got {line:?}"
                ));
            }
        }
    }
    if total == 0 {
        return StatisticalEvaluation::incomplete("statistical comparator received no samples");
    }
    if total != sample_count {
        return StatisticalEvaluation::incomplete(format!(
            "statistical comparator expected {sample_count} samples, got {total}",
        ));
    }
    let standard_deviation = binomial_standard_deviation(expected_probability, total);
    let allowed_delta = sigma * standard_deviation;
    let Ok(total_u64) = u64::try_from(total) else {
        return StatisticalEvaluation::incomplete(
            "statistical sample count exceeds the supported u64 range",
        );
    };
    let Some(accepted) =
        AcceptedCountRange::try_new(total_u64, expected_probability, allowed_delta)
    else {
        return StatisticalEvaluation::incomplete(
            "statistical comparator could not derive an accepted integer count range",
        );
    };
    let Ok(hits) = u64::try_from(hits) else {
        return StatisticalEvaluation::incomplete(
            "statistical hit count exceeds the supported u64 range",
        );
    };
    let reason = (!accepted.contains(hits)).then(|| {
        format!(
            "actual hit count {hits}/{total} is outside the accepted integer range {}..={} around expected {expected_probability:.6}",
            accepted.minimum(),
            accepted.maximum()
        )
    });
    StatisticalEvaluation::complete(sample_count, reason)
}

fn compare_bucket_samples(
    text: &str,
    sample_count: usize,
    expected: &[(String, f64)],
    sigma: f64,
) -> StatisticalEvaluation {
    let mut counts = expected
        .iter()
        .map(|(bucket, _)| (bucket.as_str(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut total = 0usize;
    for line in text.lines() {
        let Some(count) = counts.get_mut(line) else {
            return StatisticalEvaluation::incomplete(format!(
                "statistical comparator saw unplanned bucket {line:?}"
            ));
        };
        *count += 1;
        total += 1;
    }
    if total == 0 {
        return StatisticalEvaluation::incomplete("statistical comparator received no samples");
    }
    if total != sample_count {
        return StatisticalEvaluation::incomplete(format!(
            "statistical comparator expected {sample_count} samples, got {total}",
        ));
    }
    for (bucket, expected_probability) in expected {
        let count = counts.get(bucket.as_str()).copied().unwrap_or_default();
        let standard_deviation = binomial_standard_deviation(*expected_probability, total);
        let allowed_delta = sigma * standard_deviation;
        let Ok(total_u64) = u64::try_from(total) else {
            return StatisticalEvaluation::incomplete(
                "statistical sample count exceeds the supported u64 range",
            );
        };
        let Some(accepted) =
            AcceptedCountRange::try_new(total_u64, *expected_probability, allowed_delta)
        else {
            return StatisticalEvaluation::incomplete(format!(
                "statistical comparator could not derive an accepted integer count range for bucket {bucket:?}"
            ));
        };
        let Ok(count) = u64::try_from(count) else {
            return StatisticalEvaluation::incomplete(
                "statistical bucket count exceeds the supported u64 range",
            );
        };
        if !accepted.contains(count) {
            return StatisticalEvaluation::complete(
                sample_count,
                Some(format!(
                    "actual bucket {bucket:?} count {count}/{total} is outside the accepted integer range {}..={} around expected {expected_probability:.6}",
                    accepted.minimum(),
                    accepted.maximum()
                )),
            );
        }
    }
    StatisticalEvaluation::complete(sample_count, None)
}

fn binomial_standard_deviation(probability: f64, sample_count: usize) -> f64 {
    (probability * (1.0 - probability) / sample_count as f64).sqrt()
}

#[derive(Clone, Debug, PartialEq)]
struct StatisticalPlan {
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
    let mut sample_count = None;
    let mut fixed_seed = None;
    let mut false_positive_rate = None;
    let mut source = None;
    let mut tolerance = None;
    for token in plan.split(';').map(str::trim) {
        if token.is_empty() {
            continue;
        }
        if let Some(value) = token.strip_prefix("sample_count=") {
            let parsed = value
                .parse::<usize>()
                .map_err(|error| format!("invalid sample_count {value:?}: {error}"))?;
            if parsed == 0 {
                return Err("sample_count must be positive".to_string());
            }
            sample_count = Some(parsed);
            continue;
        }
        if let Some(value) = token.strip_prefix("fixed_seed=") {
            fixed_seed = Some(
                value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid fixed_seed {value:?}: {error}"))?,
            );
            continue;
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
            false_positive_rate = Some(parsed);
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
            tolerance = Some(StatisticalTolerance::Binomial {
                expected_probability: probability,
                sigma,
            });
            continue;
        }
        if let Some(rest) = token.strip_prefix("tolerate buckets ") {
            let (bucket_text, sigma) = parse_bucket_text_and_sigma(token, rest)?;
            tolerance = Some(StatisticalTolerance::Buckets {
                expected: parse_bucket_probabilities(bucket_text)?,
                sigma,
            });
            continue;
        }
        return Err(format!("unknown statistical plan token {token:?}"));
    }
    let false_positive_rate = false_positive_rate
        .ok_or_else(|| "statistical plan does not contain false_positive_rate".to_string())?;
    let tolerance = tolerance
        .ok_or_else(|| "statistical plan does not contain a statistical tolerance".to_string())?;
    validate_false_positive_budget(false_positive_rate, &tolerance)?;
    Ok(StatisticalPlan {
        sample_count: sample_count
            .ok_or_else(|| "statistical plan does not contain sample_count".to_string())?,
        fixed_seed: fixed_seed
            .ok_or_else(|| "statistical plan does not contain fixed_seed".to_string())?,
        false_positive_rate,
        source: source.unwrap_or(StatisticalSource::Stdout),
        tolerance,
    })
}

fn validate_false_positive_budget(
    false_positive_rate: f64,
    tolerance: &StatisticalTolerance,
) -> Result<(), String> {
    let (sigma, checked_rates) = match tolerance {
        StatisticalTolerance::Binomial { sigma, .. } => (*sigma, 1usize),
        StatisticalTolerance::Buckets { expected, sigma } => (*sigma, expected.len()),
    };
    let estimated_bound = (checked_rates as f64 * two_sided_normal_tail_bound(sigma)).min(1.0);
    if estimated_bound > false_positive_rate {
        return Err(format!(
            "false_positive_rate<={false_positive_rate} is tighter than the estimated {estimated_bound:.6e} false-positive bound from {sigma} sigma over {checked_rates} checked rate(s)"
        ));
    }
    Ok(())
}

fn two_sided_normal_tail_bound(sigma: f64) -> f64 {
    (std::f64::consts::FRAC_2_SQRT_PI / std::f64::consts::SQRT_2) * (-0.5 * sigma * sigma).exp()
        / sigma
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

fn parse_sigma(token: &str, sigma_text: &str, kind: &str) -> Result<f64, String> {
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
    let bucket_width = buckets
        .first()
        .map(|(bucket, _)| bucket.len())
        .unwrap_or_default();
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

fn option_value<'a>(tokens: &'a [String], option: &str) -> Option<&'a str> {
    let with_equals = format!("{option}=");
    for (index, token) in tokens.iter().enumerate() {
        if token == option {
            return tokens.get(index + 1).map(String::as_str);
        }
        if let Some(value) = token.strip_prefix(&with_equals) {
            return Some(value);
        }
    }
    None
}

#[cfg(test)]
mod qualification_tests {
    use super::*;

    #[test]
    fn qualification_summary_preserves_fixture_budget_and_exact_inputs() {
        let argv = [
            "sample".to_string(),
            "--shots".to_string(),
            "4000".to_string(),
            "--seed".to_string(),
            "17".to_string(),
        ];
        let plan = "sample_count=4000; fixed_seed=17; tolerate buckets 00=0.75,01=0.25 within 5 sigma; false_positive_rate<=0.001";
        let summary = qualification_plan_summary("fixture-plan", plan, &argv)
            .expect("valid fixture qualification plan");

        assert_eq!(summary.id, "fixture-plan");
        assert_eq!(summary.shots, 4_000);
        assert_eq!(summary.primary_seed, 17);
        assert_eq!(summary.declared_familywise_bound, 0.001);
        assert!(summary.seed_override_executable);
        assert_eq!(summary.buckets.len(), 2);
        let first = summary.buckets.first().expect("first bucket");
        assert_eq!(first.name, "00");
        assert_eq!(first.expected_probability, 0.75);
        assert!(first.allowed_delta > 0.0);
    }
}
