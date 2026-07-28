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

impl ResourceEstimate {
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

    pub(crate) const fn for_sampling_request(
        input_items: Estimate<usize>,
        expanded_operations: Estimate<usize>,
        folded_traversal: Estimate<usize>,
        output_bytes: Estimate<usize>,
    ) -> Self {
        Self {
            input_items,
            expanded_operations,
            folded_traversal,
            output_bytes,
            input_bytes: Estimate::Unknown,
            scratch_bytes: Estimate::Unknown,
            resident_bytes: Estimate::Unknown,
            work_units: Estimate::Unknown,
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
