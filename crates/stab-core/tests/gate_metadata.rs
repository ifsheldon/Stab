#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::string_slice,
    clippy::unwrap_used,
    reason = "PF1 gate metadata compatibility tests use direct assertions and ASCII pinned-source slicing for compact diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use num_complex::Complex32;
use stab_core::{
    Circuit, CircuitItem, CompiledDetectionConverter, CompiledSampler, DetectionConversionOptions,
    Gate, GateArgumentRule, GateTargetGroupKind, GateTargetRule, Probability,
    check_if_circuit_has_unsigned_stabilizer_flows, convert_measurements_to_detection_events,
    unitary_to_tableau,
};

#[test]
fn gate_metadata_accessors_match_owned_stim_gatedata_semantics() {
    // Adapted from the non-binding-specific flags in Stim v1.16.0 src/stim/gates/gates_test.py.
    let h = Gate::from_name("H").unwrap();
    let cx = Gate::from_name("CX").unwrap();
    let r = Gate::from_name("R").unwrap();
    let mr = Gate::from_name("MR").unwrap();
    let mxx = Gate::from_name("MXX").unwrap();
    let mpp = Gate::from_name("MPP").unwrap();
    let x_error = Gate::from_name("X_ERROR").unwrap();
    let detector = Gate::from_name("DETECTOR").unwrap();

    assert_eq!(h.aliases(), &["H", "H_XZ"]);
    assert_eq!(cx.aliases(), &["CNOT", "CX", "ZCX"]);
    assert_eq!(Gate::from_name("MZ").unwrap().aliases(), &["M", "MZ"]);
    assert_eq!(
        Gate::from_name("SWAPCZ").unwrap().aliases(),
        &["CZSWAP", "SWAPCZ"]
    );

    assert_eq!(h.argument_rule(), GateArgumentRule::Exact(0));
    assert_eq!(
        Gate::from_name("M").unwrap().argument_rule(),
        GateArgumentRule::OptionalProbability
    );
    assert_eq!(
        Gate::from_name("PAULI_CHANNEL_2").unwrap().argument_rule(),
        GateArgumentRule::ProbabilityList(15)
    );
    assert_eq!(
        Gate::from_name("I_ERROR").unwrap().argument_rule(),
        GateArgumentRule::AnyProbabilityList
    );
    assert_eq!(
        Gate::from_name("II_ERROR").unwrap().argument_rule(),
        GateArgumentRule::AnyProbabilityList
    );
    assert_eq!(
        Gate::from_name("OBSERVABLE_INCLUDE")
            .unwrap()
            .argument_rule(),
        GateArgumentRule::UnsignedInteger
    );
    assert_eq!(detector.argument_rule(), GateArgumentRule::Any);

    assert_eq!(h.target_rule(), GateTargetRule::AnySingleQubit);
    assert_eq!(cx.target_rule(), GateTargetRule::ClassicalControlPairs);
    assert_eq!(
        Gate::from_name("XCX").unwrap().target_rule(),
        GateTargetRule::PlainPairs
    );
    assert_eq!(mpp.target_rule(), GateTargetRule::PauliProducts);
    assert_eq!(detector.target_rule(), GateTargetRule::RecOnly);
    assert_eq!(
        Gate::from_name("E").unwrap().target_group_kind(),
        GateTargetGroupKind::AllTargets
    );

    assert!(h.is_unitary());
    assert!(cx.is_unitary());
    assert!(!r.is_unitary());
    assert!(!mxx.is_unitary());
    assert!(!x_error.is_unitary());
    assert!(!detector.is_unitary());

    assert!(r.is_reset());
    assert!(mr.is_reset());
    assert!(!h.is_reset());
    assert!(!mxx.is_reset());

    assert!(x_error.is_noisy());
    assert!(mxx.is_noisy());
    assert!(mpp.is_noisy());
    assert!(!h.is_noisy());
    assert!(!r.is_noisy());
    assert!(!Gate::from_name("MPAD").unwrap().is_noisy());
    assert!(!detector.is_noisy());

    assert!(mr.produces_measurements());
    assert!(mxx.produces_measurements());
    assert!(mpp.produces_measurements());
    assert!(!r.produces_measurements());
    assert!(!h.produces_measurements());
    assert!(!x_error.produces_measurements());

    assert!(h.is_single_qubit_gate());
    assert!(!cx.is_single_qubit_gate());
    assert!(cx.is_two_qubit_gate());
    assert!(mxx.is_two_qubit_gate());
    assert!(!mpp.is_two_qubit_gate());
    assert!(!detector.is_two_qubit_gate());

    assert!(mpp.takes_pauli_targets());
    assert!(Gate::from_name("E").unwrap().takes_pauli_targets());
    assert!(!mxx.takes_pauli_targets());
    assert!(!detector.takes_pauli_targets());

    assert!(detector.takes_measurement_record_targets());
    assert!(cx.takes_measurement_record_targets());
    assert!(
        Gate::from_name("XCZ")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("XCX")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("XCY")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("YCX")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("YCY")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(!h.takes_measurement_record_targets());
    assert!(!mpp.takes_measurement_record_targets());

    assert!(Gate::from_name("SWAP").unwrap().is_symmetric_gate());
    assert!(h.is_symmetric_gate());
    assert!(mxx.is_symmetric_gate());
    assert!(Gate::from_name("DEPOLARIZE2").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("XCX").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("YCY").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("CZ").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("CX").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("XCY").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("YCX").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("MPAD").unwrap().is_symmetric_gate());
    assert!(
        !Gate::from_name("PAULI_CHANNEL_2")
            .unwrap()
            .is_symmetric_gate()
    );
    assert!(!detector.is_symmetric_gate());

    assert_eq!(h.inverse().unwrap().canonical_name(), "H");
    assert_eq!(
        Gate::from_name("S")
            .unwrap()
            .inverse()
            .unwrap()
            .canonical_name(),
        "S_DAG"
    );
    assert_eq!(Gate::from_name("M").unwrap().inverse(), None);
    assert_eq!(
        Gate::from_name("M")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "M"
    );
    assert_eq!(
        Gate::from_name("R")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "M"
    );
    assert_eq!(
        Gate::from_name("SPP")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "SPP_DAG"
    );
}

#[test]
fn gate_tableau_metadata_matches_owned_unitary_gate_data() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.test.cc tableau and unitary inverse checks.
    let h = Gate::from_name("H").expect("H");
    let h_tableau = h.tableau().expect("H tableau");
    assert_eq!(h_tableau.x_output(0).expect("H X").to_string(), "+Z");
    assert_eq!(h_tableau.z_output(0).expect("H Z").to_string(), "+X");
    assert!(h.has_tableau());

    let cx = Gate::from_name("CX").expect("CX");
    let cx_tableau = cx.tableau().expect("CX tableau");
    assert_eq!(cx_tableau.x_output(0).expect("CX X0").to_string(), "+XX");
    assert_eq!(cx_tableau.z_output(0).expect("CX Z0").to_string(), "+Z_");
    assert_eq!(cx_tableau.x_output(1).expect("CX X1").to_string(), "+_X");
    assert_eq!(cx_tableau.z_output(1).expect("CX Z1").to_string(), "+ZZ");

    let expected_tableau_names = expected_tableau_supported_gate_names();
    assert_eq!(expected_tableau_names.len(), 46);
    let actual_tableau_names = Gate::all()
        .filter(|gate| gate.has_tableau())
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_tableau_names, expected_tableau_names);

    for gate_name in expected_tableau_names {
        let gate = Gate::from_name(gate_name).expect("gate");
        let inverse = gate.inverse().expect("unitary inverse");
        let gate_inverse_tableau = gate
            .tableau()
            .expect("gate tableau")
            .inverse()
            .expect("inverse tableau");
        assert_eq!(
            gate_inverse_tableau,
            inverse.tableau().expect("inverse gate tableau"),
            "{gate_name} inverse tableau should match inverse gate metadata"
        );
    }

    for gate in Gate::all() {
        assert_eq!(
            gate.has_tableau(),
            gate.tableau().is_ok(),
            "{} has_tableau should match tableau materialization",
            gate.canonical_name()
        );
    }

    for unsupported in ["M", "R", "DETECTOR", "SPP"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate.has_tableau(), "{unsupported}");
        let error = gate.tableau().expect_err("reject missing tableau data");
        assert!(
            error.to_string().contains("does not have tableau data"),
            "{error}"
        );
    }
}

#[test]
fn gate_flow_metadata_matches_owned_unitary_gate_data() {
    // Adapted from Stim v1.16.0 GateData flow examples and gate_data stabilizer-flow checks.
    let h = Gate::from_name("H").expect("H");
    assert!(h.has_flows());
    assert_eq!(
        flow_texts(h.flows().expect("H flows")),
        ["X -> Z", "Z -> X"].map(String::from).to_vec()
    );

    let iswap = Gate::from_name("ISWAP").expect("ISWAP");
    assert_eq!(
        flow_texts(iswap.flows().expect("ISWAP flows")),
        ["X_ -> ZY", "Z_ -> _Z", "_X -> YZ", "_Z -> Z_"]
            .map(String::from)
            .to_vec()
    );

    let sqrt_xx = Gate::from_name("SQRT_XX").expect("SQRT_XX");
    assert_eq!(
        flow_texts(sqrt_xx.flows().expect("SQRT_XX flows")),
        ["X_ -> X_", "Z_ -> -YX", "_X -> _X", "_Z -> -XY"]
            .map(String::from)
            .to_vec()
    );

    let measurement = Gate::from_name("M").expect("M");
    assert_eq!(
        flow_texts(measurement.flows().expect("M flows")),
        ["Z -> rec[-1]", "Z -> Z"].map(String::from).to_vec()
    );

    let pair_measurement = Gate::from_name("MXX").expect("MXX");
    assert_eq!(
        flow_texts(pair_measurement.flows().expect("MXX flows")),
        ["X_ -> X_", "_X -> _X", "ZZ -> ZZ", "XX -> rec[-1]"]
            .map(String::from)
            .to_vec()
    );

    let pauli_product_measurement = Gate::from_name("MPP").expect("MPP");
    assert_eq!(
        flow_texts(pauli_product_measurement.flows().expect("MPP flows")),
        [
            "XYZ__ -> rec[-2]",
            "___XX -> rec[-1]",
            "X____ -> X____",
            "_Y___ -> _Y___",
            "__Z__ -> __Z__",
            "___X_ -> ___X_",
            "____X -> ____X",
            "ZZ___ -> ZZ___",
            "_XX__ -> _XX__",
            "___ZZ -> ___ZZ",
        ]
        .map(String::from)
        .to_vec()
    );

    let pauli_product = Gate::from_name("SPP").expect("SPP");
    assert_eq!(
        flow_texts(pauli_product.flows().expect("SPP flows")),
        [
            "X__ -> X__",
            "Z__ -> -YYZ",
            "_X_ -> -XZZ",
            "_Z_ -> XXZ",
            "__X -> XYY",
            "__Z -> __Z",
        ]
        .map(String::from)
        .to_vec()
    );

    let expected_flow_names = expected_flow_supported_gate_names();
    let actual_flow_names = Gate::all()
        .filter(|gate| gate.has_flows())
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_flow_names, expected_flow_names);

    for gate_name in expected_tableau_supported_gate_names() {
        let gate = Gate::from_name(gate_name).expect("gate");
        let flows = gate.flows().expect("gate flows");
        assert_eq!(
            flows.len(),
            gate.tableau().expect("gate tableau").len() * 2,
            "{gate_name} should produce X and Z flow generators for each target"
        );
        let circuit = single_instruction_circuit(gate, gate_name);
        assert!(
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows)
                .into_iter()
                .all(|ok| ok),
            "{gate_name} flows should be satisfied by the gate"
        );
    }

    for (gate_name, circuit) in measurement_rich_flow_metadata_circuits() {
        let gate = Gate::from_name(gate_name).expect("gate");
        let flows = gate.flows().expect("gate flows");
        assert!(
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows)
                .into_iter()
                .all(|ok| ok),
            "{gate_name} flows should be satisfied by the representative circuit"
        );
    }

    for unsupported in ["MPAD", "DETECTOR", "X_ERROR", "PAULI_CHANNEL_1"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate.has_flows(), "{unsupported}");
        let error = gate.flows().expect_err("reject unsupported flow data");
        assert!(error.to_string().contains("flow metadata"), "{error}");
    }
}

#[test]
fn gate_unitary_matrix_metadata_matches_owned_gate_data() {
    // Adapted from Stim v1.16.0 GateData unitary matrix examples and inverse consistency checks.
    let h = Gate::from_name("H").expect("H");
    let h_scale = f32::sqrt(0.5);
    assert!(h.has_unitary_matrix());
    let h_matrix = h.unitary_matrix().expect("H unitary");
    assert_matrix_close(
        &h_matrix.to_vecs(),
        &[
            &[(h_scale, 0.0), (h_scale, 0.0)],
            &[(h_scale, 0.0), (-h_scale, 0.0)],
        ],
    );

    let iswap = Gate::from_name("ISWAP").expect("ISWAP");
    let iswap_matrix = iswap.unitary_matrix().expect("ISWAP unitary");
    assert_matrix_close(
        &iswap_matrix.to_vecs(),
        &[
            &[(1.0, 0.0), (0.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
            &[(0.0, 0.0), (0.0, 0.0), (0.0, 1.0), (0.0, 0.0)],
            &[(0.0, 0.0), (0.0, 1.0), (0.0, 0.0), (0.0, 0.0)],
            &[(0.0, 0.0), (0.0, 0.0), (0.0, 0.0), (1.0, 0.0)],
        ],
    );

    let sqrt_xx = Gate::from_name("SQRT_XX").expect("SQRT_XX");
    let sqrt_xx_matrix = sqrt_xx.unitary_matrix().expect("SQRT_XX unitary");
    assert_matrix_close(
        &sqrt_xx_matrix.to_vecs(),
        &[
            &[(0.5, 0.5), (0.0, 0.0), (0.0, 0.0), (0.5, -0.5)],
            &[(0.0, 0.0), (0.5, 0.5), (0.5, -0.5), (0.0, 0.0)],
            &[(0.0, 0.0), (0.5, -0.5), (0.5, 0.5), (0.0, 0.0)],
            &[(0.5, -0.5), (0.0, 0.0), (0.0, 0.0), (0.5, 0.5)],
        ],
    );

    let expected_unitary_names = expected_tableau_supported_gate_names();
    let actual_unitary_names = Gate::all()
        .filter(|gate| gate.has_unitary_matrix())
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_unitary_names, expected_unitary_names);

    for &gate_name in &expected_unitary_names {
        let gate = Gate::from_name(gate_name).expect("gate");
        let matrix = gate.unitary_matrix().expect("gate unitary");
        let matrix_rows = matrix.to_vecs();
        let dimension = matrix.dimension();
        assert!(
            matches!(dimension, 2 | 4),
            "{gate_name} should have one- or two-qubit unitary metadata"
        );
        assert_eq!(
            matrix.num_qubits(),
            if dimension == 2 { 1 } else { 2 },
            "{gate_name} unitary metadata target count"
        );
        assert!(
            matrix_rows.len() == dimension && matrix_rows.iter().all(|row| row.len() == dimension),
            "{gate_name} should have square unitary metadata"
        );
        assert_eq!(
            matrix.entry_count(),
            dimension * dimension,
            "{gate_name} unitary metadata entry count"
        );
        assert_eq!(
            unitary_to_tableau(&matrix_rows, true).expect("unitary tableau"),
            gate.tableau().expect("gate tableau"),
            "{gate_name} unitary matrix should convert to the gate tableau"
        );

        let inverse = gate.inverse().expect("unitary inverse");
        let inverse_matrix = inverse.unitary_matrix().expect("inverse unitary").to_vecs();
        let expected_inverse = conjugate_transpose(&matrix_rows);
        assert_matrix_close_matrix(
            &inverse_matrix,
            &expected_inverse,
            1e-6,
            &format!("{gate_name} inverse unitary should be the conjugate transpose"),
        );
    }

    for gate in Gate::all() {
        assert_eq!(
            gate.has_unitary_matrix(),
            gate.unitary_matrix().is_ok(),
            "{} has_unitary_matrix should match unitary matrix materialization",
            gate.canonical_name()
        );
    }

    for unsupported in ["MXX", "MPP", "SPP", "SPP_DAG", "M", "DETECTOR", "X_ERROR"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate.has_unitary_matrix(), "{unsupported}");
        let error = gate
            .unitary_matrix()
            .expect_err("reject unsupported unitary matrix data");
        assert!(error.to_string().contains("unitary matrix data"), "{error}");
    }
}

#[test]
fn gate_decomposition_metadata_matches_owned_gate_data() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.test.cc and gate_data_*.cc decomposition metadata.
    let h = Gate::from_name("H").expect("H");
    assert!(h.has_h_s_cx_m_r_decomposition());
    assert_eq!(
        h.h_s_cx_m_r_decomposition()
            .expect("H decomposition")
            .as_stim_str(),
        "\nH 0\n"
    );
    assert_eq!(
        h.h_s_cx_m_r_decomposition()
            .expect("H decomposition")
            .to_circuit()
            .expect("parse H decomposition")
            .to_stim_string(),
        "H 0\n"
    );

    let cx = Gate::from_name("CX").expect("CX");
    assert_eq!(
        cx.h_s_cx_m_r_decomposition()
            .expect("CX decomposition")
            .as_stim_str(),
        "\nCNOT 0 1\n"
    );

    let mxx = Gate::from_name("MXX").expect("MXX");
    assert_eq!(
        mxx.h_s_cx_m_r_decomposition()
            .expect("MXX decomposition")
            .to_circuit()
            .expect("parse MXX decomposition")
            .to_stim_string(),
        concat!("CX 0 1\n", "H 0\n", "M 0\n", "H 0\n", "CX 0 1\n")
    );

    let expected_decomposition_names = expected_decomposition_supported_gate_names();
    assert_eq!(expected_decomposition_names.len(), 61);
    let actual_decomposition_names = Gate::all()
        .filter(|gate| gate.has_h_s_cx_m_r_decomposition())
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_decomposition_names, expected_decomposition_names);

    let upstream_texts = upstream_decomposition_texts();
    assert_eq!(
        upstream_texts.keys().copied().collect::<BTreeSet<_>>(),
        expected_decomposition_supported_gate_names()
    );
    for gate_name in expected_decomposition_names {
        let gate = Gate::from_name(gate_name).expect("gate");
        let decomposition = gate.h_s_cx_m_r_decomposition().expect("gate decomposition");
        assert_eq!(
            decomposition.as_stim_str(),
            *upstream_texts.get(gate_name).expect("upstream text"),
            "{gate_name} decomposition text should match pinned Stim v1.16.0"
        );
        let circuit = decomposition.to_circuit().expect("parse decomposition");
        assert_h_s_cx_m_r_base(&circuit, gate_name);
    }

    for gate in Gate::all() {
        assert_eq!(
            gate.has_h_s_cx_m_r_decomposition(),
            gate.h_s_cx_m_r_decomposition().is_ok(),
            "{} has_h_s_cx_m_r_decomposition should match materialization",
            gate.canonical_name()
        );
    }

    for unsupported in [
        "DETECTOR",
        "TICK",
        "SHIFT_COORDS",
        "X_ERROR",
        "HERALDED_ERASE",
    ] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate.has_h_s_cx_m_r_decomposition(), "{unsupported}");
        let error = gate
            .h_s_cx_m_r_decomposition()
            .expect_err("reject missing decomposition data");
        assert!(error.to_string().contains("decomposition data"), "{error}");
    }
}

#[test]
fn gate_decomposition_metadata_matches_tableau_where_defined() {
    // The decomposition strings are gate-table metadata. Full circuit decomposition belongs to RPF2.
    // For fixed-shape unitary gates with non-empty decompositions, the current tableau comparator is valid.
    for gate_name in expected_tableau_supported_gate_names() {
        if matches!(gate_name, "I" | "II") {
            continue;
        }
        let gate = Gate::from_name(gate_name).expect("gate");
        let decomposition = gate
            .h_s_cx_m_r_decomposition()
            .expect("unitary gate should have decomposition")
            .to_circuit()
            .expect("parse decomposition");
        assert_eq!(
            decomposition
                .to_tableau(false, false, false)
                .expect("decomposition tableau"),
            gate.tableau().expect("gate tableau"),
            "{gate_name} decomposition should match gate tableau"
        );
    }

    for gate_name in ["M", "MR", "MXX", "MPP", "SPP", "SPP_DAG"] {
        let gate = Gate::from_name(gate_name).expect("non-tableau decomposition gate");
        assert!(gate.has_h_s_cx_m_r_decomposition(), "{gate_name}");
        assert!(
            gate.tableau().is_err(),
            "{gate_name} decomposition metadata should not imply tableau metadata"
        );
    }
}

#[test]
fn gate_execution_contract_rejects_variable_target_spp_sampler_execution() {
    // Parser validation accepts SPP/SPP_DAG targets, but sampler and detector conversion execution are explicit later gate-semantics milestones.
    for gate_name in ["SPP", "SPP_DAG"] {
        let circuit =
            Circuit::from_stim_str(&format!("{gate_name} X0 X1*Y2*Z3\n")).expect("parse SPP");
        let error = CompiledSampler::compile(&circuit)
            .expect_err("sampler should reject SPP execution")
            .to_string();
        assert!(
            error.contains("sampler subset does not support"),
            "{gate_name}: {error}"
        );
        for skip_reference_sample in [false, true] {
            let error = CompiledDetectionConverter::compile(
                &circuit,
                DetectionConversionOptions {
                    skip_reference_sample,
                },
            )
            .expect_err("detection conversion should reject SPP execution")
            .to_string();
            assert!(
                error.contains("detection conversion does not yet support"),
                "{gate_name}: {error}"
            );
        }
        let error = convert_measurements_to_detection_events(
            &circuit,
            &[Vec::new()],
            DetectionConversionOptions {
                skip_reference_sample: true,
            },
        )
        .expect_err("public conversion helper should reject SPP execution")
        .to_string();
        assert!(
            error.contains("detection conversion does not yet support"),
            "{gate_name}: {error}"
        );
    }
}

#[test]
fn gate_metadata_api_contract_table_matches_rust_accessors() {
    let support_rows = parse_gate_support_contract_table();
    assert_eq!(
        support_rows.len(),
        support_rows
            .iter()
            .map(|(gate, _row)| *gate)
            .collect::<BTreeSet<_>>()
            .len(),
        "support contract should not duplicate canonical gate rows"
    );
    let support_table = support_rows.into_iter().collect::<BTreeMap<_, _>>();
    let actual_gate_names = Gate::all()
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        support_table.keys().copied().collect::<BTreeSet<_>>(),
        actual_gate_names,
        "support contract should mention every canonical gate exactly once"
    );

    for gate in Gate::all() {
        let gate_name = gate.canonical_name();
        let row = support_table.get(gate_name).expect("contract row");
        assert!(row.validation, "{gate_name} validation column");
        assert_eq!(row.tableau, gate.has_tableau(), "{gate_name} tableau");
        assert_eq!(
            row.unitary,
            gate.has_unitary_matrix(),
            "{gate_name} unitary"
        );
        assert_eq!(row.flow, gate.has_flows(), "{gate_name} flow");
        assert_eq!(
            row.decomposition,
            gate.has_h_s_cx_m_r_decomposition(),
            "{gate_name} decomposition"
        );
    }
}

fn flow_texts(flows: Vec<stab_core::Flow>) -> Vec<String> {
    flows.into_iter().map(|flow| flow.to_string()).collect()
}

#[derive(Debug)]
struct GateSupportContractRow {
    validation: bool,
    tableau: bool,
    unitary: bool,
    flow: bool,
    decomposition: bool,
}

fn parse_gate_support_contract_table() -> Vec<(&'static str, GateSupportContractRow)> {
    include_str!("../../../docs/plans/rpf1-gate-execution-support-contract.md")
        .lines()
        .filter_map(|line| {
            if !line.starts_with("| `") {
                return None;
            }
            let cells = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            let [
                gate_cell,
                validation,
                tableau,
                unitary,
                flow,
                decomposition,
                _sampler,
                _detection_conversion,
                _analyzer,
            ] = cells.as_slice()
            else {
                panic!("support contract row shape: {line}");
            };
            let gate = gate_cell.trim_matches('`');
            Some((
                gate,
                GateSupportContractRow {
                    validation: support_contract_bool(gate, "Validation", validation),
                    tableau: support_contract_bool(gate, "Tableau", tableau),
                    unitary: support_contract_bool(gate, "Unitary", unitary),
                    flow: support_contract_bool(gate, "Flow", flow),
                    decomposition: support_contract_bool(gate, "Decomposition", decomposition),
                },
            ))
        })
        .collect()
}

fn support_contract_bool(gate: &str, column: &str, value: &str) -> bool {
    match value {
        "Yes" => true,
        "No" => false,
        _ => panic!("{gate} {column} support cell must be Yes or No, got {value:?}"),
    }
}

fn assert_h_s_cx_m_r_base(circuit: &Circuit, gate_name: &str) {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => assert!(
                matches!(
                    instruction.gate().canonical_name(),
                    "H" | "S" | "CX" | "M" | "R"
                ),
                "{gate_name} decomposition used non-base gate {}",
                instruction.gate().canonical_name()
            ),
            CircuitItem::RepeatBlock(_) => panic!("{gate_name} decomposition should not repeat"),
        }
    }
}

fn upstream_decomposition_texts() -> BTreeMap<&'static str, &'static str> {
    [
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_annotations.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_blocks.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_collapsing.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_controlled.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_hada.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_heralded.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_noisy.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pair_measure.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pauli.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pauli_product.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_period_3.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_period_4.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pp.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_swaps.cc"),
    ]
    .into_iter()
    .flat_map(upstream_decompositions_from_file)
    .collect()
}

fn upstream_decompositions_from_file(text: &'static str) -> BTreeMap<&'static str, &'static str> {
    let mut out = BTreeMap::new();
    let mut rest = text;
    while let Some(name_start) = rest.find(".name = \"") {
        rest = &rest[name_start + ".name = \"".len()..];
        let Some(name_end) = rest.find('"') else {
            break;
        };
        let name = &rest[..name_end];
        let after_name = &rest[name_end..];
        let Some(field_start) = after_name.find(".h_s_cx_m_r_decomposition = ") else {
            break;
        };
        rest = &after_name[field_start + ".h_s_cx_m_r_decomposition = ".len()..];
        let raw_prefix = "R\"CIRCUIT(";
        if !rest.starts_with(raw_prefix) {
            continue;
        }
        rest = &rest[raw_prefix.len()..];
        let Some(raw_end) = rest.find(")CIRCUIT\"") else {
            break;
        };
        out.insert(name, &rest[..raw_end]);
        rest = &rest[raw_end + ")CIRCUIT\"".len()..];
    }
    out
}

fn assert_matrix_close(actual: &[Vec<Complex32>], expected: &[&[(f32, f32)]]) {
    assert_eq!(actual.len(), expected.len());
    for (actual_row, expected_row) in actual.iter().zip(expected) {
        assert_eq!(actual_row.len(), expected_row.len());
        for (actual_value, &(expected_real, expected_imag)) in actual_row.iter().zip(*expected_row)
        {
            assert_complex_close(
                *actual_value,
                Complex32::new(expected_real, expected_imag),
                1e-6,
                "matrix entry",
            );
        }
    }
}

fn conjugate_transpose(matrix: &[Vec<Complex32>]) -> Vec<Vec<Complex32>> {
    let dimension = matrix.len();
    (0..dimension)
        .map(|row| {
            matrix
                .iter()
                .map(|source_row| {
                    source_row
                        .get(row)
                        .copied()
                        .expect("square matrix entry")
                        .conj()
                })
                .collect()
        })
        .collect()
}

fn assert_matrix_close_matrix(
    actual: &[Vec<Complex32>],
    expected: &[Vec<Complex32>],
    tolerance: f32,
    label: &str,
) {
    assert_eq!(actual.len(), expected.len(), "{label}");
    for (actual_row, expected_row) in actual.iter().zip(expected) {
        assert_eq!(actual_row.len(), expected_row.len(), "{label}");
        for (&actual_value, &expected_value) in actual_row.iter().zip(expected_row) {
            assert_complex_close(actual_value, expected_value, tolerance, label);
        }
    }
}

fn assert_complex_close(actual: Complex32, expected: Complex32, tolerance: f32, label: &str) {
    assert!(
        (actual - expected).norm() <= tolerance,
        "{label}: expected {expected:?}, got {actual:?}"
    );
}

fn single_instruction_circuit(gate: Gate, gate_name: &str) -> Circuit {
    let targets = ["", "0", "0 1"]
        .get(gate.tableau().expect("gate tableau").len())
        .copied()
        .expect("supported flow target count");
    Circuit::from_stim_str(&format!("{gate_name} {targets}\n")).expect("gate circuit")
}

fn measurement_rich_flow_metadata_circuits() -> Vec<(&'static str, Circuit)> {
    [
        ("M", "M 0\n"),
        ("MX", "MX 0\n"),
        ("MY", "MY 0\n"),
        ("R", "R 0\n"),
        ("RX", "RX 0\n"),
        ("RY", "RY 0\n"),
        ("MR", "MR 0\n"),
        ("MRX", "MRX 0\n"),
        ("MRY", "MRY 0\n"),
        ("MXX", "MXX 0 1\n"),
        ("MYY", "MYY 0 1\n"),
        ("MZZ", "MZZ 0 1\n"),
        ("MPP", "MPP X0*Y1*Z2 X3*X4\n"),
    ]
    .into_iter()
    .map(|(name, text)| {
        (
            name,
            Circuit::from_stim_str(text).expect("representative flow metadata circuit"),
        )
    })
    .collect()
}

fn expected_flow_supported_gate_names() -> BTreeSet<&'static str> {
    let mut names = expected_tableau_supported_gate_names();
    names.extend([
        "M", "MX", "MY", "R", "RX", "RY", "MR", "MRX", "MRY", "MXX", "MYY", "MZZ", "MPP", "SPP",
        "SPP_DAG",
    ]);
    names
}

fn expected_tableau_supported_gate_names() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "C_NXYZ",
        "C_NZYX",
        "C_XNYZ",
        "C_XYNZ",
        "C_XYZ",
        "C_ZNYX",
        "C_ZYNX",
        "C_ZYX",
        "CX",
        "CXSWAP",
        "CY",
        "CZ",
        "CZSWAP",
        "H",
        "H_NXY",
        "H_NXZ",
        "H_NYZ",
        "H_XY",
        "H_YZ",
        "I",
        "II",
        "ISWAP",
        "ISWAP_DAG",
        "S",
        "S_DAG",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SWAP",
        "SWAPCX",
        "X",
        "XCX",
        "XCY",
        "XCZ",
        "Y",
        "YCX",
        "YCY",
        "YCZ",
        "Z",
    ])
}

fn expected_decomposition_supported_gate_names() -> BTreeSet<&'static str> {
    BTreeSet::from([
        "C_NXYZ",
        "C_NZYX",
        "C_XNYZ",
        "C_XYNZ",
        "C_XYZ",
        "C_ZNYX",
        "C_ZYNX",
        "C_ZYX",
        "CX",
        "CXSWAP",
        "CY",
        "CZ",
        "CZSWAP",
        "H",
        "H_NXY",
        "H_NXZ",
        "H_NYZ",
        "H_XY",
        "H_YZ",
        "I",
        "II",
        "ISWAP",
        "ISWAP_DAG",
        "M",
        "MPP",
        "MR",
        "MRX",
        "MRY",
        "MX",
        "MXX",
        "MY",
        "MYY",
        "MZZ",
        "R",
        "RX",
        "RY",
        "S",
        "SPP",
        "SPP_DAG",
        "S_DAG",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SWAP",
        "SWAPCX",
        "X",
        "XCX",
        "XCY",
        "XCZ",
        "Y",
        "YCX",
        "YCY",
        "YCZ",
        "Z",
    ])
}

#[test]
fn parses_identity_error_disjoint_probability_lists_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_noisy.cc I_ERROR and II_ERROR examples.
    let circuit = Circuit::from_stim_str(concat!(
        "I_ERROR(0.1, 0.2) 0 2 4\n",
        "II_ERROR(0.1, 0.2) 0 2 4 6\n",
    ))
    .expect("parse identity error probability lists");
    assert_eq!(
        circuit.to_stim_string(),
        concat!("I_ERROR(0.1, 0.2) 0 2 4\n", "II_ERROR(0.1, 0.2) 0 2 4 6\n",)
    );

    let instructions = circuit
        .items()
        .iter()
        .map(|item| match item {
            CircuitItem::Instruction(instruction) => Some(instruction),
            CircuitItem::RepeatBlock(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .expect("identity error fixture should not repeat");
    let mut instructions = instructions.into_iter();
    let i_error = instructions.next().expect("I_ERROR");
    let ii_error = instructions.next().expect("II_ERROR");
    assert!(instructions.next().is_none());
    assert_eq!(
        i_error.probability_arguments().unwrap(),
        Some(vec![
            Probability::try_new(0.1).unwrap(),
            Probability::try_new(0.2).unwrap(),
        ])
    );
    assert_eq!(
        ii_error.probability_arguments().unwrap(),
        Some(vec![
            Probability::try_new(0.1).unwrap(),
            Probability::try_new(0.2).unwrap(),
        ])
    );

    for invalid in [
        "I_ERROR(0.8, 0.4) 0\n",
        "II_ERROR(0.8, 0.4) 0 1\n",
        "I_ERROR(-0.1) 0\n",
        "II_ERROR(2) 0 1\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn bit_target_capability_matches_stim_controlled_gate_flags() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_controlled.cc target flag split.
    for invalid in [
        "XCX rec[-1] 0\n",
        "XCY rec[-1] 0\n",
        "YCX rec[-1] 0\n",
        "YCY rec[-1] 0\n",
        "XCX sweep[0] 0\n",
        "XCY sweep[0] 0\n",
        "YCX sweep[0] 0\n",
        "YCY sweep[0] 0\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }

    for valid in [
        "XCZ 0 rec[-1]\n",
        "YCZ 0 rec[-1]\n",
        "CX rec[-1] 0\n",
        "CY rec[-1] 0\n",
        "CZ rec[-1] 0\n",
        "XCZ 0 sweep[0]\n",
        "YCZ 0 sweep[0]\n",
        "CX sweep[0] 0\n",
        "CY sweep[0] 0\n",
        "CZ sweep[0] 0\n",
    ] {
        assert!(Circuit::from_stim_str(valid).is_ok(), "{valid}");
    }
}
