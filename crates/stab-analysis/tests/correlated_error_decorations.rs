#![allow(
    clippy::expect_used,
    reason = "decorated correlated-error fixtures are fixed test inputs"
)]

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_model::{Circuit, Probability};

/// Pinned Stim's correlated-error handlers consult only the Pauli X/Z bits
/// (frame_simulator.inl:767-775), so combiner targets and inversion bits must
/// not change the analyzed detector error model on either fold path.
#[test]
fn correlated_error_decorations_do_not_change_the_analyzed_model() {
    let plain = "R 0 1\nE(0.1) X0 X1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n";
    for decorated in [
        "R 0 1\nE(0.1) X0*X1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1\nE(0.1) X0 * X1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1\nE(0.1) !X0 !X1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
        "R 0 1\nE(0.1) *X0 X1*\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n",
    ] {
        assert_analyzed_models_match(plain, decorated);
    }

    let plain_else = "R 0 1\nE(0.3) X0\nELSE_CORRELATED_ERROR(0.2) Z0 Z1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n";
    let decorated_else = "R 0 1\nE(0.3) X0\nELSE_CORRELATED_ERROR(0.2) !Z0 * Z1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n";
    assert_analyzed_models_match(plain_else, decorated_else);
}

fn assert_analyzed_models_match(plain: &str, decorated: &str) {
    for fold_loops in [false, true] {
        let options = ErrorAnalyzerOptions {
            fold_loops,
            approximate_disjoint_errors_threshold: Some(
                Probability::try_new(1.0).expect("unit threshold"),
            ),
            ..ErrorAnalyzerOptions::default()
        };
        let plain_model = circuit_to_detector_error_model(
            &Circuit::from_stim_str(plain).expect("plain fixture parses"),
            options,
        )
        .expect("plain fixture analyzes");
        let decorated_model = circuit_to_detector_error_model(
            &Circuit::from_stim_str(decorated).expect("decorated fixture parses"),
            options,
        )
        .expect("decorated fixture analyzes");
        assert_eq!(
            decorated_model.to_dem_string(),
            plain_model.to_dem_string(),
            "fold_loops={fold_loops} {decorated:?}"
        );
    }
}
