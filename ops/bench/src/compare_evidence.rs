//! Measurement aggregation and ratio evidence for benchmark comparisons.

use crate::comparability::ComparabilityClass;
use crate::error::BenchError;
use crate::report::{AllocationMeasurement, Measurement, MeasurementRatio};

pub(crate) fn aggregate_measurement_runs(
    row_id: &str,
    runs: Vec<Vec<Measurement>>,
) -> Result<Vec<Measurement>, BenchError> {
    let Some(first) = runs.first() else {
        return Ok(Vec::new());
    };
    let measurement_count = first.len();
    for run in &runs {
        if run.len() != measurement_count {
            return Err(inconsistent_measurement_runs(row_id));
        }
    }
    let mut measurements = Vec::with_capacity(measurement_count);
    for index in 0..measurement_count {
        let first_measurement = first
            .get(index)
            .ok_or_else(|| inconsistent_measurement_runs(row_id))?;
        let name = &first_measurement.name;
        if runs.iter().any(|run| {
            run.get(index)
                .is_none_or(|measurement| measurement.name != *name)
        }) {
            return Err(inconsistent_measurement_runs(row_id));
        }
        let mut seconds = runs
            .iter()
            .map(|run| {
                run.get(index)
                    .map(|measurement| measurement.seconds)
                    .ok_or_else(|| inconsistent_measurement_runs(row_id))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let variance_seconds = variance_seconds(&seconds);
        seconds.sort_by(f64::total_cmp);
        measurements.push(Measurement {
            name: name.clone(),
            seconds: seconds
                .get(seconds.len() / 2)
                .copied()
                .unwrap_or(first_measurement.seconds),
            variance_seconds,
            allocation: aggregate_allocations(&runs, index),
            iterations: aggregate_iterations(&runs, index),
        });
    }
    Ok(measurements)
}

fn aggregate_allocations(
    runs: &[Vec<Measurement>],
    measurement_index: usize,
) -> Option<AllocationMeasurement> {
    let mut allocations = runs
        .iter()
        .filter_map(|run| run.get(measurement_index))
        .filter_map(|measurement| measurement.allocation.as_ref());
    let first = allocations.next()?.clone();
    Some(allocations.fold(first, |mut aggregate, allocation| {
        aggregate.count_total = aggregate.count_total.max(allocation.count_total);
        aggregate.count_current = aggregate.count_current.max(allocation.count_current);
        aggregate.count_max = aggregate.count_max.max(allocation.count_max);
        aggregate.bytes_total = aggregate.bytes_total.max(allocation.bytes_total);
        aggregate.bytes_current = aggregate.bytes_current.max(allocation.bytes_current);
        aggregate.bytes_max = aggregate.bytes_max.max(allocation.bytes_max);
        aggregate
    }))
}

fn aggregate_iterations(runs: &[Vec<Measurement>], measurement_index: usize) -> Option<usize> {
    runs.iter()
        .filter_map(|run| run.get(measurement_index))
        .map(|measurement| measurement.iterations)
        .try_fold(0usize, |total, iterations| {
            iterations.and_then(|iterations| total.checked_add(iterations))
        })
}

fn variance_seconds(seconds: &[f64]) -> Option<f64> {
    if seconds.len() < 2 {
        return None;
    }
    let mean = seconds.iter().sum::<f64>() / seconds.len() as f64;
    let variance = seconds
        .iter()
        .map(|seconds| {
            let delta = seconds - mean;
            delta * delta
        })
        .sum::<f64>()
        / seconds.len() as f64;
    Some(variance)
}

fn inconsistent_measurement_runs(row_id: &str) -> BenchError {
    BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: "repeated Stab measurement runs produced inconsistent measurement shapes"
            .to_string(),
    }
}

pub(crate) fn paired_measurement_ratios(
    stim_measurements: &[Measurement],
    stab_measurements: &[Measurement],
    comparability: ComparabilityClass,
) -> Vec<MeasurementRatio> {
    let mut ratios = exact_name_measurement_ratios(stim_measurements, stab_measurements);
    if ratios.is_empty()
        && comparability.allows_positional_measurement_pairs()
        && stim_measurements.len() == stab_measurements.len()
    {
        ratios = positional_measurement_ratios(stim_measurements, stab_measurements);
    }
    ratios
}

fn exact_name_measurement_ratios(
    stim_measurements: &[Measurement],
    stab_measurements: &[Measurement],
) -> Vec<MeasurementRatio> {
    let mut ratios = Vec::new();
    let mut available_stab = stab_measurements.iter().collect::<Vec<_>>();
    for stim in stim_measurements {
        let stim_key = normalized_measurement_name(&stim.name);
        let Some(stab_index) = available_stab
            .iter()
            .position(|stab| normalized_measurement_name(&stab.name) == stim_key)
        else {
            continue;
        };
        let stab = available_stab.remove(stab_index);
        if let Some(ratio) = measurement_ratio(stim, stab) {
            ratios.push(ratio);
        }
    }
    ratios
}

fn positional_measurement_ratios(
    stim_measurements: &[Measurement],
    stab_measurements: &[Measurement],
) -> Vec<MeasurementRatio> {
    stim_measurements
        .iter()
        .zip(stab_measurements)
        .filter_map(|(stim, stab)| measurement_ratio(stim, stab))
        .collect()
}

fn measurement_ratio(stim: &Measurement, stab: &Measurement) -> Option<MeasurementRatio> {
    (stim.seconds > 0.0).then(|| MeasurementRatio {
        stim_name: stim.name.clone(),
        stab_name: stab.name.clone(),
        stim_seconds: stim.seconds,
        stab_seconds: stab.seconds,
        relative_ratio: stab.seconds / stim.seconds,
    })
}

fn normalized_measurement_name(name: &str) -> String {
    name.strip_prefix("stab_")
        .unwrap_or(name)
        .chars()
        .filter(|character| *character != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::aggregate_measurement_runs;
    use crate::report::Measurement;

    #[test]
    fn repeated_measurement_runs_use_median_seconds_and_validate_shape() {
        let runs = vec![
            vec![measurement_with_iterations("stab_case", 3.0, Some(2))],
            vec![measurement_with_iterations("stab_case", 1.0, Some(2))],
            vec![measurement_with_iterations("stab_case", 2.0, Some(2))],
        ];

        let aggregate = aggregate_measurement_runs("row", runs).expect("aggregate measurements");

        let aggregate_measurement = aggregate.first().expect("measurement");
        assert_eq!(aggregate_measurement.seconds, 2.0);
        assert_eq!(aggregate_measurement.iterations, Some(6));
        assert!(aggregate_measurement.variance_seconds.is_some());

        let error = aggregate_measurement_runs(
            "row",
            vec![
                vec![measurement("first_name", 1.0)],
                vec![measurement("second_name", 1.0)],
            ],
        )
        .expect_err("reject mismatched measurement names");
        assert!(
            error
                .to_string()
                .contains("inconsistent measurement shapes")
        );
    }

    fn measurement(name: &str, seconds: f64) -> Measurement {
        measurement_with_iterations(name, seconds, None)
    }

    fn measurement_with_iterations(
        name: &str,
        seconds: f64,
        iterations: Option<usize>,
    ) -> Measurement {
        Measurement {
            name: name.to_string(),
            seconds,
            variance_seconds: None,
            allocation: None,
            iterations,
        }
    }
}
