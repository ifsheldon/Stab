use super::{DemError, DemResult};

pub(super) fn validate_vector_capacity<T>(len: usize, context: &'static str) -> DemResult<()> {
    let element_size = std::mem::size_of::<T>().max(1);
    if len > (isize::MAX as usize) / element_size {
        return Err(DemError::invalid_sampler_compilation(format!(
            "{context} exceeds the platform vector capacity"
        )));
    }
    Ok(())
}

pub(super) fn try_vec_with_capacity<T>(len: usize, context: &'static str) -> DemResult<Vec<T>> {
    validate_vector_capacity::<T>(len, context)?;
    let mut values = Vec::new();
    values.try_reserve_exact(len).map_err(|error| {
        DemError::invalid_sampler_compilation(format!(
            "unable to reserve {len} values for {context}: {error}"
        ))
    })?;
    Ok(values)
}

pub(super) fn try_false_vec(len: usize, context: &'static str) -> DemResult<Vec<bool>> {
    let mut values = try_vec_with_capacity(len, context)?;
    values.resize(len, false);
    Ok(values)
}

pub(super) fn try_zero_words(len: usize, context: &'static str) -> DemResult<Vec<u64>> {
    let mut values = try_vec_with_capacity(len, context)?;
    values.resize(len, 0);
    Ok(values)
}
