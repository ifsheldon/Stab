#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "matrix fixtures are fixed committed inputs with direct assertions"
)]

use std::collections::BTreeSet;
use std::path::Path;

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::{Circuit, Probability};

/// WS2b Stage 1: every equivalence-matrix entry is a committed pinned-Stim
/// byte-exact DEM capture (`<name>.stim` + `<name>.{nofold,fold}.dem`).
/// Entries in `KNOWN_DIVERGENCES` reproduce the baseline analyzer divergence
/// recorded in docs/plans/analyzer-consolidation-plan.md and must flip to
/// byte-equal at Stage 3; everything else must byte-match today.
/// Entries whose FORWARD-engine (`nofold`) output diverges from the
/// pinned-Stim capture; the forward engine keeps its pre-consolidation
/// rounding until Stage 4 deletes it, so this list shrinks at Stage 3.
const KNOWN_NOFOLD_DIVERGENCES: &[&str] = &[
    "color_code_memory_xyz_d3_acd",
    "color_code_memory_xyz_d3_brd",
    "color_code_memory_xyz_d5_acd",
    "color_code_memory_xyz_d5_brd",
    "pauli_include_after_error",
    "pauli_include_before_error",
    "repetition_code_memory_d3_acd",
    "repetition_code_memory_d5_acd",
    "surface_code_rotated_memory_x_d3_acd",
    "surface_code_rotated_memory_x_d3_brd",
    "surface_code_rotated_memory_x_d5_acd",
    "surface_code_rotated_memory_x_d5_brd",
    "surface_code_rotated_memory_z_d3_acd",
    "surface_code_rotated_memory_z_d3_brd",
    "surface_code_rotated_memory_z_d5_acd",
    "surface_code_rotated_memory_z_d5_brd",
    "surface_code_unrotated_memory_z_d3_acd",
    "surface_code_unrotated_memory_z_d5_acd",
];

/// Entries whose reverse-path (`fold`) output diverges from the pinned-Stim
/// capture. The DEPOLARIZE-rounding residue healed when the reverse merge
/// replicated the pinned binary's fused `fma` contraction; the two remaining
/// entries are the loop-free Pauli-include witnesses, which the public fold
/// dispatch still routes to the forward engine until Stage 3.
const KNOWN_FOLD_DIVERGENCES: &[&str] =
    &["pauli_include_after_error", "pauli_include_before_error"];

fn matrix_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/consolidation_matrix")
}

fn options_for(name: &str, fold_loops: bool) -> ErrorAnalyzerOptions {
    let mut options = ErrorAnalyzerOptions {
        fold_loops,
        ..ErrorAnalyzerOptions::default()
    };
    if name.starts_with("gauge_") {
        options.allow_gauge_detectors = true;
    }
    if name.starts_with("heralded_") || name.starts_with("else_") {
        options.approximate_disjoint_errors_threshold =
            Some(Probability::try_new(1.0).expect("unit threshold"));
    }
    options
}

#[test]
fn consolidation_matrix_is_complete_and_classified() {
    let dir = matrix_dir();
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("matrix directory exists") {
        let entry = entry.expect("matrix entry");
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if let Some(name) = file_name.strip_suffix(".stim") {
            names.insert(name.to_string());
        }
    }
    assert!(
        names.len() >= 60,
        "expected the full equivalence matrix, found {} entries",
        names.len()
    );

    let mut unexpected = Vec::new();
    let mut healed = Vec::new();
    for name in &names {
        let stim_text =
            std::fs::read_to_string(dir.join(format!("{name}.stim"))).expect("matrix circuit");
        let circuit = Circuit::from_stim_str(&stim_text).expect("matrix circuit parses");
        for (mode, fold_loops, listed) in [
            ("nofold", false, KNOWN_NOFOLD_DIVERGENCES),
            ("fold", true, KNOWN_FOLD_DIVERGENCES),
        ] {
            let expected = std::fs::read_to_string(dir.join(format!("{name}.{mode}.dem")))
                .expect("matrix capture exists");
            let actual = circuit_to_detector_error_model(&circuit, options_for(name, fold_loops))
                .unwrap_or_else(|error| panic!("{name} ({mode}) analyzes: {error}"))
                .to_dem_string();
            let diverges = actual != expected;
            let is_listed = listed.contains(&name.as_str());
            if diverges && !is_listed {
                unexpected.push(format!("{name} ({mode})"));
            }
            if !diverges && is_listed {
                healed.push(format!("{name} ({mode})"));
            }
        }
    }
    assert!(
        unexpected.is_empty(),
        "matrix entries diverge from their pinned-Stim captures without being listed: {unexpected:?}"
    );
    assert!(
        healed.is_empty(),
        "listed divergences now byte-match; remove them from the mode list: {healed:?}"
    );
}
