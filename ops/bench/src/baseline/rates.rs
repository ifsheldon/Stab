use crate::report::Measurement;

use super::measurement_work;

pub(super) fn summarize_measurement_rates(row_id: &str, measurements: &[Measurement]) -> String {
    measurements
        .iter()
        .filter_map(|measurement| {
            measurement_rate_work(row_id, measurement).map(|(work, unit)| {
                let rate = if measurement.seconds > 0.0 {
                    work / measurement.seconds
                } else {
                    0.0
                };
                format!("{}={rate:.3e}{unit}", measurement.name)
            })
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn measurement_rate_work(
    row_id: &str,
    measurement: &Measurement,
) -> Option<(f64, &'static str)> {
    if row_id == "pfm-b5-wcnf-generated-qec" {
        return measurement
            .observations
            .iter()
            .find(|observation| observation.name == "clauses")
            .map(|observation| (observation.value as f64, "clauses/s"));
    }
    measurement_work(row_id, &measurement.name)
}
