use std::hint::black_box;

use stab_core::{Circuit, MissingDetectorOptions, missing_detectors};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{parse_circuit, stab_runner_error};

#[cfg(not(test))]
const UTILITY_BATCH: usize = 4096;
#[cfg(test)]
const UTILITY_BATCH: usize = 2;
#[cfg(not(test))]
const GENERATED_BATCH: usize = 64;
#[cfg(test)]
const GENERATED_BATCH: usize = 1;
const BASIC_CASES: usize = 10;
const BASIC_SUGGESTIONS: usize = 4;
const MPP_CASES: usize = 4;
const MPP_SUGGESTIONS: usize = 3;
const GENERATED_CASES: usize = 2;
const GENERATED_SUGGESTIONS: usize = 2;
const HONEYCOMB_MISSING_DETECTOR: &str = include_str!(
    "../../../../../crates/stab-core/tests/fixtures/missing_detectors_honeycomb_missing_detector.stim"
);

pub(super) fn run_basic_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_basic(row, "stab_missing_detectors_basic_cases")?,
        measure_basic(row, "stab_missing_detectors_basic_suggestions")?,
    ])
}

pub(super) fn run_mpp_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_mpp(row, "stab_pf5_missing_detectors_mpp_cases")?,
        measure_mpp(row, "stab_pf5_missing_detectors_mpp_suggestions")?,
    ])
}

pub(super) fn run_generated_code_batch(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    Ok(vec![
        measure_generated_code(row, "stab_pf5_missing_detectors_generated_cases")?,
        measure_generated_code(row, "stab_pf5_missing_detectors_generated_suggestions")?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_cases") => {
            Some(((UTILITY_BATCH * BASIC_CASES) as f64, "cases/s"))
        }
        ("m9-missing-detectors-basic-batch", "stab_missing_detectors_basic_suggestions") => {
            Some(((UTILITY_BATCH * BASIC_SUGGESTIONS) as f64, "suggestions/s"))
        }
        ("pf5-missing-detectors-mpp", "stab_pf5_missing_detectors_mpp_cases") => {
            Some(((UTILITY_BATCH * MPP_CASES) as f64, "cases/s"))
        }
        ("pf5-missing-detectors-mpp", "stab_pf5_missing_detectors_mpp_suggestions") => {
            Some(((UTILITY_BATCH * MPP_SUGGESTIONS) as f64, "suggestions/s"))
        }
        ("pf5-missing-detectors-generated-code", "stab_pf5_missing_detectors_generated_cases") => {
            Some(((GENERATED_BATCH * GENERATED_CASES) as f64, "cases/s"))
        }
        (
            "pf5-missing-detectors-generated-code",
            "stab_pf5_missing_detectors_generated_suggestions",
        ) => Some((
            (GENERATED_BATCH * GENERATED_SUGGESTIONS) as f64,
            "suggestions/s",
        )),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m9-missing-detectors-basic-batch" => Some(
            "report-only: Stab measures the Rust basic missing-detectors utility subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-missing-detectors-mpp" => Some(
            "report-only: Stab measures the Rust missing-detectors MPP and observable row-reduction subset without a faithful pinned Stim CLI timing ratio",
        ),
        "pf5-missing-detectors-generated-code" => Some(
            "report-only: Stab measures the Rust missing-detectors generated-code honeycomb and toric suffix subset without a faithful pinned Stim CLI timing ratio",
        ),
        _ => None,
    }
}

fn measure_basic(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    measure_cases(
        row,
        measurement_name,
        basic_corpus(&row.id)?,
        "missing-detectors benchmark suggestion count overflowed",
    )
}

fn measure_mpp(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    measure_cases(
        row,
        measurement_name,
        mpp_corpus(&row.id)?,
        "missing-detectors MPP benchmark suggestion count overflowed",
    )
}

fn measure_generated_code(
    row: &BenchmarkRow,
    measurement_name: &'static str,
) -> Result<Measurement, BenchError> {
    let cases = generated_code_corpus(&row.id)?;
    super::measure_stab_iterations(
        measurement_name,
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut suggestions = 0usize;
            for _ in 0..GENERATED_BATCH {
                for (circuit, ignore_non_deterministic_measurements, expected) in &cases {
                    let output = missing_detectors(
                        circuit,
                        MissingDetectorOptions {
                            ignore_non_deterministic_measurements:
                                *ignore_non_deterministic_measurements,
                        },
                    )
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                    let actual = output.to_stim_string();
                    if actual != *expected {
                        return Err(BenchError::StabRunner {
                            row_id: row.id.clone(),
                            message: format!(
                                "missing-detectors generated-code benchmark expected {expected:?} but got {actual:?}"
                            ),
                        });
                    }
                    suggestions =
                        suggestions
                            .checked_add(output.items().len())
                            .ok_or_else(|| BenchError::StabRunner {
                                row_id: row.id.clone(),
                                message:
                                    "missing-detectors generated-code suggestion count overflowed"
                                        .to_string(),
                            })?;
                }
            }
            black_box(suggestions);
            Ok(())
        },
    )
}

fn measure_cases(
    row: &BenchmarkRow,
    measurement_name: &'static str,
    cases: Vec<(Circuit, bool)>,
    overflow_message: &'static str,
) -> Result<Measurement, BenchError> {
    super::measure_stab_iterations(
        measurement_name,
        super::super::STAB_COMPARE_ITERATIONS,
        || {
            let mut suggestions = 0usize;
            for _ in 0..UTILITY_BATCH {
                for (circuit, ignore_non_deterministic_measurements) in &cases {
                    let output = missing_detectors(
                        circuit,
                        MissingDetectorOptions {
                            ignore_non_deterministic_measurements:
                                *ignore_non_deterministic_measurements,
                        },
                    )
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                    suggestions =
                        suggestions
                            .checked_add(output.items().len())
                            .ok_or_else(|| BenchError::StabRunner {
                                row_id: row.id.clone(),
                                message: overflow_message.to_string(),
                            })?;
                }
            }
            black_box(suggestions);
            Ok(())
        },
    )
}

fn basic_corpus(row_id: &str) -> Result<Vec<(Circuit, bool)>, BenchError> {
    [
        ("", false),
        ("R 0\nM 0\nDETECTOR rec[-1]\n", false),
        ("R 0\nM 0\nDETECTOR rec[-1]\nDETECTOR rec[-1]\n", false),
        ("R 0\nM 0\n", false),
        ("M 0\n", false),
        ("M 0\n", true),
        ("R 0 1\nM 0 1\nDETECTOR rec[-1]\n", false),
        ("M 0\nDETECTOR rec[-1] rec[-1]\n", false),
        ("MX 0\n", false),
        ("RX 0\nMY 0\n", false),
    ]
    .into_iter()
    .map(|(text, ignore)| parse_circuit(row_id, text).map(|circuit| (circuit, ignore)))
    .collect()
}

fn mpp_corpus(row_id: &str) -> Result<Vec<(Circuit, bool)>, BenchError> {
    [
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n",
            false,
        ),
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             DETECTOR rec[-1] rec[-3]\n\
             DETECTOR rec[-2] rec[-4]\n\
             DETECTOR rec[-1] rec[-3] rec[-2] rec[-4]\n",
            false,
        ),
        (
            "MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
        (
            "OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             MPP Z0*Z1 X0*X1\n\
             TICK\n\
             MPP Z0*Z1 X0*X1\n\
             OBSERVABLE_INCLUDE(0) Z0 Z1\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             DETECTOR rec[-2] rec[-4]\n\
             OBSERVABLE_INCLUDE(0) rec[-3]\n",
            true,
        ),
    ]
    .into_iter()
    .map(|(text, ignore)| parse_circuit(row_id, text).map(|circuit| (circuit, ignore)))
    .collect()
}

type GeneratedCase = (Circuit, bool, &'static str);

fn generated_code_corpus(row_id: &str) -> Result<Vec<GeneratedCase>, BenchError> {
    Ok(vec![
        (
            parse_circuit(row_id, HONEYCOMB_MISSING_DETECTOR)?,
            true,
            "DETECTOR rec[-377] rec[-375] rec[-374] rec[-317] rec[-315] rec[-314]\n",
        ),
        (
            parse_circuit(
                row_id,
                "R 0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15\n\
                 TICK\n\
                 MPP X0*X4*X5*X1 X2*X6*X7*X3 X10*X14*X15*X11 X8*X12*X13*X9\n\
                 TICK\n\
                 MPP X5*X9*X10*X6 X1*X13*X14*X2 X0*X12*X15*X3 X4*X8*X11*X7\n\
                 TICK\n\
                 MPP Z4*Z8*Z9*Z5 Z6*Z10*Z11*Z7 Z2*Z14*Z15*Z3 Z0*Z12*Z13*Z1\n\
                 TICK\n\
                 MPP Z1*Z5*Z6*Z2 Z9*Z13*Z14*Z10 Z8*Z12*Z15*Z11 Z0*Z4*Z7*Z3\n\
                 DETECTOR rec[-1]\n\
                 DETECTOR rec[-2]\n\
                 DETECTOR rec[-3]\n\
                 DETECTOR rec[-4]\n\
                 DETECTOR rec[-5]\n\
                 DETECTOR rec[-6]\n\
                 DETECTOR rec[-7]\n\
                 DETECTOR rec[-8]\n",
            )?,
            true,
            "DETECTOR rec[-16] rec[-15] rec[-14] rec[-13] rec[-12] rec[-11] rec[-10] rec[-9]\n",
        ),
    ])
}
