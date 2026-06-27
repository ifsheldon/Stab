#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "unit tests use direct assertions for compact diagnostics"
)]

use super::*;
use crate::Circuit;

fn approximate_options() -> ErrorAnalyzerOptions {
    threshold_options(1.0)
}

fn threshold_options(threshold: f64) -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        approximate_disjoint_errors_threshold: Some(Probability::try_new(threshold).unwrap()),
        ..ErrorAnalyzerOptions::default()
    }
}

#[test]
fn dem_targets_parse_stim_limits() {
    assert_eq!(
        "D4611686018427387903".parse::<DemTarget>().unwrap(),
        DemTarget::relative_detector(4_611_686_018_427_387_903).unwrap()
    );
    assert_eq!(
        "L4294967295".parse::<DemTarget>().unwrap(),
        DemTarget::logical_observable(4_294_967_295).unwrap()
    );
    assert_eq!("^".parse::<DemTarget>().unwrap(), DemTarget::separator());
    assert_eq!("10".parse::<DemTarget>().unwrap(), DemTarget::numeric(10));

    assert!("D4611686018427387904".parse::<DemTarget>().is_err());
    assert!("L4294967296".parse::<DemTarget>().is_err());
    assert!("D-1".parse::<DemTarget>().is_err());
    assert!("Da".parse::<DemTarget>().is_err());
    assert!("".parse::<DemTarget>().is_err());
}

#[test]
fn dem_parse_print_round_trip_includes_repeats_shifts_coordinates_and_tags() {
    let text = "error(0.125) D0\nrepeat[test\\Ctag] 100 {\n    error(0.25) D0 D1 L0 ^ D2\n    shift_detectors(1.5, 3) 10\n    detector(0.5) D0\n    logical_observable L0\n}\n";

    let dem = DetectorErrorModel::from_dem_str(text).unwrap();

    assert_eq!(dem.to_dem_string(), text);
    assert_eq!(
        DetectorErrorModel::from_dem_str(&dem.to_dem_string()).unwrap(),
        dem
    );
}

#[test]
fn dem_print_uses_stim_high_precision_float_style() {
    let mut dem = DetectorErrorModel::new();
    dem.push_instruction(
        DemInstruction::error(
            Probability::try_new(0.25 * 2.0 / 3.0).unwrap(),
            vec![DemTarget::relative_detector(0).unwrap()],
            None,
        )
        .unwrap(),
    );

    assert_eq!(
        dem.to_dem_string(),
        "error(0.1666666666666666574148081281236955) D0\n"
    );
}

#[test]
fn dem_rejects_invalid_probabilities_and_separators() {
    assert!(DetectorErrorModel::from_dem_str("error(1.5) D0\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("error(0.25) ^ D0\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("error(0.25) D0 ^\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("error(0.25) D0 ^ ^ D1\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("detector L0\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("logical_observable D0\n").is_err());
    assert!(DetectorErrorModel::from_dem_str("shift_detectors D0\n").is_err());
}

#[test]
fn dem_counts_detector_shift_detectors_and_observables_through_repeats() {
    let dem = DetectorErrorModel::from_dem_str(
        "shift_detectors 50\nrepeat 3 {\n    detector D0\n    error(0.1) D0 D2 L6\n    shift_detectors 4\n}\nlogical_observable L5\n",
    )
    .unwrap();

    assert_eq!(dem.total_detector_shift().unwrap(), 62);
    assert_eq!(dem.count_detectors().unwrap(), 61);
    assert_eq!(dem.count_observables().unwrap(), 7);
}

#[test]
fn dem_analyzer_outputs_detector_declaration_for_deterministic_detector() {
    let circuit = Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector D0\n");
}

#[test]
fn dem_analyzer_maps_simple_pauli_noise_to_detector_and_observable() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.25) 0\nX_ERROR(0.125) 1\nM 0 1\nOBSERVABLE_INCLUDE(3) rec[-1]\nDETECTOR rec[-2]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0\nerror(0.125) L3\n");
}

#[test]
fn dem_analyzer_maps_measurement_flip_probability_to_error() {
    let circuit =
        Circuit::from_stim_str("M(0.25) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0\nerror(0.25) D1\n");
}

#[test]
fn dem_analyzer_resets_clear_pending_single_qubit_errors() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.25) 0\n\
         Z_ERROR(0.25) 1\n\
         Y_ERROR(0.25) 2\n\
         R 0\n\
         RX 1\n\
         RY 2\n\
         M 0\n\
         MX 1\n\
         MY 2\n\
         DETECTOR rec[-3]\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector D0\ndetector D1\ndetector D2\n");
}

#[test]
fn dem_analyzer_ignores_identity_noise_channels() {
    let circuit = Circuit::from_stim_str(
        "I_ERROR(0.25) 0\n\
         II_ERROR(0.25) 0 1\n\
         M 0 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector D0\ndetector D1\n");
}

#[test]
fn dem_analyzer_maps_correlated_error_to_joint_detector_error() {
    let circuit = Circuit::from_stim_str(
        "CORRELATED_ERROR(0.25) X0 X1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0 D1\n");
}

#[test]
fn dem_analyzer_maps_else_correlated_error_block() {
    let circuit = Circuit::from_stim_str(
        "CORRELATED_ERROR(0.25) X0\n\
         ELSE_CORRELATED_ERROR(0.25) X1\n\
         ELSE_CORRELATED_ERROR(0.25) X2\n\
         M 0 1 2\n\
         DETECTOR rec[-3]\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.25) D0\nerror(0.1875) D1\nerror(0.140625) D2\n"
    );
}

#[test]
fn dem_analyzer_rejects_else_correlated_error_without_active_block() {
    let missing_option =
        Circuit::from_stim_str("CORRELATED_ERROR(0.25) X0\nELSE_CORRELATED_ERROR(0.25) X1\n")
            .unwrap();
    let dangling = Circuit::from_stim_str("ELSE_CORRELATED_ERROR(0.25) X1\n").unwrap();
    let separated =
        Circuit::from_stim_str("CORRELATED_ERROR(0.25) X0\nH 1\nELSE_CORRELATED_ERROR(0.25) X1\n")
            .unwrap();

    assert!(
        circuit_to_detector_error_model(&missing_option, ErrorAnalyzerOptions::default()).is_err()
    );
    assert!(circuit_to_detector_error_model(&dangling, approximate_options(),).is_err());
    assert!(circuit_to_detector_error_model(&separated, approximate_options(),).is_err());
}

#[test]
fn dem_analyzer_merges_identical_error_symptoms() {
    let circuit =
        Circuit::from_stim_str("X_ERROR(0.125) 0\nX_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n")
            .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.3125) D0\n");
}

#[test]
fn dem_analyzer_merges_measurement_flip_with_prior_matching_error() {
    let circuit =
        Circuit::from_stim_str("X_ERROR(0.125) 0\nM(0.25) 0\nDETECTOR rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.3125) D0\n");
}

#[test]
fn dem_analyzer_declares_detector_when_certain_duplicate_errors_cancel() {
    let circuit =
        Circuit::from_stim_str("X_ERROR(1) 0\nX_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector D0\n");
}

#[test]
fn dem_analyzer_approximates_disjoint_pauli_channel1_when_enabled() {
    let circuit = Circuit::from_stim_str(
        "R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.375) D0\n");
}

#[test]
fn dem_analyzer_approximates_pauli_channel1_by_measurement_basis() {
    let circuit = Circuit::from_stim_str(
        "R 0 1 2\n\
         PAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\n\
         PAULI_CHANNEL_1(0.125, 0.25, 0.375) 1\n\
         PAULI_CHANNEL_1(0.125, 0.25, 0.375) 2\n\
         MX 0\n\
         MY 1\n\
         M 2\n\
         DETECTOR rec[-3]\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.625) D0\nerror(0.5) D1\nerror(0.375) D2\n");
}

#[test]
fn dem_analyzer_reset_clears_pending_pauli_channel1() {
    let circuit = Circuit::from_stim_str(
        "PAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nR 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector D0\n");
}

#[test]
fn dem_analyzer_rejects_disjoint_pauli_channel1_without_approximation() {
    let circuit = Circuit::from_stim_str(
        "R 0\nPAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n",
    )
    .unwrap();

    let result = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default());

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_rejects_disjoint_pauli_channel1_above_threshold() {
    let circuit =
        Circuit::from_stim_str("R 0\nPAULI_CHANNEL_1(0, 0.25, 0.375) 0\nM 0\nDETECTOR rec[-1]\n")
            .unwrap();

    let result = circuit_to_detector_error_model(&circuit, threshold_options(0.3));

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_rejects_else_correlated_error_above_threshold() {
    let circuit = Circuit::from_stim_str(
        "CORRELATED_ERROR(0.25) X0\n\
         ELSE_CORRELATED_ERROR(0.75) X1\n\
         M 0 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let result = circuit_to_detector_error_model(&circuit, threshold_options(0.5));

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_maps_depolarize1_to_basis_flip_probability() {
    let circuit = Circuit::from_stim_str(
        "DEPOLARIZE1(0.25) 0 1\n\
         M 0\n\
         MX 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.1666666666666666574148081281236955) D0\nerror(0.1666666666666666574148081281236955) D1\n"
    );
}

#[test]
fn dem_analyzer_rejects_overmixing_depolarize1() {
    let circuit = Circuit::from_stim_str("DEPOLARIZE1(1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let result = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default());

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_preserves_shifted_detector_coordinates() {
    let circuit = Circuit::from_stim_str("SHIFT_COORDS(2, 3)\nM 0\nDETECTOR(5) rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "detector(7) D0\n");
}

#[test]
fn dem_analyzer_fold_loops_preserves_repeat_detector_shift() {
    let circuit = Circuit::from_stim_str("REPEAT 2 {\n    M 0\n    DETECTOR rec[-1]\n}\n").unwrap();
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .unwrap()
    .to_dem_string();

    assert_eq!(
        dem,
        "repeat 2 {\n    detector D0\n    shift_detectors 1\n}\n"
    );
}

#[test]
fn dem_analyzer_fold_loops_preserves_repeat_noise_errors() {
    let circuit = Circuit::from_stim_str(
        "REPEAT 1000 {\n    R 0\n    X_ERROR(0.25) 0\n    M 0\n    DETECTOR rec[-1]\n}\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(
        &circuit,
        ErrorAnalyzerOptions {
            fold_loops: true,
            ..ErrorAnalyzerOptions::default()
        },
    )
    .unwrap()
    .to_dem_string();

    assert_eq!(
        dem,
        "repeat 1000 {\n    error(0.25) D0\n    shift_detectors 1\n}\n"
    );
}
