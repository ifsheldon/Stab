#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_analysis::{
    ErrorAnalyzerOptions, circuit_to_detector_error_model, try_disjoint_to_independent_xyz_errors,
};
use stab_model::{Circuit, Probability};

#[test]
fn dem_analyzer_pauli_channel1_crosses_two_qubit_cliffords_like_stim() {
    let circuit = Circuit::from_stim_str(include_str!(
        "../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_two_qubit_clifford.stim"
    ))
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string();

    assert_eq!(
        dem,
        "error(0.375) D1\nerror(0.125) D2\nerror(0.375) D2 D3\nerror(0.25) D3\nerror(0.375) D4 D5\ndetector D0\n"
    );
}

#[test]
fn dem_analyzer_pauli_channel1_crosses_controlled_pauli_gates_like_stim() {
    let circuit = Circuit::from_stim_str(include_str!(
        "../../../oracle/fixtures/inputs/analyze_errors_pauli_channel1_controlled_pauli.stim"
    ))
    .expect("circuit");
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("analyze")
    .to_dem_string();

    assert_eq!(
        dem,
        "error(0.375) D0 D1\nerror(0.375) D2 D3\nerror(0.625) D4 D5\n"
    );
}

#[test]
fn dem_analyzer_pauli_channel1_newton_edge_case_requires_approximate_like_pinned_stim() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
         PAULI_CHANNEL_1(0.1792, 0.1008, 0.2592) 0\n\
         M 0\n\
         DETECTOR rec[-1]\n",
    )
    .expect("circuit");

    // Pinned Stim's Newton fallback fails to converge for this channel (the
    // vendor `dc = ab_i - ac` step), so analysis without the approximation
    // option rejects; a previous exact-Newton port accepted it.
    let error = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .expect_err("pinned Stim requires approximate_disjoint_errors here")
        .to_string();
    assert!(
        error.contains("requires approximate_disjoint_errors"),
        "{error}"
    );

    // With the option enabled the vendor keeps the raw disjoint components,
    // so the X and Y symptoms sum into one class, byte-matching pinned Stim.
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
            ..ErrorAnalyzerOptions::default()
        },
    )
    .expect("approximate analysis")
    .to_dem_string();
    assert_eq!(dem, "error(0.280000000000000026645352591003757) D0\n");
}

#[test]
fn disjoint_to_independent_pauli_channel1_newton_edge_case_fails_like_pinned_stim() {
    let actual = try_disjoint_to_independent_xyz_errors(
        Probability::try_new(0.1792).unwrap(),
        Probability::try_new(0.1008).unwrap(),
        Probability::try_new(0.2592).unwrap(),
    )
    .expect("solve");

    assert!(actual.is_none());
}

#[test]
fn pauli_channels_match_pinned_stim_combination_order_and_local_decomposition() {
    // Against MX, Z and Y share symptom D0; pinned Stim merges slot `iz` before `iy`
    // (`{0, ix, iz, iy}`), fixing the final printed digit.
    let mx_merge = Circuit::from_stim_str(
        "RX 0\nPAULI_CHANNEL_1(0.13669, 0.05589, 0.14773) 0\nMX 0\nDETECTOR rec[-1]\n",
    )
    .expect("circuit");
    let merged = circuit_to_detector_error_model(&mx_merge, ErrorAnalyzerOptions::default())
        .expect("analyze")
        .to_dem_string();
    assert_eq!(merged, "error(0.2036199999999999954436447069383576) D0\n");

    let decompose_options = ErrorAnalyzerOptions {
        decompose_errors: true,
        approximate_disjoint_errors_threshold: Some(Probability::try_new(1.0).unwrap()),
        ..ErrorAnalyzerOptions::default()
    };
    let pc1_bell = Circuit::from_stim_str(
        "R 0 1\nH 0\nCX 0 1\nPAULI_CHANNEL_1(0.13669, 0.05589, 0.14773) 0\nCX 0 1\nH 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .expect("circuit");
    assert_eq!(
        circuit_to_detector_error_model(&pc1_bell, decompose_options.clone())
            .expect("analyze")
            .to_dem_string(),
        "error(0.1776351926371489420297677952476079) D0\n\
         error(0.1656272721523460722536924549785908) D1\n\
         error(0.04030341831576356170785402355249971) D1 ^ D0\n",
    );

    let pc2_bell = Circuit::from_stim_str(
        "R 0 1\nH 0\nCX 0 1\n\
         PAULI_CHANNEL_2(0.011, 0.012, 0.013, 0.014, 0.015, 0.016, 0.017, 0.018, 0.019, 0.020, 0.021, 0.022, 0.023, 0.024, 0.025) 0 1\n\
         CX 0 1\nH 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .expect("circuit");
    assert_eq!(
        circuit_to_detector_error_model(&pc2_bell, decompose_options)
            .expect("analyze")
            .to_dem_string(),
        "error(0.06999999999999999278355033993648249) D0\n\
         error(0.07000000000000000666133814775093924) D1\n\
         error(0.06999999999999999278355033993648249) D1 ^ D0\n",
    );
}

#[test]
fn pauli_channel2_with_one_component_is_independent_without_approximation_like_pinned_stim() {
    // Pinned Stim's `check_can_approximate_disjoint` lets a channel with at most one non-zero
    // component through without `approximate_disjoint_errors` and ignores the threshold, because
    // such a channel is already an independent error. Expected DEMs were recorded with pinned
    // `stim analyze_errors` under each listed flag set.
    let threshold = |value| Some(Probability::try_new(value).expect("threshold"));
    let unfolded = ErrorAnalyzerOptions::default();
    let folded = ErrorAnalyzerOptions {
        fold_loops: true,
        ..ErrorAnalyzerOptions::default()
    };
    let decomposed_below_threshold = ErrorAnalyzerOptions {
        fold_loops: true,
        decompose_errors: true,
        approximate_disjoint_errors_threshold: threshold(0.01),
        ..ErrorAnalyzerOptions::default()
    };
    let crossing = "R 0 1 2\n\
         PAULI_CHANNEL_2(0, 0, 0, 0, 0.125, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1 1 2\n\
         M 0 1 2\n\
         DETECTOR rec[-3]\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n";
    let looped = "REPEAT 4 {\n\
         R 0 1\n\
         PAULI_CHANNEL_2[tagged](0, 0, 0, 0, 0.375, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\n\
         M 0 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         }\n";
    let zero = "R 0 1\n\
         PAULI_CHANNEL_2(0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\n\
         M 0 1\n\
         DETECTOR rec[-1]\n";
    for (circuit, options, expected) in [
        (
            crossing,
            unfolded.clone(),
            "error(0.125) D0 D1\nerror(0.125) D1 D2 L0\n",
        ),
        (
            crossing,
            ErrorAnalyzerOptions {
                decompose_errors: true,
                ..ErrorAnalyzerOptions::default()
            },
            "error(0.125) D1 ^ D0\nerror(0.125) D2 L0 ^ D1\n",
        ),
        (
            looped,
            unfolded.clone(),
            "error[tagged](0.375) D0 D1\nerror[tagged](0.375) D2 D3\nerror[tagged](0.375) D4 D5\nerror[tagged](0.375) D6 D7\n",
        ),
        (
            looped,
            folded,
            "repeat 4 {\n    error[tagged](0.375) D0 D1\n    shift_detectors 2\n}\n",
        ),
        (
            looped,
            decomposed_below_threshold,
            "repeat 4 {\n    error[tagged](0.375) D1 ^ D0\n    shift_detectors 2\n}\n",
        ),
        (zero, unfolded, "detector D0\n"),
    ] {
        let parsed = Circuit::from_stim_str(circuit).expect("circuit");
        let dem = circuit_to_detector_error_model(&parsed, options.clone())
            .unwrap_or_else(|error| panic!("{circuit:?} with {options:?}: {error}"))
            .to_dem_string();
        assert_eq!(dem, expected, "{circuit:?} with {options:?}");
    }
}
