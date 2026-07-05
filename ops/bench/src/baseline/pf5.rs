use std::hint::black_box;
use std::str::FromStr;

use stab_core::{Circuit, Flow, circuit_flow_generators, solve_for_flow_measurements};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{measure_stab_iterations, stab_runner_error};

#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
const FLOW_GENERATOR_MEASUREMENT_CASES: usize = 36;
const FLOW_GENERATOR_MEASUREMENT_FLOWS: usize = 120;
const FLOW_SOLVE_MEASUREMENT_CASES: usize = 2;
const FLOW_SOLVE_MEASUREMENT_QUERIES: usize = 15;
const FLOW_SOLVE_MEASUREMENT_PYTHON_CASES: usize = 8;
const FLOW_SOLVE_MEASUREMENT_PYTHON_QUERIES: usize = 15;

pub(super) fn run_flow_compare_row(
    row: &BenchmarkRow,
) -> Result<Option<Vec<Measurement>>, BenchError> {
    match row.id.as_str() {
        "pf5-flow-generators-measurement-rich" => {
            run_flow_generators_measurement_rich(row).map(Some)
        }
        "pf5-flow-solve-measurement-rich" => run_flow_solve_measurement_rich(row).map(Some),
        "pf5-flow-solve-measurement-python" => run_flow_solve_measurement_rich(row).map(Some),
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
        ("pf5-flow-solve-measurement-rich", "stab_pf5_flow_solve_measurement_cases") => Some((
            (UTILITY_BATCH * FLOW_SOLVE_MEASUREMENT_CASES) as f64,
            "cases/s",
        )),
        ("pf5-flow-solve-measurement-rich", "stab_pf5_flow_solve_measurement_queries") => Some((
            (UTILITY_BATCH * FLOW_SOLVE_MEASUREMENT_QUERIES) as f64,
            "queries/s",
        )),
        ("pf5-flow-solve-measurement-python", "stab_pf5_flow_solve_measurement_python_cases") => {
            Some((
                (UTILITY_BATCH * FLOW_SOLVE_MEASUREMENT_PYTHON_CASES) as f64,
                "cases/s",
            ))
        }
        ("pf5-flow-solve-measurement-python", "stab_pf5_flow_solve_measurement_python_queries") => {
            Some((
                (UTILITY_BATCH * FLOW_SOLVE_MEASUREMENT_PYTHON_QUERIES) as f64,
                "queries/s",
            ))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf5-flow-generators-measurement-rich" => Some(
            "report-only: Stab measures the Rust circuit_flow_generators scoped measurement/reset/pair-measurement/MPP/SPP/composed-measurement/unitary-mixed/bounded-repeat/feedback/MPAD/heralded-noise subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-flow-solve-measurement-rich" => Some(
            "report-only: Stab measures the Rust solve_for_flow_measurements promoted C++ upstream examples without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-flow-solve-measurement-python" => Some(
            "report-only: Stab measures the Rust solve_for_flow_measurements promoted Python upstream examples without a faithful pinned Stim CLI timing ratio",
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
            for (case_index, (circuit, expected_flow_count)) in cases.iter().enumerate() {
                let flows = circuit_flow_generators(circuit)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                if flows.len() != *expected_flow_count {
                    return Err(BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: format!(
                            "flow-generator benchmark case {case_index} expected {expected_flow_count} flows but got {}",
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

fn run_flow_solve_measurement_rich(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let (case_measurement, query_measurement) = flow_solve_measurement_names(&row.id)?;
    Ok(vec![
        measure_flow_solve_measurement_rich(row, case_measurement)?,
        measure_flow_solve_measurement_rich(row, query_measurement)?,
    ])
}

fn flow_solve_measurement_names(row_id: &str) -> Result<(&'static str, &'static str), BenchError> {
    match row_id {
        "pf5-flow-solve-measurement-rich" => Ok((
            "stab_pf5_flow_solve_measurement_cases",
            "stab_pf5_flow_solve_measurement_queries",
        )),
        "pf5-flow-solve-measurement-python" => Ok((
            "stab_pf5_flow_solve_measurement_python_cases",
            "stab_pf5_flow_solve_measurement_python_queries",
        )),
        _ => Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "unknown PF5 flow-solve benchmark row".to_string(),
        }),
    }
}

fn measure_flow_solve_measurement_rich(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = flow_solve_measurement_corpus(&row.id)?;
    measure_stab_iterations(measurement_name, super::STAB_COMPARE_ITERATIONS, || {
        let mut solved_count = 0usize;
        for _ in 0..UTILITY_BATCH {
            for (circuit, flows, expected) in &cases {
                let actual = solve_for_flow_measurements(circuit, flows)
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                if actual != *expected {
                    return Err(BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: format!(
                            "flow-solve benchmark expected {expected:?} but got {actual:?}"
                        ),
                    });
                }
                solved_count = solved_count
                    .checked_add(actual.iter().filter(|result| result.is_some()).count())
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row.id.clone(),
                        message: "flow-solve benchmark solved count overflowed".to_string(),
                    })?;
            }
        }
        black_box(solved_count);
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
        ("MPP Y0*Y1 Y2*Y3\n", 8),
        ("MPP X0 X1*X1\n", 5),
        ("SPP Z0\n", 2),
        ("SPP X0 Z0\n", 2),
        ("SPP X0*X1\n", 4),
        ("SPP_DAG Z0\n", 2),
        ("M 0\nCX rec[-1] 0\n", 2),
        ("MPP X0*X1\nCX rec[-1] 0\n", 4),
        ("M 0\nXCZ 0 rec[-1]\n", 2),
        ("M 0\nCY rec[-1] 1\n", 4),
        ("MPAD 0 1 1 0\n", 4),
        ("M 0\nTICK\nM 0\n", 3),
        ("H 0\nM 0\n", 2),
        ("M 0\nH 0\n", 2),
        ("H 0\nM 0\nS 0\n", 2),
        ("R 0\nM 0\n", 2),
        ("R 0\nH 0\nM 0\n", 1),
        ("M 0\nR 0\n", 2),
        ("REPEAT 2 {\n    M 0\n}\n", 3),
        ("M 0\nMX 1\nMY 2\n", 6),
        ("MXX 0 1\nMZZ 0 1\n", 4),
        ("MPP X0*Y1\nMPAD 0 1\n", 6),
        (
            "
            HERALDED_ERASE(0.04) 1
            HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1
            TICK
            MPP X0*Y1*Z2 Z0*Z1
            ",
            8,
        ),
    ]
    .into_iter()
    .map(|(text, expected)| parse_circuit(row_id, text).map(|circuit| (circuit, expected)))
    .collect()
}

type FlowSolveCase = (Circuit, Vec<Flow>, Vec<Option<Vec<i32>>>);

fn flow_solve_measurement_rich_corpus(row_id: &str) -> Result<Vec<FlowSolveCase>, BenchError> {
    Ok(vec![
        (
            parse_circuit(row_id, "MX 0\n")?,
            parse_flows(
                row_id,
                &["1 -> X0", "Y0 -> Y0", "X0 -> 1", "X0 -> Z0", "Y1 -> Y1"],
            )?,
            vec![Some(vec![0]), None, Some(vec![0]), None, Some(vec![])],
        ),
        (
            parse_circuit(
                row_id,
                "
                R 1 3
                CX 0 1 2 3
                CX 4 3 2 1
                M 1 3
            ",
            )?,
            parse_flows(
                row_id,
                &[
                    "Z0*Z2 -> 1",
                    "1 -> Z2*Z4",
                    "1 -> Z0*Z4",
                    "Z0*Z4 -> Z0*Z2",
                    "Z0 -> Z0",
                    "Z0 -> Z1",
                    "Z0 -> Z2",
                    "X0*X2*X4 -> X0*X2*X4",
                    "X0 -> X0",
                    "X0 -> Z0",
                ],
            )?,
            vec![
                Some(vec![0]),
                Some(vec![1]),
                Some(vec![0, 1]),
                Some(vec![1]),
                Some(vec![]),
                None,
                Some(vec![0]),
                Some(vec![]),
                None,
                None,
            ],
        ),
    ])
}

fn flow_solve_measurement_python_corpus(row_id: &str) -> Result<Vec<FlowSolveCase>, BenchError> {
    Ok(vec![
        (
            parse_circuit(row_id, "M 2\n")?,
            parse_flows(row_id, &["X2 -> X2", "Y2 -> Y2", "Z2 -> Z2", "Z2 -> 1"])?,
            vec![None, None, Some(vec![]), Some(vec![0])],
        ),
        (
            parse_circuit(row_id, "MXX 0 1\n")?,
            parse_flows(row_id, &["YY -> ZZ", "YY -> YY", "YZ -> ZY"])?,
            vec![Some(vec![0]), Some(vec![]), Some(vec![0])],
        ),
        (
            parse_circuit(row_id, "M 1 2\n")?,
            parse_flows(row_id, &["_Z -> 1"])?,
            vec![Some(vec![0])],
        ),
        (
            parse_circuit(row_id, "MX 1 2\n")?,
            parse_flows(row_id, &["_X -> 1"])?,
            vec![Some(vec![0])],
        ),
        (
            parse_circuit(row_id, "MYY 1 2 3 4\n")?,
            parse_flows(row_id, &["_YY__ -> 1"])?,
            vec![Some(vec![0])],
        ),
        (
            parse_circuit(row_id, "MPP Y1*Y2 Y3*Y4\n")?,
            parse_flows(row_id, &["_YY__ -> 1"])?,
            vec![Some(vec![0])],
        ),
        (
            parse_circuit(
                row_id,
                "
                MPP Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8
                M 0 1 2 3 4 5 6 7 8
                ",
            )?,
            parse_flows(
                row_id,
                &[
                    "1 -> Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8",
                    "Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8 -> 1",
                ],
            )?,
            vec![Some(vec![0]), Some(vec![0])],
        ),
        (
            parse_circuit(
                row_id,
                "
                M 0 1 2 3 4 5 6 7 8
                MPP Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8
                ",
            )?,
            parse_flows(
                row_id,
                &[
                    "1 -> Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8",
                    "Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8 -> 1",
                ],
            )?,
            vec![Some(vec![9]), Some(vec![9])],
        ),
    ])
}

fn flow_solve_measurement_corpus(row_id: &str) -> Result<Vec<FlowSolveCase>, BenchError> {
    match row_id {
        "pf5-flow-solve-measurement-rich" => flow_solve_measurement_rich_corpus(row_id),
        "pf5-flow-solve-measurement-python" => flow_solve_measurement_python_corpus(row_id),
        _ => Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "unknown PF5 flow-solve benchmark row".to_string(),
        }),
    }
}

#[cfg(test)]
pub(super) fn expected_flow_solve_measurement_work_for_test(
    row_id: &str,
) -> Result<(f64, f64), BenchError> {
    let cases = flow_solve_measurement_corpus(row_id)?;
    let query_count =
        cases
            .iter()
            .map(|(_, flows, _)| flows.len())
            .try_fold(0usize, |count, len| {
                count
                    .checked_add(len)
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row_id.to_string(),
                        message: "flow-solve test query count overflowed".to_string(),
                    })
            })?;
    Ok((
        (UTILITY_BATCH * cases.len()) as f64,
        (UTILITY_BATCH * query_count) as f64,
    ))
}

fn parse_circuit(row_id: &str, text: &str) -> Result<Circuit, BenchError> {
    Circuit::from_stim_str(text).map_err(|error| stab_runner_error(row_id, error))
}

fn parse_flows(row_id: &str, texts: &[&str]) -> Result<Vec<Flow>, BenchError> {
    texts
        .iter()
        .map(|text| {
            Flow::from_str(text).map_err(|error| BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: error.to_string(),
            })
        })
        .collect()
}
