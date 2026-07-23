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

fn decompose_options() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        decompose_errors: true,
        ..ErrorAnalyzerOptions::default()
    }
}

fn blocked_decompose_options() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        decompose_errors: true,
        block_decomposition_from_introducing_remnant_edges: true,
        ..ErrorAnalyzerOptions::default()
    }
}

fn ignored_blocked_decompose_options() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        decompose_errors: true,
        block_decomposition_from_introducing_remnant_edges: true,
        ignore_decomposition_failures: true,
        ..ErrorAnalyzerOptions::default()
    }
}

fn allow_gauge_options() -> ErrorAnalyzerOptions {
    ErrorAnalyzerOptions {
        allow_gauge_detectors: true,
        ..ErrorAnalyzerOptions::default()
    }
}

fn first_dem_instruction(dem: &DetectorErrorModel) -> &DemInstruction {
    dem.items()
        .first()
        .and_then(|item| match item {
            DemItem::Instruction(instruction) => Some(instruction),
            DemItem::RepeatBlock(_) => None,
        })
        .unwrap()
}

#[test]
fn dem_instruction_targets_parse_stim_limits() {
    assert_eq!(
        "D1152921504606846975".parse::<DemTarget>().unwrap(),
        DemTarget::relative_detector(1_152_921_504_606_846_975).unwrap()
    );
    assert!(DemTarget::relative_detector(4_611_686_018_427_387_903).is_ok());
    assert_eq!(
        "L4294967295".parse::<DemTarget>().unwrap(),
        DemTarget::logical_observable(4_294_967_295).unwrap()
    );
    assert_eq!("^".parse::<DemTarget>().unwrap(), DemTarget::separator());
    assert!("10".parse::<DemTarget>().is_err());
    assert_eq!(
        DetectorErrorModel::from_dem_str("shift_detectors 10\n")
            .unwrap()
            .to_dem_string(),
        "shift_detectors 10\n"
    );

    assert!("D1152921504606846976".parse::<DemTarget>().is_err());
    assert!("L4294967296".parse::<DemTarget>().is_err());
    assert!("D-1".parse::<DemTarget>().is_err());
    assert!("Da".parse::<DemTarget>().is_err());
    assert!("".parse::<DemTarget>().is_err());
}

#[test]
fn dem_instruction_target_groups_follow_stim_separators() {
    let dem = DetectorErrorModel::from_dem_str("error(0.1) D0 ^ D2 L0 ^ D1 D2 D3\n").unwrap();
    let instruction = first_dem_instruction(&dem);

    let groups: Vec<Vec<DemTarget>> = instruction
        .target_groups()
        .into_iter()
        .map(<[DemTarget]>::to_vec)
        .collect();

    assert_eq!(
        groups,
        vec![
            vec![DemTarget::relative_detector(0).unwrap()],
            vec![
                DemTarget::relative_detector(2).unwrap(),
                DemTarget::logical_observable(0).unwrap(),
            ],
            vec![
                DemTarget::relative_detector(1).unwrap(),
                DemTarget::relative_detector(2).unwrap(),
                DemTarget::relative_detector(3).unwrap(),
            ],
        ]
    );

    let dem = DetectorErrorModel::from_dem_str("error(0.1) D0\n").unwrap();
    let instruction = first_dem_instruction(&dem);
    assert_eq!(
        instruction.target_groups(),
        vec![&[DemTarget::relative_detector(0).unwrap()][..]]
    );

    let dem = DetectorErrorModel::from_dem_str("error(0.1)\n").unwrap();
    let instruction = first_dem_instruction(&dem);
    assert_eq!(instruction.target_groups(), vec![&[][..]]);
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
fn dem_parser_rejects_hostile_line_count_and_repeat_depth() {
    let too_many_lines = "\n".repeat(1_000_001);
    let error =
        DetectorErrorModel::from_dem_str(&too_many_lines).expect_err("reject too many lines");
    assert!(
        error.to_string().contains("more than 1000000 lines"),
        "{error}"
    );

    let mut too_deep = String::new();
    for _ in 0..257 {
        too_deep.push_str("repeat 1 {\n");
    }
    too_deep.push_str("error(1) D0\n");
    for _ in 0..257 {
        too_deep.push_str("}\n");
    }
    let error = DetectorErrorModel::from_dem_str(&too_deep).expect_err("reject deep repeats");
    assert!(
        error.to_string().contains("repeat nesting exceeds"),
        "{error}"
    );
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
fn dem_counts_large_repeat_detectors_without_unrolling() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 1000001 {\n    detector D0\n    shift_detectors 1\n}\n",
    )
    .unwrap();

    assert_eq!(dem.count_detectors().unwrap(), 1_000_001);

    let shift_only =
        DetectorErrorModel::from_dem_str("repeat 1000001 {\n    shift_detectors 1\n}\n").unwrap();
    assert_eq!(shift_only.count_detectors().unwrap(), 0);
}

#[test]
fn dem_public_flattening_apis_reject_excessive_repeat_expansion() {
    let dem = DetectorErrorModel::from_dem_str(
        "repeat 100001 {\n    error(0.1) D0\n    shift_detectors 1\n}\nerror(0.1) D0 L0\n",
    )
    .unwrap();

    let graphlike_error = shortest_graphlike_undetectable_logical_error(&dem, true)
        .expect_err("graphlike search should reject hostile repeat expansion")
        .to_string();
    assert!(
        graphlike_error
            .contains("DEM graphlike search currently supports repeat counts up to 100000"),
        "{graphlike_error}"
    );

    let hyper_error = find_undetectable_logical_error(&dem, usize::MAX, usize::MAX, false)
        .expect_err("hypergraph search should reject hostile repeat expansion")
        .to_string();
    assert!(
        hyper_error.contains("DEM hypergraph search currently supports repeat counts up to 100000"),
        "{hyper_error}"
    );
}

#[test]
fn dem_shortest_graphlike_logical_error_api_matches_direct_dem() {
    let dem = DetectorErrorModel::from_dem_str("error(0.1) D0\nerror(0.1) D0 L0\n").unwrap();

    let shortest = shortest_graphlike_undetectable_logical_error(&dem, false).unwrap();

    assert_eq!(shortest.to_dem_string(), "error(1) D0\nerror(1) D0 L0\n");
}

#[test]
fn dem_error_decomposition_api_returns_typed_probability_triples() {
    let disjoint = independent_to_disjoint_xyz_errors(
        Probability::try_new(0.1).unwrap(),
        Probability::try_new(0.2).unwrap(),
        Probability::try_new(0.3).unwrap(),
    )
    .unwrap();
    assert!((disjoint.x().get() - 0.11).abs() <= 1e-12);
    assert!((disjoint.y().get() - 0.15).abs() <= 1e-12);
    assert!((disjoint.z().get() - 0.23).abs() <= 1e-12);

    let independent = try_disjoint_to_independent_xyz_errors(
        Probability::try_new(0.09).unwrap(),
        Probability::try_new(0.09).unwrap(),
        Probability::try_new(0.01).unwrap(),
    )
    .unwrap()
    .unwrap();
    assert!((independent.x().get() - 0.1).abs() <= 1e-6);
    assert!((independent.y().get() - 0.1).abs() <= 1e-6);
    assert!(independent.z().get().abs() <= 1e-6);
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
fn dem_analyzer_rejects_gauge_detectors_by_default() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
         H 0\n\
         CNOT 0 1\n\
         M 0 1\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-2]\n",
    )
    .unwrap();

    let error = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap_err()
        .to_string();

    assert!(error.contains("non-deterministic detectors"));
    assert!(error.contains("D0"));
    assert!(error.contains("D1"));
}

#[test]
fn dem_analyzer_allows_gauge_detectors_as_half_probability_errors() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
         H 0\n\
         CNOT 0 1\n\
         M 0 1\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-2]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, allow_gauge_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.5) D0 D1\n");
}

#[test]
fn dem_analyzer_allows_gauge_detectors_through_hxy_basis_change() {
    let circuit = Circuit::from_stim_str(
        "RY 0\n\
         H_XY 0\n\
         CNOT 0 1\n\
         M 0 1\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-2]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, allow_gauge_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.5) D0 D1\n");
}

#[test]
fn circuit_to_dem_heralded_noise_basis_matches_upstream() {
    let cases = [
        (
            "HERALDED_PAULI_CHANNEL_1(0.25, 0, 0, 0) 0",
            "error(0.25) D0\n\
             detector(2) D0\n\
             detector(3) D1\n\
             detector(5) D2\n",
        ),
        (
            "HERALDED_PAULI_CHANNEL_1(0, 0.25, 0, 0) 0",
            "error(0.25) D0 D2\n\
             detector(2) D0\n\
             detector(3) D1\n\
             detector(5) D2\n",
        ),
        (
            "HERALDED_PAULI_CHANNEL_1(0, 0, 0.25, 0) 0",
            "error(0.25) D0 D1 D2\n\
             detector(2) D0\n\
             detector(3) D1\n\
             detector(5) D2\n",
        ),
        (
            "HERALDED_PAULI_CHANNEL_1(0, 0, 0, 0.25) 0",
            "error(0.25) D0 D1\n\
             detector(2) D0\n\
             detector(3) D1\n\
             detector(5) D2\n",
        ),
    ];

    for (noise_instruction, expected) in cases {
        let circuit = Circuit::from_stim_str(&heralded_basis_circuit(noise_instruction)).unwrap();
        let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
            .unwrap()
            .to_dem_string();
        assert_eq!(dem, expected);
    }

    let mixed = Circuit::from_stim_str(&heralded_basis_circuit(
        "HERALDED_PAULI_CHANNEL_1(0.125, 0, 0.25, 0) 0",
    ))
    .unwrap();

    let dem = circuit_to_detector_error_model(&mixed, approximate_options())
        .unwrap()
        .to_dem_string();
    assert_eq!(
        dem,
        "error(0.125) D0\n\
         error(0.25) D0 D1 D2\n\
         detector(2) D0\n\
         detector(3) D1\n\
         detector(5) D2\n"
    );

    let result = circuit_to_detector_error_model(&mixed, ErrorAnalyzerOptions::default());
    assert!(result.is_err());
}

#[test]
fn circuit_to_dem_heralded_erase_matches_upstream() {
    let allowed = Circuit::from_stim_str("HERALDED_ERASE(0.25) 0\n").unwrap();
    assert!(
        circuit_to_detector_error_model(&allowed, threshold_options(0.3)).is_ok(),
        "HERALDED_ERASE is accepted when the erasure probability is under the approximation threshold"
    );
    assert!(
        circuit_to_detector_error_model(&allowed, threshold_options(0.2)).is_err(),
        "HERALDED_ERASE rejects erasure probabilities above the approximation threshold"
    );

    let circuit = Circuit::from_stim_str(
        "MZZ 0 1\n\
         MXX 0 1\n\
         HERALDED_ERASE(0.25) 0\n\
         MZZ 0 1\n\
         MXX 0 1\n\
         DETECTOR rec[-1] rec[-4]\n\
         DETECTOR rec[-2] rec[-5]\n\
         DETECTOR rec[-3]\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();
    assert_eq!(
        dem,
        "error(0.0625) D0 D1 D2\n\
         error(0.0625) D0 D2\n\
         error(0.0625) D1 D2\n\
         error(0.0625) D2\n"
    );

    let repeated_targets = Circuit::from_stim_str(
        "M 0\n\
         HERALDED_ERASE(0.25) 9 0 9 9 9\n\
         M 0\n\
         DETECTOR rec[-1] rec[-7]\n\
         DETECTOR rec[-5]\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&repeated_targets, approximate_options())
        .unwrap()
        .to_dem_string();
    assert_eq!(dem, "error(0.125) D0 D1\nerror(0.125) D1\n");
}

fn heralded_basis_circuit(noise_instruction: &str) -> String {
    format!(
        "MXX 0 1\n\
         MZZ 0 1\n\
         {noise_instruction}\n\
         MXX 0 1\n\
         MZZ 0 1\n\
         DETECTOR(2) rec[-3]\n\
         DETECTOR(3) rec[-2] rec[-5]\n\
         DETECTOR(5) rec[-1] rec[-4]\n"
    )
}

#[test]
fn dem_analyzer_allow_gauge_detectors_still_rejects_gauge_observables() {
    let circuit = Circuit::from_stim_str("R 0\nH 0\nM 0\nOBSERVABLE_INCLUDE(0) rec[-1]\n").unwrap();

    let error = circuit_to_detector_error_model(&circuit, allow_gauge_options())
        .unwrap_err()
        .to_string();

    assert!(error.contains("non-deterministic observables"));
    assert!(error.contains("L0"));
}

#[test]
fn dem_analyzer_obs_include_pauli_targets_match_upstream() {
    let circuit = Circuit::from_stim_str(
        "OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n\
         X_ERROR(0.125) 0\n\
         Y_ERROR(0.25) 0\n\
         Z_ERROR(0.375) 0\n\
         OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();
    assert_eq!(
        dem,
        "error(0.375) L0 L1\n\
         error(0.25) L0 L2\n\
         error(0.125) L1 L2\n"
    );

    let commuting = Circuit::from_stim_str(
        "OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n\
         X_ERROR(0.125) 0\n\
         OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&commuting, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();
    assert_eq!(
        dem,
        "error(0.125) L1 L2\n\
         logical_observable L0\n\
         logical_observable L0\n"
    );

    let depolarizing_outside_boundary = Circuit::from_stim_str(
        "DEPOLARIZE1(0.125) 0\n\
         OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n\
         X_ERROR(0.25) 0\n\
         OBSERVABLE_INCLUDE(0) X0\n\
         OBSERVABLE_INCLUDE(1) Y0\n\
         OBSERVABLE_INCLUDE(2) Z0\n\
         DEPOLARIZE1(0.125) 0\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(
        &depolarizing_outside_boundary,
        ErrorAnalyzerOptions::default(),
    )
    .unwrap()
    .to_dem_string();
    assert_eq!(
        dem,
        "error(0.25) L1 L2\n\
         logical_observable L0\n\
         logical_observable L0\n"
    );

    let multi_target = Circuit::from_stim_str(
        "OBSERVABLE_INCLUDE(0) X0 Y1\n\
         Z_ERROR(0.125) 0\n\
         OBSERVABLE_INCLUDE(0) X0 Y1\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&multi_target, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();
    assert_eq!(dem, "error(0.125) L0\n");

    let multi_target_commuting = Circuit::from_stim_str(
        "OBSERVABLE_INCLUDE(0) X0 Y1\n\
         X_ERROR(0.125) 0\n\
         OBSERVABLE_INCLUDE(0) X0 Y1\n",
    )
    .unwrap();
    let dem =
        circuit_to_detector_error_model(&multi_target_commuting, ErrorAnalyzerOptions::default())
            .unwrap()
            .to_dem_string();
    assert_eq!(dem, "logical_observable L0\nlogical_observable L0\n");

    let propagated = Circuit::from_stim_str(
        "OBSERVABLE_INCLUDE(0) X0\n\
         H 0\n\
         Z_ERROR(0.125) 0\n\
         H 0\n\
         OBSERVABLE_INCLUDE(0) X0\n",
    )
    .unwrap();
    let dem = circuit_to_detector_error_model(&propagated, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();
    assert_eq!(dem, "logical_observable L0\nlogical_observable L0\n");
}

#[test]
fn dem_analyzer_rejects_unpaired_obs_include_pauli_targets() {
    let circuit = Circuit::from_stim_str("OBSERVABLE_INCLUDE(0) X0\n").unwrap();

    let error = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap_err()
        .to_string();

    assert!(error.contains("non-deterministic observables"));
    assert!(error.contains("L0"));
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
fn dem_analyzer_propagates_pauli_errors_through_h() {
    let circuit =
        Circuit::from_stim_str("RX 0\nZ_ERROR(0.25) 0\nH 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0\n");
}

#[test]
fn dem_analyzer_propagates_pauli_errors_through_cnot_order() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.25) 0\n\
         CNOT 0 1\n\
         CNOT 1 0\n\
         M 0 1\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D1\ndetector D0\n");
}

#[test]
fn dem_analyzer_propagates_pauli_errors_through_hxy() {
    let circuit = Circuit::from_stim_str(
        "RY 0\n\
         X_ERROR(0.25) 0\n\
         H_XY 0\n\
         MX 0\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.25) D0\n");
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
fn dem_analyzer_decomposes_errors_using_known_graphlike_components() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.125) 0\n\
         MR 0\n\
         X_ERROR(0.25) 0\n\
         MR 0\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1] rec[-2]\n\
         DETECTOR rec[-1] rec[-2]\n\
         OBSERVABLE_INCLUDE(5) rec[-2]\n\
         OBSERVABLE_INCLUDE(6) rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, decompose_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.25) D2 D3 L6\nerror(0.125) D2 D3 L6 ^ D0 D1 L5 L6\n"
    );
}

#[test]
fn dem_analyzer_decomposes_errors_without_remnants_when_all_components_are_known() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.125) 0\n\
         X_ERROR(0.25) 1\n\
         X_ERROR(0.375) 2\n\
         M 0 1 2\n\
         DETECTOR rec[-3] rec[-1]\n\
         DETECTOR rec[-2] rec[-1]\n\
         DETECTOR rec[-3] rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, blocked_decompose_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.125) D0 D2\nerror(0.375) D0 D2 ^ D1\nerror(0.25) D1\n"
    );
}

#[test]
fn dem_analyzer_blocks_remnant_edges_when_requested() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.125) 0\n\
         X_ERROR(0.375) 2\n\
         M 0 1 2\n\
         DETECTOR rec[-3] rec[-1]\n\
         DETECTOR rec[-2] rec[-1]\n\
         DETECTOR rec[-3] rec[-1]\n",
    )
    .unwrap();

    let error = circuit_to_detector_error_model(&circuit, blocked_decompose_options())
        .unwrap_err()
        .to_string();

    assert!(error.contains("Failed to decompose errors into graphlike components"));
    assert!(error.contains("block_decomposition_from_introducing_remnant_edges"));
}

#[test]
fn dem_analyzer_can_ignore_blocked_decomposition_failures() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.125) 0\n\
         X_ERROR(0.375) 2\n\
         M 0 1 2\n\
         DETECTOR rec[-3] rec[-1]\n\
         DETECTOR rec[-2] rec[-1]\n\
         DETECTOR rec[-3] rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ignored_blocked_decompose_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.375) D0 D1 D2\nerror(0.125) D0 D2\n");
}

#[test]
fn dem_analyzer_rejects_undecomposable_detector_triples() {
    let circuit = Circuit::from_stim_str(
        "X_ERROR(0.001) 0\n\
         M 0\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let error = circuit_to_detector_error_model(&circuit, decompose_options())
        .unwrap_err()
        .to_string();

    assert!(error.contains("Failed to decompose errors into graphlike components"));
    assert!(error.contains("D0, D1, D2"));
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
        "RX 0\n\
         RY 1\n\
         R 2\n\
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
fn dem_analyzer_propagates_pauli_channel1_through_hxy() {
    let circuit = Circuit::from_stim_str(
        "RY 0\n\
         PAULI_CHANNEL_1(0.125, 0.25, 0.375) 0\n\
         H_XY 0\n\
         MX 0\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, approximate_options())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.5) D0\n");
}

#[test]
fn dem_analyzer_allows_exact_solved_pauli_channel1_without_approximation() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
         PAULI_CHANNEL_1(0.1, 0.2, 0.15) 0\n\
         M 0\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.2999999999999999888977697537484346) D0\n");
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
fn dem_analyzer_approximates_disjoint_pauli_channel2_when_enabled() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
         PAULI_CHANNEL_2(0.125, 0.25, 0.375, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 1 0\n\
         M 0\n\
         DETECTOR rec[-1]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, threshold_options(0.38))
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "error(0.375) D0\n");
}

#[test]
fn dem_analyzer_rejects_disjoint_pauli_channel2_without_approximation() {
    let circuit = Circuit::from_stim_str(
        "PAULI_CHANNEL_2(0.125, 0.25, 0.375, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 1 0\n",
    )
    .unwrap();

    let result = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default());

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_rejects_disjoint_pauli_channel2_above_threshold() {
    let circuit = Circuit::from_stim_str(
        "PAULI_CHANNEL_2(0.125, 0.25, 0.375, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 1 0\n",
    )
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
        "R 0\n\
         RX 1\n\
         DEPOLARIZE1(0.25) 0 1\n\
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
fn dem_analyzer_maps_depolarize2_to_pair_flip_probabilities() {
    let circuit = Circuit::from_stim_str(
        "DEPOLARIZE2(0.25) 3 5\n\
         M 3\n\
         M 5\n\
         DETECTOR rec[-1]\n\
         DETECTOR rec[-2]\n",
    )
    .unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(
        dem,
        "error(0.07182558071116236508846242259096471) D0\n\
         error(0.07182558071116236508846242259096471) D0 D1\n\
         error(0.07182558071116236508846242259096471) D1\n"
    );
}

#[test]
fn dem_analyzer_rejects_overmixing_depolarize1() {
    let circuit = Circuit::from_stim_str("DEPOLARIZE1(1) 0\nM 0\nDETECTOR rec[-1]\n").unwrap();

    let result = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default());

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_rejects_overmixing_depolarize2() {
    let circuit =
        Circuit::from_stim_str("DEPOLARIZE2(1) 0 1\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n")
            .unwrap();

    let result = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default());

    assert!(result.is_err());
}

#[test]
fn dem_analyzer_preserves_shifted_detector_coordinates() {
    let circuit = Circuit::from_stim_str("SHIFT_COORDS(2, 3)\nM 0\nDETECTOR(5) rec[-1]\n").unwrap();

    let dem = circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
        .unwrap()
        .to_dem_string();

    assert_eq!(dem, "shift_detectors(2, 3) 0\ndetector(5) D0\n");
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

    assert_eq!(dem, "detector D0\ndetector D1\n");
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
