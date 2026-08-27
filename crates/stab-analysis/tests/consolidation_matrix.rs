#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "matrix fixtures are fixed committed inputs with direct assertions"
)]

use std::collections::BTreeSet;
use std::path::Path;

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::{Circuit, Probability};

const LOCAL_DECOMPOSITION_16_CIRCUIT: &str =
    include_str!("../../../oracle/fixtures/inputs/pfm_b5_analyzer_local_decomposition_16.stim");
const LOCAL_DECOMPOSITION_16_EXPECTED: &str =
    include_str!("../../../oracle/fixtures/expected/pfm_b5_analyzer_local_decomposition_16.stdout");
const REMNANT_DECOMPOSITION_CIRCUIT: &str = "E(0.1) X0 X1\n\
     E(0.1) X2 X3\n\
     E(0.1) X0 X1 X2 X3 X4 X5\n\
     M 0 1 2 3 4 5\n\
     DETECTOR rec[-6]\n\
     DETECTOR rec[-5]\n\
     DETECTOR rec[-4]\n\
     DETECTOR rec[-3]\n\
     DETECTOR rec[-2]\n\
     DETECTOR rec[-1]\n";

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
fn circuit_to_dem_selected_gate_surface_semantic_matrix_matches_stim() {
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

#[test]
fn circuit_to_dem_error_decomposition_contract_matches_stim() {
    for (name, circuit_text, expected) in [
        (
            "exact pair and singleton decomposition",
            "E(0.1) X0 X1\n\
             X_ERROR(0.1) 2\n\
             E(0.1) X0 X1 X2\n\
             M 0 1 2\n\
             DETECTOR rec[-3]\n\
             DETECTOR rec[-2]\n\
             DETECTOR rec[-1]\n",
            concat!(
                "error(0.1000000000000000055511151231257827) D0 D1\n",
                "error(0.1000000000000000055511151231257827) D0 D1 ^ D2\n",
                "error(0.1000000000000000055511151231257827) D2\n",
            ),
        ),
        (
            "greedy decomposition with one remnant edge",
            REMNANT_DECOMPOSITION_CIRCUIT,
            concat!(
                "error(0.1000000000000000055511151231257827) D0 D1\n",
                "error(0.1000000000000000055511151231257827) D0 D1 ^ D2 D3 ^ D4 D5\n",
                "error(0.1000000000000000055511151231257827) D2 D3\n",
            ),
        ),
    ] {
        let circuit = Circuit::from_stim_str(circuit_text).expect("decomposition fixture parses");
        let actual = circuit_to_detector_error_model(
            &circuit,
            ErrorAnalyzerOptions {
                decompose_errors: true,
                ..ErrorAnalyzerOptions::default()
            },
        )
        .unwrap_or_else(|error| panic!("{name} analyzes: {error}"))
        .to_dem_string();
        assert_eq!(actual, expected, "{name}");
    }

    let remnant_circuit =
        Circuit::from_stim_str(REMNANT_DECOMPOSITION_CIRCUIT).expect("remnant fixture parses");
    let error = circuit_to_detector_error_model(
        &remnant_circuit,
        ErrorAnalyzerOptions {
            decompose_errors: true,
            block_decomposition_from_introducing_remnant_edges: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect_err("blocking remnant edges rejects the pinned witness")
    .to_string();
    assert!(
        error.contains("Failed to decompose errors into graphlike components")
            && error.contains("block_decomposition_from_introducing_remnant_edges"),
        "{error}"
    );
}

#[test]
fn circuit_to_dem_local_decomposition_symptom_boundary_matches_stim() {
    let local_sixteen = Circuit::from_stim_str(LOCAL_DECOMPOSITION_16_CIRCUIT)
        .expect("sixteen-detector local decomposition fixture parses");
    let local_options = ErrorAnalyzerOptions {
        fold_loops: true,
        decompose_errors: true,
        ignore_decomposition_failures: true,
        ..ErrorAnalyzerOptions::default()
    };
    assert_eq!(
        circuit_to_detector_error_model(&local_sixteen, local_options)
            .expect("sixteen-detector local decomposition succeeds")
            .to_dem_string(),
        LOCAL_DECOMPOSITION_16_EXPECTED
    );

    let local_seventeen = Circuit::from_stim_str(&format!(
        "{LOCAL_DECOMPOSITION_16_CIRCUIT}DETECTOR rec[-1]\n"
    ))
    .expect("seventeen-detector local decomposition fixture parses");
    let error = circuit_to_detector_error_model(&local_seventeen, local_options)
        .expect_err("seventeen detector symptoms exceed Stim's local mask")
        .to_string();
    assert!(error.contains("exceeded 16 detector symptoms"), "{error}");
}
