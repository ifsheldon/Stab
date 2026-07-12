use std::collections::BTreeSet;

use statrs::distribution::{Binomial, DiscreteCDF as _};

use super::{
    BlockerCase, ComparatorKind, StatisticalPlan, validate_display_text,
    validate_gate_statistical_plan,
};

pub(super) fn validate_statistical_plan(
    case: &BlockerCase,
    evaluate_false_positive_budget: bool,
    violations: &mut Vec<String>,
) {
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
            if evaluate_false_positive_budget {
                validate_statistical_false_positive_budget(case, plan, violations);
            }
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
    let Ok(distribution) = Binomial::new(probability, shots) else {
        return 1.0;
    };

    let (lower_max, upper_min) = stab_core::__gate_contract_statistical_rejection_boundaries(
        shots,
        probability,
        allowed_delta,
    );
    lower_max
        .map(|boundary| distribution.cdf(boundary))
        .unwrap_or(0.0)
        + upper_min
            .map(|boundary| {
                boundary
                    .checked_sub(1)
                    .map(|excluded_max| distribution.sf(excluded_max))
                    .unwrap_or(1.0)
            })
            .unwrap_or(0.0)
}
