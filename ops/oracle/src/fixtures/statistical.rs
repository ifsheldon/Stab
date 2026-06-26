pub(super) fn compare_binomial_statistical_plan(plan: &str, stdout: &[u8]) -> Option<String> {
    let (expected_probability, sigma) = match parse_binomial_plan(plan) {
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
    let actual_probability = hits as f64 / total as f64;
    let standard_deviation =
        (expected_probability * (1.0 - expected_probability) / total as f64).sqrt();
    let allowed_delta = sigma * standard_deviation;
    if (actual_probability - expected_probability).abs() > allowed_delta {
        return Some(format!(
            "actual hit rate {actual_probability:.6} from {hits}/{total} samples is outside {sigma} sigma around expected {expected_probability:.6}"
        ));
    }
    None
}

fn parse_binomial_plan(plan: &str) -> Result<(f64, f64), String> {
    for token in plan.split(';').map(str::trim) {
        let Some(rest) = token.strip_prefix("tolerate binomial p=") else {
            continue;
        };
        let Some((probability, sigma)) = rest.split_once(" within ") else {
            return Err(format!("invalid binomial statistical plan token {token:?}"));
        };
        let Some(sigma) = sigma.strip_suffix(" sigma") else {
            return Err(format!(
                "invalid binomial statistical sigma token {token:?}"
            ));
        };
        let probability = probability
            .parse::<f64>()
            .map_err(|error| format!("invalid binomial probability {probability:?}: {error}"))?;
        let sigma = sigma
            .parse::<f64>()
            .map_err(|error| format!("invalid binomial sigma {sigma:?}: {error}"))?;
        if !(0.0..=1.0).contains(&probability) {
            return Err(format!(
                "binomial probability {probability} is outside the closed unit interval"
            ));
        }
        if !sigma.is_finite() || sigma <= 0.0 {
            return Err(format!(
                "binomial sigma {sigma} must be positive and finite"
            ));
        }
        return Ok((probability, sigma));
    }
    Err("statistical plan does not contain a binomial tolerance".to_string())
}
