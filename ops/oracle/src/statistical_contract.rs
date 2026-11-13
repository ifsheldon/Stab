#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AcceptedCountRange {
    minimum: u64,
    maximum: u64,
}

impl AcceptedCountRange {
    pub(crate) fn try_new(
        shots: u64,
        expected_probability: f64,
        allowed_delta: f64,
    ) -> Option<Self> {
        let (lower_rejected, upper_rejected) =
            rejection_boundaries(shots, expected_probability, allowed_delta);
        let minimum = match lower_rejected {
            Some(value) => value.checked_add(1)?,
            None => 0,
        };
        let maximum = match upper_rejected {
            Some(value) => value.checked_sub(1)?,
            None => shots,
        };
        (minimum <= maximum && maximum <= shots).then_some(Self { minimum, maximum })
    }

    pub(crate) const fn minimum(self) -> u64 {
        self.minimum
    }

    pub(crate) const fn maximum(self) -> u64 {
        self.maximum
    }

    pub(crate) const fn contains(self, count: u64) -> bool {
        self.minimum <= count && count <= self.maximum
    }
}

pub(crate) fn count_is_accepted(
    count: u64,
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
) -> bool {
    if shots == 0 || count > shots {
        return false;
    }
    let observed = count as f64 / shots as f64;
    (observed - expected_probability).abs() <= allowed_delta
}

pub(crate) fn rejection_boundaries(
    shots: u64,
    expected_probability: f64,
    allowed_delta: f64,
) -> (Option<u64>, Option<u64>) {
    if shots == 0
        || !expected_probability.is_finite()
        || !allowed_delta.is_finite()
        || allowed_delta < 0.0
    {
        return (None, None);
    }
    let rejected = |count| !count_is_accepted(count, shots, expected_probability, allowed_delta);

    let lower_max = rejected(0).then(|| {
        let mut low = 0;
        let mut high = shots;
        while low < high {
            let middle = low + (high - low).div_ceil(2);
            if rejected(middle) && middle as f64 / shots as f64 <= expected_probability {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        low
    });
    let upper_min = rejected(shots).then(|| {
        let mut low = 0;
        let mut high = shots;
        while low < high {
            let middle = low + (high - low) / 2;
            if rejected(middle) && middle as f64 / shots as f64 >= expected_probability {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    });
    (lower_max, upper_min)
}

/// Outcome of a pooled two-proportion z-test between two equal-size samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum TwoProportionOutcome {
    /// Both samples observed the same count, including the zero-variance case where both are
    /// constant at 0 or at `shots`.
    Identical,
    /// One sample is constantly 0 and the other constantly 1, which no sigma tolerance excuses.
    DeterministicDisagreement,
    /// The counts differ and the pooled standard error is positive.
    Scored { z: f64 },
}

/// Scores `first` and `second` hits, each out of `shots`, with the pooled two-proportion z-test.
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
    let shots = shots as f64;
    let pooled = (first as f64 + second as f64) / (2.0 * shots);
    let standard_error = (pooled * (1.0 - pooled) * 2.0 / shots).sqrt();
    let z = (first as f64 - second as f64) / shots / standard_error;
    Some(TwoProportionOutcome::Scored { z })
}

#[cfg(test)]
mod tests {
    use super::{
        AcceptedCountRange, TwoProportionOutcome, rejection_boundaries, two_proportion_outcome,
    };

    #[test]
    fn accepted_count_range_uses_exact_integer_boundaries() {
        let range = AcceptedCountRange::try_new(100, 0.25, 0.05).expect("accepted range");

        assert_eq!(rejection_boundaries(100, 0.25, 0.05), (Some(19), Some(31)));
        assert_eq!(range.minimum(), 20);
        assert_eq!(range.maximum(), 30);
        assert!(range.contains(range.minimum()));
        assert!(range.contains(range.maximum()));
    }

    #[test]
    fn two_proportion_outcome_scores_pooled_difference_and_zero_variance_edges() {
        // Pooled p = 0.525, standard error = sqrt(0.525 * 0.475 * 2 / 20000) ~= 0.0049937.
        let outcome = two_proportion_outcome(11_000, 10_000, 20_000);
        assert!(
            matches!(
                outcome,
                Some(TwoProportionOutcome::Scored { z }) if (z - 0.05 / 0.004_993_746).abs() < 1e-3
            ),
            "{outcome:?}"
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
}
