use std::fmt;
use std::str::FromStr;

use crate::{MemoryBudgetExceeded, ResourceAmount};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// Caller-selected number of bytes one operation may retain.
///
/// A memory budget is the single admission dimension of every materializing or executing
/// operation: the operation computes its conservative retained-storage requirement with checked
/// arithmetic and refuses work whose requirement exceeds the budget before allocating, advancing
/// randomness, or writing output. CPU time is not bounded by a budget.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryBudget(u64);

impl MemoryBudget {
    /// Default budget of one gibibyte.
    pub const DEFAULT: Self = Self(GIB);

    /// The largest budget, `u64::MAX` bytes, which admits every requirement that fits `u64`.
    ///
    /// Callers use it where storage is already bounded by values they hold and no separate
    /// budget applies.
    pub const UNLIMITED: Self = Self(u64::MAX);

    /// Budget of exactly `bytes` bytes.
    pub const fn from_bytes(bytes: u64) -> Self {
        Self(bytes)
    }

    /// Budget size in bytes.
    pub const fn bytes(self) -> u64 {
        self.0
    }

    /// Whether a conservative requirement of `required` bytes fits this budget.
    pub const fn admits(self, required: u128) -> bool {
        required <= self.0 as u128
    }

    /// Admits a conservative requirement of `required` bytes, or reports the refusal.
    pub fn admit(self, required: u128) -> Result<(), MemoryBudgetExceeded> {
        if self.admits(required) {
            Ok(())
        } else {
            Err(MemoryBudgetExceeded::new(
                ResourceAmount::saturating_from_u128(required),
                self,
            ))
        }
    }

    /// Budget size as a platform size, saturating where the platform cannot address it.
    pub fn bytes_saturating_usize(self) -> usize {
        usize::try_from(self.0).unwrap_or(usize::MAX)
    }
}

impl Default for MemoryBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl fmt::Display for MemoryBudget {
    /// Formats the budget in the largest binary unit that represents it exactly.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bytes = self.0;
        for (unit, suffix) in [(GIB, "GiB"), (MIB, "MiB"), (KIB, "KiB")] {
            if bytes != 0 && bytes.is_multiple_of(unit) {
                return write!(formatter, "{}{suffix}", bytes / unit);
            }
        }
        write!(formatter, "{bytes}")
    }
}

/// Invalid memory-budget text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryBudgetParseError {
    Syntax { text: String },
    Overflow { text: String, max: u64 },
}

impl fmt::Display for MemoryBudgetParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { text } => write!(
                formatter,
                "memory budget must be an integer number of bytes with an optional KiB, MiB, or GiB suffix, got {text:?}"
            ),
            Self::Overflow { text, max } => {
                write!(formatter, "memory budget {text:?} exceeds {max} bytes")
            }
        }
    }
}

impl std::error::Error for MemoryBudgetParseError {}

impl FromStr for MemoryBudget {
    type Err = MemoryBudgetParseError;

    /// Parses `<digits>` or `<digits><suffix>` where the suffix is `KiB`, `MiB`, or `GiB`.
    ///
    /// Signs, whitespace, decimal points, and decimal (`KB`, `MB`) units are rejected so that one
    /// spelling denotes one byte count.
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let syntax = || MemoryBudgetParseError::Syntax {
            text: text.to_owned(),
        };
        let digits_end = text
            .bytes()
            .position(|byte| !byte.is_ascii_digit())
            .unwrap_or(text.len());
        let (digits, suffix) = text.split_at(digits_end);
        if digits.is_empty() {
            return Err(syntax());
        }
        let multiplier = match suffix {
            "" => 1,
            "KiB" => KIB,
            "MiB" => MIB,
            "GiB" => GIB,
            _ => return Err(syntax()),
        };
        let overflow = || MemoryBudgetParseError::Overflow {
            text: text.to_owned(),
            max: u64::MAX,
        };
        let value = digits.parse::<u64>().map_err(|_| overflow())?;
        value.checked_mul(multiplier).map(Self).ok_or_else(overflow)
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::{MemoryBudget, MemoryBudgetParseError};

    #[test]
    fn parsing_accepts_bytes_and_binary_suffixes_and_rejects_ambiguous_spellings() {
        for (text, bytes) in [
            ("0", 0),
            ("123", 123),
            ("1KiB", 1024),
            ("64MiB", 64 << 20),
            ("1GiB", 1 << 30),
            ("16GiB", 16 << 30),
            ("18446744073709551615", u64::MAX),
        ] {
            assert_eq!(
                text.parse::<MemoryBudget>().map(MemoryBudget::bytes),
                Ok(bytes),
                "{text}"
            );
        }
        for text in [
            "", "GiB", "1 GiB", " 1", "1gib", "1GB", "1MB", "1K", "1.5GiB", "-1", "+1", "1TiB",
        ] {
            assert_matches!(
                text.parse::<MemoryBudget>(),
                Err(MemoryBudgetParseError::Syntax { .. }),
                "{text}",
            );
        }
        for text in ["18446744073709551616", "17179869184GiB"] {
            assert_matches!(
                text.parse::<MemoryBudget>(),
                Err(MemoryBudgetParseError::Overflow { .. }),
                "{text}",
            );
        }
    }

    #[test]
    fn display_round_trips_through_the_canonical_parser() {
        for bytes in [
            0,
            1,
            1023,
            1024,
            1536,
            64 << 20,
            1 << 30,
            (1 << 30) + 1,
            u64::MAX,
        ] {
            let budget = MemoryBudget::from_bytes(bytes);
            assert_eq!(budget.to_string().parse::<MemoryBudget>(), Ok(budget));
        }
        assert_eq!(MemoryBudget::DEFAULT.to_string(), "1GiB");
        assert!(MemoryBudget::from_bytes(10).admits(10));
        assert!(!MemoryBudget::from_bytes(10).admits(11));
    }
}
