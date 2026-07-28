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

pub(super) fn measure_stab_iterations_with_postprocess_and_memory_operation<State, Output>(
    name: &str,
    iterations: usize,
    state: &mut State,
    mut timed_operation: impl FnMut(&mut State) -> Result<Output, BenchError>,
    mut postprocess: impl FnMut(&mut State, Output) -> Result<(), BenchError>,
    mut memory_operation: impl FnMut() -> Result<(), BenchError>,
) -> Result<Measurement, BenchError> {
    let (seconds, variance_seconds) = measure_stab_timings_with_postprocess_and_clock(
        iterations,
        state,
        &mut timed_operation,
        &mut postprocess,
        Instant::now,
    )?;
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

fn measure_stab_timings_with_postprocess_and_clock<State, Output>(
    iterations: usize,
    state: &mut State,
    operation: &mut impl FnMut(&mut State) -> Result<Output, BenchError>,
    postprocess: &mut impl FnMut(&mut State, Output) -> Result<(), BenchError>,
    mut now: impl FnMut() -> Instant,
) -> Result<(f64, Option<f64>), BenchError> {
    let mut timings = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = now();
        let output = operation(state)?;
        let finish = now();
        let elapsed = finish
            .checked_duration_since(start)
            .ok_or(BenchError::NonMonotonicClock)?;
        postprocess(state, output)?;
        timings.push(elapsed);
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "measurement contract tests use direct assertions for precise failures"
    )]

    use std::cell::{Cell, RefCell};
    use std::time::{Duration, Instant};

    use super::measure_stab_timings_with_postprocess_and_clock;
    use crate::error::BenchError;

    #[test]
    fn output_postprocessing_occurs_after_the_finish_clock() {
        let origin = Instant::now();
        let clock_calls = Cell::new(0_usize);
        let events = RefCell::new(Vec::new());
        let mut state = ();

        let (seconds, variance) = measure_stab_timings_with_postprocess_and_clock(
            1,
            &mut state,
            &mut |_| {
                events.borrow_mut().push("raw-work");
                Ok(7_u64)
            },
            &mut |_, output| {
                events.borrow_mut().push("postprocess");
                assert_eq!(output, 7);
                Ok(())
            },
            || {
                let call = clock_calls.get();
                clock_calls.set(call + 1);
                events.borrow_mut().push(if call == 0 {
                    "start-clock"
                } else {
                    "finish-clock"
                });
                origin + Duration::from_millis((call * 5) as u64)
            },
        )
        .expect("measure one fake-clock operation");

        assert_eq!(
            events.into_inner(),
            ["start-clock", "raw-work", "finish-clock", "postprocess"]
        );
        assert_eq!(seconds, 0.005);
        assert_eq!(variance, Some(0.0));
    }

    #[test]
    fn output_timing_rejects_a_nonmonotonic_clock() {
        let origin = Instant::now();
        let clock_calls = Cell::new(0_usize);
        let mut state = ();
        let error = measure_stab_timings_with_postprocess_and_clock(
            1,
            &mut state,
            &mut |_| Ok(()),
            &mut |_, ()| Ok(()),
            || {
                let call = clock_calls.get();
                clock_calls.set(call + 1);
                origin + Duration::from_millis(if call == 0 { 5 } else { 0 })
            },
        )
        .expect_err("reject a backwards finish clock");

        assert!(matches!(error, BenchError::NonMonotonicClock));
    }
}
