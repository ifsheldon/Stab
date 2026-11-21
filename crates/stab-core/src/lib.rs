//! Ergonomic facade for Stab's product crates.
//!
//! Component crates own algorithms, models, codecs, and execution sessions. The facade exposes
//! direct component namespaces plus a small root set of common canonical value types.

pub use stab_analysis as analysis;
pub use stab_decoder as decoder;
pub use stab_engine as execution;

pub use stab_algebra::{
    CliffordString, FlexPauliString, Flow, FlowMeasurementIndex, PauliBasis, PauliPhase, PauliSign,
    PauliString, SingleQubitClifford, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau, TwoQubitClifford, TwoQubitPauli,
};
pub use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeBatchSummary, DecodeContractError,
    DecodePreflightError, DecodeSessionFailure, DecoderInputBatchView, DecoderLayout,
    DecoderModelView, DecoderModelViewError, DecoderSession, ValidatedDecodeBatch,
};
pub use stab_model::{
    AbsoluteTolerance, ByteSpan, Cancellation, Cancelled, Circuit, CircuitDetectorId,
    CircuitInstruction, CircuitItem, CircuitTick, DecompositionStep, DemDetectorId,
    DemErrorMechanismTraversalLimits, DemErrorMechanismView, DemErrorMechanismVisitError,
    DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter, DemFlattenedInstructionIter,
    DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DemRepeatCount, DemTarget,
    DetectorErrorModel, DiagnosticSeverity, Estimate, Gate, GateArgumentRule, GateCategory,
    GateDecomposition, GateFlowDescriptor, GateKind, GatePauliString, GateTargetGroupKind,
    GateTargetRule, GateUnitaryRows, MeasureRecordOffset, MemoryBudget, MemoryBudgetExceeded,
    MemoryBudgetParseError, ModelDialect, ModelError, ModelFingerprint, ModelResult, ObservableId,
    ParseError, ParseErrorCode, ParseErrorContext, ParseLimits, Pauli, Probability,
    ProbabilityStimText, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    RepeatNestingLimitError, ResourceAmount, ResourceEstimate, Target, TargetKind, ValidationError,
    ValidationErrorCode,
};
pub use stab_records::{
    BitIndex, BitPlaneBatch, BitPlaneBatchView, CodecCapability, CorrectionWidth,
    DemSampleBatchView, DemSampleSink, DetectionBatchView, DetectionSink, DetectorWidth,
    FormatError, FormatErrorCode, FormatErrorContext, MeasurementBatchView, MeasurementSink,
    MeasurementWidth, ObservablePredictionBatch, ObservableWidth, PackedShotBatch,
    PackedShotBatchView, RecordBatchView, RecordEncoding, RecordFormat, RecordLayout,
    SampledErrorWidth, ShotIndex,
};
