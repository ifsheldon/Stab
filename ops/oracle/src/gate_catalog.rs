use std::fmt::Write as _;

use stab_model::{Gate, GateArgumentRule, GateCategory, GateTargetRule};
use thiserror::Error;

use crate::{OracleError, RepoRoot, ensure_stim_helper, process::run_checked_path};

const HELPER_TARGET: &str = "stab_stim_gate_catalog";
const EXPECTED_CANONICAL_GATES: usize = 81;
const EXPECTED_ACCEPTED_NAMES: usize = 93;

const GATE_IS_UNITARY: u16 = 1 << 0;
const GATE_IS_NOISY: u16 = 1 << 1;
const GATE_ARGS_ARE_DISJOINT_PROBABILITIES: u16 = 1 << 2;
const GATE_PRODUCES_RESULTS: u16 = 1 << 3;
const GATE_IS_NOT_FUSABLE: u16 = 1 << 4;
const GATE_IS_BLOCK: u16 = 1 << 5;
const GATE_TARGETS_PAIRS: u16 = 1 << 6;
const GATE_TARGETS_PAULI_STRING: u16 = 1 << 7;
const GATE_ONLY_TARGETS_MEASUREMENT_RECORD: u16 = 1 << 8;
const GATE_CAN_TARGET_BITS: u16 = 1 << 9;
const GATE_TAKES_NO_TARGETS: u16 = 1 << 10;
const GATE_ARGS_ARE_UNSIGNED_INTEGERS: u16 = 1 << 11;
const GATE_TARGETS_COMBINERS: u16 = 1 << 12;
const GATE_IS_RESET: u16 = 1 << 13;
const GATE_IS_SINGLE_QUBIT_GATE: u16 = 1 << 15;

const ARG_COUNT_ANY: u8 = 0xff;
const ARG_COUNT_ZERO_OR_ONE: u8 = 0xfe;

#[derive(Debug, Error)]
pub(crate) enum GateCatalogError {
    #[error("pinned Stim gate-catalog helper emitted non-UTF-8 output: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("pinned Stim and Stab gate catalogs differ: {0}")]
    Mismatch(Box<str>),
}

pub(crate) fn run(root: &RepoRoot, rebuild_stim: bool) -> Result<(), OracleError> {
    let helper = ensure_stim_helper(root, rebuild_stim, HELPER_TARGET)?;
    let output = run_checked_path(&helper, std::iter::empty::<&str>(), b"", Some(&root.path))?;
    let stim = std::str::from_utf8(&output.stdout.bytes).map_err(GateCatalogError::from)?;
    let stab = model_catalog()?;
    if stim != stab {
        return Err(
            GateCatalogError::Mismatch(first_difference(stim, &stab).into_boxed_str()).into(),
        );
    }
    println!(
        "[stab-oracle] pinned Stim gate catalog matches Stab for {EXPECTED_CANONICAL_GATES} canonical gates and {EXPECTED_ACCEPTED_NAMES} accepted names"
    );
    Ok(())
}

fn model_catalog() -> Result<String, GateCatalogError> {
    let mut rows = Gate::all().map(model_row).collect::<Result<Vec<_>, _>>()?;
    rows.sort_unstable();
    let accepted_names = Gate::all().map(|gate| gate.aliases().len()).sum::<usize>();
    if rows.len() != EXPECTED_CANONICAL_GATES || accepted_names != EXPECTED_ACCEPTED_NAMES {
        return Err(GateCatalogError::Mismatch(
            format!(
                "Stab has {} canonical gates and {accepted_names} accepted names; expected {EXPECTED_CANONICAL_GATES} and {EXPECTED_ACCEPTED_NAMES}",
                rows.len()
            )
            .into_boxed_str(),
        ));
    }
    Ok(rows.concat())
}

fn model_row(gate: Gate) -> Result<String, GateCatalogError> {
    let mut aliases = gate.aliases().to_vec();
    aliases.sort_unstable();
    let inverse = gate.generalized_inverse().map_err(|error| {
        GateCatalogError::Mismatch(
            format!(
                "{} has an invalid generalized inverse: {error}",
                gate.canonical_name()
            )
            .into_boxed_str(),
        )
    })?;
    let mut row = String::new();
    writeln!(
        row,
        "{}\t{}\t{}\t{}\t{}\t{}\t{}",
        gate.canonical_name(),
        inverse.canonical_name(),
        stim_category(gate.category()),
        stim_argument_count(gate.argument_rule())?,
        stim_flags(gate),
        u8::from(gate.is_symmetric_gate()),
        aliases.join(","),
    )
    .map_err(|error| {
        GateCatalogError::Mismatch(
            format!(
                "failed to normalize {} metadata: {error}",
                gate.canonical_name()
            )
            .into_boxed_str(),
        )
    })?;
    Ok(row)
}

fn stim_flags(gate: Gate) -> u16 {
    let mut flags = 0;
    set_flag(&mut flags, GATE_IS_UNITARY, gate.is_unitary());
    set_flag(&mut flags, GATE_IS_NOISY, gate.is_noisy());
    set_flag(
        &mut flags,
        GATE_ARGS_ARE_DISJOINT_PROBABILITIES,
        matches!(
            gate.argument_rule(),
            GateArgumentRule::OptionalProbability
                | GateArgumentRule::ProbabilityList(_)
                | GateArgumentRule::AnyProbabilityList
        ),
    );
    set_flag(
        &mut flags,
        GATE_ARGS_ARE_UNSIGNED_INTEGERS,
        gate.argument_rule() == GateArgumentRule::UnsignedInteger,
    );
    set_flag(
        &mut flags,
        GATE_PRODUCES_RESULTS,
        gate.produces_measurements(),
    );
    set_flag(&mut flags, GATE_IS_NOT_FUSABLE, !gate.can_fuse());
    set_flag(
        &mut flags,
        GATE_IS_BLOCK,
        gate.category() == GateCategory::ControlFlow,
    );
    set_flag(
        &mut flags,
        GATE_IS_SINGLE_QUBIT_GATE,
        gate.is_single_qubit_gate(),
    );
    set_flag(&mut flags, GATE_IS_RESET, gate.is_reset());
    match gate.target_rule() {
        GateTargetRule::None if gate.category() != GateCategory::ControlFlow => {
            flags |= GATE_TAKES_NO_TARGETS;
        }
        GateTargetRule::PlainPairs | GateTargetRule::MeasurementPairs => {
            flags |= GATE_TARGETS_PAIRS;
        }
        GateTargetRule::ClassicalControlPairs => {
            flags |= GATE_TARGETS_PAIRS | GATE_CAN_TARGET_BITS;
        }
        GateTargetRule::RecOnly => flags |= GATE_ONLY_TARGETS_MEASUREMENT_RECORD,
        GateTargetRule::RecOrPauli => {
            flags |= GATE_ONLY_TARGETS_MEASUREMENT_RECORD | GATE_TARGETS_PAULI_STRING;
        }
        GateTargetRule::PauliProducts => {
            flags |= GATE_TARGETS_PAULI_STRING | GATE_TARGETS_COMBINERS;
        }
        GateTargetRule::PauliList => flags |= GATE_TARGETS_PAULI_STRING,
        GateTargetRule::None
        | GateTargetRule::AnySingleQubit
        | GateTargetRule::MeasurementQubits
        | GateTargetRule::MeasurementPads
        | GateTargetRule::QubitCoords => {}
    }
    flags
}

const fn set_flag(flags: &mut u16, flag: u16, enabled: bool) {
    if enabled {
        *flags |= flag;
    }
}

fn stim_argument_count(rule: GateArgumentRule) -> Result<u8, GateCatalogError> {
    match rule {
        GateArgumentRule::Exact(count) | GateArgumentRule::ProbabilityList(count) => {
            u8::try_from(count).map_err(|error| {
                GateCatalogError::Mismatch(
                    format!("gate argument count {count} does not fit Stim metadata: {error}")
                        .into_boxed_str(),
                )
            })
        }
        GateArgumentRule::Any | GateArgumentRule::AnyProbabilityList => Ok(ARG_COUNT_ANY),
        GateArgumentRule::OptionalProbability => Ok(ARG_COUNT_ZERO_OR_ONE),
        GateArgumentRule::UnsignedInteger => Ok(1),
    }
}

const fn stim_category(category: GateCategory) -> &'static str {
    match category {
        GateCategory::Annotation => "Z_Annotations",
        GateCategory::ControlFlow => "Y_Control Flow",
        GateCategory::Collapsing => "L_Collapsing Gates",
        GateCategory::Controlled | GateCategory::ParityPhasing | GateCategory::Swap => {
            "C_Two Qubit Clifford Gates"
        }
        GateCategory::HadamardLike | GateCategory::Period3 | GateCategory::Period4 => {
            "B_Single Qubit Clifford Gates"
        }
        GateCategory::Noise | GateCategory::HeraldedNoise => "F_Noise Channels",
        GateCategory::Pauli => "A_Pauli Gates",
        GateCategory::PauliProduct => "P_Generalized Pauli Product Gates",
        GateCategory::PairMeasurement => "L_Pair Measurement Gates",
    }
}

fn first_difference(stim: &str, stab: &str) -> String {
    let mut stim_lines = stim.lines();
    let mut stab_lines = stab.lines();
    for line_number in 1.. {
        match (stim_lines.next(), stab_lines.next()) {
            (Some(left), Some(right)) if left == right => {}
            (left, right) => {
                return format!("line {line_number}: Stim={left:?}, Stab={right:?}");
            }
        }
    }
    unreachable!("finite strings differ at a finite line")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    #[ignore = "live pinned-Stim differential; run through the parity owner or just oracle::gates"]
    fn pinned_stim_catalog_matches_model_metadata() {
        let root = RepoRoot::resolve(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("oracle crate is two levels below the repository root"),
        )
        .expect("resolve repository root");
        run(&root, false).expect("pinned Stim and Stab gate catalogs match");
    }
}
