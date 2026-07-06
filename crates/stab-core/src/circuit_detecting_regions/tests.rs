#![allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "parity tests use fixed detector and tick ids for compact expected maps"
)]

use super::*;
use crate::{
    CodeDistance, RepetitionCodeParams, RepetitionCodeTask, RoundCount, SurfaceCodeParams,
    SurfaceCodeTask, generate_repetition_code_circuit, generate_surface_code_circuit,
};

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
fn detecting_regions_generated_repetition_code_filters_and_regions() {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(3).unwrap(),
        CodeDistance::try_new(3).unwrap(),
        RepetitionCodeTask::Memory,
    )
    .unwrap();
    let generated = generate_repetition_code_circuit(&params).unwrap();
    let circuit = generated.circuit();

    let all_targets = all_detecting_region_targets(circuit).unwrap();
    assert_eq!(all_targets.len(), 9);
    assert_eq!(
        all_detecting_region_ticks(circuit).unwrap(),
        (0..9).collect::<Vec<_>>()
    );

    let actual = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: all_targets,
            ticks: vec![0, 1, 2, 6, 7, 8],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();

    let d0 = DemTarget::relative_detector(0).unwrap();
    assert_eq!(actual[&d0][&0].to_string(), "+ZZZ__");
    assert_eq!(actual[&d0][&1].to_string(), "+_ZZ__");
    assert_eq!(actual[&d0][&2].to_string(), "+_Z___");
    assert!(!actual[&d0].contains_key(&6));

    let d6 = DemTarget::relative_detector(6).unwrap();
    assert_eq!(actual[&d6][&6].to_string(), "+_Z___");
    assert_eq!(actual[&d6][&7].to_string(), "+ZZ___");
    assert_eq!(actual[&d6][&8].to_string(), "+ZZZ__");

    let logical = DemTarget::logical_observable(0).unwrap();
    for tick in [0, 1, 2, 6, 7, 8] {
        assert_eq!(actual[&logical][&tick].to_string(), "+____Z");
    }
}

#[test]
fn detecting_regions_generated_rotated_surface_code_filters_and_regions() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(3).unwrap(),
        CodeDistance::try_new(3).unwrap(),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .unwrap();
    let generated = generate_surface_code_circuit(&params).unwrap();
    let circuit = generated.circuit();

    let all_targets = all_detecting_region_targets(circuit).unwrap();
    let all_ticks = all_detecting_region_ticks(circuit).unwrap();
    assert_eq!(all_targets.len(), 25);
    assert_eq!(all_ticks, (0..=20).collect::<Vec<_>>());

    let selected_targets = vec![
        DemTarget::relative_detector(0).unwrap(),
        DemTarget::relative_detector(4).unwrap(),
        DemTarget::logical_observable(0).unwrap(),
    ];
    let selected_ticks = all_ticks.iter().copied().take(6).collect::<Vec<_>>();
    let actual = circuit_detecting_regions_for_targets(
        circuit,
        DetectingRegionTargetOptions {
            targets: selected_targets.clone(),
            ticks: selected_ticks.clone(),
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap();
    assert_eq!(actual.len(), 3);

    let d0 = DemTarget::relative_detector(0).unwrap();
    assert_eq!(actual[&d0][&0].to_string(), "+________Z_____ZZ__________");
    assert_eq!(actual[&d0][&1].to_string(), "+________Z_____ZZ__________");
    assert_eq!(actual[&d0][&2].to_string(), "+________Z_____Z___________");
    assert_eq!(actual[&d0][&3].to_string(), "+______________Z___________");
    assert_eq!(actual[&d0][&4].to_string(), "+______________Z___________");
    assert_eq!(actual[&d0][&5].to_string(), "+______________Z___________");

    let d4 = DemTarget::relative_detector(4).unwrap();
    assert_eq!(actual[&d4][&0].to_string(), "+__Z_______________________");
    assert_eq!(actual[&d4][&1].to_string(), "+__X_______________________");
    assert_eq!(actual[&d4][&2].to_string(), "+__XX______________________");
    assert_eq!(actual[&d4][&3].to_string(), "+_XXX_____X________________");
    assert_eq!(actual[&d4][&4].to_string(), "+_XXX_____X________________");
    assert_eq!(actual[&d4][&5].to_string(), "+_XXX______________________");

    let logical = DemTarget::logical_observable(0).unwrap();
    for (tick, expected) in [
        (0, "+_Z_Z_Z____________________"),
        (1, "+_Z_Z_Z____________________"),
        (2, "+_ZZZ_Z____________________"),
        (3, "+_Z_Z_Z____________________"),
        (4, "+_Z_Z_Z_____Z______________"),
        (5, "+_Z_Z_Z____________________"),
    ] {
        assert_eq!(actual[&logical][&tick].to_string(), expected);
    }
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
fn detecting_regions_target_shape_rejects_unpromoted_heralded_record_annotations() {
    let circuit =
        Circuit::from_stim_str("TICK\nHERALDED_ERASE(0.125) 0\nDETECTOR rec[-1]\n").unwrap();
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
        error
            .to_string()
            .contains("does not support gate HERALDED_ERASE")
    );
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
fn detecting_regions_clifford_rejects_feedback_controlled_cx() {
    let circuit =
        Circuit::from_stim_str("MXX 0 1\nCX rec[-1] 2\nTICK\nMXX 2 3\nDETECTOR rec[-1]\n").unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("plain qubit targets"));
}

#[test]
fn detecting_regions_clifford_rejects_sweep_controlled_cx() {
    let circuit =
        Circuit::from_stim_str("CX sweep[0] 2\nTICK\nMXX 2 3\nDETECTOR rec[-1]\n").unwrap();
    let error = circuit_detecting_regions(
        &circuit,
        DetectingRegionOptions {
            detectors: vec![detector(0)],
            ticks: vec![0],
            ignore_anticommutation_errors: false,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("plain qubit targets"));
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
