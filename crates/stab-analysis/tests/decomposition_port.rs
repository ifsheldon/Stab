#![allow(
    clippy::expect_used,
    reason = "decomposition fixtures are fixed test inputs"
)]

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::Circuit;

fn analyzed(circuit_text: &str, decompose: bool) -> Result<String, String> {
    let options = ErrorAnalyzerOptions {
        decompose_errors: decompose,
        ..ErrorAnalyzerOptions::default()
    };
    let circuit = Circuit::from_stim_str(circuit_text).expect("fixture circuit parses");
    circuit_to_detector_error_model(&circuit, options)
        .map(|model| model.to_dem_string())
        .map_err(|error| error.to_string())
}

/// WS2a: pinned Stim's greedy remnant path peels every known pair and single
/// before accepting a remnant of at most two missed detectors, so this
/// six-detector hyperedge decomposes as `D0 D1 ^ D2 D3 ^ D4 D5`; the old
/// one-known-plus-graphlike-rest search rejected it outright.
#[test]
fn six_detector_hyperedge_accepts_a_two_missed_detector_remnant_like_stim() {
    let dem = analyzed(
        "E(0.1) X0 X1\nE(0.1) X2 X3\nE(0.1) X0 X1 X2 X3 X4 X5\nM 0 1 2 3 4 5\nDETECTOR rec[-6]\nDETECTOR rec[-5]\nDETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        true,
    )
    .expect("remnant witness decomposes");
    assert!(
        dem.contains("D0 D1 ^ D2 D3 ^ D4 D5"),
        "expected the pinned-Stim remnant decomposition, got:\n{dem}"
    );
}

/// The exact within-problem search covers the three-detector hyperedge with
/// its known pair and single, byte-matching pinned Stim's output shape.
#[test]
fn hyperedge_decomposes_into_known_pair_and_single_like_stim() {
    let dem = analyzed(
        "E(0.1) X0 X1\nX_ERROR(0.1) 2\nE(0.1) X0 X1 X2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        true,
    )
    .expect("hyperedge witness decomposes");
    assert!(dem.contains("D0 D1 ^ D2"), "{dem}");
}

/// Structural invariants of every decomposition output: the emitted
/// components XOR back to the original problem, no component repeats, and
/// every non-remnant component exists in the known set.
#[test]
fn decomposition_components_xor_back_and_never_repeat() {
    let circuit = "E(0.1) X0 X1\nE(0.1) X2 X3\nX_ERROR(0.1) 2\nOBSERVABLE_INCLUDE(0) Z5\nE(0.1) X0 X1 X2\nE(0.1) X0 X1 X2 X3 X4 X5\nM 0 1 2 3 4 5\nDETECTOR rec[-6]\nDETECTOR rec[-5]\nDETECTOR rec[-4]\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let plain = analyzed(circuit, false).expect("plain analysis");
    let decomposed = analyzed(circuit, true).expect("decomposed analysis");

    let mut plain_symptoms = plain
        .lines()
        .filter(|line| line.starts_with("error("))
        .map(symptom_set)
        .collect::<Vec<_>>();
    plain_symptoms.sort();
    let mut decomposed_symptoms = Vec::new();
    // Non-composite graphlike lines are the known-component set; a composite
    // line may introduce at most one remnant component, which then joins the
    // known set exactly like the vendor's decompose_and_append_component_to_tail.
    let mut known_components = decomposed
        .lines()
        .filter(|line| line.starts_with("error(") && !line.contains(" ^ "))
        .filter_map(|line| {
            line.split_once(") ")
                .map(|(_, targets)| targets.to_string())
        })
        .collect::<std::collections::BTreeSet<_>>();
    for line in decomposed.lines().filter(|line| line.starts_with("error(")) {
        decomposed_symptoms.push(symptom_set(line));
        let components = line
            .split_once(") ")
            .map(|(_, targets)| targets)
            .unwrap_or_default()
            .split(" ^ ")
            .collect::<Vec<_>>();
        let mut seen = std::collections::BTreeSet::new();
        for component in &components {
            assert!(
                seen.insert(component.to_string()),
                "component {component:?} repeats in {line:?}"
            );
        }
        if components.len() > 1 {
            let unknown = components
                .iter()
                .filter(|component| !known_components.contains(**component))
                .collect::<Vec<_>>();
            assert!(
                unknown.len() <= 1,
                "decomposition {line:?} uses more than one component outside the known set: {unknown:?}"
            );
            for component in unknown {
                known_components.insert((*component).to_string());
            }
        }
    }
    decomposed_symptoms.sort();
    assert_eq!(
        plain_symptoms, decomposed_symptoms,
        "decomposition must preserve every error's total symptom set"
    );
}

fn symptom_set(line: &str) -> Vec<String> {
    let targets = line
        .split_once(") ")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    let mut set = std::collections::BTreeSet::new();
    for token in targets.split_whitespace() {
        if token == "^" {
            continue;
        }
        if !set.insert(token.to_string()) {
            set.remove(token);
        }
    }
    set.into_iter().collect()
}

/// Pinned Stim refuses to decompose components with 64 or more terms.
#[test]
fn components_with_sixty_four_terms_reject_with_stims_error_class() {
    let mut text = String::new();
    for qubit in 0..70 {
        text.push_str(&format!("E(0.1) X{qubit} X{}\n", qubit + 70));
    }
    text.push_str("E(0.1)");
    for qubit in 0..70 {
        text.push_str(&format!(" X{qubit}"));
    }
    text.push('\n');
    text.push('M');
    for qubit in 0..140 {
        text.push_str(&format!(" {qubit}"));
    }
    text.push('\n');
    for offset in 1..=140 {
        text.push_str(&format!("DETECTOR rec[-{offset}]\n"));
    }
    let error = analyzed(&text, true).expect_err("seventy-term component must reject");
    assert!(
        error.contains("more than 64 terms"),
        "expected the pinned-Stim term-cap class, got: {error}"
    );
}

/// Zero-probability classes are excluded from the known component set, so the
/// hyperedge cannot decompose through the never-firing `D0 D1` edge and takes
/// the remnant shape instead, byte-matching the probed pinned binary.
#[test]
fn zero_probability_components_are_not_usable_for_decomposition() {
    let dem = analyzed(
        "E(0) X0 X1\nX_ERROR(0.1) 2\nE(0.1) X0 X1 X2\nM 0 1 2\nDETECTOR rec[-3]\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
        true,
    )
    .expect("the remnant path covers the hyperedge");
    assert!(
        dem.contains("D2 ^ D0 D1"),
        "the zero-probability edge must not appear as a known component: {dem}"
    );
    assert!(!dem.contains("error(0) "), "{dem}");
}
