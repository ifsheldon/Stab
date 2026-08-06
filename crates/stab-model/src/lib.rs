//! Stable typed Stim circuit and detector-error-model values.

mod circuit;
mod dem;
mod diagnostics;
mod dialect;
mod error;
mod fingerprint;
mod gate;
mod ids;
mod model_bytes;
mod model_parse;
mod model_tag;
mod parse_limits;
mod resource_limit;
mod resources;
mod source_text;
mod target;
mod validation;

pub use circuit::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, RepeatBlock,
};
pub use dem::{
    DemDetectorId, DemErrorMechanismTraversalLimits, DemErrorMechanismView,
    DemErrorMechanismVisitError, DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter,
    DemFlattenedInstructionIter, DemInstruction, DemInstructionKind, DemItem, DemObservableId,
    DemRepeatBlock, DemTarget, DetectorErrorModel,
};
pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, ParseError, ParseErrorCode, ParseErrorContext,
};
pub use dialect::ModelDialect;
pub use error::{ModelError, ModelResult};
pub use fingerprint::ModelFingerprint;
pub use gate::{
    Gate, GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind, GateTargetRule,
};
pub use ids::{
    CircuitDetectorId, CircuitTick, DemRepeatCount, MeasureRecordOffset, MeasureRecordOffsetText,
    ObservableId, Probability, ProbabilityStimText, QubitId, RepeatCount,
};
pub use parse_limits::{ParseLimits, RepeatNestingLimit, RepeatNestingLimitError, SourceLineLimit};
pub use resource_limit::{
    ResourceKind, ResourceLimitContext, ResourceLimitError, ResourceOperation,
};
pub use resources::{Estimate, EstimateClass, ResourceEstimate, ResourceEstimateBuilder};
pub use target::{Pauli, Target};
pub use validation::{ValidationError, ValidationErrorCode};

/// Low-level model operations for parsers and admitted algorithms.
pub mod advanced {
    use std::fmt::Display;

    use super::{
        ByteSpan, Circuit, CircuitInstruction, DemInstruction, DemInstructionKind, DemRepeatBlock,
        DemTarget, DetectorErrorModel, Gate, GateDecomposition, MeasureRecordOffset, ModelDialect,
        ModelError, ModelResult, ParseError, ParseErrorCode, ParseErrorContext, Probability,
        RepeatBlock, RepeatCount, ResourceLimitError, Target,
    };
    pub use crate::dem::MAX_DEM_REPEAT_NESTING;
    pub use crate::dem::advanced::{
        DemBlockSummary, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
        FoldedDemTraversal, FoldedDemVisitor, shifted_coordinates, shifted_detector,
        shifted_targets,
    };
    pub use crate::gate::GateUnitaryRows;
    use smallvec::SmallVec;

    /// Inline target storage used by the exact Stim parsers.
    pub type TargetVec = SmallVec<[Target; 4]>;

    /// Stim's exclusive upper bound for encoded target values.
    pub const STIM_TARGET_VALUE_LIMIT: u32 = crate::ids::STIM_TARGET_VALUE_LIMIT;

    /// Fallible circuit builder used by analysis and execution lowering.
    ///
    /// This boundary preserves parser-style instruction fusion while keeping model storage private.
    #[derive(Debug)]
    pub struct CircuitBuilder(crate::circuit::CircuitAssembler);

    impl CircuitBuilder {
        pub fn new() -> Self {
            Self(crate::circuit::CircuitAssembler::new())
        }

        pub fn from_unfused_items(items: Vec<crate::CircuitItem>) -> Self {
            Self(crate::circuit::CircuitAssembler::from_unfused_items(items))
        }

        pub fn try_reserve_exact(&mut self, additional: usize) -> ModelResult<()> {
            self.0.try_reserve_exact(additional)
        }

        pub fn try_append_instruction(
            &mut self,
            instruction: CircuitInstruction,
        ) -> ModelResult<()> {
            self.0.try_append_instruction(instruction)
        }

        pub fn try_append_repeat_block(&mut self, repeat: RepeatBlock) -> ModelResult<()> {
            self.0.try_append_repeat_block(repeat)
        }

        pub fn finish(self) -> Circuit {
            self.0.finish()
        }
    }

    impl Default for CircuitBuilder {
        fn default() -> Self {
            Self::new()
        }
    }

    /// Constructs an instruction while preserving opaque tag bytes.
    pub fn circuit_instruction_with_tag_bytes(
        gate: Gate,
        args: Vec<f64>,
        targets: Vec<Target>,
        tag: Option<&[u8]>,
    ) -> ModelResult<CircuitInstruction> {
        CircuitInstruction::new_with_tag_bytes(gate, args, targets, tag)
    }

    /// Constructs a repeat block while preserving opaque tag bytes.
    pub fn repeat_block_with_tag_bytes(
        repeat_count: RepeatCount,
        body: Circuit,
        tag: Option<&[u8]>,
    ) -> RepeatBlock {
        RepeatBlock::new_with_tag_bytes(repeat_count, body, tag)
    }

    /// Clones an instruction without its tag.
    pub fn circuit_instruction_without_tag(instruction: &CircuitInstruction) -> CircuitInstruction {
        instruction.without_tag()
    }

    /// Returns the qubit width required to simulate a circuit.
    ///
    /// This equals [`Circuit::count_qubits`]: MPAD pad values are excluded from both counts,
    /// matching Stim v1.16.0.
    pub fn circuit_simulated_qubit_count(circuit: &Circuit) -> usize {
        circuit.count_qubits()
    }

    /// Returns the number of measurement results produced by one instruction.
    pub fn circuit_instruction_measurement_result_count(instruction: &CircuitInstruction) -> usize {
        instruction.measurement_result_count()
    }

    /// Reserves exact model storage for a bounded materializing consumer.
    pub fn dem_try_reserve_items_exact(
        model: &mut DetectorErrorModel,
        additional: usize,
    ) -> ModelResult<()> {
        model.try_reserve_items_exact(additional)
    }

    /// Constructs a DEM instruction while preserving opaque tag bytes.
    pub fn dem_instruction_with_tag_bytes(
        kind: DemInstructionKind,
        args: Vec<f64>,
        targets: Vec<DemTarget>,
        tag: Option<&[u8]>,
    ) -> ModelResult<DemInstruction> {
        DemInstruction::new_with_tag_bytes(kind, args, targets, tag)
    }

    /// Constructs a DEM repeat block while preserving opaque tag bytes.
    pub fn dem_repeat_block_with_tag_bytes(
        repeat_count: crate::DemRepeatCount,
        body: DetectorErrorModel,
        tag: Option<&[u8]>,
    ) -> DemRepeatBlock {
        DemRepeatBlock::new_with_tag_bytes(repeat_count, body, tag)
    }

    /// Removes the tag from a DEM instruction in place.
    pub fn dem_instruction_clear_tag(instruction: &mut DemInstruction) {
        instruction.clear_tag();
    }

    /// Returns the numeric detector shift carried by a shift instruction.
    pub fn dem_instruction_detector_shift(instruction: &DemInstruction) -> ModelResult<u64> {
        instruction.detector_shift()
    }

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

    /// Constructs the circuit source-line admission failure used by the exact parser.
    pub fn circuit_source_line_limit_error(
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> ModelError {
        ResourceLimitError::circuit_source_lines(actual, limit, span).into()
    }

    /// Constructs the circuit repeat-depth admission failure used by the exact parser.
    pub fn circuit_repeat_nesting_limit_error(
        source_line: usize,
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> ModelError {
        ResourceLimitError::circuit_repeat_nesting(source_line, actual, limit, span).into()
    }

    /// Constructs the DEM source-line admission failure used by the exact parser.
    pub fn dem_source_line_limit_error(actual: usize, limit: usize, span: ByteSpan) -> ModelError {
        ResourceLimitError::dem_source_lines(actual, limit, span).into()
    }

    /// Constructs the DEM repeat-depth admission failure used by the exact parser.
    pub fn dem_repeat_nesting_limit_error(
        actual: usize,
        limit: usize,
        span: ByteSpan,
    ) -> ModelError {
        ResourceLimitError::dem_repeat_nesting(actual, limit, span).into()
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
