#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "parity tests use fixed detector and tick ids for compact expected maps"
)]

use super::*;
mod generated;

fn detector(id: u64) -> DemDetectorId {
    DemDetectorId::try_new(id).unwrap()
}

fn regions(text: &str, detectors: Vec<DemDetectorId>, ticks: Vec<u64>) -> DetectingRegionMap {
    let circuit = Circuit::from_stim_str(text).unwrap();
    circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors,
            ticks,
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap()
}

#[test]
fn detecting_regions_simple_h_cx_mxx() {
    let actual = regions(
        "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
        vec![detector(0)],
        vec![0, 1],
    );

    assert_eq!(actual[&detector(0)][&0].to_string(), "+X_");
    assert_eq!(actual[&detector(0)][&1].to_string(), "+XX");
}

#[test]
fn detecting_regions_target_api_matches_mx_python_example() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
             TICK\n\
             H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MX 0 1\n\
             DETECTOR rec[-1] rec[-2]\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: all_detecting_region_targets(&circuit).unwrap(),
            ticks: all_detecting_region_ticks(&circuit).unwrap(),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    let detector = DemTarget::relative_detector(0).unwrap();

    assert_eq!(actual[&detector][&0].to_string(), "+Z_");
    assert_eq!(actual[&detector][&1].to_string(), "+X_");
    assert_eq!(actual[&detector][&2].to_string(), "+XX");
}

#[test]
fn detecting_regions_target_api_supports_mzz_example() {
    let circuit = Circuit::from_stim_str(
        "TICK\n\
             MZZ 0 1 1 2\n\
             TICK\n\
             M 2\n\
             DETECTOR rec[-1]\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![DemTarget::relative_detector(0).unwrap()],
            ticks: all_detecting_region_ticks(&circuit).unwrap(),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    let detector = DemTarget::relative_detector(0).unwrap();

    assert_eq!(actual[&detector][&0].to_string(), "+__Z");
    assert_eq!(actual[&detector][&1].to_string(), "+__Z");
}

#[test]
fn detecting_regions_target_api_ignores_tags_and_ordinary_noise_like_upstream() {
    let circuit = Circuit::from_stim_str(
        "R[test1] 0\n\
             X_ERROR[test2](0.25) 0\n\
             TICK\n\
             M[test3](0.25) 0\n\
             DETECTOR[test4](1, 2) rec[-1]\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: all_detecting_region_targets(&circuit).unwrap(),
            ticks: all_detecting_region_ticks(&circuit).unwrap(),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    let detector = DemTarget::relative_detector(0).unwrap();

    assert_eq!(actual[&detector][&0].to_string(), "+Z");
}

#[test]
fn detecting_regions_target_shape_ignores_non_record_noise_instructions() {
    let actual = regions(
        "R 0 1\n\
             X_ERROR(0.125) 0\n\
             Y_ERROR(0.125) 1\n\
             Z_ERROR(0.125) 0\n\
             I_ERROR(0.125) 1\n\
             II_ERROR(0.125) 0 1\n\
             DEPOLARIZE1(0.125) 0 1\n\
             DEPOLARIZE2(0.125) 0 1\n\
             PAULI_CHANNEL_1(0.01, 0.02, 0.03) 0\n\
             PAULI_CHANNEL_2(0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01) 0 1\n\
             E(0.125) X0 Y1\n\
             ELSE_CORRELATED_ERROR(0.125) Z0\n\
             TICK\n\
             MZZ 0 1\n\
             DETECTOR rec[-1]\n",
        vec![detector(0)],
        vec![0],
    );

    assert_eq!(actual[&detector(0)][&0].to_string(), "+ZZ");
}

#[test]
fn detecting_regions_target_shape_supports_inverted_measurement_targets() {
    let single_cases = [
        ("R 0\nTICK\nM !0\nDETECTOR rec[-1]\n", "+Z"),
        ("RX 0\nTICK\nMX !0\nDETECTOR rec[-1]\n", "+X"),
        ("RY 0\nTICK\nMY !0\nDETECTOR rec[-1]\n", "+Y"),
        ("R 0\nTICK\nMR !0\nDETECTOR rec[-1]\n", "+Z"),
        ("RX 0\nTICK\nMRX !0\nDETECTOR rec[-1]\n", "+X"),
        ("RY 0\nTICK\nMRY !0\nDETECTOR rec[-1]\n", "+Y"),
    ];
    for (text, expected) in single_cases {
        let actual = regions(text, vec![detector(0)], vec![0]);
        assert_eq!(actual[&detector(0)][&0].to_string(), expected, "{text}");
    }

    let pair_cases = [
        ("RX 0 1\nTICK\nMXX !0 1\nDETECTOR rec[-1]\n", "+XX"),
        ("RY 0 1\nTICK\nMYY !0 !1\nDETECTOR rec[-1]\n", "+YY"),
        ("R 0 1\nTICK\nMZZ 0 !1\nDETECTOR rec[-1]\n", "+ZZ"),
    ];
    for (text, expected) in pair_cases {
        let actual = regions(text, vec![detector(0)], vec![0]);
        assert_eq!(actual[&detector(0)][&0].to_string(), expected, "{text}");
    }
}

#[test]
fn detecting_regions_target_shape_supports_pauli_product_measurements() {
    let circuit = Circuit::from_stim_str(
        "RX 0\n\
         RY 1\n\
         R 2 3\n\
         TICK\n\
         MPP !X0*Y1*Z2 Z3\n\
         DETECTOR rec[-2]\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-2] rec[-1]\n",
    )
    .unwrap();
    let detector_0 = DemTarget::relative_detector(0).unwrap();
    let detector_1 = DemTarget::relative_detector(1).unwrap();
    let observable = DemTarget::logical_observable(0).unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![detector_0, detector_1, observable],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert_eq!(actual[&detector_0][&0].to_string(), "+XYZ_");
    assert_eq!(actual[&detector_1][&0].to_string(), "+___Z");
    assert_eq!(actual[&observable][&0].to_string(), "+XYZZ");
}

#[test]
fn detecting_regions_target_shape_supports_spp_unitary_products() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let circuit = Circuit::from_stim_str(&format!(
            "RY 0 1\n\
             R 2\n\
             TICK\n\
             {gate_name} X0*Y1*Z2 !X0*X1\n\
             TICK\n\
             OBSERVABLE_INCLUDE(0) Z0\n"
        ))
        .unwrap();
        let decomposed = circuit.decomposed().unwrap();
        let observable = DemTarget::logical_observable(0).unwrap();
        let options = DetectingRegionTargetOptions {
            targets: vec![observable],
            ticks: vec![0, 1],
            ignore_anticommutation_errors: true,
        };
        let actual = circuit_detecting_regions_for_targets(&circuit, options.clone()).unwrap();
        let expected = circuit_detecting_regions_for_targets(&decomposed, options).unwrap();

        assert_eq!(actual, expected, "{gate_name}");
        assert_eq!(actual[&observable][&0].to_string(), "+YX_", "{gate_name}");
        assert_eq!(actual[&observable][&1].to_string(), "+Z__", "{gate_name}");
    }
}

#[test]
fn detecting_regions_target_shape_rejects_anti_hermitian_pauli_products() {
    for text in [
        "TICK\nMPP X0*Z0\nDETECTOR rec[-1]\n",
        "TICK\nSPP X0*Z0\nTICK\nOBSERVABLE_INCLUDE(0) Z0\n",
        "TICK\nSPP_DAG X0*Z0\nTICK\nOBSERVABLE_INCLUDE(0) Z0\n",
    ] {
        let circuit = Circuit::from_stim_str(text).unwrap();
        let error = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: all_detecting_region_targets(&circuit).unwrap(),
                ticks: all_detecting_region_ticks(&circuit).unwrap(),
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("anti-Hermitian"), "{error}");
    }
}

#[test]
fn detecting_regions_target_shape_keeps_reset_and_unitaries_plain() {
    for text in [
        "R !0\nTICK\nM 0\nDETECTOR rec[-1]\n",
        "RX !0\nTICK\nMX 0\nDETECTOR rec[-1]\n",
        "RY !0\nTICK\nMY 0\nDETECTOR rec[-1]\n",
        "H !0\nTICK\nMX 0\nDETECTOR rec[-1]\n",
        "CX !0 1\nTICK\nMXX 0 1\nDETECTOR rec[-1]\n",
    ] {
        match Circuit::from_stim_str(text) {
            Ok(circuit) => {
                let error = circuit_detecting_regions(
                    &circuit,
                    DetectingRegionOptions {
                        detectors: vec![detector(0)],
                        ticks: vec![0],
                        ignore_anticommutation_errors: false,
                    },
                )
                .unwrap_err();

                assert!(
                    error.to_string().contains("plain qubit targets"),
                    "{text}: {error}"
                );
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("invalid target"),
                    "{text}: {error}"
                );
            }
        }
    }
}

#[test]
fn detecting_regions_target_api_supports_logical_observable_targets() {
    let circuit = Circuit::from_stim_str(
        "TICK\n\
             M 0\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n\
             TICK\n\
             H 1\n\
             OBSERVABLE_INCLUDE(1) X1\n",
    )
    .unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![
                DemTarget::logical_observable(0).unwrap(),
                DemTarget::logical_observable(1).unwrap(),
                DemTarget::logical_observable(1).unwrap(),
            ],
            ticks: vec![0, 1],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert_eq!(
        actual[&DemTarget::logical_observable(0).unwrap()][&0].to_string(),
        "+Z_"
    );
    assert_eq!(
        actual[&DemTarget::logical_observable(1).unwrap()][&1].to_string(),
        "+_Z"
    );
    assert_eq!(actual.len(), 2);
}

#[test]
fn detecting_regions_target_api_rejects_invalid_targets() {
    let circuit = Circuit::from_stim_str("TICK\nM 0\nDETECTOR rec[-1]\n").unwrap();
    for (target, message) in [
        (
            DemTarget::relative_detector(1).unwrap(),
            "requested detector D1",
        ),
        (
            DemTarget::logical_observable(0).unwrap(),
            "requested observable L0",
        ),
        (DemTarget::separator(), "only supports detector"),
        (DemTarget::numeric(5), "only supports detector"),
    ] {
        let error = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: vec![target],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains(message), "{target}: {error}");
    }
}

#[test]
fn detecting_regions_target_shape_supports_measurement_pads() {
    let circuit = Circuit::from_stim_str(
        "R 0\n\
             TICK\n\
             M 0\n\
             MPAD 1\n\
             DETECTOR rec[-2] rec[-1]\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .unwrap();
    assert_eq!(circuit.count_qubits(), 2);
    let detector_target = DemTarget::relative_detector(0).unwrap();
    let observable = DemTarget::logical_observable(0).unwrap();
    let actual = circuit_detecting_regions_for_targets(
        &circuit,
        DetectingRegionTargetOptions {
            targets: vec![detector_target, observable],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert_eq!(actual[&detector_target][&0].to_string(), "+Z");
    assert!(actual[&observable].is_empty());

    let all_pad_circuit =
        Circuit::from_stim_str("TICK\nMPAD(0.125) 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\n")
            .unwrap();
    let all_pad_regions = circuit_detecting_regions(
        &all_pad_circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0), detector(1)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert!(all_pad_regions[&detector(0)].is_empty());
    assert!(all_pad_regions[&detector(1)].is_empty());
}

#[test]
fn detecting_regions_target_shape_supports_heralded_record_noise() {
    for text in [
        "R 0\nTICK\nHERALDED_ERASE(0.125) 0\nM 0\nDETECTOR rec[-2] rec[-1]\n",
        "R 0\nTICK\nHERALDED_PAULI_CHANNEL_1(0.125, 0, 0, 0) 0\nM 0\nDETECTOR rec[-2] rec[-1]\n",
    ] {
        let actual = regions(text, vec![detector(0)], vec![0]);
        assert_eq!(actual[&detector(0)][&0].to_string(), "+Z", "{text}");
    }

    for text in [
        "R 0 1\nTICK\nHERALDED_ERASE(0.125) 0 1\nM 0 1\nDETECTOR rec[-4] rec[-1]\nDETECTOR rec[-3] rec[-2]\n",
        "R 0 1\nTICK\nHERALDED_PAULI_CHANNEL_1(0.125, 0, 0, 0) 0 1\nM 0 1\nDETECTOR rec[-4] rec[-1]\nDETECTOR rec[-3] rec[-2]\n",
    ] {
        let actual = regions(text, vec![detector(0), detector(1)], vec![0]);
        assert_eq!(actual[&detector(0)][&0].to_string(), "+_Z", "{text}");
        assert_eq!(actual[&detector(1)][&0].to_string(), "+Z_", "{text}");
    }

    for text in [
        "TICK\nHERALDED_ERASE(0.125) 0\nDETECTOR rec[-1]\n",
        "TICK\nHERALDED_PAULI_CHANNEL_1(0.125, 0, 0, 0) 0\nDETECTOR rec[-1]\n",
    ] {
        let circuit = Circuit::from_stim_str(text).unwrap();
        let actual = circuit_detecting_regions_for_targets(
            &circuit,
            DetectingRegionTargetOptions {
                targets: vec![DemTarget::relative_detector(0).unwrap()],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap();

        assert!(actual[&DemTarget::relative_detector(0).unwrap()].is_empty());
    }
}

#[test]
fn detecting_regions_target_shape_keeps_heralded_noise_plain_qubit_scoped() {
    for text in [
        "TICK\nHERALDED_ERASE(0.125) !0\nDETECTOR rec[-1]\n",
        "TICK\nHERALDED_PAULI_CHANNEL_1(0.125, 0, 0, 0) !0\nDETECTOR rec[-1]\n",
    ] {
        match Circuit::from_stim_str(text) {
            Ok(circuit) => {
                let error = circuit_detecting_regions_for_targets(
                    &circuit,
                    DetectingRegionTargetOptions {
                        targets: vec![DemTarget::relative_detector(0).unwrap()],
                        ticks: vec![0],
                        ignore_anticommutation_errors: false,
                    },
                )
                .unwrap_err();

                assert!(
                    error.to_string().contains("plain qubit targets"),
                    "{text}: {error}"
                );
            }
            Err(error) => {
                assert!(
                    error.to_string().contains("invalid target"),
                    "{text}: {error}"
                );
            }
        }
    }
}

#[test]
fn detecting_regions_target_api_rejects_dense_helper_expansion() {
    let high_observable = Circuit::from_stim_str("OBSERVABLE_INCLUDE(4294967296) Z0\n").unwrap();
    let error = all_detecting_region_targets(&high_observable).unwrap_err();
    assert!(error.to_string().contains("observable target"));

    let many_detectors =
        Circuit::from_stim_str("M 0\nREPEAT 1000001 {\n    DETECTOR rec[-1]\n}\n").unwrap();
    let error = all_detecting_region_targets(&many_detectors).unwrap_err();
    assert!(error.to_string().contains("materialized target"));
}

#[test]
fn detecting_regions_clifford_supports_promoted_single_qubit_gates() {
    let cases = [
        (
            "R 0\nTICK\nH 0\nS 0\nTICK\nMY 0\nDETECTOR rec[-1]\n",
            "+Z",
            "+Y",
        ),
        (
            "R 0\nTICK\nH 0\nS_DAG 0\nTICK\nMY 0\nDETECTOR rec[-1]\n",
            "+Z",
            "+Y",
        ),
        (
            "RX 0\nTICK\nH_XY 0\nTICK\nMY 0\nDETECTOR rec[-1]\n",
            "+X",
            "+Y",
        ),
        (
            "R 0\nTICK\nC_XYZ 0\nTICK\nMX 0\nDETECTOR rec[-1]\n",
            "+Z",
            "+X",
        ),
    ];
    for (text, tick0, tick1) in cases {
        let actual = regions(text, vec![detector(0)], vec![0, 1]);
        assert_eq!(actual[&detector(0)][&0].to_string(), tick0, "{text}");
        assert_eq!(actual[&detector(0)][&1].to_string(), tick1, "{text}");
    }
}

#[test]
fn detecting_regions_clifford_supports_single_qubit_clifford_gate_set() {
    let cases = [
        ("I", "RX", "+X"),
        ("X", "RX", "+X"),
        ("Y", "RX", "+X"),
        ("Z", "RX", "+X"),
        ("H", "R", "+Z"),
        ("SQRT_Y_DAG", "R", "+Z"),
        ("H_NXZ", "R", "+Z"),
        ("SQRT_Y", "R", "+Z"),
        ("S", "RY", "+Y"),
        ("H_XY", "RY", "+Y"),
        ("H_NXY", "RY", "+Y"),
        ("S_DAG", "RY", "+Y"),
        ("SQRT_X_DAG", "RX", "+X"),
        ("SQRT_X", "RX", "+X"),
        ("H_NYZ", "RX", "+X"),
        ("H_YZ", "RX", "+X"),
        ("C_XYZ", "R", "+Z"),
        ("C_XYNZ", "R", "+Z"),
        ("C_NXYZ", "R", "+Z"),
        ("C_XNYZ", "R", "+Z"),
        ("C_ZYX", "RY", "+Y"),
        ("C_ZNYX", "RY", "+Y"),
        ("C_NZYX", "RY", "+Y"),
        ("C_ZYNX", "RY", "+Y"),
    ];
    for (gate, reset, tick0) in cases {
        let text = format!("{reset} 0\nTICK\n{gate} 0\nTICK\nMX 0\nDETECTOR rec[-1]\n");
        let actual = regions(&text, vec![detector(0)], vec![0, 1]);
        assert_eq!(actual[&detector(0)][&0].to_string(), tick0, "{gate}");
        assert_eq!(actual[&detector(0)][&1].to_string(), "+X", "{gate}");
    }
}

#[test]
fn detecting_regions_clifford_supports_controlled_pauli_propagation() {
    let cases = [
        (
            "R 0 1\n\
                 TICK\n\
                 H 0\n\
                 CZ 0 1\n\
                 TICK\n\
                 MX 0\n\
                 DETECTOR rec[-1]\n",
            "+ZZ",
            "+X_",
        ),
        (
            "RX 0\n\
                 RY 1\n\
                 TICK\n\
                 CY 0 1\n\
                 TICK\n\
                 MX 0\n\
                 DETECTOR rec[-1]\n",
            "+XY",
            "+X_",
        ),
    ];
    for (text, tick0, tick1) in cases {
        let actual = regions(text, vec![detector(0)], vec![0, 1]);
        assert_eq!(actual[&detector(0)][&0].to_string(), tick0, "{text}");
        assert_eq!(actual[&detector(0)][&1].to_string(), tick1, "{text}");
    }
}

#[test]
fn detecting_regions_deduplicates_requested_ids() {
    let actual = regions(
        "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
        vec![detector(0), detector(0)],
        vec![1, 0, 1],
    );

    assert_eq!(actual.len(), 1);
    assert_eq!(actual[&detector(0)].len(), 2);
}

#[test]
fn detecting_regions_rejects_unknown_detector() {
    let circuit = Circuit::from_stim_str("MXX 0 1\nDETECTOR rec[-1]\n").unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(1)],
            ticks: vec![],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("requested detector D1"));
}

#[test]
fn detecting_regions_rejects_out_of_range_tick() {
    let circuit = Circuit::from_stim_str("TICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![1],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("requested tick 1"));
}

#[test]
fn detecting_regions_anticommutation_supports_ignored_mode() {
    let circuit = Circuit::from_stim_str("TICK\nR 0\nTICK\nMX 0\nDETECTOR rec[-1]\n").unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0, 1],
            ignore_anticommutation_errors: true,
        },
    )
    .unwrap();
    assert!(!actual[&detector(0)].contains_key(&0));
    assert_eq!(actual[&detector(0)][&1].to_string(), "+X");

    let implicit_start = Circuit::from_stim_str("TICK\nMX 0\nDETECTOR rec[-1]\n").unwrap();
    let actual = circuit_detecting_regions(
        &implicit_start,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: true,
        },
    )
    .unwrap();
    assert_eq!(actual[&detector(0)][&0].to_string(), "+X");

    let empty_ticks = circuit_detecting_regions(
        &implicit_start,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![],
            ignore_anticommutation_errors: true,
        },
    )
    .unwrap();
    assert!(empty_ticks[&detector(0)].is_empty());

    let empty_targets = circuit_detecting_regions(
        &implicit_start,
        DetectingRegionOptions {
            detectors: vec![],
            ticks: vec![0],
            ignore_anticommutation_errors: true,
        },
    )
    .unwrap();
    assert!(empty_targets.is_empty());
}

#[test]
fn detecting_regions_anticommutation_rejects_false_mode() {
    let circuit = Circuit::from_stim_str(
        "MXX 0 1\n\
             DETECTOR rec[-1]\n\
             TICK\n\
             H 0\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
    )
    .unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("anti-commuted"));
}

#[test]
fn detecting_regions_anticommutation_rejects_implicit_start_state() {
    let circuit = Circuit::from_stim_str("TICK\nMXX 0 1\nDETECTOR rec[-1]\n").unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("anti-commuted"));
}

#[test]
fn detecting_regions_anticommutation_rejects_false_mode_with_empty_filters() {
    let circuit = Circuit::from_stim_str("TICK\nMX 0\nDETECTOR rec[-1]\n").unwrap();
    for (detectors, ticks) in [(vec![detector(0)], vec![]), (vec![], vec![0])] {
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors,
                ticks,
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(error.to_string().contains("anti-commuted"));
    }
}

#[test]
fn detecting_regions_gauge_ignores_measurement_collapse_when_requested() {
    let circuit =
        Circuit::from_stim_str("RX 0\nTICK\nM 0\nTICK\nMX 0\nDETECTOR rec[-1]\n").unwrap();
    let ignored = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0, 1],
            ignore_anticommutation_errors: true,
        },
    )
    .unwrap();
    assert_eq!(ignored[&detector(0)][&0].to_string(), "+X");
    assert_eq!(ignored[&detector(0)][&1].to_string(), "+X");

    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0, 1],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("anti-commuted"));
}

#[test]
fn detecting_regions_gauge_allows_product_measurement_cancellation() {
    let actual = regions(
        "RX 0 1\n\
             TICK\n\
             MZZ 0 1\n\
             TICK\n\
             MX 0 1\n\
             DETECTOR rec[-1] rec[-2]\n",
        vec![detector(0)],
        vec![0, 1],
    );

    assert_eq!(actual[&detector(0)][&0].to_string(), "+XX");
    assert_eq!(actual[&detector(0)][&1].to_string(), "+XX");
}

#[test]
fn detecting_regions_omits_identity_snapshots() {
    let actual = regions(
        "H 0\n\
             TICK\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n\
             TICK\n",
        vec![detector(0)],
        vec![2],
    );

    assert!(actual[&detector(0)].is_empty());
}

#[test]
fn detecting_regions_repeat_supports_bounded_ticks() {
    let actual = regions(
        "H 0\n\
             REPEAT 2 {\n\
                 TICK\n\
             }\n\
             CX 0 1\n\
             TICK\n\
             MXX 0 1\n\
             DETECTOR rec[-1]\n",
        vec![detector(0)],
        vec![0, 1, 2],
    );

    assert_eq!(actual[&detector(0)][&0].to_string(), "+X_");
    assert_eq!(actual[&detector(0)][&1].to_string(), "+X_");
    assert_eq!(actual[&detector(0)][&2].to_string(), "+XX");
}

#[test]
fn detecting_regions_clifford_supports_swap_gate() {
    let circuit = Circuit::from_stim_str("R 0 1\nTICK\nSWAP 0 1\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert_eq!(actual[&detector(0)][&0].to_string(), "+_Z");
}

#[test]
fn detecting_regions_clifford_supports_promoted_controlled_pauli_gate() {
    let circuit =
        Circuit::from_stim_str("R 0\nRX 1\nTICK\nXCX 0 1\nM 0\nDETECTOR rec[-1]\n").unwrap();
    let actual = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    assert_eq!(actual[&detector(0)][&0].to_string(), "+ZX");
}

#[test]
fn detecting_regions_target_shape_supports_measurement_record_feedback() {
    let cases = [
        (
            "CX record-first",
            "R 0 1\n\
             M 0\n\
             TICK\n\
             CX rec[-1] 1\n\
             M 1\n\
             DETECTOR rec[-1]\n",
            "+_Z",
        ),
        (
            "CY record-first",
            "RY 0 1\n\
             MY 0\n\
             TICK\n\
             CY rec[-1] 1\n\
             MY 1\n\
             DETECTOR rec[-1]\n",
            "+_Y",
        ),
        (
            "CZ record-first",
            "RX 0 1\n\
             MX 0\n\
             TICK\n\
             CZ rec[-1] 1\n\
             MX 1\n\
             DETECTOR rec[-1]\n",
            "+_X",
        ),
        (
            "CZ record-second",
            "RX 0 1\n\
             MX 0\n\
             TICK\n\
             CZ 1 rec[-1]\n\
             MX 1\n\
             DETECTOR rec[-1]\n",
            "+_X",
        ),
        (
            "XCZ record-second",
            "R 0 1\n\
             M 0\n\
             TICK\n\
             XCZ 1 rec[-1]\n\
             M 1\n\
             DETECTOR rec[-1]\n",
            "+_Z",
        ),
        (
            "YCZ record-second",
            "RY 0 1\n\
             MY 0\n\
             TICK\n\
             YCZ 1 rec[-1]\n\
             MY 1\n\
             DETECTOR rec[-1]\n",
            "+_Y",
        ),
    ];

    for (name, circuit, expected) in cases {
        let actual = regions(circuit, vec![detector(0)], vec![0]);

        assert_eq!(actual[&detector(0)][&0].to_string(), expected, "{name}");
    }
}

#[test]
fn detecting_regions_target_shape_supports_sweep_controlled_pauli_noops() {
    let cases = [
        ("CX sweep-first", "R 0", "CX sweep[0] 0", "M 0", "+Z"),
        ("CY sweep-first", "R 0", "CY sweep[0] 0", "M 0", "+Z"),
        ("CZ sweep-first", "RX 0", "CZ sweep[0] 0", "MX 0", "+X"),
        ("CZ sweep-second", "RX 0", "CZ 0 sweep[0]", "MX 0", "+X"),
        ("XCZ sweep-second", "R 0", "XCZ 0 sweep[0]", "M 0", "+Z"),
        ("YCZ sweep-second", "RY 0", "YCZ 0 sweep[0]", "MY 0", "+Y"),
    ];

    for (name, preparation, operation, measurement, expected) in cases {
        let circuit =
            format!("{preparation}\nTICK\n{operation}\n{measurement}\nDETECTOR rec[-1]\n");
        let actual = regions(&circuit, vec![detector(0)], vec![0]);

        assert_eq!(actual[&detector(0)][&0].to_string(), expected, "{name}");
    }
}

#[test]
fn detecting_regions_target_shape_rejects_unsupported_feedback_shapes() {
    let cases = [
        (
            "CX record-second",
            "R 0 1\n\
             M 0\n\
             TICK\n\
             CX 1 rec[-1]\n\
             M 1\n\
             DETECTOR rec[-1]\n",
            "target position",
        ),
        (
            "XCZ record-first",
            "R 0 1\n\
             M 0\n\
             TICK\n\
             XCZ rec[-1] 1\n\
             M 1\n\
             DETECTOR rec[-1]\n",
            "target position",
        ),
        (
            "CY record-record",
            "M 0 1\n\
             TICK\n\
             CY rec[-1] rec[-2]\n\
             M 2\n\
             DETECTOR rec[-1]\n",
            "exactly one plain qubit target",
        ),
    ];

    for (name, circuit, expected_error) in cases {
        let circuit = Circuit::from_stim_str(circuit).unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(
            error.to_string().contains(expected_error),
            "{name}: {error}"
        );
    }
}

#[test]
fn detecting_regions_target_shape_rejects_unpromoted_sweep_shapes() {
    let cases = [
        (
            "sweep-sweep",
            "CX sweep[0] sweep[1]\nTICK\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "record-sweep",
            "M 0\nTICK\nCX rec[-1] sweep[0]\nM 1\nDETECTOR rec[-1]\n",
        ),
        (
            "CX sweep-second",
            "R 0\nTICK\nCX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "CY sweep-second",
            "R 0\nTICK\nCY 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "XCZ sweep-first",
            "R 0\nTICK\nXCZ sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
        ),
        (
            "YCZ sweep-first",
            "RY 0\nTICK\nYCZ sweep[0] 0\nMY 0\nDETECTOR rec[-1]\n",
        ),
    ];

    for (name, circuit) in cases {
        let circuit = Circuit::from_stim_str(circuit).unwrap();
        let error = circuit_detecting_regions(
            &circuit,
            DetectingRegionOptions {
                detectors: vec![detector(0)],
                ticks: vec![0],
                ignore_anticommutation_errors: false,
            },
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("simple detecting-region extraction"),
            "{name}: {error}"
        );
    }
}

#[test]
fn detecting_regions_repeat_rejects_excessive_expansion() {
    let circuit =
        Circuit::from_stim_str("REPEAT 1000001 {\n    TICK\n}\nMXX 0 1\nDETECTOR rec[-1]\n")
            .unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("expanded repeat iterations"));
}
