use thiserror::Error;

use crate::{
    ByteSpan, ModelDialect, ModelError, ModelResult, ResourceEstimate, ResourceKind,
    ResourceLimitError,
};

/// Maximum number of source bytes admitted by a parser.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceByteLimit(usize);

impl SourceByteLimit {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

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

/// Maximum number of compact source declarations admitted by a parser.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RepresentedInstructionLimit(usize);

impl RepresentedInstructionLimit {
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    pub const fn get(self) -> usize {
        self.0
    }
}

/// Maximum number of target values retained by a parsed model.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RepresentedTargetLimit(usize);

impl RepresentedTargetLimit {
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
/// Limits are inclusive and apply to the compact source representation before repeat expansion or
/// adjacent circuit-instruction fusion. Numeric representability and identifier domains remain
/// semantic parser contracts instead of configurable resource policy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParseLimits {
    source_bytes: SourceByteLimit,
    source_lines: SourceLineLimit,
    represented_instructions: RepresentedInstructionLimit,
    represented_targets: RepresentedTargetLimit,
    repeat_nesting: RepeatNestingLimit,
}

impl ParseLimits {
    pub const DEFAULT_SOURCE_BYTES: SourceByteLimit = SourceByteLimit::new(64 * 1024 * 1024);
    pub const DEFAULT_SOURCE_LINES: SourceLineLimit = SourceLineLimit::new(1_000_000);
    pub const DEFAULT_REPRESENTED_INSTRUCTIONS: RepresentedInstructionLimit =
        RepresentedInstructionLimit::new(1_000_000);
    pub const DEFAULT_REPRESENTED_TARGETS: RepresentedTargetLimit =
        RepresentedTargetLimit::new(32_000_000);
    pub const DEFAULT_REPEAT_NESTING: RepeatNestingLimit =
        RepeatNestingLimit(RepeatNestingLimit::HARD_MAX);

    pub const fn new(
        source_bytes: SourceByteLimit,
        source_lines: SourceLineLimit,
        represented_instructions: RepresentedInstructionLimit,
        represented_targets: RepresentedTargetLimit,
        repeat_nesting: RepeatNestingLimit,
    ) -> Self {
        Self {
            source_bytes,
            source_lines,
            represented_instructions,
            represented_targets,
            repeat_nesting,
        }
    }

    pub const fn source_byte_limit(self) -> SourceByteLimit {
        self.source_bytes
    }

    pub const fn source_line_limit(self) -> SourceLineLimit {
        self.source_lines
    }

    pub const fn represented_instruction_limit(self) -> RepresentedInstructionLimit {
        self.represented_instructions
    }

    pub const fn represented_target_limit(self) -> RepresentedTargetLimit {
        self.represented_targets
    }

    pub const fn repeat_nesting_limit(self) -> RepeatNestingLimit {
        self.repeat_nesting
    }

    pub const fn with_source_byte_limit(mut self, source_bytes: SourceByteLimit) -> Self {
        self.source_bytes = source_bytes;
        self
    }

    pub const fn with_source_line_limit(mut self, source_lines: SourceLineLimit) -> Self {
        self.source_lines = source_lines;
        self
    }

    pub const fn with_represented_instruction_limit(
        mut self,
        represented_instructions: RepresentedInstructionLimit,
    ) -> Self {
        self.represented_instructions = represented_instructions;
        self
    }

    pub const fn with_represented_target_limit(
        mut self,
        represented_targets: RepresentedTargetLimit,
    ) -> Self {
        self.represented_targets = represented_targets;
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
        Self::new(
            Self::DEFAULT_SOURCE_BYTES,
            Self::DEFAULT_SOURCE_LINES,
            Self::DEFAULT_REPRESENTED_INSTRUCTIONS,
            Self::DEFAULT_REPRESENTED_TARGETS,
            Self::DEFAULT_REPEAT_NESTING,
        )
    }
}

pub(crate) struct ParseAdmission {
    dialect: ModelDialect,
    limits: ParseLimits,
    represented_instructions: usize,
    represented_targets: usize,
}

impl ParseAdmission {
    pub(crate) fn new(
        dialect: ModelDialect,
        input_len: usize,
        limits: ParseLimits,
    ) -> ModelResult<Self> {
        Self::admit_source_bytes(dialect, input_len, limits)?;
        Ok(Self {
            dialect,
            limits,
            represented_instructions: 0,
            represented_targets: 0,
        })
    }

    pub(crate) fn admit_source_bytes(
        dialect: ModelDialect,
        input_len: usize,
        limits: ParseLimits,
    ) -> ModelResult<()> {
        let byte_limit = limits.source_byte_limit().get();
        if input_len > byte_limit {
            return Err(ResourceLimitError::new(
                dialect,
                ResourceKind::SourceBytes,
                None,
                input_len,
                byte_limit,
                ByteSpan::from_valid_range(byte_limit, 1),
            )
            .into());
        }
        Ok(())
    }

    pub(crate) fn admit_source_line(&self, source_line: usize, span: ByteSpan) -> ModelResult<()> {
        let limit = self.limits.source_line_limit().get();
        if source_line > limit {
            return Err(ResourceLimitError::new(
                self.dialect,
                ResourceKind::SourceLines,
                Some(source_line),
                source_line,
                limit,
                span,
            )
            .into());
        }
        Ok(())
    }

    pub(crate) fn admit_instruction(
        &mut self,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let actual = self
            .represented_instructions
            .checked_add(1)
            .ok_or_else(|| {
                ModelError::invalid_domain_value(
                    "represented instruction count",
                    "parser counter overflow",
                )
            })?;
        let limit = self.limits.represented_instruction_limit().get();
        if actual > limit {
            return Err(ResourceLimitError::new(
                self.dialect,
                ResourceKind::RepresentedInstructions,
                Some(source_line),
                actual,
                limit,
                span,
            )
            .into());
        }
        self.represented_instructions = actual;
        Ok(())
    }

    pub(crate) fn admit_target(&mut self, source_line: usize, span: ByteSpan) -> ModelResult<()> {
        self.admit_targets(1, source_line, span)
    }

    pub(crate) fn admit_targets(
        &mut self,
        count: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let actual = self.represented_targets.checked_add(count).ok_or_else(|| {
            ModelError::invalid_domain_value("represented target count", "parser counter overflow")
        })?;
        let limit = self.limits.represented_target_limit().get();
        if actual > limit {
            let first_excess = limit.checked_add(1).unwrap_or(actual);
            return Err(ResourceLimitError::new(
                self.dialect,
                ResourceKind::RepresentedTargets,
                Some(source_line),
                first_excess,
                limit,
                span,
            )
            .into());
        }
        self.represented_targets = actual;
        Ok(())
    }

    pub(crate) fn target_budget_allows_upper_bound(&self, upper_bound: usize) -> bool {
        self.limits
            .represented_target_limit()
            .get()
            .checked_sub(self.represented_targets)
            .is_some_and(|remaining| upper_bound <= remaining)
    }

    pub(crate) fn admit_repeat_nesting(
        &self,
        source_line: usize,
        actual: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let limit = self.limits.repeat_nesting_limit().get();
        if actual > limit {
            return Err(ResourceLimitError::new(
                self.dialect,
                ResourceKind::RepeatNesting,
                Some(source_line),
                actual,
                limit,
                span,
            )
            .into());
        }
        Ok(())
    }
}
