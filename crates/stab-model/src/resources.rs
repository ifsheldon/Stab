/// Whether a resource estimate is exact, an upper bound, or unavailable.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EstimateClass {
    Exact,
    UpperBound,
    Unknown,
}

/// A resource quantity together with the strength of the estimate.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum Estimate<T> {
    Exact(T),
    UpperBound(T),
    #[default]
    Unknown,
}

impl<T> Estimate<T> {
    pub const fn class(&self) -> EstimateClass {
        match self {
            Self::Exact(_) => EstimateClass::Exact,
            Self::UpperBound(_) => EstimateClass::UpperBound,
            Self::Unknown => EstimateClass::Unknown,
        }
    }

    pub const fn value(&self) -> Option<&T> {
        match self {
            Self::Exact(value) | Self::UpperBound(value) => Some(value),
            Self::Unknown => None,
        }
    }
}

/// Cheap resource information collected without executing the described operation.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ResourceEstimate {
    input_bytes: Estimate<usize>,
    input_items: Estimate<usize>,
    expanded_operations: Estimate<usize>,
    folded_traversal: Estimate<usize>,
    scratch_bytes: Estimate<usize>,
    resident_bytes: Estimate<usize>,
    output_bytes: Estimate<usize>,
    work_units: Estimate<usize>,
}

/// Builds a [`ResourceEstimate`] by naming only the quantities known to the caller.
#[must_use = "resource estimate builders must be finished with build"]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ResourceEstimateBuilder {
    estimate: ResourceEstimate,
}

impl ResourceEstimate {
    const UNKNOWN: Self = Self {
        input_bytes: Estimate::Unknown,
        input_items: Estimate::Unknown,
        expanded_operations: Estimate::Unknown,
        folded_traversal: Estimate::Unknown,
        scratch_bytes: Estimate::Unknown,
        resident_bytes: Estimate::Unknown,
        output_bytes: Estimate::Unknown,
        work_units: Estimate::Unknown,
    };

    /// Starts a domain-neutral estimate with every quantity classified as unknown.
    pub const fn builder() -> ResourceEstimateBuilder {
        ResourceEstimateBuilder {
            estimate: Self::UNKNOWN,
        }
    }

    pub(crate) fn for_text_parse(input: &str) -> Self {
        Self::for_model_bytes(input.as_bytes())
    }

    pub(crate) fn for_model_bytes(input: &[u8]) -> Self {
        let physical_lines = if input.is_empty() {
            0
        } else {
            input.iter().filter(|byte| **byte == b'\n').count()
                + usize::from(input.last() != Some(&b'\n'))
        };
        Self {
            input_bytes: Estimate::Exact(input.len()),
            input_items: Estimate::Exact(physical_lines),
            ..Self::default()
        }
    }

    pub const fn input_bytes(&self) -> Estimate<usize> {
        self.input_bytes
    }

    pub const fn input_items(&self) -> Estimate<usize> {
        self.input_items
    }

    pub const fn expanded_operations(&self) -> Estimate<usize> {
        self.expanded_operations
    }

    pub const fn folded_traversal(&self) -> Estimate<usize> {
        self.folded_traversal
    }

    pub const fn scratch_bytes(&self) -> Estimate<usize> {
        self.scratch_bytes
    }

    pub const fn resident_bytes(&self) -> Estimate<usize> {
        self.resident_bytes
    }

    pub const fn output_bytes(&self) -> Estimate<usize> {
        self.output_bytes
    }

    pub const fn work_units(&self) -> Estimate<usize> {
        self.work_units
    }
}

impl ResourceEstimateBuilder {
    pub const fn input_bytes(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.input_bytes = estimate;
        self
    }

    pub const fn input_items(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.input_items = estimate;
        self
    }

    pub const fn expanded_operations(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.expanded_operations = estimate;
        self
    }

    pub const fn folded_traversal(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.folded_traversal = estimate;
        self
    }

    pub const fn scratch_bytes(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.scratch_bytes = estimate;
        self
    }

    pub const fn resident_bytes(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.resident_bytes = estimate;
        self
    }

    pub const fn output_bytes(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.output_bytes = estimate;
        self
    }

    pub const fn work_units(mut self, estimate: Estimate<usize>) -> Self {
        self.estimate.work_units = estimate;
        self
    }

    #[must_use]
    pub const fn build(self) -> ResourceEstimate {
        self.estimate
    }
}
