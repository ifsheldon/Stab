#![allow(
    clippy::expect_used,
    clippy::string_slice,
    reason = "canonical owner tests use fixed compatibility fixtures and inspect exact values"
)]

use std::collections::{BTreeMap, BTreeSet};

use num_complex::Complex32;
use stab_algebra::{Flow, unitary_to_tableau};
use stab_analysis::{
    check_if_circuit_has_unsigned_stabilizer_flows, circuit_to_tableau, circuit_without_noise,
    decomposed_circuit, flattened_circuit, gate_decomposition_to_circuit, gate_flows,
    gate_h_s_cx_m_r_decomposition, gate_has_flows, gate_has_h_s_cx_m_r_decomposition,
    gate_has_tableau, gate_has_unitary_matrix, gate_tableau, gate_unitary_matrix,
};
use stab_model::{Circuit, CircuitItem, Gate};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid circuit fixture")
}

#[test]
fn pf1_gate_tableau_metadata() {
    let h = Gate::from_name("H").expect("H");
    let h_tableau = gate_tableau(h).expect("H tableau");
    assert_eq!(h_tableau.x_output(0).expect("X output").to_string(), "+Z");
    assert_eq!(h_tableau.z_output(0).expect("Z output").to_string(), "+X");

    let cx = Gate::from_name("CX").expect("CX");
    let cx_tableau = gate_tableau(cx).expect("CX tableau");
    assert_eq!(
        [
            cx_tableau.x_output(0).expect("X0").to_string(),
            cx_tableau.z_output(0).expect("Z0").to_string(),
            cx_tableau.x_output(1).expect("X1").to_string(),
            cx_tableau.z_output(1).expect("Z1").to_string(),
        ],
        ["+XX", "+Z_", "+_X", "+ZZ"]
    );
    for gate in Gate::all() {
        assert_eq!(gate_has_tableau(gate), gate_tableau(gate).is_ok());
    }
}

#[test]
fn cq2_circuit_api_flattened_transform() {
    let source = circuit(
        "SHIFT_COORDS(5, 0)\n\
         QUBIT_COORDS(1, 2, 3) 0\n\
         REPEAT[tag] 2 {\n\
             M 0\n\
             DETECTOR(1, 0) rec[-1]\n\
             SHIFT_COORDS(0, 1)\n\
         }\n",
    );
    assert_eq!(
        flattened_circuit(&source)
            .expect("flatten")
            .to_stim_string(),
        concat!(
            "QUBIT_COORDS(6, 2, 3) 0\n",
            "M 0\n",
            "DETECTOR(6, 0) rec[-1]\n",
            "M 0\n",
            "DETECTOR(6, 1) rec[-1]\n",
        )
    );
    assert!(
        flattened_circuit(&circuit("REPEAT 1000001 {\n    H 0\n}\n")).is_err(),
        "materializing above the source-owned expansion limit must fail"
    );
}

#[test]
fn cq2_gate_unitary_matrix_contract() {
    // Adapted from Stim v1.16.0 GateData unitary matrix examples and inverse consistency checks.
    let h = Gate::from_name("H").expect("H");
    let h_scale = f32::sqrt(0.5);
    let h_matrix = gate_unitary_matrix(h).expect("H matrix");
    assert!(gate_has_unitary_matrix(h));
    assert_matrix_close(
        &h_matrix.to_vecs(),
        &[
            &[(h_scale, 0.0), (h_scale, 0.0)],
            &[(h_scale, 0.0), (-h_scale, 0.0)],
        ],
    );

    let iswap = Gate::from_name("ISWAP").expect("ISWAP");
    let iswap_matrix = gate_unitary_matrix(iswap).expect("ISWAP unitary");
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
    let sqrt_xx_matrix = gate_unitary_matrix(sqrt_xx).expect("SQRT_XX unitary");
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
        .filter(|gate| gate_has_unitary_matrix(*gate))
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_unitary_names, expected_unitary_names);

    for &gate_name in &expected_unitary_names {
        let gate = Gate::from_name(gate_name).expect("gate");
        let matrix = gate_unitary_matrix(gate).expect("gate unitary");
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
            gate_tableau(gate).expect("gate tableau"),
            "{gate_name} unitary matrix should convert to the gate tableau"
        );

        let inverse = gate.inverse().expect("unitary inverse");
        let inverse_matrix = gate_unitary_matrix(inverse)
            .expect("inverse unitary")
            .to_vecs();
        let expected_inverse = conjugate_transpose(&matrix_rows);
        assert_matrix_close_matrix(
            &inverse_matrix,
            &expected_inverse,
            1e-6,
            &format!("{gate_name} inverse unitary should be the conjugate transpose"),
        );
    }

    let cx = Gate::from_name("CX").expect("CX");
    let cx_matrix_rows = gate_unitary_matrix(cx).expect("CX matrix").to_vecs();
    let wrong_endian_tableau =
        unitary_to_tableau(&cx_matrix_rows, false).expect("wrong-endian CX is still Clifford");
    assert_ne!(
        wrong_endian_tableau,
        gate_tableau(cx).expect("CX tableau"),
        "CX unitary metadata must use Stim's little-endian qubit order"
    );

    for gate in Gate::all() {
        assert_eq!(
            gate_has_unitary_matrix(gate),
            gate_unitary_matrix(gate).is_ok(),
            "{} has_unitary_matrix should match unitary matrix materialization",
            gate.canonical_name()
        );
    }

    for unsupported in ["MXX", "MPP", "SPP", "SPP_DAG", "M", "DETECTOR", "X_ERROR"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate_has_unitary_matrix(gate), "{unsupported}");
        let error = gate_unitary_matrix(gate).expect_err("reject unsupported unitary matrix data");
        assert!(error.to_string().contains("unitary matrix data"), "{error}");
    }
}

#[test]
fn cq2_circuit_api_without_noise_transform() {
    let source = circuit(
        "H[tag] 0\n\
         X_ERROR(0.25) 0\n\
         HERALDED_ERASE[herald](0.5) 1\n\
         REPEAT[loop] 2 {\n\
             DEPOLARIZE1(0.1) 0\n\
             M[measure](0.2) 0\n\
             DETECTOR[det] rec[-1]\n\
         }\n",
    );
    assert_eq!(
        circuit_without_noise(&source)
            .expect("remove noise")
            .to_stim_string(),
        concat!(
            "H[tag] 0\n",
            "MPAD[herald] 0\n",
            "REPEAT[loop] 2 {\n",
            "    M[measure] 0\n",
            "    DETECTOR[det] rec[-1]\n",
            "}\n",
        )
    );
    assert!(
        source.to_stim_string().contains("X_ERROR"),
        "the transform must not mutate its source"
    );
}

#[test]
fn cq2_gate_decomposition_metadata_contract() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.test.cc and gate_data_*.cc metadata.
    let h = Gate::from_name("H").expect("H");
    assert!(gate_has_h_s_cx_m_r_decomposition(h));
    assert_eq!(
        gate_h_s_cx_m_r_decomposition(h)
            .expect("H decomposition")
            .as_stim_str(),
        "\nH 0\n"
    );
    assert_eq!(
        gate_decomposition_to_circuit(gate_h_s_cx_m_r_decomposition(h).expect("H decomposition"))
            .expect("parse H decomposition")
            .to_stim_string(),
        "H 0\n"
    );

    let cx = Gate::from_name("CX").expect("CX");
    assert_eq!(
        gate_h_s_cx_m_r_decomposition(cx)
            .expect("CX decomposition")
            .as_stim_str(),
        "\nCNOT 0 1\n"
    );

    let mxx = Gate::from_name("MXX").expect("MXX");
    assert_eq!(
        gate_decomposition_to_circuit(
            gate_h_s_cx_m_r_decomposition(mxx).expect("MXX decomposition")
        )
        .expect("parse MXX decomposition")
        .to_stim_string(),
        concat!("CX 0 1\n", "H 0\n", "M 0\n", "H 0\n", "CX 0 1\n")
    );

    let expected_decomposition_names = expected_decomposition_supported_gate_names();
    assert_eq!(expected_decomposition_names.len(), 61);
    let actual_decomposition_names = Gate::all()
        .filter(|gate| gate_has_h_s_cx_m_r_decomposition(*gate))
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
        let decomposition = gate_h_s_cx_m_r_decomposition(gate).expect("gate decomposition");
        assert_eq!(
            decomposition.as_stim_str(),
            *upstream_texts.get(gate_name).expect("upstream text"),
            "{gate_name} decomposition text should match pinned Stim v1.16.0"
        );
        let decomposed = gate_decomposition_to_circuit(decomposition).expect("parse decomposition");
        assert_h_s_cx_m_r_base(&decomposed, gate_name);
    }

    for gate in Gate::all() {
        assert_eq!(
            gate_has_h_s_cx_m_r_decomposition(gate),
            gate_h_s_cx_m_r_decomposition(gate).is_ok(),
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
        assert!(!gate_has_h_s_cx_m_r_decomposition(gate), "{unsupported}");
        let error =
            gate_h_s_cx_m_r_decomposition(gate).expect_err("reject missing decomposition data");
        assert!(error.to_string().contains("decomposition data"), "{error}");
    }

    for gate_name in expected_tableau_supported_gate_names() {
        if matches!(gate_name, "I" | "II") {
            continue;
        }
        let gate = Gate::from_name(gate_name).expect("gate");
        let decomposition = gate_decomposition_to_circuit(
            gate_h_s_cx_m_r_decomposition(gate).expect("unitary gate should have decomposition"),
        )
        .expect("parse decomposition");
        assert_eq!(
            circuit_to_tableau(&decomposition, false, false, false).expect("decomposition tableau"),
            gate_tableau(gate).expect("gate tableau"),
            "{gate_name} decomposition should match gate tableau"
        );
    }

    for gate_name in ["M", "MR", "MXX", "MPP", "SPP", "SPP_DAG"] {
        let gate = Gate::from_name(gate_name).expect("non-tableau decomposition gate");
        assert!(gate_has_h_s_cx_m_r_decomposition(gate), "{gate_name}");
        assert!(
            gate_tableau(gate).is_err(),
            "{gate_name} decomposition metadata should not imply tableau metadata"
        );
    }
}

#[test]
fn cq2_gate_flow_metadata_contract() {
    // Adapted from Stim v1.16.0 GateData flow examples and gate_data stabilizer-flow checks.
    let h = Gate::from_name("H").expect("H");
    assert!(gate_has_flows(h));
    assert_eq!(
        flow_texts(gate_flows(h).expect("H flows")),
        ["X -> Z", "Z -> X"].map(String::from).to_vec()
    );

    let iswap = Gate::from_name("ISWAP").expect("ISWAP");
    assert_eq!(
        flow_texts(gate_flows(iswap).expect("ISWAP flows")),
        ["X_ -> ZY", "Z_ -> _Z", "_X -> YZ", "_Z -> Z_"]
            .map(String::from)
            .to_vec()
    );

    let sqrt_xx = Gate::from_name("SQRT_XX").expect("SQRT_XX");
    assert_eq!(
        flow_texts(gate_flows(sqrt_xx).expect("SQRT_XX flows")),
        ["X_ -> X_", "Z_ -> -YX", "_X -> _X", "_Z -> -XY"]
            .map(String::from)
            .to_vec()
    );

    let measurement = Gate::from_name("M").expect("M");
    assert_eq!(
        flow_texts(gate_flows(measurement).expect("M flows")),
        ["Z -> rec[-1]", "Z -> Z"].map(String::from).to_vec()
    );

    let pair_measurement = Gate::from_name("MXX").expect("MXX");
    assert_eq!(
        flow_texts(gate_flows(pair_measurement).expect("MXX flows")),
        ["X_ -> X_", "_X -> _X", "ZZ -> ZZ", "XX -> rec[-1]"]
            .map(String::from)
            .to_vec()
    );

    let pauli_product_measurement = Gate::from_name("MPP").expect("MPP");
    assert_eq!(
        flow_texts(gate_flows(pauli_product_measurement).expect("MPP flows")),
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
        flow_texts(gate_flows(pauli_product).expect("SPP flows")),
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
        .filter(|gate| gate_has_flows(*gate))
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_flow_names, expected_flow_names);

    for gate_name in expected_tableau_supported_gate_names() {
        let gate = Gate::from_name(gate_name).expect("gate");
        let flows = gate_flows(gate).expect("gate flows");
        assert_eq!(
            flows.len(),
            gate_tableau(gate).expect("gate tableau").len() * 2,
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
        let flows = gate_flows(gate).expect("gate flows");
        assert!(
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows)
                .into_iter()
                .all(|ok| ok),
            "{gate_name} flows should be satisfied by the representative circuit"
        );
    }

    for unsupported in ["MPAD", "DETECTOR", "X_ERROR", "PAULI_CHANNEL_1"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate_has_flows(gate), "{unsupported}");
        let error = gate_flows(gate).expect_err("reject unsupported flow data");
        assert!(error.to_string().contains("flow metadata"), "{error}");
    }
}

#[test]
fn cq2_circuit_api_decomposed_transform() {
    let source = circuit(
        "ISWAP[tag] 0 1\n\
         MPP[measure] X0*X1 Y0*Y1\n\
         DETECTOR[det] rec[-1]\n",
    );
    let decomposed = decomposed_circuit(&source).expect("decompose");
    assert!(
        decomposed
            .items()
            .iter()
            .filter_map(CircuitItem::as_instruction)
            .all(|instruction| !matches!(
                instruction.gate().canonical_name(),
                "ISWAP" | "MPP" | "MXX" | "MYY" | "MZZ"
            ))
    );
    assert!(decomposed.to_stim_string().contains("[measure]"));
    assert!(decomposed.to_stim_string().contains("DETECTOR[det]"));
    assert!(
        decomposed_circuit(&circuit("SPP X0*Z0\n")).is_err(),
        "anti-Hermitian Pauli products must remain rejected"
    );
}

fn flow_texts(flows: Vec<Flow>) -> Vec<String> {
    flows.into_iter().map(|flow| flow.to_string()).collect()
}

fn assert_h_s_cx_m_r_base(circuit: &Circuit, gate_name: &str) {
    for item in circuit.items() {
        let instruction = item
            .as_instruction()
            .expect("gate decomposition should not contain repeat blocks");
        assert!(
            matches!(
                instruction.gate().canonical_name(),
                "H" | "S" | "CX" | "M" | "R"
            ),
            "{gate_name} decomposition used non-base gate {}",
            instruction.gate().canonical_name()
        );
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
        .get(gate_tableau(gate).expect("gate tableau").len())
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
