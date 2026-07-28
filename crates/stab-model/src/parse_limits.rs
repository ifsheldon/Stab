use thiserror::Error;

use crate::ResourceEstimate;

/// Maximum number of physical source lines admitted by a parser.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceLineLimit(usize);

impl SourceLineLimit {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

/// Maximum repeat-block nesting admitted by a parser.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RepeatNestingLimit(usize);

impl RepeatNestingLimit {
    /// Hard ceiling shared by parsing and recursive model consumers.
    pub const HARD_MAX: usize = 256;

    pub const fn try_new(value: usize) -> Result<Self, RepeatNestingLimitError> {
        if value <= Self::HARD_MAX {
            Ok(Self(value))
        } else {
            Err(RepeatNestingLimitError {
                requested: value,
                hard_max: Self::HARD_MAX,
            })
        }
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

/// Invalid attempt to configure repeat nesting beyond Stab's recursive safety envelope.
#[derive(Clone, Copy, Debug, Eq, Error, Hash, PartialEq)]
#[error("repeat nesting limit {requested} exceeds the non-overridable hard maximum {hard_max}")]
pub struct RepeatNestingLimitError {
    requested: usize,
    hard_max: usize,
}

impl RepeatNestingLimitError {
    pub const fn requested(self) -> usize {
        self.requested
    }

    pub const fn hard_max(self) -> usize {
        self.hard_max
    }
}

/// Configurable safety budgets for circuit and detector-error-model text parsing.
///
/// Numeric representability, target identifiers, and other semantic bounds are intentionally not
/// fields of this policy. Repeat nesting can be tightened but cannot exceed the hard ceiling shared
/// by recursive model consumers.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParseLimits {
    source_lines: SourceLineLimit,
    repeat_nesting: RepeatNestingLimit,
}

impl ParseLimits {
    pub const DEFAULT_SOURCE_LINES: SourceLineLimit = SourceLineLimit::new(1_000_000);
    pub const DEFAULT_REPEAT_NESTING: RepeatNestingLimit =
        RepeatNestingLimit(RepeatNestingLimit::HARD_MAX);

    pub const fn new(source_lines: SourceLineLimit, repeat_nesting: RepeatNestingLimit) -> Self {
        Self {
            source_lines,
            repeat_nesting,
        }
    }

    pub const fn source_line_limit(self) -> SourceLineLimit {
        self.source_lines
    }

    pub const fn repeat_nesting_limit(self) -> RepeatNestingLimit {
        self.repeat_nesting
    }

    pub const fn with_source_line_limit(mut self, source_lines: SourceLineLimit) -> Self {
        self.source_lines = source_lines;
        self
    }

    pub const fn with_repeat_nesting_limit(mut self, repeat_nesting: RepeatNestingLimit) -> Self {
        self.repeat_nesting = repeat_nesting;
        self
    }

    pub fn estimate(self, input: &str) -> ResourceEstimate {
        ResourceEstimate::for_text_parse(input)
    }

    /// Estimates byte-oriented model parsing without requiring metadata to be valid UTF-8.
    pub fn estimate_bytes(self, input: &[u8]) -> ResourceEstimate {
        ResourceEstimate::for_model_bytes(input)
    }
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self::new(Self::DEFAULT_SOURCE_LINES, Self::DEFAULT_REPEAT_NESTING)
    }
}
