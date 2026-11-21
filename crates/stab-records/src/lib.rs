//! Stable typed result records and Stim-compatible codecs.

mod batches;
mod diagnostics;
mod encoder;
mod index;
mod record_stream;
mod result_formats;
mod result_packed;
mod result_text;
mod sinks;
mod widths;

pub use batches::{
    BitPlaneBatch, BitPlaneBatchView, DemSampleBatchView, DetectionBatchView, MeasurementBatchView,
    ObservablePredictionBatch, ObservablePredictionBatchViewMut, PackedShotBatch,
    PackedShotBatchView, RecordBatchView, RecordLayout,
};
pub use diagnostics::{FormatError, FormatErrorCode, FormatErrorContext};
pub use encoder::{RecordBatchPart, RecordEncoder, RecordPart, RecordWriteError};
pub use index::{BitIndex, ShotIndex};
pub use record_stream::{
    HitsDuplicates, RecordFraming, RecordStreamReadError, RecordStreamReader, RecordStreamStorage,
};
pub use result_formats::{
    CodecCapability, DetsLayout, DetsResultType, RecordEncoding, RecordFormat, codec_capabilities,
    validate_ptb64_shot_count,
};
pub use sinks::{
    DemSampleCodecSink, DemSampleEncodedRecords, DemSampleSink, DetectionCodecSink, DetectionSink,
    MeasurementCodecSink, MeasurementSink,
};
pub use stab_foundation::{ByteSpan, DiagnosticSeverity, Estimate};
pub use widths::{
    CorrectionWidth, DetectorWidth, MeasurementWidth, ObservableWidth, SampledErrorWidth,
};

pub type RecordResult<T> = Result<T, FormatError>;
