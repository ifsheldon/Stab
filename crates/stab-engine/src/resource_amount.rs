use std::fmt::{Display, Formatter};

/// Resource amount reported exactly when it fits `u64`, or as an explicit lower bound otherwise.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceAmount {
    value: u64,
    lower_bound: bool,
}

impl ResourceAmount {
    pub const fn exact(value: u64) -> Self {
        Self {
            value,
            lower_bound: false,
        }
    }

    /// The exact amount when it fits `u64`, otherwise `u64::MAX` as a lower bound.
    pub(crate) fn saturating_from_u128(value: u128) -> Self {
        u64::try_from(value).map_or(
            Self {
                value: u64::MAX,
                lower_bound: true,
            },
            Self::exact,
        )
    }

    pub const fn value(self) -> u64 {
        self.value
    }

    pub const fn is_lower_bound(self) -> bool {
        self.lower_bound
    }
}

impl Display for ResourceAmount {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if self.lower_bound {
            write!(formatter, "at least {}", self.value)
        } else {
            self.value.fmt(formatter)
        }
    }
}
