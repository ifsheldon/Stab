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
    Tableau,
};
pub use stab_decoder::{
    DecodeBatchError, DecodeBatchStatus, DecodeBatchSummary, DecodeCancellation,
    DecodeContractError, DecodePreflightError, DecodeSessionFailure, DecoderInputBatchView,
    DecoderLayout, DecoderModelView, DecoderModelViewError, DecoderSession, ValidatedDecodeBatch,
};
pub use stab_model::{
    ByteSpan, Circuit, CircuitDetectorId, CircuitInstruction, CircuitItem, CircuitTick,
    DemDetectorId, DemErrorMechanismTraversalLimits, DemErrorMechanismView,
    DemErrorMechanismVisitError, DemErrorMechanismVisitor, DemErrorTarget, DemErrorTargetIter,
    DemFlattenedInstructionIter, DemInstruction, DemInstructionKind, DemItem, DemObservableId,
    DemRepeatBlock, DemRepeatCount, DemTarget, DetectorErrorModel, DiagnosticSeverity, Estimate,
    EstimateClass, Gate, GateArgumentRule, GateCategory, GateDecomposition, GateTargetGroupKind,
    GateTargetRule, MeasureRecordOffset, ModelDialect, ModelError, ModelFingerprint, ModelResult,
    ObservableId, ParseError, ParseErrorCode, ParseErrorContext, ParseLimits, Pauli, Probability,
    ProbabilityStimText, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    RepeatNestingLimitError, ResourceEstimate, SourceLineLimit, Target, ValidationError,
    ValidationErrorCode,
};
pub use stab_records::{
    BitPlane64Batch, BitPlane64BatchView, CodecCapability, CorrectionWidth, DemSampleBatchView,
    DemSampleSink, DetectionBatchView, DetectionSink, DetectorWidth, EncodedSizeEstimate,
    FormatError, FormatErrorCode, FormatErrorContext, MeasurementBatchView, MeasurementSink,
    MeasurementWidth, ObservablePredictionBatch, ObservableWidth, PackedShotBatch,
    PackedShotBatchView, RecordEncoding, RecordFormat, SampledErrorWidth,
};
