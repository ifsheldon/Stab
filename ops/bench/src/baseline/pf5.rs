use std::hint::black_box;

use stab_core::{Circuit, circuit_flow_generators};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_iterations, stab_runner_error};

#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
const FLOW_GENERATOR_MEASUREMENT_CASES: usize = 17;
const FLOW_GENERATOR_MEASUREMENT_FLOWS: usize = 52;

pub(super) fn run_flow_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "pf5-flow-generators-measurement-rich" => {
            run_flow_generators_measurement_rich(row).map(Some)
        }
        _ => Ok(None),
    }
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("pf5-flow-generators-measurement-rich", "stab_pf5_flow_generators_measurement_cases") => {
            Some((
                (UTILITY_BATCH * FLOW_GENERATOR_MEASUREMENT_CASES) as f64,
                "cases/s",
            ))
        }
        ("pf5-flow-generators-measurement-rich", "stab_pf5_flow_generators_measurement_flows") => {
            Some((
                (UTILITY_BATCH * FLOW_GENERATOR_MEASUREMENT_FLOWS) as f64,
                "flows/s",
            ))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf5-flow-generators-measurement-rich" => Some(
            "report-only: Stab measures the Rust circuit_flow_generators scoped measurement/reset/pair-measurement/feedback/MPAD subset without a faithful pinned Stim CLI timing ratio",
        ),
        _ => None,
    }
}

fn run_flow_generators_measurement_rich(
    row: &BenchmarkRow,
) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_flow_generators_measurement_rich(
            row,
            "stab_pf5_flow_generators_measurement_cases",
        )?,
        measure_flow_generators_measurement_rich(
            row,
            "stab_pf5_flow_generators_measurement_flows",
        )?,
    ])
}

fn measure_flow_generators_measurement_rich(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = flow_generator_measurement_rich_corpus(&row.id)?;
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut flow_count = 0usize;
        for _ in 0..UTILITY_BATCH {
            for (circuit, expected_flow_count) in &cases {
                let flows = circuit_flow_generators(circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                if flows.len() != *expected_flow_count {
                    return Err(BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: format!(
                            "flow-generator benchmark expected {expected_flow_count} flows but got {}",
                            flows.len()
                        ),
                    });
                }
                flow_count =
                    flow_count
                        .checked_add(flows.len())
                        .ok_or_else(|| BenchError::StabRunner {
                            row_id: row.id.clone(),
                            message: "flow-generator benchmark flow count overflowed".to_string(),
                        })?;
            }
        }
        black_box(flow_count);
        Ok(())
    })
}

fn flow_generator_measurement_rich_corpus(
    row_id: &str,
) -> Result<Vec<(Circuit, usize)>, BenchError> {
    [
        ("M 0\n", 2),
        ("M 0 0\n", 3),
        ("MX 0\n", 2),
        ("MY 0\n", 2),
        ("R 0\n", 1),
        ("RX 0\n", 1),
        ("RY 0\n", 1),
        ("MR 0\n", 2),
        ("MRX 0\n", 2),
        ("MRY 0\n", 2),
        ("MXX 2 0\n", 6),
        ("MYY 3 1 2 3\n", 8),
        ("MZZ 3 1 2 3\n", 8),
        ("M 0\nCX rec[-1] 0\n", 2),
        ("M 0\nXCZ 0 rec[-1]\n", 2),
        ("M 0\nCY rec[-1] 1\n", 4),
        ("MPAD 0 1 1 0\n", 4),
    ]
    .into_iter()
    .map(|(text, expected)| parse_circuit(row_id, text).map(|circuit| (circuit, expected)))
    .collect()
}

fn parse_circuit(row_id: &str, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(row_id, error))
}
