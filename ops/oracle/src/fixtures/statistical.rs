pub(super) fn compare_binomial_statistical_plan(plan: &str, stdout: &[u8]) -> Option<String> {
    let plan = match parse_binomial_plan(plan) {
        Ok(plan) => plan,
        Err(reason) => return Some(reason),
    };
    let text = match std::str::from_utf8(stdout) {
        Ok(text) => text,
        Err(error) => return Some(format!("statistical stdout is not UTF-8: {error}")),
    };
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
    if total != plan.sample_count {
        return Some(format!(
            "statistical comparator expected {} samples, got {total}",
            plan.sample_count
        ));
    }
    let actual_probability = hits as f64 / total as f64;
    let standard_deviation =
        (plan.expected_probability * (1.0 - plan.expected_probability) / total as f64).sqrt();
    let allowed_delta = plan.sigma * standard_deviation;
    if (actual_probability - plan.expected_probability).abs() > allowed_delta {
        return Some(format!(
            "actual hit rate {actual_probability:.6} from {hits}/{total} samples is outside {} sigma around expected {:.6}",
            plan.sigma, plan.expected_probability
        ));
    }
    None
}

pub(super) fn validate_binomial_statistical_plan(
    plan: &str,
    argv_tokens: &[String],
) -> Option<String> {
    let plan = match parse_binomial_plan(plan) {
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct BinomialPlan {
    expected_probability: f64,
    sigma: f64,
    sample_count: usize,
    fixed_seed: u64,
}

fn parse_binomial_plan(plan: &str) -> Result<BinomialPlan, String> {
    let mut expected_probability = None;
    let mut sigma = None;
    let mut sample_count = None;
    let mut fixed_seed = None;
    let mut false_positive_rate = None;
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
            if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                return Err(format!(
                    "false_positive_rate {parsed} is outside the closed unit interval"
                ));
            }
            false_positive_rate = Some(parsed);
            continue;
        }
        let Some(rest) = token.strip_prefix("tolerate binomial p=") else {
            return Err(format!("unknown statistical plan token {token:?}"));
        };
        let Some((probability, sigma_text)) = rest.split_once(" within ") else {
            return Err(format!("invalid binomial statistical plan token {token:?}"));
        };
        let Some(sigma_text) = sigma_text.strip_suffix(" sigma") else {
            return Err(format!(
                "invalid binomial statistical sigma token {token:?}"
            ));
        };
        let probability = probability
            .parse::<f64>()
            .map_err(|error| format!("invalid binomial probability {probability:?}: {error}"))?;
        let parsed_sigma = sigma_text
            .parse::<f64>()
            .map_err(|error| format!("invalid binomial sigma {sigma_text:?}: {error}"))?;
        if !(0.0..=1.0).contains(&probability) {
            return Err(format!(
                "binomial probability {probability} is outside the closed unit interval"
            ));
        }
        if !parsed_sigma.is_finite() || parsed_sigma <= 0.0 {
            return Err(format!(
                "binomial sigma {parsed_sigma} must be positive and finite"
            ));
        }
        expected_probability = Some(probability);
        sigma = Some(parsed_sigma);
    }
    false_positive_rate
        .ok_or_else(|| "statistical plan does not contain false_positive_rate".to_string())?;
    Ok(BinomialPlan {
        expected_probability: expected_probability
            .ok_or_else(|| "statistical plan does not contain a binomial tolerance".to_string())?,
        sigma: sigma.ok_or_else(|| "statistical plan does not contain a sigma".to_string())?,
        sample_count: sample_count
            .ok_or_else(|| "statistical plan does not contain sample_count".to_string())?,
        fixed_seed: fixed_seed
            .ok_or_else(|| "statistical plan does not contain fixed_seed".to_string())?,
    })
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
