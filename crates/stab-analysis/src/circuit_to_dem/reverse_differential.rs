#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "differential fixtures are fixed committed inputs with direct assertions"
)]

//! WS2b Stage 2: run the reverse engine over the committed equivalence
//! matrix in both fold settings and classify every divergence from the
//! pinned-Stim captures by name. This unit-level differential exercises the
//! crate-private seam; the external `consolidation_matrix` harness keeps
//! covering the public dispatch.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use stab_model::{Circuit, DetectorErrorModel, Probability};

use crate::{AnalysisResult, ErrorAnalyzerOptions};

/// WS2b Stage 2 differential seam: run the reverse engine regardless of loop
/// structure (loops unroll under the existing expansion budget when
/// `fold_loops` is off). Test-scoped until Stage 3 flips the public dispatch
/// in `circuit_to_dem.rs` onto this same `reverse_fold::try_analyze` entry.
fn circuit_to_detector_error_model_via_reverse(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> AnalysisResult<DetectorErrorModel> {
    super::reverse_fold::try_analyze(circuit, options)
}

/// Matrix entries whose unrolled reverse (`nofold`) output diverges from the
/// pinned-Stim `nofold` capture. Empty since the reverse path replicated the
/// pinned binary's fused `fma(old, 1 - p, (1 - old) * p)` merge contraction;
/// any regression fails the differential as an unexpected divergence.
const KNOWN_REVERSE_NOFOLD_DIVERGENCES: &[&str] = &[];

/// Matrix entries whose reverse `fold` output diverges from the pinned-Stim
/// `fold` capture. Empty for the same reason as the `nofold` list.
const KNOWN_REVERSE_FOLD_DIVERGENCES: &[&str] = &[];

fn matrix_dir() -> PathBuf {
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

fn reverse_dem(circuit: &Circuit, options: ErrorAnalyzerOptions) -> AnalysisResult<String> {
    circuit_to_detector_error_model_via_reverse(circuit, options).map(|model| model.to_dem_string())
}

#[test]
fn reverse_engine_matches_pinned_captures_outside_the_listed_divergences() {
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
    assert!(names.len() >= 60, "found {} entries", names.len());

    let mut unexpected = Vec::new();
    let mut healed = Vec::new();
    for name in &names {
        let stim_text =
            std::fs::read_to_string(dir.join(format!("{name}.stim"))).expect("matrix circuit");
        let circuit = Circuit::from_stim_str(&stim_text).expect("matrix circuit parses");
        for (mode, fold_loops, listed) in [
            ("nofold", false, KNOWN_REVERSE_NOFOLD_DIVERGENCES),
            ("fold", true, KNOWN_REVERSE_FOLD_DIVERGENCES),
        ] {
            let expected = std::fs::read_to_string(dir.join(format!("{name}.{mode}.dem")))
                .expect("matrix capture exists");
            let actual = reverse_dem(&circuit, options_for(name, fold_loops))
                .unwrap_or_else(|error| panic!("{name} ({mode}) analyzes via reverse: {error}"));
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
        "reverse-engine divergences without a listing: {unexpected:?}"
    );
    assert!(
        healed.is_empty(),
        "listed reverse divergences now byte-match; remove them: {healed:?}"
    );
}
