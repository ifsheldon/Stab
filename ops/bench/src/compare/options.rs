use super::CompareOptions;
use crate::error::BenchError;

pub(super) fn validate_compare_options(options: &CompareOptions) -> Result<(), BenchError> {
    if options.require_profiler_notes && options.report.is_none() {
        return Err(BenchError::ProfilerNotesRequireReport);
    }
    if options.memory_baseline.is_some() && !options.require_memory_gate {
        return Err(BenchError::MemoryBaselineRequiresGate);
    }
    if options.beta_waivers.is_some() && !options.require_beta_gate {
        return Err(BenchError::BetaWaiversRequireGate);
    }
    if options.regression_waivers.is_some() && options.thresholds.is_none() {
        return Err(BenchError::RegressionWaiversRequireThresholds);
    }
    if options.track_allocations
        && (options.require_beta_gate
            || options.beta_waivers.is_some()
            || options.thresholds.is_some()
            || options.regression_waivers.is_some())
    {
        return Err(BenchError::AllocationTrackingTimingGateConflict);
    }
    if options.require_memory_gate && !options.track_allocations {
        return Err(BenchError::MemoryGateRequiresAllocationTracking);
    }
    if options.require_memory_gate && options.memory_baseline.is_none() {
        return Err(BenchError::MemoryGateRequiresBaseline);
    }
    if options.measurement_runs == 0 {
        return Err(BenchError::InvalidMeasurementRuns);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::validate_compare_options;
    use crate::compare::CompareOptions;
    use crate::error::BenchError;

    #[test]
    fn allocation_instrumentation_rejects_timing_gate_options() {
        let mut options = compare_options();
        options.track_allocations = true;
        options.require_beta_gate = true;
        assert!(matches!(
            validate_compare_options(&options),
            Err(BenchError::AllocationTrackingTimingGateConflict)
        ));

        options.require_beta_gate = false;
        options.thresholds = Some(PathBuf::from("thresholds.json"));
        assert!(matches!(
            validate_compare_options(&options),
            Err(BenchError::AllocationTrackingTimingGateConflict)
        ));
    }

    fn compare_options() -> CompareOptions {
        CompareOptions {
            baseline: PathBuf::from("baseline.json"),
            milestone: None,
            profile: "release".to_string(),
            primary: false,
            only: Vec::new(),
            report: None,
            require_profiler_notes: false,
            profiler_notes_dirs: Vec::new(),
            require_beta_gate: false,
            beta_waivers: None,
            require_memory_gate: false,
            memory_baseline: None,
            thresholds: None,
            regression_waivers: None,
            track_allocations: false,
            warmup: false,
            measurement_runs: 1,
            strict: false,
        }
    }
}
