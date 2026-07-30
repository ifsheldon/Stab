use std::hint::black_box;

use stab_core::{Gate, ModelError, ValidationError};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{
    TINY_DIRECT_COMPARE_REPETITIONS, measure_stab_batched, semantic_preflight::require_exact,
    stab_runner_error,
};

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
const STIM_V116_CANONICAL_GATES: [(&str, usize); 81] = [
    ("DETECTOR", 381),
    ("OBSERVABLE_INCLUDE", 157),
    ("TICK", 464),
    ("QUBIT_COORDS", 439),
    ("SHIFT_COORDS", 14),
    ("REPEAT", 21),
    ("MPAD", 192),
    ("MX", 476),
    ("MY", 119),
    ("M", 310),
    ("MRX", 115),
    ("MRY", 360),
    ("MR", 58),
    ("RX", 358),
    ("RY", 1),
    ("R", 451),
    ("XCX", 94),
    ("XCY", 197),
    ("XCZ", 440),
    ("YCX", 448),
    ("YCY", 183),
    ("YCZ", 386),
    ("CX", 208),
    ("CY", 363),
    ("CZ", 6),
    ("H", 169),
    ("H_XY", 350),
    ("H_YZ", 333),
    ("H_NXY", 461),
    ("H_NXZ", 454),
    ("H_NYZ", 114),
    ("DEPOLARIZE1", 216),
    ("DEPOLARIZE2", 63),
    ("X_ERROR", 209),
    ("Y_ERROR", 239),
    ("Z_ERROR", 269),
    ("I_ERROR", 79),
    ("II_ERROR", 215),
    ("PAULI_CHANNEL_1", 288),
    ("PAULI_CHANNEL_2", 443),
    ("E", 494),
    ("ELSE_CORRELATED_ERROR", 364),
    ("HERALDED_ERASE", 314),
    ("HERALDED_PAULI_CHANNEL_1", 388),
    ("I", 402),
    ("X", 313),
    ("Y", 34),
    ("Z", 267),
    ("C_XYZ", 48),
    ("C_ZYX", 502),
    ("C_NXYZ", 245),
    ("C_XNYZ", 161),
    ("C_XYNZ", 253),
    ("C_NZYX", 297),
    ("C_ZNYX", 49),
    ("C_ZYNX", 237),
    ("SQRT_X", 319),
    ("SQRT_X_DAG", 342),
    ("SQRT_Y", 445),
    ("SQRT_Y_DAG", 143),
    ("S", 172),
    ("S_DAG", 121),
    ("II", 399),
    ("SQRT_XX", 318),
    ("SQRT_XX_DAG", 341),
    ("SQRT_YY", 394),
    ("SQRT_YY_DAG", 140),
    ("SQRT_ZZ", 106),
    ("SQRT_ZZ_DAG", 231),
    ("MPP", 501),
    ("SPP", 425),
    ("SPP_DAG", 223),
    ("SWAP", 81),
    ("ISWAP", 24),
    ("CXSWAP", 390),
    ("SWAPCX", 47),
    ("CZSWAP", 76),
    ("ISWAP_DAG", 332),
    ("MXX", 5),
    ("MYY", 409),
    ("MZZ", 281),
];
const STIM_V116_ALIAS_RESULTS: [&str; 12] = [
    "M", "MR", "R", "CX", "CX", "CY", "CZ", "H", "S", "S_DAG", "E", "CZSWAP",
];

#[derive(Clone, Debug, Eq, PartialEq)]
enum GateLookupOutcome {
    Resolved(&'static str),
    UnknownGate(String),
    OtherError(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateLookupSnapshot {
    canonical_names: Vec<&'static str>,
    canonical_hashes: Vec<usize>,
    canonical_lookups: Vec<GateLookupOutcome>,
    alias_lookups: Vec<GateLookupOutcome>,
    lowercase_lookups: Vec<GateLookupOutcome>,
    invalid_lookups: Vec<GateLookupOutcome>,
}

pub(crate) fn run_gate_lookup_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    validate_gate_lookup_preflight(&row.id, &capture_gate_lookup_snapshot())?;
    let canonical_names = Gate::all().map(Gate::canonical_name).collect::<Vec<_>>();
    let lowercase_names = canonical_names
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .collect::<Vec<_>>();
    Ok(vec![
        measure_gate_name_hash_set("stab_gate_data_hash_all_gate_names", &canonical_names)?,
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
            Some((Gate::all().len() as f64, "hashes/s"))
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
            "partial-match: Stab pairs canonical all-gate name hashing with the pinned Stim gate_data_hash_all_gate_names perf filter and reports alias, lowercase, and invalid lookup contracts separately",
        ),
        _ => None,
    }
}

fn capture_gate_lookup_snapshot() -> GateLookupSnapshot {
    let canonical_names = Gate::all().map(Gate::canonical_name).collect::<Vec<_>>();
    let canonical_hashes = canonical_names
        .iter()
        .map(|name| Gate::stim_name_hash(name))
        .collect();
    let canonical_lookups = STIM_V116_CANONICAL_GATES
        .iter()
        .map(|(name, _)| gate_lookup_outcome(name))
        .collect();
    let alias_lookups = GATE_LOOKUP_ALIASES
        .iter()
        .map(|name| gate_lookup_outcome(name))
        .collect();
    let lowercase_lookups = STIM_V116_CANONICAL_GATES
        .iter()
        .map(|(name, _)| gate_lookup_outcome(&name.to_ascii_lowercase()))
        .collect();
    let invalid_lookups = GATE_LOOKUP_INVALID
        .iter()
        .map(|name| gate_lookup_outcome(name))
        .collect();
    GateLookupSnapshot {
        canonical_names,
        canonical_hashes,
        canonical_lookups,
        alias_lookups,
        lowercase_lookups,
        invalid_lookups,
    }
}

fn expected_gate_lookup_snapshot() -> GateLookupSnapshot {
    let canonical_names = STIM_V116_CANONICAL_GATES
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>();
    GateLookupSnapshot {
        canonical_hashes: STIM_V116_CANONICAL_GATES
            .iter()
            .map(|(_, hash)| *hash)
            .collect(),
        canonical_lookups: canonical_names
            .iter()
            .map(|name| GateLookupOutcome::Resolved(name))
            .collect(),
        alias_lookups: STIM_V116_ALIAS_RESULTS
            .iter()
            .map(|name| GateLookupOutcome::Resolved(name))
            .collect(),
        lowercase_lookups: canonical_names
            .iter()
            .map(|name| GateLookupOutcome::Resolved(name))
            .collect(),
        invalid_lookups: GATE_LOOKUP_INVALID
            .iter()
            .map(|name| GateLookupOutcome::UnknownGate((*name).to_string()))
            .collect(),
        canonical_names,
    }
}

fn gate_lookup_outcome(name: &str) -> GateLookupOutcome {
    match Gate::from_name(name) {
        Ok(gate) => GateLookupOutcome::Resolved(gate.canonical_name()),
        Err(ModelError::Validation(ValidationError::UnknownGate(name))) => {
            GateLookupOutcome::UnknownGate(name)
        }
        Err(error) => GateLookupOutcome::OtherError(error.to_string()),
    }
}

fn validate_gate_lookup_preflight(
    row_id: &str,
    actual: &GateLookupSnapshot,
) -> Result<(), BenchError> {
    let expected = expected_gate_lookup_snapshot();
    require_exact(
        row_id,
        "canonical gate-name table",
        actual.canonical_names.as_slice(),
        expected.canonical_names.as_slice(),
    )?;
    require_exact(
        row_id,
        "canonical gate-name hashes",
        actual.canonical_hashes.as_slice(),
        expected.canonical_hashes.as_slice(),
    )?;
    require_exact(
        row_id,
        "canonical gate-name lookups",
        actual.canonical_lookups.as_slice(),
        expected.canonical_lookups.as_slice(),
    )?;
    require_exact(
        row_id,
        "gate alias lookups",
        actual.alias_lookups.as_slice(),
        expected.alias_lookups.as_slice(),
    )?;
    require_exact(
        row_id,
        "lowercase gate-name lookups",
        actual.lowercase_lookups.as_slice(),
        expected.lowercase_lookups.as_slice(),
    )?;
    require_exact(
        row_id,
        "invalid gate-name lookups",
        actual.invalid_lookups.as_slice(),
        expected.invalid_lookups.as_slice(),
    )
}

fn measure_gate_name_hash_set(name: &str, gate_names: &[&str]) -> Result<Measurement, BenchError> {
    measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
        let mut checksum = 0usize;
        for gate_name in gate_names {
            checksum = checksum.wrapping_add(Gate::stim_name_hash(gate_name));
        }
        black_box(checksum);
        Ok(())
    })
}

fn measure_gate_lookup_success_set(
    row: &BenchmarkRow,
    name: &str,
    gate_names: &[&str],
) -> Result<Measurement, BenchError> {
    measure_stab_batched(name, TINY_DIRECT_COMPARE_REPETITIONS, || {
        let mut checksum = 0usize;
        for gate_name in gate_names {
            let gate =
                Gate::from_name(gate_name).map_err(|error| stab_runner_error(&row.id, error))?;
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
                let gate = Gate::from_name(gate_name.as_str())
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

#[cfg(test)]
mod tests {
    use super::*;

    const ROW_ID: &str = "m4-gate-lookup";

    #[test]
    fn gate_lookup_preflight_accepts_the_frozen_stim_v116_contract() {
        validate_gate_lookup_preflight(ROW_ID, &capture_gate_lookup_snapshot())
            .expect("frozen gate lookup contract");
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_length_canonical_name_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual
            .canonical_names
            .first_mut()
            .expect("canonical gate names") = "SELECTOR";

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-length canonical-name mutation must fail");
        assert!(error.to_string().contains("canonical gate-name table"));
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_length_alias_result_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual.alias_lookups.get_mut(3).expect("CNOT alias result") =
            GateLookupOutcome::Resolved("CY");

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-length alias-result mutation must fail");
        assert!(error.to_string().contains("gate alias lookups"));
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_length_canonical_result_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual
            .canonical_lookups
            .first_mut()
            .expect("canonical lookup result") = GateLookupOutcome::Resolved("SELECTOR");

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-length canonical-result mutation must fail");
        assert!(error.to_string().contains("canonical gate-name lookups"));
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_length_lowercase_result_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual
            .lowercase_lookups
            .first_mut()
            .expect("lowercase lookup result") = GateLookupOutcome::Resolved("SELECTOR");

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-length lowercase-result mutation must fail");
        assert!(error.to_string().contains("lowercase gate-name lookups"));
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_cardinality_invalid_result_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual
            .invalid_lookups
            .first_mut()
            .expect("invalid lookup result") = GateLookupOutcome::Resolved("H");

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-cardinality invalid-result mutation must fail");
        assert!(error.to_string().contains("invalid gate-name lookups"));
    }

    #[test]
    fn gate_lookup_preflight_rejects_same_cardinality_hash_mutation() {
        let mut actual = capture_gate_lookup_snapshot();
        *actual
            .canonical_hashes
            .first_mut()
            .expect("canonical gate hash") ^= 1;

        let error = validate_gate_lookup_preflight(ROW_ID, &actual)
            .expect_err("same-cardinality hash mutation must fail");
        assert!(error.to_string().contains("canonical gate-name hashes"));
    }
}
