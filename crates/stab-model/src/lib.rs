//! Stable typed Stim circuit and detector-error-model values.

mod diagnostics;
mod dialect;
mod error;
mod gate;
mod ids;
mod parse_limits;
mod resources;
mod target;

pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, ParseError, ParseErrorCode, ParseErrorContext,
};
pub use dialect::ModelDialect;
pub use error::{ModelError, ModelResult};
pub use gate::{
    Gate, GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind, GateTargetRule,
};
pub use ids::{
    CircuitDetectorId, DemRepeatCount, MeasureRecordOffset, MeasureRecordOffsetText, ObservableId,
    Probability, QubitId, RepeatCount,
};
pub use parse_limits::{ParseLimits, RepeatNestingLimit, RepeatNestingLimitError, SourceLineLimit};
pub use resources::{Estimate, EstimateClass, ResourceEstimate};
pub use target::{Pauli, Target};

/// Low-level model operations for parsers and admitted algorithms.
pub mod advanced {
    use std::fmt::Display;

    use super::{
        ByteSpan, Estimate, Gate, GateDecomposition, MeasureRecordOffset, ModelDialect,
        ModelResult, ParseError, ParseErrorCode, ParseErrorContext, Probability, ResourceEstimate,
        Target,
    };
    pub use crate::gate::GateUnitaryRows;
    use smallvec::SmallVec;

    /// Inline target storage used by the exact Stim parsers.
    pub type TargetVec = SmallVec<[Target; 4]>;

    /// Stim's exclusive upper bound for encoded target values.
    pub const STIM_TARGET_VALUE_LIMIT: u32 = crate::ids::STIM_TARGET_VALUE_LIMIT;

    /// Constructs a span after the caller has proved that the range does not overflow.
    pub const fn byte_span_from_valid_range(byte_start: usize, byte_length: usize) -> ByteSpan {
        ByteSpan::from_valid_range(byte_start, byte_length)
    }

    /// Bounds attacker-controlled text using the model diagnostic contract.
    pub fn bounded_parse_diagnostic_text(value: &str) -> String {
        crate::diagnostics::bounded_parse_diagnostic_text(value)
    }

    /// Constructs a model parse diagnostic with separate machine and compatibility messages.
    pub fn parse_error_with_human_message(
        code: ParseErrorCode,
        message: impl Into<String>,
        human_message: impl Into<String>,
        span: ByteSpan,
        context: ParseErrorContext,
    ) -> ParseError {
        ParseError::with_human_message(code, message, human_message, span, context)
    }

    /// Constructs the UTF-8 diagnostic recorded by the byte-oriented admission scanner.
    pub fn invalid_utf8_parse_error(
        dialect: ModelDialect,
        byte_start: usize,
        byte_length: usize,
        error_length: Option<usize>,
    ) -> ParseError {
        ParseError::invalid_utf8_at(dialect, byte_start, byte_length, error_length)
    }

    /// Returns the schema-one fingerprint discriminator for a model dialect.
    pub const fn model_dialect_fingerprint_discriminator(dialect: ModelDialect) -> u8 {
        dialect.fingerprint_discriminator()
    }

    /// Assembles the resource vocabulary emitted by the current sampling facade.
    pub const fn resource_estimate_for_sampling_request(
        input_items: Estimate<usize>,
        expanded_operations: Estimate<usize>,
        folded_traversal: Estimate<usize>,
        output_bytes: Estimate<usize>,
    ) -> ResourceEstimate {
        ResourceEstimate::for_sampling_request(
            input_items,
            expanded_operations,
            folded_traversal,
            output_bytes,
        )
    }

    /// Constructs a probability after the caller has proved its domain.
    pub fn probability_from_valid(value: f64) -> Probability {
        Probability::from_valid_probability(value)
    }

    /// Preserves Stim's parsed `rec[-0]` spelling.
    pub fn measure_record_offset_from_stim_text(value: i32) -> ModelResult<MeasureRecordOffset> {
        MeasureRecordOffset::from_stim_text(value)
    }

    /// Reports whether an offset preserves Stim's parsed `rec[-0]` spelling.
    pub fn measure_record_offset_is_negative_zero(offset: MeasureRecordOffset) -> bool {
        offset.is_negative_zero()
    }

    /// Formats an offset using Stim target syntax.
    pub fn measure_record_offset_stim_text(offset: MeasureRecordOffset) -> impl Display {
        offset.stim_text()
    }

    /// Parses one possibly combined target token into caller-provided storage.
    pub fn parse_target_token_into(token: &str, targets: &mut TargetVec) -> ModelResult<()> {
        crate::target::parse_target_token_into(token, targets)
    }

    /// Parses the fast path for a whitespace-separated plain-qubit target list.
    pub fn parse_plain_qubit_target_text(text: &str) -> ModelResult<Option<TargetVec>> {
        crate::target::parse_plain_qubit_target_text(text)
    }

    /// Looks up any canonical or aliased Stim v1.16.0 gate name.
    #[inline]
    pub fn lookup_gate(name: &str) -> Option<Gate> {
        Gate::lookup_name(name)
    }

    /// Looks up the parser's small common-gate fast path.
    #[inline]
    pub fn lookup_simple_plain_gate(name: &str) -> Option<Gate> {
        Gate::from_simple_plain_name(name)
    }

    /// Validates arguments and targets against a gate's closed syntax descriptor.
    #[inline]
    pub fn validate_gate(gate: Gate, args: &[f64], targets: &[Target]) -> ModelResult<()> {
        gate.validate(args, targets)
    }

    /// Validates only targets against a gate's closed syntax descriptor.
    #[inline]
    pub fn validate_gate_targets(gate: Gate, targets: &[Target]) -> ModelResult<()> {
        gate.validate_targets(targets)
    }

    /// Returns the common parser fast-path `H` gate.
    #[inline]
    pub fn plain_h_gate() -> Gate {
        Gate::plain_h()
    }

    /// Returns the common parser fast-path `M` gate.
    #[inline]
    pub fn plain_m_gate() -> Gate {
        Gate::plain_m()
    }

    /// Returns the common parser fast-path `CX` gate.
    #[inline]
    pub fn plain_cx_gate() -> Gate {
        Gate::plain_cx()
    }

    /// Returns the common parser fast-path `S` gate.
    #[inline]
    pub fn plain_s_gate() -> Gate {
        Gate::plain_s()
    }

    /// Returns the common parser fast-path `DETECTOR` gate.
    #[inline]
    pub fn plain_detector_gate() -> Gate {
        Gate::plain_detector()
    }

    /// Returns the common parser fast-path `TICK` gate.
    #[inline]
    pub fn plain_tick_gate() -> Gate {
        Gate::plain_tick()
    }

    /// Returns the raw pinned flow descriptors for a gate.
    #[inline]
    pub fn gate_flow_descriptors(gate: Gate) -> Option<&'static [&'static str]> {
        crate::gate::gate_flow_descriptors(gate.canonical_name())
    }

    /// Returns the raw pinned scalar unitary rows for a gate.
    #[inline]
    pub fn gate_unitary_rows(gate: Gate) -> Option<GateUnitaryRows> {
        crate::gate::gate_unitary_rows(gate.canonical_name())
    }

    /// Returns the raw pinned H/S/CX/M/R decomposition descriptor for a gate.
    #[inline]
    pub fn gate_decomposition(gate: Gate) -> Option<GateDecomposition> {
        crate::gate::gate_decomposition_text(gate.canonical_name()).map(GateDecomposition::new)
    }
}
