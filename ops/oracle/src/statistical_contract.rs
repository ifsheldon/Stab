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
            stab_core::__gate_contract_statistical_rejection_boundaries(
                shots,
                expected_probability,
                allowed_delta,
            );
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

#[cfg(test)]
mod tests {
    use super::AcceptedCountRange;

    #[test]
    fn accepted_count_range_matches_the_core_integer_boundary_contract() {
        let range = AcceptedCountRange::try_new(100, 0.25, 0.05).expect("accepted range");
        let (lower, upper) =
            stab_core::__gate_contract_statistical_rejection_boundaries(100, 0.25, 0.05);

        assert_eq!(range.minimum(), lower.map_or(0, |value| value + 1));
        assert_eq!(range.maximum(), upper.map_or(100, |value| value - 1));
        assert!(range.contains(range.minimum()));
        assert!(range.contains(range.maximum()));
    }
}
