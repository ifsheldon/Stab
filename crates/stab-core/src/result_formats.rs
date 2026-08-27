use crate::{CircuitError, CircuitResult};

pub use stab_records::{
    BitPlane64Batch, BitPlane64BatchView, CodecCapability, CorrectionWidth, DemSampleBatchView,
    DemSampleCodecSink, DemSampleEncodedRecords, DemSampleSink, DetectionBatchView,
    DetectionCodecSink, DetectionSink, DetectorWidth, DetsLayout, DetsResultType, DetsToken,
    EncodedSizeEstimate, MeasureRecord, MeasureRecordBatch, MeasureRecordBatchWriter,
    MeasureRecordWriter, MeasurementBatchView, MeasurementCodecSink, MeasurementSink,
    MeasurementWidth, ObservablePredictionBatch, ObservableWidth, PackedShotBatch,
    PackedShotBatchView, RecordEncoding, RecordFormat, SampledErrorWidth, SparseShot,
};

pub(crate) use stab_records::codec_capabilities;

pub fn write_records(records: &[Vec<bool>], format: RecordFormat) -> CircuitResult<Vec<u8>> {
    stab_records::write_records(records, format).map_err(record_error)
}

pub fn write_ptb64_records_checked(records: &[Vec<bool>]) -> CircuitResult<Vec<u8>> {
    validate_ptb64_shot_count(records.len())?;
    stab_records::write_ptb64_records_checked(records).map_err(record_error)
}

pub fn write_bit_plane_64_batch(batch: BitPlane64BatchView<'_>) -> CircuitResult<Vec<u8>> {
    validate_ptb64_shot_count(batch.shot_count())?;
    stab_records::write_bit_plane_64_batch(batch).map_err(record_error)
}

pub fn read_ptb64_records(
    input: &[u8],
    bits_per_record: usize,
    max_shots: usize,
) -> CircuitResult<Vec<Vec<bool>>> {
    validate_ptb64_shot_count(max_shots)?;
    stab_records::read_ptb64_records(input, bits_per_record, max_shots).map_err(record_error)
}

pub fn read_ptb64_records_all(
    input: &[u8],
    bits_per_record: usize,
) -> CircuitResult<Vec<Vec<bool>>> {
    stab_records::read_ptb64_records_all(input, bits_per_record).map_err(record_error)
}

pub fn ptb64_record_count(input: &[u8], bits_per_record: usize) -> CircuitResult<usize> {
    stab_records::ptb64_record_count(input, bits_per_record).map_err(record_error)
}

pub fn validate_ptb64_shot_count(shots: usize) -> CircuitResult<()> {
    if !shots.is_multiple_of(64) {
        return Err(CircuitError::invalid_sampler_compilation(
            "shots must be a multiple of 64 to use ptb64 format",
        ));
    }
    Ok(())
}

pub fn read_records(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
) -> CircuitResult<Vec<Vec<bool>>> {
    stab_records::read_records(input, format, bits_per_record).map_err(record_error)
}

pub fn read_measurement_records(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
) -> CircuitResult<Vec<Vec<bool>>> {
    stab_records::read_measurement_records(input, format, bits_per_record).map_err(record_error)
}

pub fn read_dets_records(input: &[u8], layout: DetsLayout) -> CircuitResult<Vec<Vec<bool>>> {
    stab_records::read_dets_records(input, layout).map_err(record_error)
}

fn record_error(error: stab_records::FormatError) -> CircuitError {
    CircuitError::from(crate::FormatError::from(error))
}
