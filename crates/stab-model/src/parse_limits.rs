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
/// model, charged before each declaration, target, argument, and tag is retained. Repeat nesting is the shared
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

/// Heap bookkeeping charged per DEM declaration for its argument, target, and tag allocations.
const DEM_DECLARATION_HEAP_OVERHEAD_BYTES: u64 = 32;
/// DEM target vectors grow by doubling, so each retained DEM target and argument is charged for
/// up to two slots.
const DEM_SLOTS_PER_VALUE: u64 = 2;

/// Tag bytes are charged twice, covering the parsed text and its retained copy.
const TAG_COPIES: u64 = 2;
/// The boxed tag record a tagged circuit item retains beside its tag bytes.
const CIRCUIT_TAG_RECORD_BYTES: u64 = size_of::<crate::model_tag::ModelTag>() as u64;

/// Retained bytes of one parsed declaration, excluding its targets, arguments, and tag.
///
/// A circuit item is stored at exactly this size; its targets, arguments, and tag are exact-size
/// allocations charged separately, so a circuit's charge is its retained payload, excluding
/// allocator bookkeeping. A DEM declaration keeps the older conservative charge with heap
/// bookkeeping.
const fn declaration_bytes(dialect: ModelDialect) -> u64 {
    match dialect {
        ModelDialect::StimCircuit => size_of::<CircuitItem>() as u64,
        ModelDialect::DetectorErrorModel => {
            size_of::<DemItem>() as u64 + DEM_DECLARATION_HEAP_OVERHEAD_BYTES
        }
    }
}

/// Retained bytes of one parsed target.
///
/// A circuit target is charged one slot whether it sits inline in its item or in exact-size heap
/// storage, so a short instruction is charged up to four targets' bytes that its item already
/// holds. A DEM target is charged two slots.
const fn target_bytes(dialect: ModelDialect) -> u64 {
    match dialect {
        ModelDialect::StimCircuit => size_of::<Target>() as u64,
        ModelDialect::DetectorErrorModel => size_of::<DemTarget>() as u64 * DEM_SLOTS_PER_VALUE,
    }
}

/// Retained bytes of one parenthesized argument.
const fn argument_bytes(dialect: ModelDialect) -> u64 {
    match dialect {
        ModelDialect::StimCircuit => size_of::<f64>() as u64,
        ModelDialect::DetectorErrorModel => size_of::<f64>() as u64 * DEM_SLOTS_PER_VALUE,
    }
}

/// Retained bytes of a declaration's tag of `tag_bytes` unescaped bytes.
const fn tag_charge(dialect: ModelDialect, tag_bytes: usize) -> u64 {
    if tag_bytes == 0 {
        return 0;
    }
    let bytes = (tag_bytes as u64).saturating_mul(TAG_COPIES);
    match dialect {
        ModelDialect::StimCircuit => bytes.saturating_add(CIRCUIT_TAG_RECORD_BYTES),
        ModelDialect::DetectorErrorModel => bytes,
    }
}

/// Retained bytes of one declaration in `dialect` under [`ParseLimits`] accounting.
pub(crate) const fn model_declaration_admitted_bytes(
    dialect: ModelDialect,
    target_count: usize,
    argument_count: usize,
    tag_bytes: usize,
) -> u64 {
    declaration_bytes(dialect)
        .saturating_add((target_count as u64).saturating_mul(target_bytes(dialect)))
        .saturating_add((argument_count as u64).saturating_mul(argument_bytes(dialect)))
        .saturating_add(tag_charge(dialect, tag_bytes))
}

/// Conservative bytes of one tag byte recovered from non-UTF-8 input, whose arena grows by
/// doubling.
const RECOVERED_TAG_BYTE_BYTES: u64 = 2;
/// Conservative bytes of one recovered tag's arena end offset, whose vector grows by doubling.
const RECOVERED_TAG_BYTES: u64 = size_of::<usize>() as u64 * 2;

#[derive(Clone, Copy)]
pub(crate) struct ParseAdmission {
    dialect: ModelDialect,
    limits: ParseLimits,
    /// Source bytes plus the retained bytes charged so far; never above the budget.
    charged_bytes: u64,
    /// Source bytes plus the transient preparation bytes, both excluded from the model bytes.
    source_bytes: u64,
    /// Parse-time scratch and spare vector capacity, also excluded from the model bytes.
    transient_bytes: u64,
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
            source_bytes: input_len as u64,
            transient_bytes: 0,
        })
    }

    pub(crate) fn admit_source_bytes(
        dialect: ModelDialect,
        input_len: usize,
        limits: ParseLimits,
    ) -> ModelResult<()> {
        let budget = limits.memory_budget();
        budget.admit(input_len as u128).map_err(|exceeded| {
            let span_start = budget.bytes_saturating_usize();
            ResourceLimitError::parse_memory_budget(
                dialect,
                ResourceKind::SourceBytes,
                None,
                exceeded,
                crate::diagnostics::input_span(span_start, 1),
            )
            .into()
        })
    }

    /// The retained model bytes charged so far, excluding the source bytes.
    pub(crate) const fn retained_model_bytes(&self) -> u64 {
        self.charged_bytes
            .saturating_sub(self.source_bytes)
            .saturating_sub(self.transient_bytes)
    }

    fn charge(&mut self, bytes: u128, source_line: usize, span: ByteSpan) -> ModelResult<()> {
        self.charged_bytes = self.admitted_total(bytes, source_line, span)?;
        Ok(())
    }

    /// The charged total after `bytes` more, or the typed refusal when it exceeds the budget.
    fn admitted_total(&self, bytes: u128, source_line: usize, span: ByteSpan) -> ModelResult<u64> {
        let required = u128::from(self.charged_bytes).saturating_add(bytes);
        let budget = self.limits.memory_budget();
        if let Err(exceeded) = budget.admit(required) {
            return Err(ResourceLimitError::parse_memory_budget(
                self.dialect,
                ResourceKind::RetainedBytes,
                Some(source_line),
                exceeded,
                span,
            )
            .into());
        }
        // `required` fits the u64 budget, so the conversion cannot fail after admission.
        u64::try_from(required).map_err(|_| {
            ModelError::invalid_domain_value("parse admission", "charged bytes overflow")
        })
    }

    /// Charges transient storage that byte-input preparation holds while the model is parsed,
    /// such as the sanitized text copy, without counting it as retained model bytes.
    fn admit_preparation(
        &mut self,
        bytes: u128,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let charged = self.admitted_total(bytes, source_line, span)?;
        let added = charged.saturating_sub(self.charged_bytes);
        self.charged_bytes = charged;
        self.source_bytes = self.source_bytes.saturating_add(added);
        Ok(())
    }

    /// Charges the sanitized copy of `byte_count` non-UTF-8 input bytes before it is allocated.
    pub(crate) fn admit_sanitized_copy(
        &mut self,
        byte_count: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        self.admit_preparation(byte_count as u128, source_line, span)
    }

    /// Charges one tag of at most `raw_bytes` bytes recovered from non-UTF-8 input before its
    /// arena storage is reserved.
    pub(crate) fn admit_recovered_tag(
        &mut self,
        raw_bytes: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let bytes = (raw_bytes as u128)
            .saturating_mul(u128::from(RECOVERED_TAG_BYTE_BYTES))
            .saturating_add(u128::from(RECOVERED_TAG_BYTES));
        self.admit_preparation(bytes, source_line, span)
    }

    /// Returns the charge of the sanitized copy admitted by [`Self::admit_sanitized_copy`] once
    /// the copy is freed.
    pub(crate) fn release_sanitized_copy(&mut self, byte_count: usize) {
        let bytes = (byte_count as u64)
            .min(self.source_bytes)
            .min(self.charged_bytes);
        self.charged_bytes -= bytes;
        self.source_bytes -= bytes;
    }

    /// Charges, as retained model bytes, what an opaque tag restored from non-UTF-8 input keeps
    /// beyond the tag charge its parsed declaration already paid, before it is allocated.
    ///
    /// `locate` finds the tag's source line and span and runs only to report a refusal.
    pub(crate) fn admit_opaque_tag_overhead(
        &mut self,
        bytes: usize,
        locate: impl FnOnce() -> ModelResult<(usize, ByteSpan)>,
    ) -> ModelResult<()> {
        if bytes == 0 {
            return Ok(());
        }
        let required = u128::from(self.charged_bytes).saturating_add(bytes as u128);
        if let Ok(charged) = u64::try_from(required)
            && self.limits.memory_budget().admits(required)
        {
            self.charged_bytes = charged;
            return Ok(());
        }
        let (source_line, span) = locate()?;
        self.charge(bytes as u128, source_line, span)
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

    /// Charges one parenthesized argument before it is retained.
    pub(crate) fn admit_argument(&mut self, source_line: usize, span: ByteSpan) -> ModelResult<()> {
        self.charge(u128::from(argument_bytes(self.dialect)), source_line, span)
    }

    /// Charges `bytes` of parse-time storage, such as scratch buffers and the spare capacity of
    /// growing item and target vectors, before it is reserved; the bytes count against the budget
    /// until [`Self::release_transient`] returns them, but never as retained model bytes.
    pub(crate) fn admit_transient(
        &mut self,
        bytes: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let charged = self.admitted_total(bytes as u128, source_line, span)?;
        let added = charged.saturating_sub(self.charged_bytes);
        self.charged_bytes = charged;
        self.transient_bytes = self.transient_bytes.saturating_add(added);
        Ok(())
    }

    /// Returns transient bytes charged by [`Self::admit_transient`] once their storage is freed.
    pub(crate) fn release_transient(&mut self, bytes: usize) {
        let bytes = (bytes as u64).min(self.transient_bytes);
        self.charged_bytes = self.charged_bytes.saturating_sub(bytes);
        self.transient_bytes -= bytes;
    }

    /// Returns retained bytes charged for a declaration that fused into the previous
    /// instruction: its item slot, its arguments, and its tag, which the fused instruction
    /// already holds. Its targets stay charged.
    pub(crate) fn refund_fused_declaration(&mut self, argument_count: usize, tag_bytes: usize) {
        let bytes = model_declaration_admitted_bytes(self.dialect, 0, argument_count, tag_bytes);
        let model = self.retained_model_bytes();
        self.charged_bytes = self.charged_bytes.saturating_sub(bytes.min(model));
    }

    /// Checks that `count` parsed arguments fit beside the charged bytes without retaining
    /// them, for argument lists that are parsed and then discarded.
    pub(crate) fn admit_transient_arguments(
        &self,
        count: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let bytes = (count as u128).saturating_mul(u128::from(argument_bytes(self.dialect)));
        self.admitted_total(bytes, source_line, span).map(|_| ())
    }

    /// Charges a declaration's parenthesized arguments and its tag.
    pub(crate) fn admit_payload(
        &mut self,
        argument_count: usize,
        tag_bytes: usize,
        source_line: usize,
        span: ByteSpan,
    ) -> ModelResult<()> {
        let bytes = (argument_count as u128)
            .saturating_mul(u128::from(argument_bytes(self.dialect)))
            .saturating_add(u128::from(tag_charge(self.dialect, tag_bytes)));
        if bytes == 0 {
            return Ok(());
        }
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
            return Err(ResourceLimitError::parse_repeat_nesting(
                self.dialect,
                source_line,
                actual as u64,
                limit as u64,
                span,
            )
            .into());
        }
        Ok(())
    }
}
