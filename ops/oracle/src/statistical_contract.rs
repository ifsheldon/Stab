//! Count tests shared by the one-sample and two-sample fixture comparators.

/// A one-sample tolerance: `count / shots` must lie within `allowed_delta` of
/// `expected_probability`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CountTolerance {
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
}

impl CountTolerance {
    /// Returns `None` for zero shots or a non-finite or negative tolerance.
    pub(crate) fn try_new(
        shots: u64,
        expected_probability: f64,
        allowed_delta: f64,
    ) -> Option<Self> {
        (shots > 0
            && expected_probability.is_finite()
            && allowed_delta.is_finite()
            && allowed_delta >= 0.0)
            .then_some(Self {
                shots,
                expected_probability,
                allowed_delta,
            })
    }

    pub(crate) fn accepts(self, count: u64) -> bool {
        count <= self.shots
            && (count as f64 / self.shots as f64 - self.expected_probability).abs()
                <= self.allowed_delta
    }

    /// The accepted counts for diagnostics, as `expected ± delta` in counts.
    pub(crate) fn describe(self) -> String {
        let shots = self.shots as f64;
        format!(
            "accepted count {:.1} +/- {:.1} of {} (p={:.6})",
            self.expected_probability * shots,
            self.allowed_delta * shots,
            self.shots,
            self.expected_probability
        )
    }
}

/// Mills-ratio upper bound on the two-sided standard normal tail beyond `sigma`, used as the
/// per-test false-positive rate when a plan's Bonferroni budget is validated.
pub(crate) fn two_sided_normal_tail_bound(sigma: f64) -> f64 {
    (std::f64::consts::FRAC_2_SQRT_PI / std::f64::consts::SQRT_2) * (-0.5 * sigma * sigma).exp()
        / sigma
}

/// Outcome of comparing two equal-size samples' hit counts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TwoProportionOutcome {
    /// Both samples observed the same count, including the zero-variance case where both are
    /// constant at 0 or at `shots`.
    Identical,
    /// One sample is constantly 0 and the other constantly 1, which no sigma tolerance excuses.
    DeterministicDisagreement,
    /// The counts differ and neither is constant: the pooled two-proportion z score.
    Scored { z: f64 },
    /// Exactly one sample is constant (count 0 or `shots`), where the pooled z score has little
    /// power: the two-sided p-value of Fisher's exact conditional test.
    Exact { p_value: f64 },
}

impl TwoProportionOutcome {
    /// Whether the outcome rejects equality at `sigma`, whose per-test false-positive rate is
    /// `per_test_alpha` (the bound the plan's budget was validated against).
    pub(crate) fn rejects(self, sigma: f64, per_test_alpha: f64) -> bool {
        match self {
            Self::Identical => false,
            Self::DeterministicDisagreement => true,
            Self::Scored { z } => z.abs() > sigma,
            Self::Exact { p_value } => p_value < per_test_alpha,
        }
    }
}

/// Compares `first` and `second` hits, each out of `shots`.
///
/// Returns `None` when `shots` is zero or either count exceeds `shots`.
pub(crate) fn two_proportion_outcome(
    first: u64,
    second: u64,
    shots: u64,
) -> Option<TwoProportionOutcome> {
    if shots == 0 || first > shots || second > shots {
        return None;
    }
    if first == second {
        return Some(TwoProportionOutcome::Identical);
    }
    if first.min(second) == 0 && first.max(second) == shots {
        return Some(TwoProportionOutcome::DeterministicDisagreement);
    }
    if first.min(second) == 0 {
        return Some(TwoProportionOutcome::Exact {
            p_value: one_side_constant_p_value(first.max(second), shots),
        });
    }
    if first.max(second) == shots {
        // Count misses instead: the side at `shots` never misses.
        return Some(TwoProportionOutcome::Exact {
            p_value: one_side_constant_p_value(shots - first.min(second), shots),
        });
    }
    let shots = shots as f64;
    let pooled = (first as f64 + second as f64) / (2.0 * shots);
    let standard_error = (pooled * (1.0 - pooled) * 2.0 / shots).sqrt();
    let z = (first as f64 - second as f64) / shots / standard_error;
    Some(TwoProportionOutcome::Scored { z })
}

/// Two-sided Fisher exact p-value when one of two `shots`-size samples has 0 events and the other
/// has `events` (`0 < events < shots`).
///
/// Conditioned on the `events` total, the first sample's count is hypergeometric, and with equal
/// sample sizes that distribution is symmetric, so the two-sided p-value is twice
/// `P(X = 0) = C(shots, events) / C(2 shots, events) = prod_{i < events} (shots - i) / (2 shots - i)`,
/// accumulated in log space.
fn one_side_constant_p_value(events: u64, shots: u64) -> f64 {
    let shots = shots as f64;
    let mut log_probability = 0.0f64;
    let mut index = 0.0f64;
    let mut remaining = events;
    while remaining > 0 {
        log_probability += ((shots - index) / (2.0 * shots - index)).ln();
        index += 1.0;
        remaining -= 1;
    }
    (2.0 * log_probability.exp()).min(1.0)
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::{CountTolerance, TwoProportionOutcome, two_proportion_outcome};

    #[test]
    fn count_tolerance_accepts_exactly_the_counts_within_the_delta() {
        let tolerance = CountTolerance::try_new(100, 0.25, 0.05).expect("tolerance");

        assert!(!tolerance.accepts(19));
        assert!(tolerance.accepts(20));
        assert!(tolerance.accepts(30));
        assert!(!tolerance.accepts(31));
        assert!(!tolerance.accepts(101));
        assert_eq!(CountTolerance::try_new(0, 0.25, 0.05), None);
    }

    #[test]
    fn two_proportion_outcome_scores_pooled_difference_and_zero_variance_edges() {
        // Pooled p = 0.525, standard error = sqrt(0.525 * 0.475 * 2 / 20000) ~= 0.0049937.
        let outcome = two_proportion_outcome(11_000, 10_000, 20_000);
        assert_matches!(
            outcome,
            Some(TwoProportionOutcome::Scored { z }) if (z - 0.05 / 0.004_993_746).abs() < 1e-3
        );
        assert_eq!(
            two_proportion_outcome(0, 0, 20),
            Some(TwoProportionOutcome::Identical)
        );
        assert_eq!(
            two_proportion_outcome(20, 20, 20),
            Some(TwoProportionOutcome::Identical)
        );
        assert_eq!(
            two_proportion_outcome(0, 20, 20),
            Some(TwoProportionOutcome::DeterministicDisagreement)
        );
        assert_eq!(two_proportion_outcome(21, 0, 20), None);
        assert_eq!(two_proportion_outcome(0, 0, 0), None);
    }

    #[test]
    fn one_constant_side_uses_the_exact_conditional_test() {
        // 6 sigma two-sided normal tail bound, the per-test alpha of a 6-sigma plan.
        let alpha = super::two_sided_normal_tail_bound(6.0);
        let p_value = |first, second| {
            let outcome = two_proportion_outcome(first, second, 20_000);
            let Some(TwoProportionOutcome::Exact { p_value }) = outcome else {
                unreachable!("expected an exact outcome, got {outcome:?}");
            };
            p_value
        };

        // The pooled z score is about sqrt(k), so it accepted up to ~36 events against none.
        // Conditionally, P = 2 * prod (n - i) / (2n - i) ~= 2^(1 - k).
        assert!((p_value(0, 3) - 0.25).abs() < 1e-3);
        assert!(p_value(0, 35) < alpha);
        assert!(p_value(31, 0) < alpha);
        assert!(p_value(0, 28) > alpha);
        // The constant-at-shots side is tested through its misses.
        assert!(p_value(20_000, 19_965) < alpha);
        assert!(p_value(19_997, 20_000) > alpha);
        assert!(
            two_proportion_outcome(0, 3, 20_000)
                .is_some_and(|outcome| !outcome.rejects(6.0, alpha))
        );
        assert!(
            two_proportion_outcome(0, 35, 20_000)
                .is_some_and(|outcome| outcome.rejects(6.0, alpha))
        );
    }
}
