use std::collections::BTreeSet;

use super::{
    BlockerCase, ComparatorKind, StatisticalPlan, validate_display_text,
    validate_gate_statistical_plan,
};

pub(super) fn validate_statistical_plan(case: &BlockerCase, violations: &mut Vec<String>) {
    match (case.comparator, &case.statistical_plan) {
        (ComparatorKind::Statistical, Some(plan)) => {
            if !(10_000..=10_000_000).contains(&plan.shots) {
                violations.push(format!(
                    "case {:?} statistical shots must be within 10000..=10000000",
                    case.id
                ));
            }
            if !(1.0..=8.0).contains(&plan.sigma_multiplier) {
                violations.push(format!(
                    "case {:?} statistical sigma multiplier must be within [1, 8]",
                    case.id
                ));
            }
            if !(0.0..=0.05).contains(&plan.absolute_probability_floor) {
                violations.push(format!(
                    "case {:?} statistical absolute probability floor must be within [0, 0.05]",
                    case.id
                ));
            }
            if !(0.0..=0.0001).contains(&plan.familywise_false_positive_budget)
                || plan.familywise_false_positive_budget == 0.0
            {
                violations.push(format!(
                    "case {:?} statistical false-positive budget must be within (0, 0.0001]",
                    case.id
                ));
            }
            if !(2..=32).contains(&plan.buckets.len()) {
                violations.push(format!(
                    "case {:?} statistical plan must name 2..=32 buckets",
                    case.id
                ));
            }
            let mut buckets = BTreeSet::new();
            for bucket in &plan.buckets {
                validate_display_text("statistical bucket", &bucket.name, violations);
                if !buckets.insert(&bucket.name) {
                    violations.push(format!(
                        "case {:?} repeats statistical bucket {:?}",
                        case.id, bucket.name
                    ));
                }
                if !bucket.expected_probability.is_finite()
                    || !(0.0..=1.0).contains(&bucket.expected_probability)
                {
                    violations.push(format!(
                        "case {:?} statistical bucket {:?} probability {} is outside [0, 1]",
                        case.id, bucket.name, bucket.expected_probability
                    ));
                }
            }
            validate_statistical_false_positive_budget(case, plan, violations);
            validate_gate_statistical_plan(case, plan, violations);
        }
        (ComparatorKind::Statistical, None) => violations.push(format!(
            "case {:?} statistical comparator lacks a reproducible plan",
            case.id
        )),
        (_, Some(_)) => violations.push(format!(
            "case {:?} has a statistical plan but does not use a statistical comparator",
            case.id
        )),
        (_, None) => {}
    }
}

fn validate_statistical_false_positive_budget(
    case: &BlockerCase,
    plan: &StatisticalPlan,
    violations: &mut Vec<String>,
) {
    if plan.shots == 0
        || !plan.sigma_multiplier.is_finite()
        || !plan.absolute_probability_floor.is_finite()
        || !plan.familywise_false_positive_budget.is_finite()
    {
        return;
    }
    let mut familywise_bound = 0.0;
    for bucket in &plan.buckets {
        if !bucket.expected_probability.is_finite()
            || !(0.0..=1.0).contains(&bucket.expected_probability)
        {
            continue;
        }
        let standard_deviation =
            (bucket.expected_probability * (1.0 - bucket.expected_probability) / plan.shots as f64)
                .sqrt();
        let allowed_delta = plan
            .absolute_probability_floor
            .max(plan.sigma_multiplier * standard_deviation);
        familywise_bound +=
            binomial_rejection_probability(plan.shots, bucket.expected_probability, allowed_delta);
    }
    if familywise_bound > plan.familywise_false_positive_budget {
        violations.push(format!(
            "case {:?} exact binomial familywise rejection probability {familywise_bound:.6e} exceeds declared budget {:.6e}",
            case.id, plan.familywise_false_positive_budget
        ));
    }
}

pub(super) fn binomial_rejection_probability(
    shots: u64,
    probability: f64,
    allowed_delta: f64,
) -> f64 {
    if shots == 0 || allowed_delta >= 1.0 {
        return 0.0;
    }
    if !allowed_delta.is_finite() || !probability.is_finite() {
        return 1.0;
    }
    if probability == 0.0 || probability == 1.0 {
        return 0.0;
    }
    let Ok(bounded_shots) = u32::try_from(shots) else {
        return 1.0;
    };

    let lower_boundary = probability - allowed_delta;
    let upper_boundary = probability + allowed_delta;
    let lower_max = if lower_boundary <= 0.0 {
        None
    } else {
        Some(u64::from(last_count_below(bounded_shots, lower_boundary)))
    };
    let upper_min = if upper_boundary >= 1.0 {
        None
    } else {
        Some(u64::from(first_count_above(bounded_shots, upper_boundary)))
    };
    lower_max
        .map(|boundary| binomial_lower_tail(shots, probability, boundary))
        .unwrap_or(0.0)
        + upper_min
            .map(|boundary| binomial_upper_tail(shots, probability, boundary))
            .unwrap_or(0.0)
}

fn last_count_below(shots: u32, probability: f64) -> u32 {
    let mut low = 0;
    let mut high = shots;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if f64::from(middle) / f64::from(shots) < probability {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

fn first_count_above(shots: u32, probability: f64) -> u32 {
    let mut low = 0;
    let mut high = shots;
    while low < high {
        let middle = low + (high - low) / 2;
        if f64::from(middle) / f64::from(shots) > probability {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn binomial_lower_tail(shots: u64, probability: f64, boundary: u64) -> f64 {
    let mut term = binomial_probability_mass(shots, probability, boundary);
    let mut sum = term;
    for k in (1..=boundary).rev() {
        term *= k as f64 / (shots - k + 1) as f64 * (1.0 - probability) / probability;
        sum += term;
        if term <= sum * f64::EPSILON {
            break;
        }
    }
    sum.min(1.0)
}

fn binomial_upper_tail(shots: u64, probability: f64, boundary: u64) -> f64 {
    let mut term = binomial_probability_mass(shots, probability, boundary);
    let mut sum = term;
    for k in boundary..shots {
        term *= (shots - k) as f64 / (k + 1) as f64 * probability / (1.0 - probability);
        sum += term;
        if term <= sum * f64::EPSILON {
            break;
        }
    }
    sum.min(1.0)
}

fn binomial_probability_mass(shots: u64, probability: f64, successes: u64) -> f64 {
    let smaller_side = successes.min(shots - successes);
    let mut log_coefficient = 0.0;
    for offset in 1..=smaller_side {
        log_coefficient += ((shots - smaller_side + offset) as f64).ln() - (offset as f64).ln();
    }
    (log_coefficient
        + successes as f64 * probability.ln()
        + (shots - successes) as f64 * (-probability).ln_1p())
    .exp()
}
