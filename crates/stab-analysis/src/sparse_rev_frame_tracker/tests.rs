#![allow(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::unwrap_used,
    reason = "unit tests use direct fixed-width tracker assertions for compact diagnostics"
)]

use super::*;
use stab_algebra::{PauliSign, SingleQubitClifford};
use stab_model::{CircuitItem, Gate, MeasureRecordOffset};

use crate::{circuit_to_tableau, decomposed_circuit};

fn measurement_record_count(circuit: &Circuit) -> AnalysisResult<usize> {
    usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::ReverseFrameTracking,
            "test circuit measurement count does not fit usize",
        )
    })
}

fn tracker_from_pauli_text(text: &str) -> SparseReverseFrameTracker {
    let mut tracker = SparseReverseFrameTracker::new(text.len(), 0, 0, AnticommutationPolicy::Fail);
    let sensitivity = [DemTarget::logical_observable(0).unwrap()];
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

fn qid(id: u32) -> QubitId {
    QubitId::new(id).unwrap()
}

fn rec(offset: i32) -> Target {
    Target::measurement_record(MeasureRecordOffset::try_new(offset).unwrap())
}

fn single_pauli_set(id: u64) -> TargetXorSet {
    TargetXorSet::from_target(DemTarget::logical_observable(id).unwrap())
}

fn detector_set(id: u64) -> TargetXorSet {
    TargetXorSet::from_target(DemTarget::relative_detector(id).unwrap())
}

#[test]
fn sparse_rev_frame_tracker_allocates_only_active_qubit_entries() {
    const HIGH_QUBIT: u32 = 1_000_000;
    let target = DemTarget::logical_observable(0).unwrap();
    let mut tracker =
        SparseReverseFrameTracker::new(HIGH_QUBIT as usize + 1, 0, 0, AnticommutationPolicy::Fail);

    assert!(tracker.xs.is_empty());
    assert!(tracker.zs.is_empty());
    tracker.toggle_xs(qid(HIGH_QUBIT), &[target]).unwrap();
    assert_eq!(tracker.xs.len(), 1);
    assert!(tracker.xs_for(qid(HIGH_QUBIT)).unwrap().contains(&target));
    assert!(tracker.xs_for(qid(0)).unwrap().is_empty());

    tracker.toggle_xs(qid(HIGH_QUBIT), &[target]).unwrap();
    assert!(tracker.xs.is_empty());
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
        let inverse_gate = parsed_gate.generalized_inverse();
        let expected_circuit = circuit(&format!("{} 0\n", inverse_gate.canonical_name()));
        let expected_tableau = circuit_to_tableau(&expected_circuit, false, false, false).unwrap();
        let instruction = instruction(&format!("{} 0\n", gate.canonical_name()));
        for (input_text, input_basis) in basis_cases {
            let mut actual = tracker_from_pauli_text(input_text);
            actual.undo_instruction(&instruction).unwrap();
            let expected = expected_tableau
                .apply(&stab_algebra::advanced::pauli_from_bases_unbudgeted(
                    PauliSign::Plus,
                    [input_basis],
                ))
                .unwrap()
                .get(0)
                .unwrap();
            let actual = actual
                .region_for_target(target, MemoryBudget::DEFAULT)
                .unwrap()
                .get(0)
                .unwrap();
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
        let expected_circuit = circuit(&format!("{} 0 1\n", inverse_gate.canonical_name()));
        let expected_tableau = circuit_to_tableau(&expected_circuit, false, false, false).unwrap();
        let instruction = instruction(&format!("{gate_name} 0 1\n"));
        for left_basis in basis_cases {
            for right_basis in basis_cases {
                let input_text = text_from_bases([left_basis, right_basis]);
                let mut actual = tracker_from_pauli_text(&input_text);
                actual.undo_instruction(&instruction).unwrap();
                let expected = expected_tableau
                    .apply(&stab_algebra::advanced::pauli_from_bases_unbudgeted(
                        PauliSign::Plus,
                        [left_basis, right_basis],
                    ))
                    .unwrap();
                let actual = actual
                    .region_for_target(target, MemoryBudget::DEFAULT)
                    .unwrap();
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
fn pf6_sparse_rev_spp_matches_decomposed_tableau_unsigned() {
    let target = DemTarget::logical_observable(0).unwrap();
    let basis_cases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    for (gate_name, inverse_name) in [("SPP", "SPP_DAG"), ("SPP_DAG", "SPP")] {
        let instruction = instruction(&format!("{gate_name} X0*Y1*Z2\n"));
        let expected_circuit = circuit(&format!("{inverse_name} X0*Y1*Z2\n"));
        let expected_circuit = decomposed_circuit(&expected_circuit).unwrap();
        let expected_tableau = circuit_to_tableau(&expected_circuit, false, false, false).unwrap();
        for left_basis in basis_cases {
            for middle_basis in basis_cases {
                for right_basis in basis_cases {
                    let input_text = text_from_bases([left_basis, middle_basis, right_basis]);
                    let mut actual = tracker_from_pauli_text(&input_text);
                    actual.undo_instruction(&instruction).unwrap();
                    let expected = expected_tableau
                        .apply(&stab_algebra::advanced::pauli_from_bases_unbudgeted(
                            PauliSign::Plus,
                            [left_basis, middle_basis, right_basis],
                        ))
                        .unwrap();
                    let actual = actual
                        .region_for_target(target, MemoryBudget::DEFAULT)
                        .unwrap();
                    for index in 0..3 {
                        assert_eq!(
                            actual.get(index).unwrap(),
                            expected.get(index).unwrap(),
                            "{gate_name} {input_text} qubit {index}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn pf6_sparse_rev_spp_handles_multiple_groups_and_inverted_products() {
    let instruction_text = "SPP X0*X1 !Z1*Z2\n";
    let instruction = instruction(instruction_text);
    let expected_circuit = circuit(instruction_text);
    let expected_circuit = decomposed_circuit(&expected_circuit).unwrap();
    let expected_tableau = circuit_to_tableau(&expected_circuit, false, false, false)
        .unwrap()
        .inverse()
        .unwrap();
    let mut actual = tracker_from_pauli_text("ZYX");
    actual.undo_instruction(&instruction).unwrap();

    let expected = expected_tableau
        .apply(&stab_algebra::advanced::pauli_from_bases_unbudgeted(
            PauliSign::Plus,
            [PauliBasis::Z, PauliBasis::Y, PauliBasis::X],
        ))
        .unwrap();
    let actual = actual
        .region_for_target(
            DemTarget::logical_observable(0).unwrap(),
            MemoryBudget::DEFAULT,
        )
        .unwrap();
    for index in 0..3 {
        assert_eq!(
            actual.get(index).unwrap(),
            expected.get(index).unwrap(),
            "multi-group SPP qubit {index}"
        );
    }
}

#[test]
fn pf6_sparse_rev_spp_rejects_anti_hermitian_products() {
    let mut actual = tracker_from_pauli_text("Z");
    let error = actual
        .undo_instruction(&instruction("SPP X0*Z0\n"))
        .unwrap_err();

    assert!(error.to_string().contains("anti-Hermitian"));
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
fn sparse_rev_frame_tracker_measurements_reject_anticommuting_basis() {
    // Like vendor ErrorAnalyzer::undo_M*_with_context, targets resolve one at a time, so the
    // tracker state after a rejection is unspecified; callers discard it.
    for (gate, input) in [("MX", "XIZ"), ("MY", "YIZ"), ("M", "YIZ")] {
        let mut actual = tracker_from_pauli_text(input);
        actual.measurement_count = 2;
        let err = actual
            .undo_instruction(&instruction(&format!("{gate} 0 2\n")))
            .unwrap_err();
        assert!(err.to_string().contains("anti-commuted"));
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
        TargetXorSet::from_target(DemTarget::logical_observable(0).unwrap()),
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
            TargetXorSet::from_target(DemTarget::logical_observable(0).unwrap()),
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
    for (gate, targets, pauli_text) in [
        ("CX", vec![rec(-5), q(0)], "ZII"),
        ("CY", vec![rec(-5), q(0)], "XII"),
        ("CZ", vec![rec(-5), q(0)], "XII"),
        ("XCZ", vec![q(0), rec(-5)], "ZII"),
        ("YCZ", vec![q(0), rec(-5)], "XII"),
    ] {
        let target = Gate::from_name(gate).unwrap();
        let instruction = CircuitInstruction::new(target, Vec::new(), targets, None).unwrap();
        let mut actual = tracker_from_pauli_text(pauli_text);
        actual.measurement_count = 12;
        actual.undo_instruction(&instruction).unwrap();

        let mut expected = tracker_from_pauli_text(pauli_text);
        expected.measurement_count = 12;
        expected.rec_bits.insert(
            7,
            TargetXorSet::from_target(DemTarget::logical_observable(0).unwrap()),
        );
        assert_eq!(actual, expected, "{gate}");
    }
}

#[test]
fn sparse_rev_frame_tracker_rejects_invalid_feedback_target_positions() {
    for (text, expected) in [
        ("CX 0 rec[-5]\n", "CX target rec[-5] is not a qubit"),
        ("CY 0 rec[-5]\n", "CY target rec[-5] is not a qubit"),
        ("XCZ rec[-5] 0\n", "XCZ target rec[-5] is not a qubit"),
        ("YCZ rec[-5] 0\n", "YCZ target rec[-5] is not a qubit"),
    ] {
        // Only the analyzer surface pins the vendor tracker's rejection of a
        // non-qubit Pauli slot; flow-mode trackers keep the PFM-B4 contract.
        let mut tracker = SparseReverseFrameTracker::new_for_error_analysis(3, 12, 0, false);
        tracker.measurement_count = 12;
        let error = match tracker.undo_instruction(&instruction(text)) {
            Ok(_) => panic!("accepted invalid feedback target position in {text:?}"),
            Err(error) => error,
        };
        assert!(error.to_string().contains(expected), "{error}");

        let mut flow_tracker = tracker_from_pauli_text("ZII");
        flow_tracker.measurement_count = 12;
        assert!(
            flow_tracker.undo_instruction(&instruction(text)).is_ok(),
            "flow-mode tracker keeps the lenient PFM-B4 contract for {text:?}"
        );
    }
}

#[test]
fn sparse_rev_frame_tracker_pair_measurements_subset() {
    for (gate, expected_text) in [("MXX", "XXI"), ("MYY", "YYI"), ("MZZ", "ZZI")] {
        let mut actual = tracker_from_pauli_text("III");
        actual.measurement_count = 2;
        actual.rec_bits.insert(
            1,
            TargetXorSet::from_target(DemTarget::logical_observable(0).unwrap()),
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
    let mut actual = SparseReverseFrameTracker::new(6, 2, 0, AnticommutationPolicy::Fail);
    actual.rec_bits.insert(0, single_pauli_set(0));
    actual.rec_bits.insert(1, single_pauli_set(1));
    actual
        .undo_instruction(&instruction("MPP X0*Y1*Z2 Z5\n"))
        .unwrap();

    let mut expected = tracker_from_pauli_text("XYZIIZ");
    expected.xs.insert(qid(0), single_pauli_set(0));
    expected.xs.insert(qid(1), single_pauli_set(0));
    expected.zs.insert(qid(1), single_pauli_set(0));
    expected.zs.insert(qid(2), single_pauli_set(0));
    expected.zs.insert(qid(5), single_pauli_set(1));
    expected.measurement_count = 0;
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_rejects_anti_hermitian_mpp_products() {
    let mut actual = SparseReverseFrameTracker::new(1, 1, 0, AnticommutationPolicy::Fail);
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
        AnticommutationPolicy::Fail,
    );
    actual.undo_circuit(&circuit).unwrap();

    let mut expected = SparseReverseFrameTracker::new(1, 0, 0, AnticommutationPolicy::Fail);
    expected
        .zs
        .entry(qid(0))
        .or_default()
        .xor_item(DemTarget::relative_detector(0).unwrap());
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
        AnticommutationPolicy::Record,
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
        AnticommutationPolicy::Fail,
    );
    assert!(tracker.undo_circuit(&circuit).is_err());
}

#[test]
fn sparse_rev_frame_tracker_observable_include_paulis_subset() {
    let mut tracker = SparseReverseFrameTracker::new(4, 4, 4, AnticommutationPolicy::Fail);
    tracker
        .undo_circuit(&circuit("OBSERVABLE_INCLUDE(5) X1 Y2 Z3 rec[-1]\n"))
        .unwrap();

    assert!(tracker.xs_for(qid(0)).unwrap().is_empty());
    assert!(tracker.zs_for(qid(0)).unwrap().is_empty());
    assert_eq!(
        tracker.xs_for(qid(1)).unwrap(),
        single_pauli_set(5).as_slice()
    );
    assert!(tracker.zs_for(qid(1)).unwrap().is_empty());
    assert_eq!(
        tracker.xs_for(qid(2)).unwrap(),
        single_pauli_set(5).as_slice()
    );
    assert_eq!(
        tracker.zs_for(qid(2)).unwrap(),
        single_pauli_set(5).as_slice()
    );
    assert!(tracker.xs_for(qid(3)).unwrap().is_empty());
    assert_eq!(
        tracker.zs_for(qid(3)).unwrap(),
        single_pauli_set(5).as_slice()
    );
    assert_eq!(tracker.rec_bits.get(&3), Some(&single_pauli_set(5)));
}

#[test]
fn sparse_rev_frame_tracker_shifted_copy_matches_record_and_detector_offsets() {
    let mut actual = SparseReverseFrameTracker::new(10, 200, 300, AnticommutationPolicy::Fail);
    let mut expected = SparseReverseFrameTracker::new(10, 2000, 3000, AnticommutationPolicy::Fail);
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.rec_bits.insert(195, single_pauli_set(2));
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.rec_bits.insert(1995, single_pauli_set(2));
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual
        .rec_bits
        .get_mut(&195)
        .unwrap()
        .xor_item(DemTarget::relative_detector(293).unwrap());
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected
        .rec_bits
        .get_mut(&1995)
        .unwrap()
        .xor_item(DemTarget::relative_detector(293).unwrap());
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected
        .rec_bits
        .get_mut(&1995)
        .unwrap()
        .xor_item(DemTarget::relative_detector(293).unwrap());
    expected
        .rec_bits
        .get_mut(&1995)
        .unwrap()
        .xor_item(DemTarget::relative_detector(2993).unwrap());
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.xs.insert(qid(5), single_pauli_set(3));
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.xs.insert(qid(5), single_pauli_set(3));
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.zs.insert(qid(6), single_pauli_set(3));
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.zs.insert(qid(6), single_pauli_set(3));
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual
        .xs
        .get_mut(&qid(5))
        .unwrap()
        .xor_item(DemTarget::relative_detector(287).unwrap());
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected
        .xs
        .get_mut(&qid(5))
        .unwrap()
        .xor_item(DemTarget::relative_detector(2987).unwrap());
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual
        .zs
        .get_mut(&qid(6))
        .unwrap()
        .xor_item(DemTarget::relative_detector(287).unwrap());
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected
        .zs
        .get_mut(&qid(6))
        .unwrap()
        .xor_item(DemTarget::relative_detector(2987).unwrap());
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual
        .observable_effects
        .insert(ObservableId::from(9), detector_set(286));
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected
        .observable_effects
        .insert(ObservableId::from(9), detector_set(2986));
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.gauge_errors.push(detector_set(285));
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.gauge_errors.push(detector_set(2985));
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.anticommutations.insert(Anticommutation {
        target: DemTarget::relative_detector(284).unwrap(),
        location: TrackerLocation {
            qubit: qid(7),
            basis: TrackerBasis::Y,
        },
    });
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.anticommutations.insert(Anticommutation {
        target: DemTarget::relative_detector(2984).unwrap(),
        location: TrackerLocation {
            qubit: qid(7),
            basis: TrackerBasis::Y,
        },
    });
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    actual.error_analysis_mode = true;
    assert!(!shifted_repeat::is_shifted_copy(&actual, &expected));
    expected.error_analysis_mode = true;
    assert!(shifted_repeat::is_shifted_copy(&actual, &expected));

    shifted_repeat::shift(&mut actual, 1800, 2700).unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn sparse_rev_frame_tracker_folds_shifted_measurement_repeat_period() {
    let body = circuit(
        "
        CX 0 1 1 2 2 3 3 0
        M 0 0 1
        DETECTOR rec[-2] rec[-3]
        OBSERVABLE_INCLUDE(3) rec[-1]
        ",
    );

    let mut folded = SparseReverseFrameTracker::new(
        20,
        5_000_000_000_000,
        4_000_000_000_000,
        AnticommutationPolicy::Fail,
    );
    let mut unrolled = folded.clone();
    shifted_repeat::undo_loop(&mut folded, &body, 500).unwrap();
    shifted_repeat::undo_loop_by_unrolling(&mut unrolled, &body, 500).unwrap();
    assert_eq!(folded, unrolled);

    shifted_repeat::undo_loop(&mut folded, &body, 1_000_000_000_001).unwrap();
    assert_eq!(
        folded.zs_for(qid(0)).unwrap(),
        single_pauli_set(3).as_slice()
    );
    assert_eq!(folded.measurement_count, 1_999_999_998_497);
    assert_eq!(folded.detector_count, 2_999_999_999_499);
}

#[test]
fn sparse_rev_frame_tracker_repeat_blocks_track_measurements_and_detectors() {
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
        AnticommutationPolicy::Fail,
    );
    tracker.undo_circuit(&circuit).unwrap();

    let mut expected = SparseReverseFrameTracker::new(1, 0, 0, AnticommutationPolicy::Fail);
    expected.zs.insert(qid(0), detector_set(0));
    expected
        .zs
        .get_mut(&qid(0))
        .unwrap()
        .xor_item(DemTarget::relative_detector(1).unwrap());
    assert_eq!(tracker, expected);
}

#[test]
fn sparse_rev_frame_tracker_accepts_mpad_and_discards_record_sensitivity() {
    let mut actual = tracker_from_pauli_text("IIZ");
    actual.measurement_count = 2;
    actual.rec_bits.insert(
        1,
        TargetXorSet::from_target(DemTarget::relative_detector(5).unwrap()),
    );
    actual.undo_instruction(&instruction("MPAD 0\n")).unwrap();

    let mut expected = tracker_from_pauli_text("IIZ");
    expected.measurement_count = 1;
    assert_eq!(actual, expected);
}

#[test]
fn live_bytes_matches_recomputed_state_across_instruction_kinds() {
    let sample = circuit(
        "
        R 0 1 2
        H 0
        CX 0 1
        S 1
        SPP X0*Z1
        M 0
        CX rec[-1] 2
        MR 1 2
        MPAD 0
        DETECTOR rec[-4]
        OBSERVABLE_INCLUDE(0) rec[-1]
        ",
    );
    let mut tracker = SparseReverseFrameTracker::new_for_error_analysis(
        sample.count_qubits(),
        measurement_record_count(&sample).unwrap(),
        sample.count_detectors().unwrap(),
        true,
    );
    for item in sample.items().iter().rev() {
        let CircuitItem::Instruction(inst) = item else {
            panic!("flat fixture");
        };
        tracker.undo_instruction(inst).unwrap();
        assert_eq!(
            tracker.meter.live_bytes,
            budget::recompute_live_bytes(&tracker),
            "after undoing {inst:?}"
        );
    }
    tracker.undo_implicit_rz_at_start_of_circuit().unwrap();
    assert!(!tracker.take_gauge_errors().is_empty());
    assert_eq!(
        tracker.meter.live_bytes,
        budget::recompute_live_bytes(&tracker)
    );

    // Also exercise recorded anticommutations when fail_on_anticommute is false.
    let anticommuting = circuit("R 0\nM 0\nR 0\nMX 0\nDETECTOR rec[-1]\n");
    let mut non_failing = SparseReverseFrameTracker::new(
        anticommuting.count_qubits(),
        measurement_record_count(&anticommuting).unwrap(),
        anticommuting.count_detectors().unwrap(),
        AnticommutationPolicy::Record,
    );
    for item in anticommuting.items().iter().rev() {
        let CircuitItem::Instruction(inst) = item else {
            panic!("flat fixture");
        };
        non_failing.undo_instruction(inst).unwrap();
        assert_eq!(
            non_failing.meter.live_bytes,
            budget::recompute_live_bytes(&non_failing),
        );
    }
    assert!(
        non_failing.target_anticommuted(DemTarget::relative_detector(0).unwrap()),
        "expected recorded anticommutation"
    );
}

/// One `MPP` product of `K` Z terms whose record is sensitive to `S` detectors XORs those `S`
/// targets into `K` fresh qubit sets inside a single instruction. Admission must refuse that
/// growth before it happens, so the instruction's peak allocation stays within the budget plus
/// the uncounted bookkeeping: map nodes, the instruction's term list, and the error message.
#[test]
fn wide_product_merge_is_refused_before_it_outgrows_the_budget() {
    const TERMS: usize = 2000;
    const DETECTORS: u64 = 1000;
    const BUDGET: u64 = 1 << 20;
    const SLACK: u64 = 16 << 10;

    let mut text = String::from("MPP");
    for qubit in 0..TERMS {
        text.push_str(if qubit == 0 { " Z" } else { "*Z" });
        text.push_str(&qubit.to_string());
    }
    text.push('\n');
    let product = instruction(&text);
    let detector = instruction("DETECTOR rec[-1]\n");
    let prepared = |budget: Option<MemoryBudget>| {
        let mut tracker =
            SparseReverseFrameTracker::new(TERMS, 1, DETECTORS, AnticommutationPolicy::Fail);
        if let Some(budget) = budget {
            tracker.set_memory_budget(
                AnalysisOperation::DetectingRegions,
                ResourceKind::LiveStateUnits,
                budget,
            );
        }
        for _ in 0..DETECTORS {
            tracker.undo_instruction(&detector).unwrap();
        }
        tracker
    };

    let mut unbounded = prepared(None);
    let unbounded_peak = allocation_counter::measure(|| {
        unbounded.undo_instruction(&product).unwrap();
    })
    .bytes_max;
    assert!(
        unbounded_peak > 8 * BUDGET,
        "the fixture must outgrow the budget by far, peaked at {unbounded_peak} bytes"
    );

    let mut bounded = prepared(Some(MemoryBudget::from_bytes(BUDGET)));
    let mut result = Ok(());
    let peak = allocation_counter::measure(|| {
        result = bounded.undo_instruction(&product);
    })
    .bytes_max;
    let Err(AnalysisError::ResourceLimit(limit)) = result else {
        panic!("expected a typed budget refusal, got {result:?}");
    };
    assert_eq!(limit.resource(), ResourceKind::LiveStateUnits);
    assert_eq!(limit.limit(), BUDGET);
    assert!(
        bounded.live_bytes() <= BUDGET,
        "refused growth must not be retained: {} bytes live",
        bounded.live_bytes()
    );
    assert!(
        peak <= BUDGET + SLACK,
        "peak allocation {peak} exceeds the {BUDGET}-byte budget plus {SLACK} bytes of slack"
    );
}
