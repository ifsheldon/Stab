use std::time::{Duration, Instant};

use crate::allocations::measure_tracked_memory;
use crate::error::BenchError;
use crate::report::Measurement;

use super::{STAB_COMPARE_ITERATIONS, duration_variance_seconds};

pub(super) fn measure_stab(
    name: &str,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    measure_stab_iterations(name, STAB_COMPARE_ITERATIONS, &mut operation)
}

pub(super) fn measure_stab_iterations(
    name: &str,
    iterations: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let (seconds, variance_seconds) = measure_stab_timings(iterations, &mut operation)?;
    let tracked_memory = measure_tracked_memory(&mut operation)?;
    Ok(stab_measurement(
        name,
        iterations,
        seconds,
        variance_seconds,
        tracked_memory,
    ))
}

pub(super) fn measure_stab_iterations_with_memory_operation(
    name: &str,
    iterations: usize,
    mut timed_operation: impl FnMut() -> Result<(), BenchError>,
    mut memory_operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let (seconds, variance_seconds) = measure_stab_timings(iterations, &mut timed_operation)?;
    let tracked_memory = measure_tracked_memory(&mut memory_operation)?;
    Ok(stab_measurement(
        name,
        iterations,
        seconds,
        variance_seconds,
        tracked_memory,
    ))
}

fn measure_stab_timings(
    iterations: usize,
    operation: &mut impl FnMut() -> Result<(), BenchError>,
) -> Result<(f64, Option<f64>), BenchError> {
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        operation()?;
        timings.push(start.elapsed());
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    Ok((seconds, variance_seconds))
}

fn stab_measurement(
    name: &str,
    iterations: usize,
    seconds: f64,
    variance_seconds: Option<f64>,
    tracked_memory: crate::allocations::TrackedMemoryMeasurement,
) -> Measurement {
    Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        resident_delta_bytes: tracked_memory.resident_delta_bytes_max,
        observations: Vec::new(),
        iterations: Some(iterations),
    }
}

pub(super) fn measure_stab_batched(
    name: &str,
    repetitions: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    measure_stab_batched_iterations(name, STAB_COMPARE_ITERATIONS, repetitions, &mut operation)
}

fn measure_stab_batched_iterations(
    name: &str,
    iterations: usize,
    repetitions: usize,
    mut operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        for _ in 0..repetitions {
            operation()?;
        }
        timings.push(start.elapsed().div_f64(repetitions as f64));
    }
    let variance_seconds = duration_variance_seconds(&timings);
    timings.sort();
    let seconds = timings
        .get(timings.len() / 2)
        .map(Duration::as_secs_f64)
        .unwrap_or_default();
    let tracked_memory = measure_tracked_memory(|| {
        for _ in 0..repetitions {
            operation()?;
        }
        Ok(())
    })?;
    Ok(Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds,
        allocation: tracked_memory.allocation,
        resident_bytes: tracked_memory.resident_bytes_max,
        resident_delta_bytes: tracked_memory.resident_delta_bytes_max,
        observations: Vec::new(),
        iterations: Some(iterations),
    })
}
