use std::hint::black_box;

use stab_core::{Gate, GateArgumentRule};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::{TINY_DIRECT_COMPARE_REPETITIONS, measure_stab_batched, stab_runner_error};

pub(super) fn run_gate_metadata_row(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let gates = Gate::all().collect::<Vec<_>>();
    let aliases = gates
        .iter()
        .flat_map(|gate| gate.aliases().iter().copied())
        .collect::<Vec<_>>();

    Ok(vec![
        measure_stab_batched(
            "stab_gate_metadata_flags_all_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for gate in &gates {
                    checksum ^= gate.canonical_name().len();
                    checksum ^= gate.aliases().len();
                    checksum ^= gate.category() as usize;
                    checksum ^= argument_rule_checksum(gate.argument_rule());
                    checksum ^= gate.target_rule() as usize;
                    checksum ^= gate.target_group_kind() as usize;
                    checksum ^= usize::from(gate.is_unitary());
                    checksum ^= usize::from(gate.is_reset()) << 1;
                    checksum ^= usize::from(gate.is_noisy()) << 2;
                    checksum ^= usize::from(gate.produces_measurements()) << 3;
                    checksum ^= usize::from(gate.is_single_qubit_gate()) << 4;
                    checksum ^= usize::from(gate.is_two_qubit_gate()) << 5;
                    checksum ^= usize::from(gate.takes_measurement_record_targets()) << 6;
                    checksum ^= usize::from(gate.takes_pauli_targets()) << 7;
                    checksum ^= usize::from(gate.is_symmetric_gate()) << 8;
                    checksum ^= usize::from(gate.can_fuse()) << 9;
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_inverse_all_gates",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for gate in &gates {
                    if let Some(inverse) = gate.inverse() {
                        checksum ^= inverse.canonical_name().len();
                    }
                    let generalized_inverse = gate
                        .generalized_inverse()
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= generalized_inverse.canonical_name().len();
                }
                black_box(checksum);
                Ok(())
            },
        )?,
        measure_stab_batched(
            "stab_gate_metadata_alias_lookup_all_aliases",
            TINY_DIRECT_COMPARE_REPETITIONS,
            || {
                let mut checksum = 0usize;
                for alias in &aliases {
                    let gate = Gate::from_name(alias)
                        .map_err(|error| stab_runner_error(&row.id, error))?;
                    checksum ^= gate.canonical_name().len();
                }
                black_box(checksum);
                Ok(())
            },
        )?,
    ])
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if row_id != "pf1-gate-metadata-lookup" {
        return None;
    }
    match name {
        "stab_gate_metadata_flags_all_gates" => Some((Gate::all().len() as f64, "gates/s")),
        "stab_gate_metadata_inverse_all_gates" => Some((Gate::all().len() as f64, "gates/s")),
        "stab_gate_metadata_alias_lookup_all_aliases" => {
            Some((gate_alias_count() as f64, "lookups/s"))
        }
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    match row_id {
        "pf1-gate-metadata-lookup" => Some(
            "contract-only: Stab measures Rust gate metadata accessors and alias lookup against the PF1 public API; pinned Stim GateData is a Python binding surface without a faithful Rust direct baseline",
        ),
        _ => None,
    }
}

fn gate_alias_count() -> usize {
    Gate::all().map(|gate| gate.aliases().len()).sum()
}

fn argument_rule_checksum(rule: GateArgumentRule) -> usize {
    match rule {
        GateArgumentRule::Exact(count) => count,
        GateArgumentRule::Any => 1,
        GateArgumentRule::OptionalProbability => 2,
        GateArgumentRule::ProbabilityList(count) => 3 ^ count,
        GateArgumentRule::AnyProbabilityList => 4,
        GateArgumentRule::UnsignedInteger => 5,
    }
}
