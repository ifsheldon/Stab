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

#[cfg(test)]
mod tests {
    use super::{AcceptedCountRange, rejection_boundaries};

    #[test]
    fn accepted_count_range_uses_exact_integer_boundaries() {
        let range = AcceptedCountRange::try_new(100, 0.25, 0.05).expect("accepted range");

        assert_eq!(rejection_boundaries(100, 0.25, 0.05), (Some(19), Some(31)));
        assert_eq!(range.minimum(), 20);
        assert_eq!(range.maximum(), 30);
        assert!(range.contains(range.minimum()));
        assert!(range.contains(range.maximum()));
    }
}
