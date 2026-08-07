#![allow(
    clippy::expect_used,
    reason = "reverse-family fixtures are fixed test inputs"
)]

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::{Circuit, Probability};

/// WS2b Stage 0: the reverse tracker now analyzes the three formerly
/// fallback-only families, byte-matching pinned Stim v1.16.0 in both fold
/// modes (each expectation probed against the pinned binary).
fn analyzed(circuit_text: &str, fold_loops: bool) -> Result<String, String> {
    let options = ErrorAnalyzerOptions {
        fold_loops,
        approximate_disjoint_errors_threshold: Some(
            Probability::try_new(1.0).expect("unit threshold"),
        ),
        ..ErrorAnalyzerOptions::default()
    };
    let circuit = Circuit::from_stim_str(circuit_text).expect("fixture circuit parses");
    circuit_to_detector_error_model(&circuit, options)
        .map(|model| model.to_dem_string())
        .map_err(|error| error.to_string())
}

#[test]
fn heralded_erase_analyzes_identically_in_both_fold_modes() {
    let circuit = "R 0\nHERALDED_ERASE(0.25) 0\nM 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n";
    let expected = "error(0.125) D0\nerror(0.125) D0 D1 L0\n";
    assert_eq!(analyzed(circuit, false).expect("nofold"), expected);
    assert_eq!(analyzed(circuit, true).expect("fold"), expected);
}

#[test]
fn heralded_pauli_channel_uses_the_vendor_slot_order() {
    // hi + hz stay herald-only against a Z-basis measurement; hx + hy flip it.
    let circuit = "R 0\nHERALDED_PAULI_CHANNEL_1(0.05, 0.1, 0.15, 0.2) 0\nM 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n";
    let expected = "error(0.25) D0\nerror(0.25) D0 D1\n";
    assert_eq!(analyzed(circuit, false).expect("nofold"), expected);
    assert_eq!(analyzed(circuit, true).expect("fold"), expected);
}

#[test]
fn heralded_erase_folds_inside_repeat_blocks_like_stim() {
    let circuit = "R 0 1\nREPEAT 2 {\nHERALDED_ERASE(0.25) 0\nM 0\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nR 0\n}\n";
    let expected = "error(0.125) D0\nerror(0.125) D0 D1\nerror(0.125) D2\nerror(0.125) D2 D3\n";
    assert_eq!(analyzed(circuit, false).expect("nofold"), expected);
    assert_eq!(analyzed(circuit, true).expect("fold"), expected);
}

#[test]
fn else_correlated_chains_telescope_like_stim() {
    let circuit = "R 0 1\nE(0.3) X0\nELSE_CORRELATED_ERROR(0.2) X0 X1\nELSE_CORRELATED_ERROR(0.1) X1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n";
    let expected = "error(0.2999999999999999888977697537484346) D0\nerror(0.139999999999999985567100679872965) D0 D1\nerror(0.05599999999999999422684027194918599) D1\n";
    assert_eq!(analyzed(circuit, false).expect("nofold"), expected);
    assert_eq!(analyzed(circuit, true).expect("fold"), expected);
}

#[test]
fn else_correlated_chains_analyze_inside_repeat_blocks() {
    let circuit = "R 0 1\nREPEAT 2 {\nE(0.3) X0\nELSE_CORRELATED_ERROR(0.2) X0 X1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nR 0 1\n}\n";
    let expected = "error(0.2999999999999999888977697537484346) D0\nerror(0.139999999999999985567100679872965) D0 D1\nerror(0.2999999999999999888977697537484346) D2\nerror(0.139999999999999985567100679872965) D2 D3\n";
    assert_eq!(analyzed(circuit, false).expect("nofold"), expected);
    assert_eq!(analyzed(circuit, true).expect("fold"), expected);
}

#[test]
fn reverse_family_error_classes_match_across_fold_modes() {
    for fold_loops in [false, true] {
        let options = ErrorAnalyzerOptions {
            fold_loops,
            ..ErrorAnalyzerOptions::default()
        };
        let heralded =
            Circuit::from_stim_str("R 0\nHERALDED_ERASE(0.25) 0\nM 0\nDETECTOR rec[-1]\n")
                .expect("heralded fixture");
        let error = circuit_to_detector_error_model(&heralded, options)
            .expect_err("heralded analysis requires the approximate option");
        assert!(
            error
                .to_string()
                .contains("HERALDED_ERASE requires approximate_disjoint_errors"),
            "fold_loops={fold_loops}: {error}"
        );

        let chain = Circuit::from_stim_str(
            "R 0 1\nE(0.3) X0\nELSE_CORRELATED_ERROR(0.2) X1\nM 0 1\nDETECTOR rec[-1]\n",
        )
        .expect("chain fixture");
        let error = circuit_to_detector_error_model(&chain, options)
            .expect_err("chains require the approximate option");
        assert!(
            error
                .to_string()
                .contains("ELSE_CORRELATED_ERROR requires approximate_disjoint_errors"),
            "fold_loops={fold_loops}: {error}"
        );
    }
}
