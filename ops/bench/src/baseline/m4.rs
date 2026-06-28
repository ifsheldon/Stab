use std::hint::black_box;

use stab_core::Gate;

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{TINY_DIRECT_COMPARE_REPETITIONS, measure_stab_batched, stab_runner_error};

const GATE_LOOKUP_ALIASES: &[&str] = &[
    "MZ",
    "MRZ",
    "RZ",
    "CNOT",
    "ZCX",
    "ZCY",
    "ZCZ",
    "H_XZ",
    "SQRT_Z",
    "SQRT_Z_DAG",
    "CORRELATED_ERROR",
    "SWAPCZ",
];
const GATE_LOOKUP_INVALID: &[&str] = &[
    "",
    "H2345",
    "CNOTS",
    "SQRT_Q",
    "OBSERVABLE",
    "DETECTOR!",
    "PAULI_CHANNEL_3",
    "UNKNOWN_GATE",
];

pub(crate) fn run_gate_lookup_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let canonical_names = Gate::all().map(Gate::canonical_name).collect::<Vec<_>>();
    let lowercase_names = canonical_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    Ok(vec![
        measure_gate_lookup_success_set(
            row,
            "stab_gate_data_hash_all_gate_names",
            &canonical_names,
        )?,
        measure_gate_lookup_success_set(
            row,
            "stab_gate_lookup_aliases_contract",
            GATE_LOOKUP_ALIASES,
        )?,
        measure_gate_lookup_lowercase_set(row, &lowercase_names)?,
        measure_gate_lookup_invalid_set(row)?,
    ])
}

pub(crate) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    match (row_id, name) {
        ("m4-gate-lookup", "stab_gate_data_hash_all_gate_names") => {
            Some((Gate::all().len() as f64, "lookups/s"))
        }
        ("m4-gate-lookup", "stab_gate_lookup_aliases_contract") => {
            Some((GATE_LOOKUP_ALIASES.len() as f64, "lookups/s"))
        }
        ("m4-gate-lookup", "stab_gate_lookup_lowercase_contract") => {
            Some((Gate::all().len() as f64, "lookups/s"))
        }
        ("m4-gate-lookup", "stab_gate_lookup_invalid_contract") => {
            Some((GATE_LOOKUP_INVALID.len() as f64, "lookups/s"))
        }
        _ => None,
    }
}

pub(crate) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "m4-gate-lookup" => Some(
            "partial-match: Stab pairs canonical all-gate lookup with the pinned Stim gate lookup perf filter and reports alias, lowercase, and invalid lookup contracts separately",
        ),
        _ => None,
    }
}

fn measure_gate_lookup_success_set(
    row: &BenchmarkRow,
    name: &str,
    gate_names: &[&str],
) -> Result<Measurement, BenchError> {
    measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
        let mut checksum = 0usize;
        for gate_name in gate_names {
            let gate = Gate::from_name(black_box(*gate_name))
                .map_err(|error| stab_runner_error(&row.id, error))?;
            checksum ^= gate.canonical_name().len();
        }
        black_box(checksum);
        Ok(())
    })
}

fn measure_gate_lookup_lowercase_set(
    row: &BenchmarkRow,
    gate_names: &[String],
) -> Result<Measurement, BenchError> {
    measure_stab_batched(
        "stab_gate_lookup_lowercase_contract",
        TINY_DIRECT_COMPARE_REPETITIONS,
        || {
            let mut checksum = 0usize;
            for gate_name in gate_names {
                let gate = Gate::from_name(black_box(gate_name.as_str()))
                    .map_err(|error| stab_runner_error(&row.id, error))?;
                checksum ^= gate.canonical_name().len();
            }
            black_box(checksum);
            Ok(())
        },
    )
}

fn measure_gate_lookup_invalid_set(row: &BenchmarkRow) -> Result<Measurement, BenchError> {
    measure_stab_batched(
        "stab_gate_lookup_invalid_contract",
        TINY_DIRECT_COMPARE_REPETITIONS,
        || {
            let mut rejected = 0usize;
            for gate_name in GATE_LOOKUP_INVALID {
                match Gate::from_name(black_box(*gate_name)) {
                    Ok(gate) => {
                        return Err(stab_runner_error(
                            &row.id,
                            format!(
                                "invalid benchmark gate {gate_name:?} resolved as {}",
                                gate.canonical_name()
                            ),
                        ));
                    }
                    Err(error) => {
                        black_box(error);
                        rejected += 1;
                    }
                }
            }
            black_box(rejected);
            Ok(())
        },
    )
}
