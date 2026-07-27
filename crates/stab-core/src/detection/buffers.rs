use crate::resources::DetectionBufferLimitSubject;
use crate::{CircuitError, CircuitResult, DetectionEventRecord, ResourceLimitError};

pub(super) fn validate_buffer_bits(
    subject: DetectionBufferLimitSubject,
    shots: usize,
    bits_per_shot: usize,
    max_materialized_bits: usize,
) -> CircuitResult<()> {
    let kind = detection_buffer_subject_name(subject);
    let units_per_shot = bits_per_shot.max(1);
    let total = shots.checked_mul(units_per_shot).ok_or_else(|| {
        CircuitError::invalid_result_format(format!("{kind} bit count overflowed"))
    })?;
    if total > max_materialized_bits {
        return Err(ResourceLimitError::detection_materialized_bits(
            subject,
            resource_amount(total, "detection buffer bits")?,
            resource_amount(max_materialized_bits, "detection buffer bit limit")?,
        )
        .into());
    }
    Ok(())
}

pub(super) fn try_reserve_detection_record_slots(
    records: &mut Vec<DetectionEventRecord>,
    shots: usize,
) -> CircuitResult<()> {
    validate_vector_capacity::<DetectionEventRecord>(
        shots,
        "detection conversion record container",
    )?;
    records.try_reserve_exact(shots).map_err(|error| {
        CircuitError::invalid_result_format(format!(
            "could not allocate detection record container for {shots} shots: {error}"
        ))
    })
}

const fn detection_buffer_subject_name(subject: DetectionBufferLimitSubject) -> &'static str {
    match subject {
        DetectionBufferLimitSubject::MeasurementSamples => "measurement samples",
        DetectionBufferLimitSubject::DetectionRecords => "detection records",
        DetectionBufferLimitSubject::SweepRecords => "sweep records",
    }
}

pub(super) fn resource_amount(value: usize, context: &str) -> CircuitResult<u64> {
    u64::try_from(value).map_err(|_| {
        CircuitError::invalid_result_format(format!("{context} does not fit resource diagnostics"))
    })
}

pub(super) fn validate_vector_capacity<T>(len: usize, context: &'static str) -> CircuitResult<()> {
    let element_size = std::mem::size_of::<T>().max(1);
    if len > (isize::MAX as usize) / element_size {
        return Err(CircuitError::invalid_sampler_compilation(format!(
            "{context} exceeds the platform vector capacity"
        )));
    }
    Ok(())
}

pub(super) fn try_vec_with_capacity<T>(len: usize, context: &'static str) -> CircuitResult<Vec<T>> {
    validate_vector_capacity::<T>(len, context)?;
    let mut values = Vec::new();
    values.try_reserve_exact(len).map_err(|error| {
        CircuitError::invalid_sampler_compilation(format!(
            "unable to reserve {len} values for {context}: {error}"
        ))
    })?;
    Ok(values)
}

pub(super) fn try_false_vec(len: usize, context: &'static str) -> CircuitResult<Vec<bool>> {
    let mut values = try_vec_with_capacity(len, context)?;
    values.resize(len, false);
    Ok(values)
}

pub(super) fn try_clone_bool_slice(
    values: &[bool],
    context: &'static str,
) -> CircuitResult<Vec<bool>> {
    let mut cloned = try_vec_with_capacity(values.len(), context)?;
    cloned.extend_from_slice(values);
    Ok(cloned)
}

pub(super) fn try_clone_detection_record(
    record: &DetectionEventRecord,
) -> CircuitResult<DetectionEventRecord> {
    Ok(DetectionEventRecord {
        detectors: try_clone_bool_slice(&record.detectors, "detection conversion detector record")?,
        observables: try_clone_bool_slice(
            &record.observables,
            "detection conversion observable record",
        )?,
    })
}
