use std::io::{self, Write};

use stab_core::{
    BitPlane64Batch, CircuitError, DemSampleBatchView, DetectionBatchView,
    DetectionObservableOutputMode, PackedShotBatchView, RecordFormat, SampleFormat,
    advanced::records::{DetsResultType, MeasureRecordWriter},
};

use crate::{
    CliError,
    streaming::{FileOutputSink, OutputSink},
};

pub(crate) struct DetectionBatchEncoder {
    detector_width: usize,
    observable_width: usize,
    observable_mode: DetectionObservableOutputMode,
    primary: RecordBatchEncoder,
    observable: Option<RecordBatchEncoder>,
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
            primary: RecordBatchEncoder::try_new(primary_format, primary_width)?,
            observable: observable_format
                .map(|format| RecordBatchEncoder::try_new(format, observable_width))
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
        primary_output.write_with(|output| {
            self.primary
                .write_detection_batch(batch, self.observable_mode, output)
        })?;
        if let (Some(encoder), Some(output)) = (&mut self.observable, observable_output) {
            output.write_with(|output| {
                encoder.write_single_batch(batch.observables(), DetsResultType::Observable, output)
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
        self.primary.validate_finish()?;
        if let Some(observable) = &self.observable {
            observable.validate_finish()?;
        }
        primary_output.write_with(|output| output.flush())?;
        if let Some(output) = observable_output {
            output.write_with(|output| output.flush())?;
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
    sampled_errors: Option<RecordBatchEncoder>,
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
                .map(|format| RecordBatchEncoder::try_new(format, sampled_error_width))
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
            output.write_with(|output| {
                encoder.write_single_batch(records, DetsResultType::Measurement, output)
            })?;
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
            sampled_errors.validate_finish()?;
        }
        self.detection.finish(primary_output, observable_output)?;
        if let Some(output) = sampled_error_output {
            output.write_with(|output| output.flush())?;
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

enum RecordBatchEncoder {
    Records {
        format: SampleFormat,
        width: usize,
        writer: MeasureRecordWriter,
    },
    Ptb64 {
        planes: BitPlane64Batch,
        pending_shots: usize,
    },
}

impl RecordBatchEncoder {
    fn try_new(format: RecordFormat, width: usize) -> Result<Self, CliError> {
        if format == RecordFormat::Ptb64 {
            return Ok(Self::Ptb64 {
                planes: BitPlane64Batch::zeros(64, width)
                    .map_err(|error| CliError::from(CircuitError::from(error)))?,
                pending_shots: 0,
            });
        }
        match format.sample_format() {
            Some(sample_format) => Ok(Self::records(sample_format, width)),
            None => Err(invalid_result_format(
                "record format is not supported by the CLI batch encoder",
            )),
        }
    }

    fn records(format: SampleFormat, width: usize) -> Self {
        Self::Records {
            format,
            width,
            writer: MeasureRecordWriter::new(format),
        }
    }

    fn write_detection_batch(
        &mut self,
        batch: DetectionBatchView<'_>,
        observable_mode: DetectionObservableOutputMode,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        for shot_index in 0..batch.shot_count() {
            let detectors = batch.detectors().shot(shot_index).map_err(format_io)?;
            let observables = batch.observables().shot(shot_index).map_err(format_io)?;
            let mut output_bit = 0;
            if observable_mode == DetectionObservableOutputMode::Prepend {
                self.write_part(DetsResultType::Observable, observables, &mut output_bit)?;
            }
            self.write_part(DetsResultType::Detector, detectors, &mut output_bit)?;
            if observable_mode == DetectionObservableOutputMode::Append {
                self.write_part(DetsResultType::Observable, observables, &mut output_bit)?;
            }
            self.finish_record(output_bit, output)?;
        }
        self.flush_records(output)
    }

    fn write_single_batch(
        &mut self,
        batch: PackedShotBatchView<'_>,
        result_type: DetsResultType,
        output: &mut dyn Write,
    ) -> io::Result<()> {
        for shot_index in 0..batch.shot_count() {
            let record = batch.shot(shot_index).map_err(format_io)?;
            let mut output_bit = 0;
            self.write_part(result_type, record, &mut output_bit)?;
            self.finish_record(output_bit, output)?;
        }
        self.flush_records(output)
    }

    fn write_part(
        &mut self,
        result_type: DetsResultType,
        bits: stab_core::advanced::storage::BitSlice<'_>,
        output_bit: &mut usize,
    ) -> io::Result<()> {
        match self {
            Self::Records { format, writer, .. } => {
                if *format == SampleFormat::Dets {
                    writer.begin_dets_result_type(result_type);
                }
                writer.write_packed_record(bits).map_err(format_io)?;
            }
            Self::Ptb64 {
                planes,
                pending_shots,
            } => {
                for bit_index in 0..bits.len() {
                    let bit = bits.get(bit_index).ok_or_else(|| {
                        io::Error::other(
                            "packed result bit escaped its declared width during PTB64 routing",
                        )
                    })?;
                    planes
                        .set(*output_bit, *pending_shots, bit)
                        .map_err(format_io)?;
                    *output_bit = output_bit
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("PTB64 routed result width overflowed"))?;
                }
                return Ok(());
            }
        }
        *output_bit = output_bit
            .checked_add(bits.len())
            .ok_or_else(|| io::Error::other("routed result width overflowed"))?;
        Ok(())
    }

    fn finish_record(&mut self, actual_width: usize, output: &mut dyn Write) -> io::Result<()> {
        let expected_width = self.width();
        if actual_width != expected_width {
            return Err(io::Error::other(format!(
                "routed result has width {actual_width} but the encoder expects {expected_width}"
            )));
        }
        match self {
            Self::Records { writer, .. } => writer.write_end(),
            Self::Ptb64 {
                planes,
                pending_shots,
            } => {
                *pending_shots += 1;
                if *pending_shots == 64 {
                    for bit_index in 0..planes.bits_per_shot() {
                        let word = planes
                            .plane(bit_index)
                            .map_err(format_io)?
                            .words()
                            .first()
                            .copied()
                            .ok_or_else(|| {
                                io::Error::other("PTB64 plane omitted its required 64-shot word")
                            })?;
                        output.write_all(&word.to_le_bytes())?;
                    }
                    *pending_shots = 0;
                }
            }
        }
        Ok(())
    }

    fn flush_records(&mut self, output: &mut dyn Write) -> io::Result<()> {
        let Self::Records { writer, .. } = self else {
            return Ok(());
        };
        output.write_all(writer.buffered_bytes())?;
        writer.clear_buffered_bytes().map_err(format_io)
    }

    fn validate_finish(&self) -> Result<(), CliError> {
        if let Self::Ptb64 { pending_shots, .. } = self
            && *pending_shots != 0
        {
            return Err(CliError::IncompletePtb64OutputGroup {
                count: *pending_shots,
            });
        }
        Ok(())
    }

    fn width(&self) -> usize {
        match self {
            Self::Records { width, .. } => *width,
            Self::Ptb64 { planes, .. } => planes.bits_per_shot(),
        }
    }
}

fn format_io(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

fn invalid_result_format(message: impl Into<String>) -> CliError {
    CliError::from(CircuitError::invalid_result_format(message))
}

#[cfg(test)]
mod tests {
    use stab_core::{DetectionBatchView, PackedShotBatch};

    use super::{DetectionObservableOutputMode, DetsResultType, RecordBatchEncoder, RecordFormat};

    #[test]
    fn detection_batch_encoding_preserves_namespace_order_across_partitions() {
        let detectors =
            PackedShotBatch::from_records(&[vec![true, false]], 2).expect("detector records");
        let observables =
            PackedShotBatch::from_records(&[vec![true]], 1).expect("observable records");
        let first = DetectionBatchView::try_new(detectors.view(), observables.view())
            .expect("detection batch");
        let detectors =
            PackedShotBatch::from_records(&[vec![false, true]], 2).expect("detector records");
        let observables =
            PackedShotBatch::from_records(&[vec![false]], 1).expect("observable records");
        let second = DetectionBatchView::try_new(detectors.view(), observables.view())
            .expect("detection batch");

        let mut encoder = RecordBatchEncoder::try_new(RecordFormat::Dets, 3).expect("DETS encoder");
        let mut output = Vec::new();
        encoder
            .write_detection_batch(first, DetectionObservableOutputMode::Prepend, &mut output)
            .expect("first batch");
        encoder
            .write_detection_batch(second, DetectionObservableOutputMode::Prepend, &mut output)
            .expect("second batch");
        encoder.validate_finish().expect("complete output");

        assert_eq!(output, b"shot L0 D0\nshot D1\n");
    }

    #[test]
    fn sampled_error_dets_output_uses_measurement_namespace() {
        let errors = PackedShotBatch::from_records(&[vec![true, false, true]], 3)
            .expect("sampled-error records");
        let mut encoder = RecordBatchEncoder::try_new(RecordFormat::Dets, 3).expect("DETS encoder");
        let mut output = Vec::new();

        encoder
            .write_single_batch(errors.view(), DetsResultType::Measurement, &mut output)
            .expect("sampled-error batch");

        assert_eq!(output, b"shot M0 M2\n");
    }

    #[test]
    fn ptb64_routing_writes_complete_groups_and_rejects_a_trailing_record() {
        let records = (0..64).map(|shot| vec![shot % 2 == 0]).collect::<Vec<_>>();
        let records =
            PackedShotBatch::from_records(&records, 1).expect("complete PTB64 record group");
        let mut encoder =
            RecordBatchEncoder::try_new(RecordFormat::Ptb64, 1).expect("PTB64 encoder");
        let mut output = Vec::new();

        encoder
            .write_single_batch(records.view(), DetsResultType::Measurement, &mut output)
            .expect("complete PTB64 group");
        encoder.validate_finish().expect("complete output");
        assert_eq!(output, 0x5555_5555_5555_5555_u64.to_le_bytes());

        let trailing =
            PackedShotBatch::from_records(&[vec![true]], 1).expect("trailing record batch");
        encoder
            .write_single_batch(trailing.view(), DetsResultType::Measurement, &mut output)
            .expect("buffer trailing record");
        let error = encoder
            .validate_finish()
            .expect_err("trailing PTB64 record should be rejected");
        assert!(matches!(
            error,
            crate::CliError::IncompletePtb64OutputGroup { count: 1 }
        ));
    }
}
