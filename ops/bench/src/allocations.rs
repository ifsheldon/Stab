use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::BenchError;
use crate::report::AllocationMeasurement;

static ALLOCATION_TRACKING_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub(crate) struct AllocationTrackingGuard {
    previous: bool,
}

impl AllocationTrackingGuard {
    pub(crate) fn set(enabled: bool) -> Result<Self, BenchError> {
        if enabled && !cfg!(feature = "count-allocations") {
            return Err(BenchError::AllocationTrackingUnavailable);
        }
        let previous = ALLOCATION_TRACKING_ENABLED.swap(enabled, Ordering::SeqCst);
        Ok(Self { previous })
    }
}

impl Drop for AllocationTrackingGuard {
    fn drop(&mut self) {
        ALLOCATION_TRACKING_ENABLED.store(self.previous, Ordering::SeqCst);
    }
}

pub(crate) fn measure_allocations(
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Option<AllocationMeasurement>, BenchError> {
    if !ALLOCATION_TRACKING_ENABLED.load(Ordering::SeqCst) {
        return Ok(None);
    }
    measure_allocations_enabled(&mut operation)
}

#[cfg(feature = "count-allocations")]
fn measure_allocations_enabled(
    operation: &mut impl FnMut() -> Result<(), BenchError>,
) -> Result<Option<AllocationMeasurement>, BenchError> {
    let mut result = Ok(());
    let info = allocation_counter::measure(|| {
        result = operation();
    });
    result?;
    Ok(Some(AllocationMeasurement {
        count_total: info.count_total,
        count_current: info.count_current,
        count_max: info.count_max,
        bytes_total: info.bytes_total,
        bytes_current: info.bytes_current,
        bytes_max: info.bytes_max,
    }))
}

#[cfg(not(feature = "count-allocations"))]
fn measure_allocations_enabled(
    _operation: &mut impl FnMut() -> Result<(), BenchError>,
) -> Result<Option<AllocationMeasurement>, BenchError> {
    Err(BenchError::AllocationTrackingUnavailable)
}

#[cfg(test)]
mod tests {
    use super::AllocationTrackingGuard;
    use crate::error::BenchError;

    #[test]
    fn allocation_tracking_guard_requires_count_allocations_feature() {
        let result = AllocationTrackingGuard::set(true);
        if cfg!(feature = "count-allocations") {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(BenchError::AllocationTrackingUnavailable)
            ));
        }
    }
}
