use stab_bits::BitSlice;

use crate::{
    BitPlane64Batch, DemSampleBatchView, DetectionBatchView, DetectorWidth, DetsResultType,
    FormatError, MeasureRecordWriter, MeasurementBatchView, MeasurementWidth, ObservableWidth,
    PackedShotBatch, RecordFormat, RecordResult, SampleFormat, SampledErrorWidth,
    write_bit_plane_64_batch,
};

/// Receives complete batches of measurement records.
pub trait MeasurementSink {
    type Error;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error>;

    fn finish(&mut self) -> Result<(), Self::Error>;
}

/// Receives complete batches of detector and observable records.
pub trait DetectionSink {
    type Error;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error>;

    fn finish(&mut self) -> Result<(), Self::Error>;
}

/// Receives complete batches produced by a detector-error-model sampler.
pub trait DemSampleSink {
    type Error;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error>;

    fn finish(&mut self) -> Result<(), Self::Error>;
}

/// In-memory result codec implementing [`MeasurementSink`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementCodecSink {
    width: MeasurementWidth,
    encoder: PackedRecordEncoder,
}

impl MeasurementCodecSink {
    pub fn try_new(format: RecordFormat, width: MeasurementWidth) -> RecordResult<Self> {
        Ok(Self {
            width,
            encoder: PackedRecordEncoder::try_new(format, width.get())?,
        })
    }

    pub const fn format(&self) -> RecordFormat {
        self.encoder.format()
    }

    pub const fn width(&self) -> MeasurementWidth {
        self.width
    }

    pub fn into_bytes(mut self) -> RecordResult<Vec<u8>> {
        self.finish()?;
        self.encoder.into_bytes()
    }
}

impl MeasurementSink for MeasurementCodecSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> RecordResult<()> {
        if batch.width() != self.width {
            return Err(width_mismatch(
                "measurement",
                batch.width().get(),
                self.width.get(),
            ));
        }
        self.encoder.reserve_records(batch.records().shot_count())?;
        for shot_index in 0..batch.records().shot_count() {
            let record = batch.records().shot(shot_index)?;
            self.encoder.write_parts(&[RecordPart {
                result_type: DetsResultType::Measurement,
                bits: record,
            }])?;
        }
        Ok(())
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.encoder.finish()
    }
}

/// In-memory result codec implementing [`DetectionSink`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionCodecSink {
    detector_width: DetectorWidth,
    observable_width: ObservableWidth,
    encoder: PackedRecordEncoder,
}

impl DetectionCodecSink {
    pub fn try_new(
        format: RecordFormat,
        detector_width: DetectorWidth,
        observable_width: ObservableWidth,
    ) -> RecordResult<Self> {
        let total_width = detector_width
            .get()
            .checked_add(observable_width.get())
            .ok_or_else(|| FormatError::invalid_data("detection record width overflowed"))?;
        Ok(Self {
            detector_width,
            observable_width,
            encoder: PackedRecordEncoder::try_new(format, total_width)?,
        })
    }

    pub const fn format(&self) -> RecordFormat {
        self.encoder.format()
    }

    pub const fn detector_width(&self) -> DetectorWidth {
        self.detector_width
    }

    pub const fn observable_width(&self) -> ObservableWidth {
        self.observable_width
    }

    pub fn into_bytes(mut self) -> RecordResult<Vec<u8>> {
        self.finish()?;
        self.encoder.into_bytes()
    }

    fn validate_batch(&self, batch: DetectionBatchView<'_>) -> RecordResult<()> {
        if batch.detector_width() != self.detector_width {
            return Err(width_mismatch(
                "detector",
                batch.detector_width().get(),
                self.detector_width.get(),
            ));
        }
        if batch.observable_width() != self.observable_width {
            return Err(width_mismatch(
                "observable",
                batch.observable_width().get(),
                self.observable_width.get(),
            ));
        }
        Ok(())
    }

    fn validate_finish(&self) -> RecordResult<()> {
        self.encoder.validate_finish()
    }
}

impl DetectionSink for DetectionCodecSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> RecordResult<()> {
        self.validate_batch(batch)?;
        self.encoder.reserve_records(batch.shot_count())?;
        for shot_index in 0..batch.shot_count() {
            let detectors = batch.detectors().shot(shot_index)?;
            let observables = batch.observables().shot(shot_index)?;
            self.encoder.write_parts(&[
                RecordPart {
                    result_type: DetsResultType::Detector,
                    bits: detectors,
                },
                RecordPart {
                    result_type: DetsResultType::Observable,
                    bits: observables,
                },
            ])?;
        }
        Ok(())
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.encoder.finish()
    }
}

/// Bytes emitted by [`DemSampleCodecSink`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemSampleEncodedRecords {
    detection_records: Vec<u8>,
    sampled_error_records: Option<Vec<u8>>,
}

impl DemSampleEncodedRecords {
    pub fn detection_records(&self) -> &[u8] {
        &self.detection_records
    }

    pub fn sampled_error_records(&self) -> Option<&[u8]> {
        self.sampled_error_records.as_deref()
    }

    pub fn into_parts(self) -> (Vec<u8>, Option<Vec<u8>>) {
        (self.detection_records, self.sampled_error_records)
    }
}

/// In-memory result codecs implementing [`DemSampleSink`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DemSampleCodecSink {
    detection: DetectionCodecSink,
    sampled_errors: Option<(SampledErrorWidth, PackedRecordEncoder)>,
}

impl DemSampleCodecSink {
    pub fn try_new(
        detection_format: RecordFormat,
        detector_width: DetectorWidth,
        observable_width: ObservableWidth,
        sampled_errors: Option<(RecordFormat, SampledErrorWidth)>,
    ) -> RecordResult<Self> {
        let sampled_errors = sampled_errors
            .map(|(format, width)| {
                PackedRecordEncoder::try_new(format, width.get()).map(|encoder| (width, encoder))
            })
            .transpose()?;
        Ok(Self {
            detection: DetectionCodecSink::try_new(
                detection_format,
                detector_width,
                observable_width,
            )?,
            sampled_errors,
        })
    }

    pub const fn detector_width(&self) -> DetectorWidth {
        self.detection.detector_width()
    }

    pub const fn observable_width(&self) -> ObservableWidth {
        self.detection.observable_width()
    }

    pub fn sampled_error_width(&self) -> Option<SampledErrorWidth> {
        self.sampled_errors.as_ref().map(|(width, _)| *width)
    }

    pub fn into_records(mut self) -> RecordResult<DemSampleEncodedRecords> {
        self.finish()?;
        let detection_records = self.detection.encoder.into_bytes()?;
        let sampled_error_records = self
            .sampled_errors
            .map(|(_, encoder)| encoder.into_bytes())
            .transpose()?;
        Ok(DemSampleEncodedRecords {
            detection_records,
            sampled_error_records,
        })
    }

    fn validate_batch(&self, batch: DemSampleBatchView<'_>) -> RecordResult<()> {
        self.detection.validate_batch(batch.detection())?;
        match (batch.sampled_error_width(), self.sampled_error_width()) {
            (None, None) => Ok(()),
            (Some(actual), Some(expected)) if actual == expected => Ok(()),
            (Some(actual), Some(expected)) => Err(width_mismatch(
                "sampled-error",
                actual.get(),
                expected.get(),
            )),
            (Some(_), None) => Err(FormatError::invalid_data(
                "DEM sample batch contains sampled errors but the sink has no sampled-error codec",
            )),
            (None, Some(_)) => Err(FormatError::invalid_data(
                "DEM sample batch omitted sampled errors required by the sink",
            )),
        }
    }
}

impl DemSampleSink for DemSampleCodecSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> RecordResult<()> {
        self.validate_batch(batch)?;
        self.detection.write_batch(batch.detection())?;
        if let (Some(records), Some((_, encoder))) =
            (batch.sampled_errors(), self.sampled_errors.as_mut())
        {
            encoder.reserve_records(records.shot_count())?;
            for shot_index in 0..records.shot_count() {
                encoder.write_parts(&[RecordPart {
                    result_type: DetsResultType::Measurement,
                    bits: records.shot(shot_index)?,
                }])?;
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.detection.validate_finish()?;
        if let Some((_, encoder)) = &self.sampled_errors {
            encoder.validate_finish()?;
        }
        self.detection.finish()?;
        if let Some((_, encoder)) = &mut self.sampled_errors {
            encoder.finish()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct RecordPart<'a> {
    result_type: DetsResultType,
    bits: BitSlice<'a>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PackedRecordEncoder {
    format: RecordFormat,
    width: usize,
    record_writer: Option<MeasureRecordWriter>,
    ptb64: Option<Ptb64Buffer>,
    finished: bool,
}

impl PackedRecordEncoder {
    fn try_new(format: RecordFormat, width: usize) -> RecordResult<Self> {
        let (record_writer, ptb64) = match sample_format(format) {
            Some(format) => (Some(MeasureRecordWriter::new(format)), None),
            None => (
                None,
                Some(Ptb64Buffer {
                    records: PackedShotBatch::zeros(64, width)?,
                    pending_shots: 0,
                    output: Vec::new(),
                }),
            ),
        };
        Ok(Self {
            format,
            width,
            record_writer,
            ptb64,
            finished: false,
        })
    }

    const fn format(&self) -> RecordFormat {
        self.format
    }

    fn reserve_records(&mut self, record_count: usize) -> RecordResult<()> {
        let additional = if let Some(buffer) = &self.ptb64 {
            let complete_groups = buffer
                .pending_shots
                .checked_add(record_count)
                .ok_or_else(|| FormatError::invalid_data("ptb64 record count overflowed"))?
                / 64;
            complete_groups
                .checked_mul(self.width)
                .and_then(|words| words.checked_mul(size_of::<u64>()))
                .ok_or_else(|| FormatError::invalid_data("ptb64 output size overflowed"))?
        } else {
            match self.format.estimate_output_bytes(record_count, self.width) {
                crate::EncodedSizeEstimate::Exact(bytes) => bytes,
                crate::EncodedSizeEstimate::Unknown => 0,
            }
        };
        if additional == 0 {
            return Ok(());
        }
        if let Some(writer) = &mut self.record_writer {
            writer.reserve_output(additional)?;
        } else if let Some(buffer) = &mut self.ptb64 {
            buffer.output.try_reserve(additional).map_err(|error| {
                FormatError::invalid_data(format!(
                    "ptb64 writer could not reserve {additional} output bytes: {error}"
                ))
            })?;
        }
        Ok(())
    }

    fn write_parts(&mut self, parts: &[RecordPart<'_>]) -> RecordResult<()> {
        if self.finished {
            return Err(FormatError::invalid_data(
                "cannot write a result batch after sink finalization",
            ));
        }
        let actual_width = parts.iter().try_fold(0_usize, |total, part| {
            total
                .checked_add(part.bits.len())
                .ok_or_else(|| FormatError::invalid_data("result record width overflowed"))
        })?;
        if actual_width != self.width {
            return Err(width_mismatch("result", actual_width, self.width));
        }

        if let Some(buffer) = &mut self.ptb64 {
            buffer.write_parts(parts)?;
            return Ok(());
        }

        let writer = self.record_writer.as_mut().ok_or_else(|| {
            FormatError::invalid_data("record codec has no active encoding state")
        })?;
        if self.format == RecordFormat::Dets {
            for part in parts {
                writer.begin_dets_result_type(part.result_type);
                writer.write_packed_record(part.bits)?;
            }
        } else {
            for part in parts {
                writer.write_packed_record(part.bits)?;
            }
        }
        writer.write_end();
        Ok(())
    }

    fn validate_finish(&self) -> RecordResult<()> {
        if let Some(buffer) = &self.ptb64
            && buffer.pending_shots != 0
        {
            return Err(FormatError::invalid_data(format!(
                "ptb64 sink requires complete groups of 64 records, got {} trailing records",
                buffer.pending_shots
            )));
        }
        Ok(())
    }

    fn finish(&mut self) -> RecordResult<()> {
        if self.finished {
            return Ok(());
        }
        self.validate_finish()?;
        self.finished = true;
        Ok(())
    }

    fn into_bytes(mut self) -> RecordResult<Vec<u8>> {
        self.finish()?;
        match (self.record_writer, self.ptb64) {
            (Some(writer), None) => Ok(writer.into_bytes()),
            (None, Some(buffer)) => Ok(buffer.output),
            _ => Err(FormatError::invalid_data(
                "record codec finished with inconsistent encoding state",
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Ptb64Buffer {
    records: PackedShotBatch,
    pending_shots: usize,
    output: Vec<u8>,
}

impl Ptb64Buffer {
    fn write_parts(&mut self, parts: &[RecordPart<'_>]) -> RecordResult<()> {
        let mut output_bit = 0;
        for part in parts {
            for input_bit in 0..part.bits.len() {
                let bit = part.bits.get(input_bit).ok_or_else(|| {
                    FormatError::invalid_data(
                        "packed record bit escaped its declared width while buffering ptb64",
                    )
                })?;
                self.records.set(self.pending_shots, output_bit, bit)?;
                output_bit = output_bit.checked_add(1).ok_or_else(|| {
                    FormatError::invalid_data("ptb64 buffered record width overflowed")
                })?;
            }
        }
        self.pending_shots += 1;
        if self.pending_shots == 64 {
            let planes = BitPlane64Batch::from_shot_major(self.records.view())?;
            self.output
                .extend_from_slice(&write_bit_plane_64_batch(planes.view())?);
            self.pending_shots = 0;
        }
        Ok(())
    }
}

const fn sample_format(format: RecordFormat) -> Option<SampleFormat> {
    match format {
        RecordFormat::ZeroOne => Some(SampleFormat::ZeroOne),
        RecordFormat::B8 => Some(SampleFormat::B8),
        RecordFormat::R8 => Some(SampleFormat::R8),
        RecordFormat::Hits => Some(SampleFormat::Hits),
        RecordFormat::Dets => Some(SampleFormat::Dets),
        RecordFormat::Ptb64 => None,
    }
}

fn width_mismatch(kind: &str, actual: usize, expected: usize) -> FormatError {
    FormatError::with_context(
        crate::FormatErrorCode::InvalidRecordWidth,
        format!("{kind} batch has width {actual} but the sink expects {expected}"),
        None,
        crate::FormatErrorContext::RecordWidth {
            actual_bits: actual,
            expected_bits: expected,
        },
    )
}
