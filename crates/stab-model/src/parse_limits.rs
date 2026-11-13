use thiserror::Error;

use crate::{
    ByteSpan, CircuitItem, DemItem, DemTarget, MemoryBudget, ModelDialect, ModelError, ModelResult,
    ResourceEstimate, ResourceKind, ResourceLimitError, Target,
};

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
/// The memory budget bounds the source bytes plus the conservative retained size of the parsed
/// model, charged before each declaration and target is retained. Repeat nesting is the shared
/// recursion envelope. Numeric representability and identifier domains remain semantic parser
/// contracts instead of configurable resource policy.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParseLimits {
    memory_budget: MemoryBudget,
    repeat_nesting: RepeatNestingLimit,
}

impl ParseLimits {
    pub const DEFAULT_MEMORY_BUDGET: MemoryBudget = MemoryBudget::DEFAULT;
    pub const DEFAULT_REPEAT_NESTING: RepeatNestingLimit =
        RepeatNestingLimit(RepeatNestingLimit::HARD_MAX);

    pub const fn new(memory_budget: MemoryBudget, repeat_nesting: RepeatNestingLimit) -> Self {
        Self {
            memory_budget,
            repeat_nesting,
        }
    }

    pub const fn memory_budget(self) -> MemoryBudget {
        self.memory_budget
    }

    pub const fn repeat_nesting_limit(self) -> RepeatNestingLimit {
        self.repeat_nesting
    }

    #[must_use]
    pub const fn with_memory_budget(mut self, memory_budget: MemoryBudget) -> Self {
        self.memory_budget = memory_budget;
        self
    }

    #[must_use]
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
        Self::new(Self::DEFAULT_MEMORY_BUDGET, Self::DEFAULT_REPEAT_NESTING)
    }
}

/// Heap bookkeeping charged per declaration for its argument, target, and tag allocations.
const DECLARATION_HEAP_OVERHEAD_BYTES: u64 = 32;
/// Target vectors grow by doubling, including when adjacent instructions fuse, so each retained
/// target is charged for up to two slots.
const TARGET_SLOTS_PER_TARGET: u64 = 2;

/// Conservative retained bytes of one parsed declaration, excluding its targets.
const fn declaration_bytes(dialect: ModelDialect) -> u64 {
    let bytes = match dialect {
        ModelDialect::StimCircuit => size_of::<CircuitItem>(),
        ModelDialect::DetectorErrorModel => size_of::<DemItem>(),
    };
    bytes as u64 + DECLARATION_HEAP_OVERHEAD_BYTES
}

/// Conservative retained bytes of one parsed target.
const fn target_bytes(dialect: ModelDialect) -> u64 {
    let bytes = match dialect {
        ModelDialect::StimCircuit => size_of::<Target>(),
        ModelDialect::DetectorErrorModel => size_of::<DemTarget>(),
    };
    bytes as u64 * TARGET_SLOTS_PER_TARGET
}

pub(crate) struct ParseAdmission {
    dialect: ModelDialect,
    limits: ParseLimits,
    /// Source bytes plus the retained bytes charged so far; never above the budget.
    charged_bytes: u64,
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
            charged_bytes: input_len as u64,
        })
    }

    pub(crate) fn admit_source_bytes(
        dialect: ModelDialect,
        input_len: usize,
        limits: ParseLimits,
    ) -> ModelResult<()> {
        let budget = limits.memory_budget();
        if !budget.admits(input_len as u128) {
            let span_start = budget.bytes_usize();
            return Err(ResourceLimitError::new(
                dialect,
                ResourceKind::SourceBytes,
                None,
                input_len as u64,
                budget.bytes(),
                ByteSpan::from_valid_range(span_start, 1),
            )
            .into());
        }
        Ok(())
    }

    fn charge(&mut self, bytes: u128, source_line: usize, span: ByteSpan) -> ModelResult<()> {
        let required = u128::from(self.charged_bytes).saturating_add(bytes);
        let budget = self.limits.memory_budget();
        if !budget.admits(required) {
            return Err(ResourceLimitError::new(
                self.dialect,
                ResourceKind::RetainedBytes,
                Some(source_line),
                u64::try_from(required).unwrap_or(u64::MAX),
                budget.bytes(),
                span,
            )
            .into());
        }
        // `required` fits the u64 budget, so the conversion cannot fail after admission.
        self.charged_bytes = u64::try_from(required).map_err(|_| {
            ModelError::invalid_domain_value("parse admission", "charged bytes overflow")
        })?;
        Ok(())
    }

    pub(crate) fn admit_instruction(
        &mut self,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        self.charge(
            u128::from(declaration_bytes(self.dialect)),
            source_line,
            span,
        )
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
        let bytes = (count as u128).saturating_mul(u128::from(target_bytes(self.dialect)));
        self.charge(bytes, source_line, span)
    }

    /// Whether `upper_bound` more targets fit the remaining budget, so a fast path that retains
    /// them before charging cannot exceed it.
    pub(crate) fn target_budget_allows_upper_bound(&self, upper_bound: usize) -> bool {
        let bytes = (upper_bound as u128).saturating_mul(u128::from(target_bytes(self.dialect)));
        self.limits
            .memory_budget()
            .admits(u128::from(self.charged_bytes).saturating_add(bytes))
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
                actual as u64,
                limit as u64,
                span,
            )
            .into());
        }
        Ok(())
    }
}
