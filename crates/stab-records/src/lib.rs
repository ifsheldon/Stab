//! Stable typed result records and Stim-compatible codecs.

mod batches;
mod diagnostics;
mod estimate;
mod record_stream;
mod result_formats;
mod result_packed;
mod result_streaming;
mod result_text;
mod sinks;
mod widths;

pub use batches::{
    BitPlane64Batch, BitPlane64BatchView, DemSampleBatchView, DetectionBatchView,
    MeasurementBatchView, ObservablePredictionBatch, ObservablePredictionBatchViewMut,
    PackedShotBatch, PackedShotBatchView,
};
pub use diagnostics::{
    ByteSpan, DiagnosticSeverity, FormatError, FormatErrorCode, FormatErrorContext,
};
pub use estimate::EncodedSizeEstimate;
pub use record_stream::{RecordStreamReadError, RecordStreamReader};
pub use result_formats::{
    CodecCapability, DetsLayout, DetsResultType, DetsToken, MeasureRecord, MeasureRecordBatch,
    MeasureRecordBatchWriter, MeasureRecordWriter, RecordEncoding, RecordFormat, SparseShot,
    codec_capabilities, ptb64_record_count, read_dets_records, read_measurement_records,
    read_ptb64_records, read_ptb64_records_all, read_records, validate_ptb64_shot_count,
    write_bit_plane_64_batch, write_ptb64_records_checked, write_records,
};
pub use result_streaming::{
    RecordStreamError, for_each_dets_packed_record, for_each_dets_record,
    for_each_dets_sparse_shot, for_each_dets_token_record, for_each_packed_record,
    for_each_ptb64_record, for_each_ptb64_record_all, for_each_record, for_each_sparse_record,
    try_for_each_dets_packed_record, try_for_each_dets_record, try_for_each_dets_sparse_shot,
    try_for_each_dets_token_record, try_for_each_packed_record, try_for_each_ptb64_record,
    try_for_each_ptb64_record_all, try_for_each_record, try_for_each_sparse_record,
};
pub use sinks::{
    DemSampleCodecSink, DemSampleEncodedRecords, DemSampleSink, DetectionCodecSink, DetectionSink,
    MeasurementCodecSink, MeasurementSink,
};
pub use widths::{
    CorrectionWidth, DetectorWidth, MeasurementWidth, ObservableWidth, SampledErrorWidth,
};

pub type RecordResult<T> = Result<T, FormatError>;
