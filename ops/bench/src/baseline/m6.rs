use std::hint::black_box;

use stab_core::{CliffordString, SingleQubitClifford};

use super::{
    measure_stab, measure_stab_batched, semantic_preflight::require_exact, stab_runner_error,
};
use crate::error::BenchError;
use crate::report::{Measurement, MeasurementObservation};

const CLIFFORD_QUBITS: usize = 10_000;
const CLIFFORD_SHORT_RHS_REPETITIONS: usize = 64;
const CLIFFORD_SHORT_RHS_CASES: [(&str, usize); 3] = [
    ("stab_clifford_string_short_rhs_left_10K", 10_000),
    ("stab_clifford_string_short_rhs_left_100K", 100_000),
    ("stab_clifford_string_short_rhs_left_1M", 1_000_000),
];

pub(super) fn clifford_string(row_id: &str) -> Result<Vec<Measurement>, BenchError> {
    let mut left = CliffordString::identity(CLIFFORD_QUBITS)
        .map_err(|error| stab_runner_error(row_id, error))?;
    let right = CliffordString::identity(CLIFFORD_QUBITS)
        .map_err(|error| stab_runner_error(row_id, error))?;
    validate_clifford_product(
        row_id,
        &left,
        &right,
        1,
        "equal-width Clifford product",
        &left,
    )?;
    let mut measurements = vec![measure_stab(
        "stab_clifford_string_multiplication_10K",
        || {
            left.right_multiply_in_place(&right)
                .map_err(|error| stab_runner_error(row_id, error))?;
            black_box(&left);
            Ok(())
        },
    )?];
    let short_right = CliffordString::from_gates([SingleQubitClifford::H])
        .map_err(|error| stab_runner_error(row_id, error))?;
    for (name, left_width) in CLIFFORD_SHORT_RHS_CASES {
        let mut left =
            CliffordString::from_gates(std::iter::repeat_n(SingleQubitClifford::H, left_width))
                .map_err(|error| stab_runner_error(row_id, error))?;
        let first_product =
            CliffordString::from_gates(std::iter::once(SingleQubitClifford::I).chain(
                std::iter::repeat_n(SingleQubitClifford::H, left_width.saturating_sub(1)),
            ))
            .map_err(|error| stab_runner_error(row_id, error))?;
        validate_clifford_product(
            row_id,
            &left,
            &short_right,
            1,
            "short-RHS first Clifford product",
            &first_product,
        )?;
        validate_clifford_product(
            row_id,
            &left,
            &short_right,
            CLIFFORD_SHORT_RHS_REPETITIONS,
            "short-RHS Clifford product cycle",
            &left,
        )?;
        let mut measurement = measure_stab_batched(name, CLIFFORD_SHORT_RHS_REPETITIONS, || {
            left.right_multiply_in_place(&short_right)
                .map_err(|error| stab_runner_error(row_id, error))?;
            black_box(&left);
            Ok(())
        })?;
        measurement.observations = [
            ("left_qubits", left_width),
            ("right_qubits", short_right.len()),
            ("batch_repetitions", CLIFFORD_SHORT_RHS_REPETITIONS),
        ]
        .into_iter()
        .map(|(name, value)| {
            u64::try_from(value)
                .map(|value| MeasurementObservation {
                    name: name.to_string(),
                    value,
                })
                .map_err(|error| stab_runner_error(row_id, error))
        })
        .collect::<Result<Vec<_>, _>>()?;
        measurements.push(measurement);
    }
    Ok(measurements)
}

fn validate_clifford_product(
    row_id: &str,
    initial: &CliffordString,
    right: &CliffordString,
    repetitions: usize,
    contract: &str,
    expected: &CliffordString,
) -> Result<(), BenchError> {
    let mut actual = initial.clone();
    for _ in 0..repetitions {
        actual
            .right_multiply_in_place(right)
            .map_err(|error| stab_runner_error(row_id, error))?;
    }
    require_exact(row_id, contract, &actual, expected)
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m6-clifford-string", "stab_clifford_string_multiplication_10K") => {
            Some((CLIFFORD_QUBITS as f64, "single-qubit-products/s"))
        }
        (
            "m6-clifford-string",
            "stab_clifford_string_short_rhs_left_10K"
            | "stab_clifford_string_short_rhs_left_100K"
            | "stab_clifford_string_short_rhs_left_1M",
        ) => Some((1.0, "right-qubit-products/s")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clifford_preflight_rejects_same_width_wrong_content() {
        let actual = CliffordString::from_gates([SingleQubitClifford::H, SingleQubitClifford::I])
            .expect("actual Clifford");
        let expected = CliffordString::identity(2).expect("identity Clifford");
        let error = require_exact(
            "m6-clifford-string",
            "equal-width Clifford product",
            &actual,
            &expected,
        )
        .expect_err("same-width Clifford mutation must fail");
        assert!(error.to_string().contains("wrong content"));
    }

    #[test]
    fn short_rhs_preflight_checks_the_exact_product() {
        let left = CliffordString::from_gates([SingleQubitClifford::H, SingleQubitClifford::H])
            .expect("left Clifford");
        let right = CliffordString::from_gates([SingleQubitClifford::H]).expect("right Clifford");
        let expected = CliffordString::from_gates([SingleQubitClifford::I, SingleQubitClifford::H])
            .expect("expected Clifford");
        validate_clifford_product(
            "m6-clifford-string",
            &left,
            &right,
            1,
            "short-RHS first Clifford product",
            &expected,
        )
        .expect("exact short-RHS product");
        validate_clifford_product(
            "m6-clifford-string",
            &left,
            &right,
            CLIFFORD_SHORT_RHS_REPETITIONS,
            "short-RHS Clifford product cycle",
            &left,
        )
        .expect("exact short-RHS cycle");
    }
}
