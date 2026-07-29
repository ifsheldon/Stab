use super::error::{DetectionError as CircuitError, DetectionResult as CircuitResult};

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
