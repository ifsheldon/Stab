#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "unit tests use direct fixed-width tracker assertions for compact diagnostics"
)]

use super::*;
use crate::{Gate, MeasureRecordOffset, measurement_record_count};

fn tracker_from_pauli_text(text: &str) -> SparseReverseFrameTracker {
    let mut tracker = SparseReverseFrameTracker::new(text.len(), 0, 0, true);
    let sensitivity = BTreeSet::from([DemTarget::logical_observable(0).unwrap()]);
    for (index, character) in text.chars().enumerate() {
        let qubit = QubitId::new(u32::try_from(index).unwrap()).unwrap();
        match character {
            'I' => {}
            'X' => tracker.toggle_xs(qubit, &sensitivity).unwrap(),
            'Y' => {
                tracker.toggle_xs(qubit, &sensitivity).unwrap();
                tracker.toggle_zs(qubit, &sensitivity).unwrap();
            }
            'Z' => tracker.toggle_zs(qubit, &sensitivity).unwrap(),
            _ => panic!("unexpected Pauli text character {character}"),
        }
    }
    tracker
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).unwrap()
}

fn instruction(text: &str) -> CircuitInstruction {
    let parsed = circuit(text);
    let Some(CircuitItem::Instruction(instruction)) = parsed.items().first() else {
        panic!("expected one instruction in {text}");
    };
    instruction.clone()
}

fn assert_undo_tableau(instruction_text: &str, cases: &[&str]) {
    let instruction = instruction(instruction_text);
    for &case in cases {
        let (input, expected) = case.split_once(' ').unwrap();
        let mut actual = tracker_from_pauli_text(input);
        actual.undo_instruction(&instruction).unwrap();
        assert_eq!(actual, tracker_from_pauli_text(expected), "{input}");
    }
}

fn text_from_bases(bases: impl IntoIterator<Item = PauliBasis>) -> String {
    bases
        .into_iter()
        .map(|basis| match basis {
            PauliBasis::I => 'I',
            PauliBasis::X => 'X',
            PauliBasis::Y => 'Y',
            PauliBasis::Z => 'Z',
        })
        .collect()
}

fn q(id: u32) -> Target {
    Target::qubit(QubitId::new(id).unwrap(), false)
}

fn rec(offset: i32) -> Target {
    Target::measurement_record(MeasureRecordOffset::try_new(offset).unwrap())
}

fn single_pauli_set(id: u64) -> BTreeSet<DemTarget> {
    BTreeSet::from([DemTarget::logical_observable(id).unwrap()])
}

#[test]
fn sparse_rev_frame_tracker_undo_tableau_cx_subset() {
    assert_undo_tableau(
        "CX 0 1\n",
        &["II II", "IZ ZZ", "ZI ZI", "XI XX", "IX IX", "YY XZ"],
    );
}

#[test]
fn sparse_rev_frame_tracker_undo_tableau_cy_subset() {
    assert_undo_tableau(
        "CY 0 1\n",
        &[
            "II II", "IX ZX", "IY IY", "IZ ZZ", "XI XY", "XX YZ", "XY XI", "XZ YX", "YI YY",
            "YX XZ", "YY YI", "YZ XX", "ZI ZI", "ZX IX", "ZY ZY", "ZZ IZ",
        ],
    );
}

#[test]
fn sparse_rev_frame_tracker_undo_single_qubit_cliffords_match_tableau() {
    let target = DemTarget::logical_observable(0).unwrap();
    let basis_cases = [
        ("I", PauliBasis::I),
        ("X", PauliBasis::X),
        ("Y", PauliBasis::Y),
        ("Z", PauliBasis::Z),
    ];
    for gate in SingleQubitClifford::all() {
        let parsed_gate = Gate::from_name(gate.canonical_name()).unwrap();
        let inverse_gate = parsed_gate.best_candidate_inverse().unwrap();
        let expected_tableau = circuit(&format!("{} 0\n", inverse_gate.canonical_name()))
            .to_tableau(false, false, false)
            .unwrap();
        let instruction = instruction(&format!("{} 0\n", gate.canonical_name()));
        for (input_text, input_basis) in basis_cases {
            let mut actual = tracker_from_pauli_text(input_text);
            actual.undo_instruction(&instruction).unwrap();
            let expected = expected_tableau
                .apply(&PauliString::from_bases(PauliSign::Plus, [input_basis]))
                .unwrap()
                .get(0)
                .unwrap();
            let actual = actual.region_for_target(target).unwrap().get(0).unwrap();
            assert_eq!(actual, expected, "{} {input_text}", gate.canonical_name());
        }
    }
}

#[test]
fn sparse_rev_frame_tracker_undo_fixed_two_qubit_gates_match_tableau() {
    let target = DemTarget::logical_observable(0).unwrap();
    let basis_cases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    let gate_names = [
        "II",
        "XCX",
        "XCY",
        "XCZ",
        "YCX",
        "YCY",
        "YCZ",
        "SWAP",
        "ISWAP",
        "ISWAP_DAG",
        "CXSWAP",
        "SWAPCX",
        "CZSWAP",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
    ];
    for gate_name in gate_names {
        let gate = Gate::from_name(gate_name).unwrap();
        let inverse_gate = gate.inverse().unwrap();
        let expected_tableau = circuit(&format!("{} 0 1\n", inverse_gate.canonical_name()))
            .to_tableau(false, false, false)
            .unwrap();
        let instruction = instruction(&format!("{gate_name} 0 1\n"));
        for left_basis in basis_cases {
            for right_basis in basis_cases {
                let input_text = text_from_bases([left_basis, right_basis]);
                let mut actual = tracker_from_pauli_text(&input_text);
                actual.undo_instruction(&instruction).unwrap();
                let expected = expected_tableau
                    .apply(&PauliString::from_bases(
                        PauliSign::Plus,
                        [left_basis, right_basis],
                    ))
                    .unwrap();
                let actual = actual.region_for_target(target).unwrap();
                assert_eq!(
                    actual.get(0).unwrap(),
                    expected.get(0).unwrap(),
                    "{gate_name} {input_text} left"
                );
                assert_eq!(
                    actual.get(1).unwrap(),
                    expected.get(1).unwrap(),
                    "{gate_name} {input_text} right"
                );
            }
        }
    }
}

#[test]
fn sparse_rev_frame_tracker_measurements_preserve_matching_basis() {
    for (gate, input) in [("MX", "XXX"), ("MY", "YYY"), ("M", "ZZZ")] {
        let mut actual = tracker_from_pauli_text(input);
        actual.measurement_count = 2;
        actual
            .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
            .unwrap();
        let mut expected = tracker_from_pauli_text(input);
        expected.measurement_count = 0;
        assert_eq!(actual, expected);
    }
}

#[test]
fn sparse_rev_frame_tracker_measurements_reject_anticommuting_basis_without_mutation() {
    for (gate, input) in [("MX", "XIZ"), ("MY", "YIZ"), ("M", "YIZ")] {
        let mut actual = tracker_from_pauli_text(input);
        actual.measurement_count = 2;
        let before = actual.clone();
        let err = actual
            .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
            .unwrap_err();
        assert!(err.to_string().contains("anti-commuted"));
        assert_eq!(actual, before);
    }
}

#[test]
fn sparse_rev_frame_tracker_measure_resets_clear_then_move_feedback() {
    let mut actual = tracker_from_pauli_text("XXX");
    actual.measurement_count = 2;
    actual.undo_instruction(&instruction("MRX 0 2\n")).unwrap();
    let mut expected = tracker_from_pauli_text("IXI");
    expected.measurement_count = 0;
    assert_eq!(actual, expected);

    let mut actual = tracker_from_pauli_text("III");
    actual.measurement_count = 2;
    actual.rec_bits.insert(
        0,
        BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
    );
    actual.undo_instruction(&instruction("MRX 0 2\n")).unwrap();
    let mut expected = tracker_from_pauli_text("XII");
    expected.measurement_count = 0;
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_feedback_from_measurement_subset() {
    for (gate, expected_text) in [("MX", "XII"), ("MY", "YII"), ("M", "ZII")] {
        let mut actual = tracker_from_pauli_text("III");
        actual.measurement_count = 2;
        actual.rec_bits.insert(
            0,
            BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
        );
        actual
            .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
            .unwrap();
        let mut expected = tracker_from_pauli_text(expected_text);
        expected.measurement_count = 0;
        assert_eq!(actual, expected);
    }
}

#[test]
fn sparse_rev_frame_tracker_feedback_into_measurement_subset() {
    let target = Gate::from_name("CX").unwrap();
    let cx = CircuitInstruction::new(target, Vec::new(), vec![rec(-5), q(0)], None).unwrap();
    let mut actual = tracker_from_pauli_text("ZII");
    actual.measurement_count = 12;
    actual.undo_instruction(&cx).unwrap();

    let mut expected = tracker_from_pauli_text("ZII");
    expected.measurement_count = 12;
    expected.rec_bits.insert(
        7,
        BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
    );
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_pair_measurements_subset() {
    for (gate, expected_text) in [("MXX", "XXI"), ("MYY", "YYI"), ("MZZ", "ZZI")] {
        let mut actual = tracker_from_pauli_text("III");
        actual.measurement_count = 2;
        actual.rec_bits.insert(
            1,
            BTreeSet::from([DemTarget::logical_observable(0).unwrap()]),
        );
        actual
            .undo_instruction(&instruction(&format!("{gate} 0 1\n")))
            .unwrap();

        let mut expected = tracker_from_pauli_text(expected_text);
        expected.measurement_count = 1;
        assert_eq!(actual, expected);
    }
}

#[test]
fn sparse_rev_frame_tracker_mpp_measurements_subset() {
    let mut actual = SparseReverseFrameTracker::new(6, 2, 0, true);
    actual.rec_bits.insert(0, single_pauli_set(0));
    actual.rec_bits.insert(1, single_pauli_set(1));
    actual
        .undo_instruction(&instruction("MPP X0*Y1*Z2 Z5\n"))
        .unwrap();

    let mut expected = tracker_from_pauli_text("XYZIIZ");
    expected.xs[0] = single_pauli_set(0);
    expected.xs[1] = single_pauli_set(0);
    expected.zs[1] = single_pauli_set(0);
    expected.zs[2] = single_pauli_set(0);
    expected.zs[5] = single_pauli_set(1);
    expected.measurement_count = 0;
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_rejects_anti_hermitian_mpp_products() {
    let mut actual = SparseReverseFrameTracker::new(1, 1, 0, true);
    let error = actual
        .undo_instruction(&instruction("MPP X0*Z0\n"))
        .unwrap_err();

    assert!(error.to_string().contains("anti-Hermitian"));
}

#[test]
fn sparse_rev_frame_tracker_undo_circuit_feedback_subset() {
    let circuit = circuit(
        "
        MR 0
        CX rec[-1] 0
        M 0
        DETECTOR rec[-1]
        ",
    );
    let mut actual = SparseReverseFrameTracker::new(
        circuit.count_qubits(),
        measurement_record_count(&circuit).unwrap(),
        1,
        true,
    );
    actual.undo_circuit(&circuit).unwrap();

    let mut expected = SparseReverseFrameTracker::new(1, 0, 0, true);
    expected.zs[0].insert(DemTarget::relative_detector(0).unwrap());
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_tracks_anticommutation_when_requested() {
    let circuit = circuit(
        "
        RX 0
        M 0
        DETECTOR rec[-1]
        ",
    );
    let mut tracker = SparseReverseFrameTracker::new(
        circuit.count_qubits(),
        measurement_record_count(&circuit).unwrap(),
        1,
        false,
    );
    tracker.undo_circuit(&circuit).unwrap();

    assert_eq!(
        tracker.anticommutations,
        BTreeSet::from([Anticommutation {
            target: DemTarget::relative_detector(0).unwrap(),
            location: TrackerLocation {
                qubit: QubitId::new(0).unwrap(),
                basis: TrackerBasis::X,
            },
        }])
    );
}

#[test]
fn sparse_rev_frame_tracker_fails_anticommutation_by_default() {
    let circuit = circuit(
        "
        RX 0
        M 0
        DETECTOR rec[-1]
        ",
    );
    let mut tracker = SparseReverseFrameTracker::new(
        circuit.count_qubits(),
        measurement_record_count(&circuit).unwrap(),
        1,
        true,
    );
    assert!(tracker.undo_circuit(&circuit).is_err());
}

#[test]
fn sparse_rev_frame_tracker_observable_include_paulis_subset() {
    let mut tracker = SparseReverseFrameTracker::new(4, 4, 4, true);
    tracker
        .undo_circuit(&circuit("OBSERVABLE_INCLUDE(5) X1 Y2 Z3 rec[-1]\n"))
        .unwrap();

    assert!(tracker.xs[0].is_empty());
    assert!(tracker.zs[0].is_empty());
    assert_eq!(tracker.xs[1], single_pauli_set(5));
    assert!(tracker.zs[1].is_empty());
    assert_eq!(tracker.xs[2], single_pauli_set(5));
    assert_eq!(tracker.zs[2], single_pauli_set(5));
    assert!(tracker.xs[3].is_empty());
    assert_eq!(tracker.zs[3], single_pauli_set(5));
    assert_eq!(tracker.rec_bits.get(&3), Some(&single_pauli_set(5)));
}

#[test]
fn sparse_rev_frame_tracker_unrolls_repeat_blocks_for_now() {
    let circuit = circuit(
        "
        REPEAT 2 {
            M 0
            DETECTOR rec[-1]
        }
        ",
    );
    let mut tracker = SparseReverseFrameTracker::new(
        circuit.count_qubits(),
        measurement_record_count(&circuit).unwrap(),
        2,
        true,
    );
    tracker.undo_circuit(&circuit).unwrap();

    let mut expected = SparseReverseFrameTracker::new(1, 0, 0, true);
    expected.zs[0].insert(DemTarget::relative_detector(0).unwrap());
    expected.zs[0].insert(DemTarget::relative_detector(1).unwrap());
    assert_eq!(tracker, expected);
}

#[test]
fn sparse_rev_frame_tracker_accepts_mpad_and_discards_record_sensitivity() {
    let mut actual = tracker_from_pauli_text("IIZ");
    actual.measurement_count = 2;
    actual.rec_bits.insert(
        1,
        BTreeSet::from([DemTarget::relative_detector(5).unwrap()]),
    );
    actual.undo_instruction(&instruction("MPAD 0\n")).unwrap();

    let mut expected = tracker_from_pauli_text("IIZ");
    expected.measurement_count = 1;
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_target_pauli_mapping_is_explicit() {
    assert_eq!(TrackerBasis::from_pauli(Pauli::X), TrackerBasis::X);
    assert_eq!(TrackerBasis::from_pauli(Pauli::Y), TrackerBasis::Y);
    assert_eq!(TrackerBasis::from_pauli(Pauli::Z), TrackerBasis::Z);
}
