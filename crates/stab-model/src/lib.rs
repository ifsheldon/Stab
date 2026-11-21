//! Stable typed Stim circuit and detector-error-model values.

mod approximate;
mod circuit;
mod controlled_pauli;
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
mod model_tree;
mod parse_limits;
mod resource_limit;
mod resources;
mod source_text;
mod target;
mod validation;

pub use circuit::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, FoldedCircuitSummary, FoldedCount, FoldedWalkError,
    RepeatBlock,
};
pub use dem::{
    DemDetectorId, DemErrorMechanismTraversalLimits, DemErrorMechanismView,
    DemErrorMechanismVisitError, DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter,
    DemFlattenedInstructionIter, DemInstruction, DemInstructionKind, DemInstructionView, DemItem,
    DemRepeatBlock, DemTarget, DemTargetKind, DetectorErrorModel,
};
pub use diagnostics::{ParseError, ParseErrorCode, ParseErrorContext};
pub use dialect::ModelDialect;
pub use error::{ModelError, ModelResult};
pub use fingerprint::ModelFingerprint;
pub use gate::{
    Gate, GateArgumentRule, GateCategory, GateDecomposition, GateKind, GateTargetGroupKind,
    GateTargetRule,
};
pub use ids::{
    AbsoluteTolerance, CircuitDetectorId, CircuitTick, DemRepeatCount, MeasureRecordOffset,
    MeasureRecordOffsetText, ObservableId, Probability, ProbabilityStimText, QubitId, RepeatCount,
};
pub use parse_limits::{ParseLimits, RepeatNestingLimit, RepeatNestingLimitError};
pub use resource_limit::{ResourceKind, ResourceLimitError, ResourceOperation};
pub use resources::{ResourceEstimate, ResourceEstimateBuilder};
pub use stab_foundation::{
    ByteSpan, Cancellation, Cancelled, DiagnosticSeverity, Estimate, MemoryBudget,
    MemoryBudgetExceeded, MemoryBudgetParseError, ResourceAmount,
};
pub use target::{Pauli, Target, TargetKind};
pub use validation::{ValidationError, ValidationErrorCode};

/// Low-level model operations for parsers and admitted algorithms.
///
/// Every item here relies on an invariant the caller has already established, or exposes raw
/// pinned table data whose typed interpretation belongs to a higher layer.
pub mod advanced {
    use super::{
        Circuit, CircuitInstruction, DemDetectorId, DemInstruction, DemRepeatBlock, DemTarget,
        DetectorErrorModel, Gate, GateDecomposition, ModelDialect, ModelResult, ObservableId,
        Probability, RepeatBlock, RepeatCount, Target,
    };
    pub use crate::circuit::{FoldedCircuitEvent, FoldedVisitError, visit_folded_circuit};
    pub use crate::controlled_pauli::{ClassicalControl, ControlledPauliTargetPair};
    pub use crate::dem::advanced::{
        DemBlockSummary, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
        FoldedDemTraversal, FoldedDemVisitor, shifted_targets,
    };
    pub use crate::gate::GateUnitaryRows;
    pub use stab_foundation::CancellationCountdown;

    /// Conservative retained bytes of one circuit declaration under [`crate::ParseLimits`].
    pub const fn circuit_declaration_admitted_bytes(
        target_count: usize,
        argument_count: usize,
        tag_bytes: usize,
    ) -> u64 {
        crate::parse_limits::model_declaration_admitted_bytes(
            ModelDialect::StimCircuit,
            target_count,
            argument_count,
            tag_bytes,
        )
    }

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

    /// Constructs a DEM `detector` instruction while preserving opaque tag bytes.
    pub fn dem_detector_instruction_with_tag_bytes(
        coordinates: Vec<f64>,
        detector: DemDetectorId,
        tag: Option<&[u8]>,
    ) -> ModelResult<DemInstruction> {
        DemInstruction::detector_with_tag_bytes(coordinates, detector, tag)
    }

    /// Constructs a DEM `logical_observable` instruction while preserving opaque tag bytes.
    pub fn dem_logical_observable_instruction_with_tag_bytes(
        observable: ObservableId,
        tag: Option<&[u8]>,
    ) -> DemInstruction {
        DemInstruction::logical_observable_with_tag_bytes(observable, tag)
    }

    /// Constructs a DEM `shift_detectors` instruction while preserving opaque tag bytes.
    pub fn dem_shift_detectors_instruction_with_tag_bytes(
        coordinates: Vec<f64>,
        detector_shift: u64,
        tag: Option<&[u8]>,
    ) -> ModelResult<DemInstruction> {
        DemInstruction::shift_detectors_with_tag_bytes(coordinates, detector_shift, tag)
    }

    /// Constructs a DEM repeat block while preserving opaque tag bytes.
    pub fn dem_repeat_block_with_tag_bytes(
        repeat_count: crate::DemRepeatCount,
        body: DetectorErrorModel,
        tag: Option<&[u8]>,
    ) -> DemRepeatBlock {
        DemRepeatBlock::new_with_tag_bytes(repeat_count, body, tag)
    }

    /// Constructs a validated DEM error instruction from a borrowed target slice.
    pub fn dem_error_instruction_from_targets(
        probability: Probability,
        targets: &[DemTarget],
        tag: Option<&[u8]>,
    ) -> ModelResult<DemInstruction> {
        DemInstruction::error_from_targets(probability, targets, tag)
    }

    /// Returns the raw pinned flow descriptors for a gate.
    #[inline]
    pub fn gate_flow_descriptors(gate: Gate) -> Option<&'static [&'static str]> {
        crate::gate::gate_flow_descriptors(gate.kind())
    }

    /// Returns the raw pinned scalar unitary rows for a gate.
    #[inline]
    pub fn gate_unitary_rows(gate: Gate) -> Option<GateUnitaryRows> {
        crate::gate::gate_unitary_rows(gate.kind())
    }

    /// Returns the raw pinned H/S/CX/M/R decomposition descriptor for a gate.
    #[inline]
    pub fn gate_decomposition(gate: Gate) -> Option<GateDecomposition> {
        crate::gate::gate_decomposition_text(gate.kind()).map(GateDecomposition::new)
    }
}
