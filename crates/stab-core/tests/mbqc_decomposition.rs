#![allow(
    clippy::expect_used,
    reason = "M6 MBQC decomposition parity tests mirror compact upstream table entries"
)]

use stab_core::{Gate, mbqc_decomposition};

#[test]
fn mbqc_decomposition_returns_none_for_non_flow_gates_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/mbqc_decomposition.cc.
    for gate in [
        "DETECTOR",
        "OBSERVABLE_INCLUDE",
        "TICK",
        "QUBIT_COORDS",
        "SHIFT_COORDS",
        "MPAD",
        "X_ERROR",
        "PAULI_CHANNEL_1",
        "HERALDED_ERASE",
    ] {
        assert!(
            mbqc_decomposition(gate_by_name(gate))
                .expect("decomposition lookup")
                .is_none(),
            "{gate}"
        );
    }
}

#[test]
fn mbqc_decomposition_parses_selected_measurement_entries_like_stim() {
    assert_eq!(
        mbqc_string("M"),
        "M 0\n",
        "MZ is canonicalized to M by the Rust circuit printer"
    );
    assert_eq!(mbqc_string("MRX"), "MX 0\nCZ rec[-1] 0\n");
    assert_eq!(mbqc_string("R"), "M 0\nCX rec[-1] 0\n");
}

#[test]
fn mbqc_decomposition_parses_selected_unitary_entries_like_stim() {
    assert_eq!(mbqc_string("I"), "");
    assert_eq!(mbqc_string("X"), "X 0\n");
    assert_eq!(
        mbqc_string("H_XY"),
        concat!(
            "MX 1\n",
            "MZZ 0 1\n",
            "MY 1\n",
            "X 0\n",
            "CZ rec[-3] 0 rec[-2] 0 rec[-1] 0\n",
        )
    );
    assert_eq!(
        mbqc_string("S"),
        concat!(
            "MY 1\n",
            "MZZ 0 1\n",
            "MX 1\n",
            "CZ rec[-3] 0 rec[-2] 0 rec[-1] 0\n",
        )
    );
    assert_eq!(
        mbqc_string("CX"),
        concat!(
            "MX 2\n",
            "MZZ 0 2\n",
            "MXX 1 2\n",
            "M 2\n",
            "CX rec[-3] 1 rec[-1] 1\n",
            "CZ rec[-4] 0 rec[-2] 0\n",
        )
    );
}

#[test]
fn mbqc_decomposition_current_subset_is_valid_stim_text() {
    for (gate, expected) in supported_mbqc_cases() {
        let decomposition = mbqc_decomposition(gate_by_name(gate)).expect("decomposition lookup");
        assert!(
            decomposition.is_some(),
            "{gate} should have a decomposition"
        );
        let circuit = decomposition.expect("checked decomposition");
        let printed = circuit.to_stim_string();
        assert_eq!(printed, expected, "{gate}");
        let reparsed = stab_core::Circuit::from_stim_str(&printed).expect("reparse decomposition");
        assert_eq!(reparsed, circuit, "{gate}");
    }
}

#[test]
fn cq2_circuit_api_mbqc_decomposition_contract_matches_selected_stim_scope() {
    mbqc_decomposition_returns_none_for_non_flow_gates_like_stim();
    mbqc_decomposition_parses_selected_measurement_entries_like_stim();
    mbqc_decomposition_parses_selected_unitary_entries_like_stim();
    mbqc_decomposition_current_subset_is_valid_stim_text();
}

fn mbqc_string(gate: &str) -> String {
    mbqc_decomposition(gate_by_name(gate))
        .expect("decomposition lookup")
        .expect("decomposition")
        .to_stim_string()
}

fn gate_by_name(name: &str) -> Gate {
    Gate::from_name(name).expect("gate")
}

fn supported_mbqc_cases() -> [(&'static str, &'static str); 17] {
    [
        ("MX", "MX 0\n"),
        ("MY", "MY 0\n"),
        ("M", "M 0\n"),
        ("MRX", "MX 0\nCZ rec[-1] 0\n"),
        ("MRY", "MY 0\nCX rec[-1] 0\n"),
        ("MR", "M 0\nCX rec[-1] 0\n"),
        ("RX", "MX 0\nCZ rec[-1] 0\n"),
        ("RY", "MY 0\nCX rec[-1] 0\n"),
        ("R", "M 0\nCX rec[-1] 0\n"),
        (
            "H_XY",
            concat!(
                "MX 1\n",
                "MZZ 0 1\n",
                "MY 1\n",
                "X 0\n",
                "CZ rec[-3] 0 rec[-2] 0 rec[-1] 0\n",
            ),
        ),
        (
            "S",
            concat!(
                "MY 1\n",
                "MZZ 0 1\n",
                "MX 1\n",
                "CZ rec[-3] 0 rec[-2] 0 rec[-1] 0\n",
            ),
        ),
        ("I", ""),
        ("II", ""),
        ("X", "X 0\n"),
        ("Y", "Y 0\n"),
        ("Z", "Z 0\n"),
        (
            "CX",
            concat!(
                "MX 2\n",
                "MZZ 0 2\n",
                "MXX 1 2\n",
                "M 2\n",
                "CX rec[-3] 1 rec[-1] 1\n",
                "CZ rec[-4] 0 rec[-2] 0\n",
            ),
        ),
    ]
}
