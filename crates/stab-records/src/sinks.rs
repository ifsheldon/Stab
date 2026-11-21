use crate::{
    DemSampleBatchView, DetectionBatchView, DetectorWidth, DetsResultType, FormatError,
    MeasurementBatchView, MeasurementWidth, ObservableWidth, RecordBatchPart, RecordEncoder,
    RecordFormat, RecordResult, RecordWriteError, SampledErrorWidth,
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
    output: EncodedOutput,
}

impl MeasurementCodecSink {
    pub fn try_new(format: RecordFormat, width: MeasurementWidth) -> RecordResult<Self> {
        Ok(Self {
            width,
            output: EncodedOutput::try_new(format, width.get())?,
        })
    }

    pub const fn format(&self) -> RecordFormat {
        self.output.encoder.format()
    }

    pub const fn width(&self) -> MeasurementWidth {
        self.width
    }

    pub fn reserve_records(&mut self, record_count: usize) -> RecordResult<()> {
        if self.output.finished {
            return Err(FormatError::invalid_data(
                "cannot reserve result records after sink finalization",
            ));
        }
        self.output.reserve_records(record_count)
    }

    pub fn into_bytes(mut self) -> RecordResult<Vec<u8>> {
        self.finish()?;
        Ok(self.output.bytes)
    }
}

impl MeasurementSink for MeasurementCodecSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> RecordResult<()> {
        self.output.ensure_open()?;
        if batch.width() != self.width {
            return Err(width_mismatch(
                "measurement",
                batch.width().get(),
                self.width.get(),
            ));
        }
        self.output.reserve_records(batch.shot_count())?;
        let EncodedOutput { encoder, bytes, .. } = &mut self.output;
        let result = encoder.write_batch(
            &[RecordBatchPart::new(
                DetsResultType::Measurement,
                batch.records(),
            )],
            bytes,
        );
        result.map_err(in_memory_error)
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.output.finish()
    }
}

/// In-memory result codec implementing [`DetectionSink`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionCodecSink {
    detector_width: DetectorWidth,
    observable_width: ObservableWidth,
    output: EncodedOutput,
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
            output: EncodedOutput::try_new(format, total_width)?,
        })
    }

    pub const fn format(&self) -> RecordFormat {
        self.output.encoder.format()
    }

    pub const fn detector_width(&self) -> DetectorWidth {
        self.detector_width
    }

    pub const fn observable_width(&self) -> ObservableWidth {
        self.observable_width
    }

    pub fn into_bytes(mut self) -> RecordResult<Vec<u8>> {
        self.finish()?;
        Ok(self.output.bytes)
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
}

impl DetectionSink for DetectionCodecSink {
    type Error = FormatError;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> RecordResult<()> {
        self.output.ensure_open()?;
        self.validate_batch(batch)?;
        self.output.write_batch(&[
            RecordBatchPart::new(DetsResultType::Detector, batch.detectors()),
            RecordBatchPart::new(DetsResultType::Observable, batch.observables()),
        ])
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.output.finish()
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
    sampled_errors: Option<(SampledErrorWidth, EncodedOutput)>,
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
                EncodedOutput::try_new(format, width.get()).map(|output| (width, output))
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
        Ok(DemSampleEncodedRecords {
            detection_records: self.detection.output.bytes,
            sampled_error_records: self.sampled_errors.map(|(_, output)| output.bytes),
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
        if let (Some(records), Some((_, output))) =
            (batch.sampled_errors(), self.sampled_errors.as_mut())
        {
            output.ensure_open()?;
            output.write_batch(&[RecordBatchPart::new(DetsResultType::Measurement, records)])?;
        }
        Ok(())
    }

    fn finish(&mut self) -> RecordResult<()> {
        self.detection.output.validate_finish()?;
        if let Some((_, output)) = &self.sampled_errors {
            output.validate_finish()?;
        }
        self.detection.output.finish()?;
        if let Some((_, output)) = &mut self.sampled_errors {
            output.finish()?;
        }
        Ok(())
    }
}

/// One encoder draining into caller-owned in-memory output with a finalization latch.
#[derive(Clone, Debug, Eq, PartialEq)]
struct EncodedOutput {
    encoder: RecordEncoder,
    bytes: Vec<u8>,
    finished: bool,
}

impl EncodedOutput {
    fn try_new(format: RecordFormat, width: usize) -> RecordResult<Self> {
        Ok(Self {
            encoder: RecordEncoder::try_new(format, width)?,
            bytes: Vec::new(),
            finished: false,
        })
    }

    fn ensure_open(&self) -> RecordResult<()> {
        if self.finished {
            return Err(FormatError::invalid_data(
                "cannot write a result batch after sink finalization",
            ));
        }
        Ok(())
    }

    fn reserve_records(&mut self, record_count: usize) -> RecordResult<()> {
        let format = self.encoder.format();
        let width = self.encoder.width();
        let additional = if format == RecordFormat::Ptb64 {
            let complete_groups = self
                .encoder
                .pending_group_records()
                .checked_add(record_count)
                .ok_or_else(|| FormatError::invalid_data("ptb64 record count overflowed"))?
                / 64;
            complete_groups
                .checked_mul(width)
                .and_then(|words| words.checked_mul(size_of::<u64>()))
                .ok_or_else(|| FormatError::invalid_data("ptb64 output size overflowed"))?
        } else {
            format
                .estimate_output_bytes(record_count, width)
                .value()
                .copied()
                .unwrap_or(0)
        };
        self.bytes.try_reserve(additional).map_err(|error| {
            FormatError::invalid_data(format!(
                "result sink could not reserve {additional} output bytes: {error}"
            ))
        })
    }

    fn write_batch(&mut self, parts: &[RecordBatchPart<'_>]) -> RecordResult<()> {
        self.ensure_open()?;
        let shot_count = parts.first().map_or(0, |part| part.records().shot_count());
        self.reserve_records(shot_count)?;
        self.encoder
            .write_batch(parts, &mut self.bytes)
            .map_err(in_memory_error)
    }

    fn validate_finish(&self) -> RecordResult<()> {
        let pending = self.encoder.pending_group_records();
        if pending != 0 {
            return Err(FormatError::invalid_data(format!(
                "ptb64 sink requires complete groups of 64 records, got {pending} trailing records"
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
}

/// Writing into a `Vec<u8>` cannot fail, so only encoding errors reach in-memory sinks.
fn in_memory_error(error: RecordWriteError) -> FormatError {
    match error {
        RecordWriteError::Format(error) => error,
        RecordWriteError::Io(error) => {
            FormatError::invalid_data(format!("in-memory result output failed: {error}"))
        }
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
