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
/// Since the Stage 3 flip both fold modes run on the reverse engine and every
/// entry must byte-match its capture; the divergence lists are empty, and any
/// entry that stops matching fails as an unexpected divergence.
const KNOWN_NOFOLD_DIVERGENCES: &[&str] = &[];

/// Fold-mode divergences, empty for the same reason as the `nofold` list.
const KNOWN_FOLD_DIVERGENCES: &[&str] = &[];

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
    assert_eq!(
        names.len(),
        63,
        "the committed equivalence matrix owns exactly sixty-three entries"
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
