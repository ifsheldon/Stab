use crate::{CircuitError, CircuitResult, DetectionEventRecord};

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
        detectors: try_clone_bool_slice(&record.detectors, "DEM detector record")?,
        observables: try_clone_bool_slice(&record.observables, "DEM observable record")?,
    })
}
