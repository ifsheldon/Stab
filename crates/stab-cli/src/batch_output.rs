//! Output routing for detection and DEM-sample batches.
//!
//! The CLI decides which stream receives which columns: detectors with observables appended,
//! prepended, or split into `--obs_out`, and sampled errors into `--err_out`. Every byte is
//! produced by the Records-owned [`RecordEncoder`].

use std::io::{self, Write};

use stab_records::{
    DemSampleBatchView, DetectionBatchView, DetsResultType, FormatError, FormatErrorCode,
    PackedShotBatchView, RecordBatchPart, RecordEncoder, RecordFormat, RecordWriteError,
};

use crate::{
    CliError,
    streaming::{FileOutputSink, OutputSink},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DetectionObservableOutputMode {
    DetectorsOnly,
    Append,
    Prepend,
}

pub(crate) struct DetectionBatchEncoder {
    detector_width: usize,
    observable_width: usize,
    observable_mode: DetectionObservableOutputMode,
    primary: RecordEncoder,
    observable: Option<RecordEncoder>,
}

impl DetectionBatchEncoder {
    pub(crate) fn try_new(
        detector_width: usize,
        observable_width: usize,
        observable_mode: DetectionObservableOutputMode,
        primary_format: RecordFormat,
        observable_format: Option<RecordFormat>,
    ) -> Result<Self, CliError> {
        let primary_width = match observable_mode {
            DetectionObservableOutputMode::DetectorsOnly => detector_width,
            DetectionObservableOutputMode::Append | DetectionObservableOutputMode::Prepend => {
                detector_width
                    .checked_add(observable_width)
                    .ok_or_else(|| invalid_result_format("detection output width overflowed"))?
            }
        };
        Ok(Self {
            detector_width,
            observable_width,
            observable_mode,
            primary: RecordEncoder::try_new(primary_format, primary_width)?,
            observable: observable_format
                .map(|format| RecordEncoder::try_new(format, observable_width))
                .transpose()?,
        })
    }

    pub(crate) fn write_batch<W>(
        &mut self,
        batch: DetectionBatchView<'_>,
        primary_output: &mut OutputSink<'_, W>,
        observable_output: Option<&mut FileOutputSink>,
    ) -> Result<(), CliError>
    where
        W: Write,
    {
        self.validate_batch(batch)?;
        self.validate_observable_route(observable_output.is_some())?;
        let detectors = RecordBatchPart::new(DetsResultType::Detector, batch.detectors());
        let observables = RecordBatchPart::new(DetsResultType::Observable, batch.observables());
        let primary_parts = match self.observable_mode {
            DetectionObservableOutputMode::DetectorsOnly => &[detectors][..],
            DetectionObservableOutputMode::Append => &[detectors, observables],
            DetectionObservableOutputMode::Prepend => &[observables, detectors],
        };
        primary_output.write_with(|output| {
            self.primary
                .write_batch(primary_parts, output)
                .map_err(record_write_io)
        })?;
        if let (Some(encoder), Some(output)) = (&mut self.observable, observable_output) {
            output.write_with(|output| {
                encoder
                    .write_batch(&[observables], output)
                    .map_err(record_write_io)
            })?;
        }
        Ok(())
    }

    pub(crate) fn finish<W>(
        &mut self,
        primary_output: &mut OutputSink<'_, W>,
        observable_output: Option<&mut FileOutputSink>,
    ) -> Result<(), CliError>
    where
        W: Write,
    {
        self.validate_observable_route(observable_output.is_some())?;
        validate_complete_groups(&self.primary)?;
        if let Some(observable) = &self.observable {
            validate_complete_groups(observable)?;
        }
        primary_output.flush()?;
        if let Some(output) = observable_output {
            output.flush()?;
        }
        Ok(())
    }

    fn validate_batch(&self, batch: DetectionBatchView<'_>) -> Result<(), CliError> {
        if batch.detector_width().get() != self.detector_width {
            return Err(invalid_result_format(format!(
                "detector batch has width {} but the output expects {}",
                batch.detector_width().get(),
                self.detector_width
            )));
        }
        if batch.observable_width().get() != self.observable_width {
            return Err(invalid_result_format(format!(
                "observable batch has width {} but the output expects {}",
                batch.observable_width().get(),
                self.observable_width
            )));
        }
        Ok(())
    }

    fn validate_observable_route(&self, has_output: bool) -> Result<(), CliError> {
        if self.observable.is_some() != has_output {
            return Err(CliError::IoPlanInvariant {
                message: "detection batch encoder and observable output route disagree",
            });
        }
        Ok(())
    }
}

pub(crate) struct DemSampleBatchEncoder {
    detection: DetectionBatchEncoder,
    sampled_error_width: usize,
    sampled_errors: Option<RecordEncoder>,
}

impl DemSampleBatchEncoder {
    pub(crate) fn try_new(
        detector_width: usize,
        observable_width: usize,
        sampled_error_width: usize,
        observable_mode: DetectionObservableOutputMode,
        primary_format: RecordFormat,
        observable_format: Option<RecordFormat>,
        sampled_error_format: Option<RecordFormat>,
    ) -> Result<Self, CliError> {
        Ok(Self {
            detection: DetectionBatchEncoder::try_new(
                detector_width,
                observable_width,
                observable_mode,
                primary_format,
                observable_format,
            )?,
            sampled_error_width,
            sampled_errors: sampled_error_format
                .map(|format| RecordEncoder::try_new(format, sampled_error_width))
                .transpose()?,
        })
    }

    pub(crate) fn write_batch<W>(
        &mut self,
        batch: DemSampleBatchView<'_>,
        primary_output: &mut OutputSink<'_, W>,
        observable_output: Option<&mut FileOutputSink>,
        sampled_error_output: Option<&mut FileOutputSink>,
    ) -> Result<(), CliError>
    where
        W: Write,
    {
        self.validate_sampled_errors(batch, sampled_error_output.is_some())?;
        self.detection
            .write_batch(batch.detection(), primary_output, observable_output)?;
        if let (Some(records), Some(encoder), Some(output)) = (
            batch.sampled_errors(),
            &mut self.sampled_errors,
            sampled_error_output,
        ) {
            output.write_with(|output| write_single(encoder, records, output))?;
        }
        Ok(())
    }

    pub(crate) fn finish<W>(
        &mut self,
        primary_output: &mut OutputSink<'_, W>,
        observable_output: Option<&mut FileOutputSink>,
        sampled_error_output: Option<&mut FileOutputSink>,
    ) -> Result<(), CliError>
    where
        W: Write,
    {
        if self.sampled_errors.is_some() != sampled_error_output.is_some() {
            return Err(CliError::IoPlanInvariant {
                message: "DEM batch encoder and sampled-error output route disagree",
            });
        }
        if let Some(sampled_errors) = &self.sampled_errors {
            validate_complete_groups(sampled_errors)?;
        }
        self.detection.finish(primary_output, observable_output)?;
        if let Some(output) = sampled_error_output {
            output.flush()?;
        }
        Ok(())
    }

    fn validate_sampled_errors(
        &self,
        batch: DemSampleBatchView<'_>,
        has_output: bool,
    ) -> Result<(), CliError> {
        let actual_width = batch.sampled_error_width().map(|width| width.get());
        if actual_width.is_some_and(|width| width != self.sampled_error_width) {
            return Err(invalid_result_format(format!(
                "sampled-error batch has width {actual_width:?} but the plan declares {}",
                self.sampled_error_width
            )));
        }
        if has_output && actual_width.is_none() {
            return Err(invalid_result_format(
                "sampled-error output was requested but the DEM batch omitted sampled errors",
            ));
        }
        if self.sampled_errors.is_some() != has_output {
            return Err(CliError::IoPlanInvariant {
                message: "DEM batch encoder and sampled-error output route disagree",
            });
        }
        Ok(())
    }
}

/// Sampled errors use the measurement namespace, so DETS writes them as `M<index>` tokens.
fn write_single(
    encoder: &mut RecordEncoder,
    records: PackedShotBatchView<'_>,
    output: &mut dyn Write,
) -> io::Result<()> {
    encoder
        .write_batch(
            &[RecordBatchPart::new(DetsResultType::Measurement, records)],
            output,
        )
        .map_err(record_write_io)
}

fn validate_complete_groups(encoder: &RecordEncoder) -> Result<(), CliError> {
    match encoder.pending_group_records() {
        0 => Ok(()),
        count => Err(CliError::IncompletePtb64OutputGroup { count }),
    }
}

/// Keeps writer failures as the original I/O error, so the existing write diagnostics and the
/// broken-pipe exit path see them unchanged, and reports encoder layout errors as invalid data.
pub(crate) fn record_write_io(error: RecordWriteError) -> io::Error {
    match error {
        RecordWriteError::Io(error) => error,
        RecordWriteError::Format(error) => {
            io::Error::new(io::ErrorKind::InvalidData, error.to_string())
        }
    }
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::Record(FormatError::new(
        FormatErrorCode::InvalidData,
        message,
        None,
    ))
}

#[cfg(test)]
mod tests;
