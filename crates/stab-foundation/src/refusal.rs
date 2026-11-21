use std::fmt::{Display, Formatter};

use crate::MemoryBudget;

/// A resource amount reported exactly when it fits `u64`, or as an explicit lower bound
/// otherwise.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ResourceAmount {
    value: u64,
    lower_bound: bool,
}

impl ResourceAmount {
    /// Exactly `value`.
    pub const fn exact(value: u64) -> Self {
        Self {
            value,
            lower_bound: false,
        }
    }

    /// The exact amount when it fits `u64`, otherwise `u64::MAX` as a lower bound.
    pub fn saturating_from_u128(value: u128) -> Self {
        u64::try_from(value).map_or(
            Self {
                value: u64::MAX,
                lower_bound: true,
            },
            Self::exact,
        )
    }

    /// This amount plus `bytes`, a lower bound when either this amount is one or the sum does
    /// not fit `u64`.
    pub const fn saturating_add(self, bytes: u64) -> Self {
        match self.value.checked_add(bytes) {
            Some(value) => Self {
                value,
                lower_bound: self.lower_bound,
            },
            None => Self {
                value: u64::MAX,
                lower_bound: true,
            },
        }
    }

    /// The amount, or its lower bound when [`Self::is_lower_bound`].
    pub const fn value(self) -> u64 {
        self.value
    }

    /// Whether the true amount is larger than [`Self::value`] can represent.
    pub const fn is_lower_bound(self) -> bool {
        self.lower_bound
    }
}

impl Display for ResourceAmount {
    /// Writes the value, prefixed with `at least` when it is a lower bound.
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if self.lower_bound {
            write!(formatter, "at least {}", self.value)
        } else {
            self.value.fmt(formatter)
        }
    }
}

/// A memory budget refused a conservative storage requirement before anything was allocated.
///
/// This value records only the fact: how much was required and which budget refused it. The
/// component that refused the work keeps its own typed description of what it was building, and
/// every error type that can carry this refusal answers it through a `memory_budget_exceeded`
/// accessor, so a caller can tell that raising the budget admits the work without reading
/// message text. Limits that are not memory budgets, such as fixed nesting or count caps, are
/// never reported with this value.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MemoryBudgetExceeded {
    required: ResourceAmount,
    budget: MemoryBudget,
}

impl MemoryBudgetExceeded {
    /// `budget` refused a requirement of `required` bytes.
    pub const fn new(required: ResourceAmount, budget: MemoryBudget) -> Self {
        Self { required, budget }
    }

    /// The conservative number of bytes the refused work required.
    pub const fn required(self) -> ResourceAmount {
        self.required
    }

    /// The budget that refused the work.
    pub const fn budget(self) -> MemoryBudget {
        self.budget
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryBudgetExceeded, ResourceAmount};
    use crate::MemoryBudget;

    #[test]
    fn amounts_beyond_u64_saturate_to_an_explicit_lower_bound() {
        let exact = ResourceAmount::saturating_from_u128(u128::from(u64::MAX));
        assert!(!exact.is_lower_bound());
        assert_eq!(exact.to_string(), u64::MAX.to_string());
        let bound = ResourceAmount::saturating_from_u128(u128::from(u64::MAX) + 1);
        assert!(bound.is_lower_bound());
        assert_eq!(bound.value(), u64::MAX);
        assert_eq!(bound.to_string(), format!("at least {}", u64::MAX));
        assert_eq!(
            ResourceAmount::exact(2).saturating_add(3),
            ResourceAmount::exact(5)
        );
        assert!(
            ResourceAmount::exact(u64::MAX)
                .saturating_add(1)
                .is_lower_bound()
        );
        assert!(bound.saturating_add(0).is_lower_bound());
    }

    #[test]
    fn admission_refuses_exactly_the_requirements_above_the_budget() {
        let budget = MemoryBudget::from_bytes(10);
        assert_eq!(budget.admit(10), Ok(()));
        assert_eq!(
            budget.admit(11),
            Err(MemoryBudgetExceeded::new(ResourceAmount::exact(11), budget))
        );
        assert_eq!(MemoryBudget::UNLIMITED.admit(u128::from(u64::MAX)), Ok(()));
        let refused = MemoryBudget::UNLIMITED.admit(u128::MAX);
        assert!(refused.is_err_and(|refusal| refusal.required().is_lower_bound()));
    }
}
