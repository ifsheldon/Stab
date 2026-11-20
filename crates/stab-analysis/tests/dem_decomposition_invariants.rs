#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "matrix invariant tests use committed fixtures with direct assertion diagnostics"
)]

//! Error decomposition must only re-express symptoms, never change them. For every
//! consolidation-matrix circuit in both fold modes this checks, like Stim's
//! `honeycomb_code_decomposes` and `is_graph_like` (error_analyzer.test.cc):
//!
//! - the decomposed DEM describes the same independent-error distribution as the undecomposed
//!   DEM once each error's `^`-separated components are XORed back together;
//! - every component is non-empty and graphlike-or-reported: when strict decomposition succeeds
//!   every component has at most two detectors and ignoring failures changes nothing; when it
//!   fails, the lenient model really contains a non-graphlike component.

use std::collections::BTreeSet;
use std::path::Path;

use stab_analysis::{
    AnalysisError, AnalysisOperation, ErrorAnalyzerOptions, circuit_to_detector_error_model,
};
use stab_model::{Circuit, Probability};

#[path = "support/dem_semantics.rs"]
mod dem_semantics;

use dem_semantics::{ResolvedDem, Symptom, assert_same_distribution};

fn matrix_circuits() -> Vec<(String, Circuit)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/consolidation_matrix");
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(&dir).expect("matrix directory exists") {
        let file_name = entry.expect("matrix entry").file_name();
        if let Some(name) = file_name.to_string_lossy().strip_suffix(".stim") {
            names.insert(name.to_string());
        }
    }
    names
        .into_iter()
        .map(|name| {
            let text =
                std::fs::read_to_string(dir.join(format!("{name}.stim"))).expect("matrix circuit");
            let circuit = Circuit::from_stim_str(&text).expect("matrix circuit parses");
            (name, circuit)
        })
        .collect()
}

/// Mirrors the per-family options of `consolidation_matrix.rs`.
fn base_options(name: &str, fold_loops: bool) -> ErrorAnalyzerOptions {
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

fn detector_count(symptoms: &BTreeSet<Symptom>) -> usize {
    symptoms
        .iter()
        .filter(|symptom| matches!(symptom, Symptom::Detector(_)))
        .count()
}

#[test]
fn decomposed_matrix_errors_preserve_symptoms_and_are_graphlike() {
    let circuits = matrix_circuits();
    assert_eq!(circuits.len(), 63, "consolidation matrix size changed");
    let mut strict_failures = Vec::new();
    let mut split_errors = 0_usize;
    for (name, circuit) in &circuits {
        for fold_loops in [false, true] {
            let context = format!("{name} (fold_loops={fold_loops})");
            let base = base_options(name, fold_loops);
            let analyze = |options| circuit_to_detector_error_model(circuit, options);
            let plain = analyze(base.clone()).unwrap_or_else(|error| panic!("{context}: {error}"));
            let lenient = analyze(ErrorAnalyzerOptions {
                decompose_errors: true,
                ignore_decomposition_failures: true,
                ..base.clone()
            })
            .unwrap_or_else(|error| panic!("{context} lenient: {error}"));

            let lenient_view = ResolvedDem::new(&lenient);
            assert_same_distribution(
                &lenient_view.symptom_distribution(),
                &ResolvedDem::new(&plain).symptom_distribution(),
                1e-12,
                &context,
            );
            let mut has_non_graphlike_component = false;
            for error in &lenient_view.errors {
                assert!(
                    error
                        .components
                        .iter()
                        .all(|component| !component.is_empty()),
                    "{context}: empty decomposition component in {error:?}"
                );
                if error.components.len() > 1 {
                    split_errors += 1;
                }
                has_non_graphlike_component |= error
                    .components
                    .iter()
                    .any(|component| detector_count(component) > 2);
            }

            match analyze(ErrorAnalyzerOptions {
                decompose_errors: true,
                ..base.clone()
            }) {
                Ok(strict) => {
                    assert!(
                        !has_non_graphlike_component,
                        "{context}: strict decomposition accepted a non-graphlike component"
                    );
                    assert_eq!(
                        strict.to_dem_string(),
                        lenient.to_dem_string(),
                        "{context}: ignoring failures changed a successful decomposition"
                    );
                }
                Err(AnalysisError::ErrorDecompositionFailed { .. }) => {
                    assert!(
                        has_non_graphlike_component,
                        "{context}: strict decomposition failed but every lenient component is graphlike"
                    );
                    strict_failures.push(context);
                }
                Err(error) => panic!("{context}: unexpected strict failure kind: {error}"),
            }
        }
    }
    // Guard against the matrix silently losing the coverage this test exists for.
    assert!(split_errors > 0, "no matrix error was decomposed");
    assert!(
        strict_failures.len() < circuits.len(),
        "every matrix circuit failed strict decomposition: {strict_failures:?}"
    );
}

/// Pinned Stim moves a globally decomposed class onto its rewritten key after every undecomposed
/// class is already present, and its fused merge is asymmetric. Here `E` flips `D0 D1 D2`, which
/// decomposes onto the `D0 D1 ^ D2` class that `DEPOLARIZE2` already built locally; merging in
/// class-key order instead rounded the last bits differently. Expected output is the pinned
/// Stim v1.16.0 `stim analyze_errors --decompose_errors` capture.
#[test]
fn global_decomposition_merges_into_existing_classes_in_stim_order() {
    let circuit = Circuit::from_stim_str(
        "R 0 1 2\nDEPOLARIZE2(0.1) 0 2\nCX 0 1\nE(0.2) X0 X1 X2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .expect("circuit");
    for fold_loops in [false, true] {
        let dem = circuit_to_detector_error_model(
            &circuit,
            ErrorAnalyzerOptions {
                fold_loops,
                decompose_errors: true,
                ..ErrorAnalyzerOptions::default()
            },
        )
        .expect("analyze");
        assert_eq!(
            dem.to_dem_string(),
            concat!(
                "error(0.02741843737473917755331775936156191) D0 D1\n",
                "error(0.2164510624248435488592434694510303) D0 D1 ^ D2\n",
                "error(0.02741843737473917755331775936156191) D2\n",
            ),
            "fold_loops={fold_loops}"
        );
    }
}

/// A gauge chain whose start-of-circuit gauge error is one component of `rounds - 1` detectors,
/// each of which is a known single-detector error.
fn long_gauge_component(rounds: u64) -> Circuit {
    Circuit::from_stim_str(&format!(
        "M 2\nMRX(0.125) 0\nREPEAT {rounds} {{\n    DETECTOR rec[-1] rec[-2]\n    MY 0\n    MRX(0.003) 1\n}}\n"
    ))
    .expect("circuit")
}

/// Pinned Stim v1.16.0 shifts a plain `int` in `brute_force_decomp_helper`, so a component with
/// 32 or more terms loses terms (`D33` is dropped from this 33-term case). Stab's search mask is
/// 64 bits wide, so every printed component XORs back to the undecomposed error.
#[test]
fn decomposition_of_components_past_32_terms_preserves_every_term() {
    for fold_loops in [false, true] {
        let context = format!("fold_loops={fold_loops}");
        let base = ErrorAnalyzerOptions {
            fold_loops,
            allow_gauge_detectors: true,
            ..ErrorAnalyzerOptions::default()
        };
        let circuit = long_gauge_component(34);
        let plain = circuit_to_detector_error_model(&circuit, base.clone()).expect("plain");
        let decomposed = circuit_to_detector_error_model(
            &circuit,
            ErrorAnalyzerOptions {
                decompose_errors: true,
                ..base.clone()
            },
        )
        .expect("decomposed");

        let view = ResolvedDem::new(&decomposed);
        assert_same_distribution(
            &view.symptom_distribution(),
            &ResolvedDem::new(&plain).symptom_distribution(),
            1e-12,
            &context,
        );
        let widest = view
            .errors
            .iter()
            .map(|error| {
                error
                    .components
                    .iter()
                    .fold(BTreeSet::new(), |mut sum, component| {
                        for symptom in component {
                            if !sum.remove(symptom) {
                                sum.insert(*symptom);
                            }
                        }
                        sum
                    })
                    .len()
            })
            .max();
        assert_eq!(
            widest,
            Some(33),
            "{context}: the 33-detector error lost terms"
        );

        // Past the 64-bit mask, Stab refuses exactly as pinned Stim does instead of guessing.
        let error = circuit_to_detector_error_model(
            &long_gauge_component(70),
            ErrorAnalyzerOptions {
                decompose_errors: true,
                ignore_decomposition_failures: true,
                ..base
            },
        )
        .expect_err("components of 64 or more terms are not decomposed");
        assert!(
            matches!(&error, AnalysisError::UnsupportedInput { operation: AnalysisOperation::CircuitToDetectorErrorModel, detail, .. } if detail.contains("more than 64 terms")),
            "{context}: {error}"
        );
    }
}
