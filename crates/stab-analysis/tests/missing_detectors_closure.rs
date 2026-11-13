#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "matrix closure tests use committed fixtures with direct assertion diagnostics"
)]

//! `missing_detectors` must return a complete, deterministic completion: after appending its
//! suggestions a second run finds nothing, and the suggested detectors are accepted by the error
//! analyzer without gauge allowance. Checked on the consolidation-matrix circuits after removing a
//! deterministic subset of their detectors and after removing all of them.

use std::collections::BTreeSet;
use std::path::Path;

use stab_analysis::{
    ErrorAnalyzerOptions, MissingDetectorOptions, circuit_to_detector_error_model,
    flattened_circuit, missing_detectors,
};
use stab_model::{Circuit, Probability};

const KNOWN_INPUT: MissingDetectorOptions = MissingDetectorOptions {
    ignore_non_deterministic_measurements: false,
    cancellation: stab_model::Cancellation::never(),
};

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

/// Flattened circuit text with every `keep(index)`-rejected `DETECTOR` line removed.
fn without_detectors(flat: &str, keep: impl Fn(usize) -> bool) -> (String, usize) {
    let mut text = String::new();
    let mut detector_index = 0_usize;
    let mut removed = 0_usize;
    for line in flat.lines() {
        if line.starts_with("DETECTOR") {
            let kept = keep(detector_index);
            detector_index += 1;
            if !kept {
                removed += 1;
                continue;
            }
        }
        text.push_str(line);
        text.push('\n');
    }
    (text, removed)
}

fn suggestions(text: &str, context: &str) -> Circuit {
    let circuit = Circuit::from_stim_str(text).expect("stripped circuit parses");
    missing_detectors(&circuit, KNOWN_INPUT)
        .unwrap_or_else(|error| panic!("{context}: missing_detectors failed: {error}"))
}

/// Appends the suggestions and returns the completed circuit after checking closure.
fn completed_and_closed(text: &str, context: &str) -> (Circuit, usize) {
    let suggested = suggestions(text, context);
    let suggested_text = suggested.to_stim_string();
    let suggested_count = suggested_text
        .lines()
        .filter(|line| line.starts_with("DETECTOR"))
        .count();
    assert_eq!(
        suggested_count,
        suggested_text.lines().count(),
        "{context}: suggestions contain non-detector instructions:\n{suggested_text}"
    );
    let completed_text = format!("{text}{suggested_text}");
    let second = suggestions(&completed_text, context);
    assert!(
        second.to_stim_string().is_empty(),
        "{context}: suggestions were incomplete; second run found:\n{}",
        second.to_stim_string()
    );
    (
        Circuit::from_stim_str(&completed_text).expect("completed circuit parses"),
        suggested_count,
    )
}

fn analyzer_options(name: &str) -> ErrorAnalyzerOptions {
    let mut options = ErrorAnalyzerOptions::default();
    if name.starts_with("heralded_") || name.starts_with("else_") {
        options.approximate_disjoint_errors_threshold =
            Some(Probability::try_new(1.0).expect("unit threshold"));
    }
    options
}

#[test]
fn missing_detector_suggestions_close_and_are_deterministic() {
    let circuits = matrix_circuits();
    assert_eq!(circuits.len(), 63, "consolidation matrix size changed");
    let mut total_restored = 0_usize;
    // Detector structure does not depend on the noise model, so generated codes keep only their
    // noiseless and most-noisy (`acd`) variants; distance-5 surface codes repeat the distance-3
    // structure at several seconds each in debug builds.
    for (name, circuit) in circuits.iter().filter(|(name, _)| {
        let generated = name.contains("_memory_");
        let noise_variant_kept = name.ends_with("_none") || name.ends_with("_acd");
        let slow = name.starts_with("surface_code_") && name.contains("_d5_");
        !generated || (noise_variant_kept && !slow)
    }) {
        let flat = flattened_circuit(circuit)
            .expect("matrix circuit flattens")
            .to_stim_string();
        let originally_complete = suggestions(&flat, name).to_stim_string().is_empty();
        let gauge = name.starts_with("gauge_");

        // Remove every third detector, offset per circuit so different positions are exercised.
        let offset = name.len() % 3;
        let (partial, removed) = without_detectors(&flat, |index| (index + offset) % 3 != 0);
        let context = format!("{name} (partial strip)");
        let (completed, restored) = completed_and_closed(&partial, &context);
        if originally_complete && !gauge {
            // Matrix detectors are independent deterministic parities, so the removed ones span
            // the missing space except where record-only observables already cover it. The
            // suggestions are a basis of that space: no more than removed, and no fewer than
            // removed minus the observables.
            let observables = flat
                .lines()
                .filter_map(|line| line.strip_prefix("OBSERVABLE_INCLUDE("))
                .filter_map(|rest| rest.split(')').next())
                .collect::<BTreeSet<_>>()
                .len();
            assert!(
                restored <= removed && restored + observables >= removed,
                "{context}: restored {restored} detectors after removing {removed} \
                 with {observables} observables"
            );
        }
        total_restored += restored;
        if !gauge {
            circuit_to_detector_error_model(&completed, analyzer_options(name))
                .unwrap_or_else(|error| panic!("{context}: completed circuit rejected: {error}"));
        }

        // With every detector removed the suggestions alone must be deterministic, even for
        // circuits whose own detectors are gauges.
        let (bare, _) = without_detectors(&flat, |_| false);
        let context = format!("{name} (full strip)");
        let (completed, _) = completed_and_closed(&bare, &context);
        circuit_to_detector_error_model(&completed, analyzer_options(name))
            .unwrap_or_else(|error| panic!("{context}: suggested detectors rejected: {error}"));
    }
    assert!(
        total_restored > 0,
        "no matrix circuit exercised a restoration"
    );
}
