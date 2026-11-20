//! Stable typed result records and Stim-compatible codecs.

mod batches;
mod diagnostics;
mod encoder;
mod estimate;
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
pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, FormatError, FormatErrorCode, FormatErrorContext,
};
pub use encoder::{RecordBatchPart, RecordEncoder, RecordPart, RecordWriteError};
pub use estimate::EncodedSizeEstimate;
pub use index::{BitIndex, ShotIndex};
pub use record_stream::{
    HitsDuplicates, RecordFraming, RecordStreamReadError, RecordStreamReader, RecordStreamStorage,
};
pub use result_formats::{
    CodecCapability, DetsLayout, DetsResultType, MeasureRecordWriter, RecordEncoding, RecordFormat,
    codec_capabilities, ptb64_record_count, read_dets_records, read_measurement_records,
    read_ptb64_records, read_ptb64_records_all, read_records, validate_ptb64_shot_count,
    write_ptb64_records_checked, write_records,
};
pub use sinks::{
    DemSampleCodecSink, DemSampleEncodedRecords, DemSampleSink, DetectionCodecSink, DetectionSink,
    MeasurementCodecSink, MeasurementSink,
};
pub use widths::{
    CorrectionWidth, DetectorWidth, MeasurementWidth, ObservableWidth, SampledErrorWidth,
};

pub type RecordResult<T> = Result<T, FormatError>;
