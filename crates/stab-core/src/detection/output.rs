use super::{
    DetectionConversionOutput, DetectionEventRecord, DetectionObservableOutputMode,
    buffers::{try_clone_bool_slice, try_vec_with_capacity},
};
use crate::{
    CircuitError, CircuitResult, SampleFormat,
    result_formats::{DetsResultType, MeasureRecordWriter, write_ptb64_records_checked},
};

pub fn write_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        // `begin_dets_result_type` is a no-op on non-DETS writers, mirroring upstream Stim's
        // single writer-driving loop in measurements_to_detection_events.cc.
        if observable_mode == DetectionObservableOutputMode::Prepend {
            writer.begin_dets_result_type(DetsResultType::Observable);
            writer.write_bits(&record.observables);
        }
        writer.begin_dets_result_type(DetsResultType::Detector);
        writer.write_bits(&record.detectors);
        if observable_mode == DetectionObservableOutputMode::Append {
            writer.begin_dets_result_type(DetsResultType::Observable);
            writer.write_bits(&record.observables);
        }
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_observable_records(
    output: &DetectionConversionOutput,
    format: SampleFormat,
) -> CircuitResult<Vec<u8>> {
    let mut writer = MeasureRecordWriter::new(format);
    for record in &output.records {
        validate_record_widths(output, record)?;
        writer.begin_dets_result_type(DetsResultType::Observable);
        writer.write_bits(&record.observables);
        writer.write_end();
    }
    Ok(writer.into_bytes())
}

pub fn write_ptb64_detection_records(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<u8>> {
    let records = detection_records_as_bits(output, observable_mode)?;
    write_ptb64_records_checked(&records)
}

pub fn write_ptb64_observable_records(
    output: &DetectionConversionOutput,
) -> CircuitResult<Vec<u8>> {
    let records = observable_records_as_bits(output)?;
    write_ptb64_records_checked(&records)
}

fn detection_records_as_bits(
    output: &DetectionConversionOutput,
    observable_mode: DetectionObservableOutputMode,
) -> CircuitResult<Vec<Vec<bool>>> {
    let mut records =
        try_vec_with_capacity(output.records.len(), "ptb64 detection record container")?;
    for record in &output.records {
        validate_record_widths(output, record)?;
        let capacity = match observable_mode {
            DetectionObservableOutputMode::DetectorsOnly => output.detector_count,
            DetectionObservableOutputMode::Append | DetectionObservableOutputMode::Prepend => {
                output
                    .detector_count
                    .checked_add(output.observable_count)
                    .ok_or_else(|| {
                        CircuitError::invalid_result_format(
                            "detection record width overflowed while writing ptb64 output",
                        )
                    })?
            }
        };
        let mut bits = try_vec_with_capacity(capacity, "ptb64 detection record")?;
        if observable_mode == DetectionObservableOutputMode::Prepend {
            bits.extend_from_slice(&record.observables);
        }
        bits.extend_from_slice(&record.detectors);
        if observable_mode == DetectionObservableOutputMode::Append {
            bits.extend_from_slice(&record.observables);
        }
        records.push(bits);
    }
    Ok(records)
}

fn observable_records_as_bits(output: &DetectionConversionOutput) -> CircuitResult<Vec<Vec<bool>>> {
    let mut records =
        try_vec_with_capacity(output.records.len(), "ptb64 observable record container")?;
    for record in &output.records {
        validate_record_widths(output, record)?;
        records.push(try_clone_bool_slice(
            &record.observables,
            "ptb64 observable record",
        )?);
    }
    Ok(records)
}

fn validate_record_widths(
    output: &DetectionConversionOutput,
    record: &DetectionEventRecord,
) -> CircuitResult<()> {
    if record.detectors.len() != output.detector_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} detector bits but expected {}",
            record.detectors.len(),
            output.detector_count
        )));
    }
    if record.observables.len() != output.observable_count {
        return Err(CircuitError::invalid_result_format(format!(
            "detection record has {} observable bits but expected {}",
            record.observables.len(),
            output.observable_count
        )));
    }
    Ok(())
}
