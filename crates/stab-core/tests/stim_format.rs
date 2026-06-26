#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "M4 compatibility tests use direct assertions for compact diagnostics"
)]

use std::str::FromStr;

use stab_core::{
    Circuit, CircuitItem, Gate, GateCategory, MeasureRecordOffset, ObservableId, Pauli,
    Probability, QubitId, RepeatCount, Target,
};

#[test]
fn parses_and_prints_basic_m4_fixture() {
    let input = include_str!("../../../oracle/fixtures/inputs/parser_basic.stim");
    let expected = include_str!("../../../oracle/fixtures/expected/m4_parser_basic.stdout");

    let circuit = Circuit::from_stim_str(input).expect("parse fixture");

    assert_eq!(circuit.to_stim_string(), expected);
    assert_eq!(
        Circuit::from_stim_str(&circuit.to_stim_string()).expect("parse canonical"),
        circuit
    );
}

#[test]
fn parses_targets_tags_arguments_comments_and_repeat_blocks() {
    let circuit = Circuit::from_stim_str(
        r#"
            # leading comment
            QUBIT_COORDS[layout\n1](1, 2.5) 0
            MPP !X0*Y1 Z2
            CX sweep[3] 2
            DETECTOR(0, 1) rec[-1] # trailing comment
            OBSERVABLE_INCLUDE(2) rec[-1]
            REPEAT 3 {
                TICK
                M !0
            }
        "#,
    )
    .expect("parse circuit");

    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "QUBIT_COORDS[layout\\n1](1, 2.5) 0\n",
            "MPP !X0*Y1 Z2\n",
            "CX sweep[3] 2\n",
            "DETECTOR(0, 1) rec[-1]\n",
            "OBSERVABLE_INCLUDE(2) rec[-1]\n",
            "REPEAT 3 {\n",
            "    TICK\n",
            "    M !0\n",
            "}\n",
        )
    );

    let items = circuit.items();
    assert_eq!(items.len(), 6);
    let instruction = items
        .get(1)
        .and_then(CircuitItemExt::as_instruction)
        .expect("MPP instruction");
    assert_eq!(
        instruction.targets(),
        &[
            Target::pauli(Pauli::X, QubitId::new(0).unwrap(), true),
            Target::combiner(),
            Target::pauli(Pauli::Y, QubitId::new(1).unwrap(), false),
            Target::pauli(Pauli::Z, QubitId::new(2).unwrap(), false),
        ]
    );

    let repeat = items
        .get(5)
        .and_then(CircuitItemExt::as_repeat_block)
        .expect("repeat block");
    assert_eq!(repeat.repeat_count(), RepeatCount::try_new(3).unwrap());
    assert_eq!(repeat.body().items().len(), 2);
}

#[test]
fn parses_and_prints_repeat_tags_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit.test.cc parse_tag.
    let circuit = Circuit::from_stim_str(
        r#"
            X_ERROR[test](0.125)
            X_ERROR[no_fuse](0.125) 1
            X_ERROR(0.125) 2
            X_ERROR[](0.125) 3
            REPEAT[looper] 5 {
                CX[within] 0 1
            }
        "#,
    )
    .expect("parse tagged repeat");
    let items = circuit.items();

    assert_eq!(items.len(), 4);
    let first = items
        .first()
        .and_then(CircuitItemExt::as_instruction)
        .expect("first instruction");
    let second = items
        .get(1)
        .and_then(CircuitItemExt::as_instruction)
        .expect("second instruction");
    let third = items
        .get(2)
        .and_then(CircuitItemExt::as_instruction)
        .expect("third instruction");
    let repeat = items
        .get(3)
        .and_then(CircuitItemExt::as_repeat_block)
        .expect("repeat block");

    assert_eq!(first.tag(), Some("test"));
    assert_eq!(second.tag(), Some("no_fuse"));
    assert_eq!(third.tag(), None);
    assert_eq!(
        third.targets(),
        &[
            Target::qubit(QubitId::new(2).unwrap(), false),
            Target::qubit(QubitId::new(3).unwrap(), false),
        ]
    );
    assert_eq!(repeat.tag(), Some("looper"));
    assert_eq!(repeat.repeat_count(), RepeatCount::try_new(5).unwrap());
    assert_eq!(
        circuit.to_stim_string(),
        concat!(
            "X_ERROR[test](0.125)\n",
            "X_ERROR[no_fuse](0.125) 1\n",
            "X_ERROR(0.125) 2 3\n",
            "REPEAT[looper] 5 {\n",
            "    CX[within] 0 1\n",
            "}\n",
        )
    );
}

#[test]
fn fuses_compatible_adjacent_instructions_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit.test.cc append_op_fuse.
    assert_eq!(
        Circuit::from_stim_str("H 1\nH 2 3\n")
            .expect("parse H fuse fixture")
            .to_stim_string(),
        "H 1 2 3\n"
    );
    assert_eq!(
        Circuit::from_stim_str("M 0 1\nM 2 3\n")
            .expect("parse M fuse fixture")
            .to_stim_string(),
        "M 0 1 2 3\n"
    );
    assert_eq!(
        Circuit::from_stim_str("CX 2 3\nCX rec[-5] 3\n")
            .expect("parse pair-gate fuse fixture")
            .to_stim_string(),
        "CX 2 3 rec[-5] 3\n"
    );
    assert_eq!(
        Circuit::from_stim_str("H[test1] 0\nH[test1] 1\nH[test2] 2\nH[test1] 3\n")
            .expect("parse tagged fuse fixture")
            .to_stim_string(),
        concat!("H[test1] 0 1\n", "H[test2] 2\n", "H[test1] 3\n")
    );
    assert_eq!(
        Circuit::from_stim_str("R 0\nR\n")
            .expect("parse empty-target fuse fixture")
            .to_stim_string(),
        "R 0\n"
    );
    assert_eq!(
        Circuit::from_stim_str("TICK\nTICK\n")
            .expect("parse non-fusable TICK fixture")
            .to_stim_string(),
        "TICK\nTICK\n"
    );
    assert_eq!(
        Circuit::from_stim_str("DETECTOR rec[-2] rec[-2]\nDETECTOR rec[-1] rec[-1]\n")
            .expect("parse non-fusable DETECTOR fixture")
            .to_stim_string(),
        "DETECTOR rec[-2] rec[-2]\nDETECTOR rec[-1] rec[-1]\n"
    );
}

#[test]
fn strips_tags_recursively_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit.test.cc without_tags.
    let initial = Circuit::from_stim_str(
        r#"
            H[test-1] 0
            REPEAT[test-2] 100 {
                REPEAT[test-3] 100 {
                    M[test-4](0.125) 0
                }
            }
        "#,
    )
    .expect("parse tagged circuit");

    assert_eq!(
        initial.without_tags().to_stim_string(),
        concat!(
            "H 0\n",
            "REPEAT 100 {\n",
            "    REPEAT 100 {\n",
            "        M(0.125) 0\n",
            "    }\n",
            "}\n",
        )
    );
}

#[test]
fn target_groups_follow_stim_circuit_instruction_semantics() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit_instruction.test.cc.
    let circuit = Circuit::from_stim_str(
        r#"
            X
            CX
            S 1
            H 0 2
            TICK
            CX 0 1 2 3
            CY 3 5
            SPP
            MPP X0*X1*Z2 Z7 X5*X9
            SPP Z5
        "#,
    )
    .expect("parse target group fixture");
    let items = circuit.items();

    assert_eq!(
        target_groups(items.first().unwrap()),
        Vec::<Vec<Target>>::new()
    );
    assert_eq!(
        target_groups(items.get(1).unwrap()),
        Vec::<Vec<Target>>::new()
    );
    assert_eq!(
        target_groups(items.get(2).unwrap()),
        vec![vec![Target::qubit(QubitId::new(1).unwrap(), false)]]
    );
    assert_eq!(
        target_groups(items.get(3).unwrap()),
        vec![
            vec![Target::qubit(QubitId::new(0).unwrap(), false)],
            vec![Target::qubit(QubitId::new(2).unwrap(), false)]
        ]
    );
    assert_eq!(
        target_groups(items.get(4).unwrap()),
        Vec::<Vec<Target>>::new()
    );
    assert_eq!(
        target_groups(items.get(5).unwrap()),
        vec![
            vec![
                Target::qubit(QubitId::new(0).unwrap(), false),
                Target::qubit(QubitId::new(1).unwrap(), false)
            ],
            vec![
                Target::qubit(QubitId::new(2).unwrap(), false),
                Target::qubit(QubitId::new(3).unwrap(), false)
            ],
        ]
    );
    assert_eq!(
        target_groups(items.get(6).unwrap()),
        vec![vec![
            Target::qubit(QubitId::new(3).unwrap(), false),
            Target::qubit(QubitId::new(5).unwrap(), false)
        ]]
    );
    assert_eq!(
        target_groups(items.get(7).unwrap()),
        Vec::<Vec<Target>>::new()
    );
    assert_eq!(
        target_groups(items.get(8).unwrap()),
        vec![
            vec![
                Target::pauli(Pauli::X, QubitId::new(0).unwrap(), false),
                Target::combiner(),
                Target::pauli(Pauli::X, QubitId::new(1).unwrap(), false),
                Target::combiner(),
                Target::pauli(Pauli::Z, QubitId::new(2).unwrap(), false),
            ],
            vec![Target::pauli(Pauli::Z, QubitId::new(7).unwrap(), false)],
            vec![
                Target::pauli(Pauli::X, QubitId::new(5).unwrap(), false),
                Target::combiner(),
                Target::pauli(Pauli::X, QubitId::new(9).unwrap(), false),
            ],
        ]
    );
    assert_eq!(
        target_groups(items.get(9).unwrap()),
        vec![vec![Target::pauli(
            Pauli::Z,
            QubitId::new(5).unwrap(),
            false
        )]]
    );
}

#[test]
fn parses_mpp_optional_probability_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit.test.cc parse_mpp.
    let circuit = Circuit::from_stim_str("MPP(0.125) X1*Y2 Z3 * Z4\n").expect("parse MPP");
    let instruction = circuit
        .items()
        .first()
        .and_then(CircuitItemExt::as_instruction)
        .expect("MPP instruction");

    assert_eq!(instruction.args(), &[0.125]);
    assert_eq!(circuit.to_stim_string(), "MPP(0.125) X1*Y2 Z3*Z4\n");
    assert!(Circuit::from_stim_str("MPP(1.1) X1\n").is_err());
    assert!(Circuit::from_stim_str("MPP(-0.5) X1\n").is_err());
    for invalid in [
        "H *\n",
        "MPP 0\n",
        "MPP *\n",
        "MPP * X1\n",
        "MPP * X1 *\n",
        "MPP X1 *\n",
        "MPP X1 * * Y2\n",
        "MPP X1**Y2\n",
        "MPP(1.1) X1**Y2\n",
        "MPP(-0.5) X1**Y2\n",
        "MPP X1*rec[-1]\n",
        "MPP rec[-1]\n",
        "MPP sweep[0]\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
    let repeated_qubit = Circuit::from_stim_str("MPP X1*X1\n").expect("repeated qubit");
    let repeated_qubit_instruction = repeated_qubit
        .items()
        .first()
        .and_then(CircuitItemExt::as_instruction)
        .expect("MPP instruction");
    assert_eq!(repeated_qubit_instruction.targets().len(), 3);
}

#[test]
fn parses_spp_and_spp_dag_pauli_products_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/circuit/circuit.test.cc parse_spp and parse_spp_dag.
    for gate in ["SPP", "SPP_DAG"] {
        for invalid in [
            format!("{gate} 1\n"),
            format!("{gate} rec[-1]\n"),
            format!("{gate} sweep[0]\n"),
            format!("{gate} rec[-1]*X0\n"),
        ] {
            assert!(Circuit::from_stim_str(&invalid).is_err(), "{invalid}");
        }

        assert_eq!(
            Circuit::from_stim_str(&format!("{gate}\n"))
                .expect("empty SPP")
                .items()
                .len(),
            1
        );
        let circuit = Circuit::from_stim_str(&format!("{gate} X0 X1*Y2*Z3\n")).expect("parse SPP");
        let instruction = circuit
            .items()
            .first()
            .and_then(CircuitItemExt::as_instruction)
            .expect("SPP instruction");
        assert_eq!(
            instruction.target_groups(),
            &[
                &[Target::pauli(Pauli::X, QubitId::new(0).unwrap(), false)][..],
                &[
                    Target::pauli(Pauli::X, QubitId::new(1).unwrap(), false),
                    Target::combiner(),
                    Target::pauli(Pauli::Y, QubitId::new(2).unwrap(), false),
                    Target::combiner(),
                    Target::pauli(Pauli::Z, QubitId::new(3).unwrap(), false),
                ][..],
            ]
        );
        assert_eq!(
            Circuit::from_stim_str(&format!("{gate} X1 Z2\n"))
                .expect("parse two products")
                .to_stim_string(),
            format!("{gate} X1 Z2\n")
        );
    }
}

#[test]
fn parses_mpad_optional_probability_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_annotations.cc MPAD examples.
    let circuit = Circuit::from_stim_str("MPAD(0.125) 0\nMPAD 1\n").expect("parse MPAD");
    assert_eq!(circuit.to_stim_string(), "MPAD(0.125) 0\nMPAD 1\n");

    let gate = Gate::from_name("MPAD").expect("MPAD");
    assert_eq!(gate.category(), GateCategory::Annotation);

    for invalid in [
        "MPAD(1.1) 0\n",
        "MPAD(0.1, 0.2) 0\n",
        "MPAD 2\n",
        "MPAD !0\n",
        "MPAD rec[-1]\n",
        "MPAD sweep[0]\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn parses_observable_include_pauli_targets_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_annotations.cc OBSERVABLE_INCLUDE examples.
    let circuit = Circuit::from_stim_str("OBSERVABLE_INCLUDE(0) X0 Z1 rec[-1]\n")
        .expect("parse observable include");
    assert_eq!(
        circuit.to_stim_string(),
        "OBSERVABLE_INCLUDE(0) X0 Z1 rec[-1]\n"
    );

    for invalid in [
        "OBSERVABLE_INCLUDE(0) 0\n",
        "OBSERVABLE_INCLUDE(0) sweep[0]\n",
        "OBSERVABLE_INCLUDE(0) X0*Z1\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn gates_lookup_is_case_insensitive_and_canonicalizes_aliases() {
    let h = Gate::from_name("h").expect("H");
    let h_xz = Gate::from_name("H_XZ").expect("H_XZ");
    let cnot = Gate::from_name("Cnot").expect("CNOT");
    let cx = Gate::from_name("CX").expect("CX");
    let m = Gate::from_name("MZ").expect("MZ");

    assert_eq!(h, h_xz);
    assert_eq!(h.canonical_name(), "H");
    assert_eq!(cnot, cx);
    assert_eq!(cx.canonical_name(), "CX");
    assert_eq!(m.canonical_name(), "M");
    assert_eq!(
        Gate::from_name("DETECTOR").unwrap().category(),
        GateCategory::Annotation
    );
    assert!(Gate::from_name("H2345").is_err());
}

#[test]
fn gate_inverse_metadata_matches_stim_v116() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.test.cc inverse metadata checks.
    for (gate, inverse) in [
        ("H", "H"),
        ("CX", "CX"),
        ("RX", "MX"),
        ("RY", "MY"),
        ("R", "M"),
        ("SQRT_X", "SQRT_X_DAG"),
        ("SQRT_X_DAG", "SQRT_X"),
        ("SQRT_Y", "SQRT_Y_DAG"),
        ("SQRT_Y_DAG", "SQRT_Y"),
        ("S", "S_DAG"),
        ("S_DAG", "S"),
        ("C_XYZ", "C_ZYX"),
        ("C_ZYX", "C_XYZ"),
        ("C_NXYZ", "C_ZYNX"),
        ("C_ZYNX", "C_NXYZ"),
        ("C_XNYZ", "C_ZNYX"),
        ("C_ZNYX", "C_XNYZ"),
        ("C_XYNZ", "C_NZYX"),
        ("C_NZYX", "C_XYNZ"),
        ("SQRT_XX", "SQRT_XX_DAG"),
        ("SQRT_XX_DAG", "SQRT_XX"),
        ("SQRT_YY", "SQRT_YY_DAG"),
        ("SQRT_YY_DAG", "SQRT_YY"),
        ("SQRT_ZZ", "SQRT_ZZ_DAG"),
        ("SQRT_ZZ_DAG", "SQRT_ZZ"),
        ("SPP", "SPP_DAG"),
        ("SPP_DAG", "SPP"),
        ("ISWAP", "ISWAP_DAG"),
        ("ISWAP_DAG", "ISWAP"),
        ("CXSWAP", "SWAPCX"),
        ("SWAPCX", "CXSWAP"),
        ("CZSWAP", "CZSWAP"),
        ("X_ERROR", "X_ERROR"),
    ] {
        assert_eq!(
            Gate::from_name(gate)
                .unwrap()
                .best_candidate_inverse()
                .unwrap()
                .canonical_name(),
            inverse,
            "{gate}"
        );
    }

    assert_eq!(
        Gate::from_name("H_XZ")
            .unwrap()
            .best_candidate_inverse()
            .unwrap()
            .canonical_name(),
        "H"
    );
}

#[test]
fn typed_boundaries_reject_invalid_values() {
    assert_eq!(QubitId::new(4).unwrap().get(), 4);
    assert!(MeasureRecordOffset::try_new(0).is_err());
    assert!(MeasureRecordOffset::try_new(1).is_err());
    assert!(RepeatCount::try_new(0).is_err());
    assert!(Probability::try_new(-0.1).is_err());
    assert!(Probability::try_new(1.1).is_err());
    assert!(Probability::try_new(f64::NAN).is_err());
    assert_eq!(ObservableId::new(2).get(), 2);
}

#[test]
fn parser_reports_invalid_gate_target_and_repeat_errors() {
    assert!(Circuit::from_stim_str("UNKNOWN 0\n").is_err());
    assert!(Circuit::from_stim_str("M rec[-1]\n").is_err());
    assert!(Circuit::from_stim_str("DETECTOR 0\n").is_err());
    assert!(Circuit::from_stim_str("REPEAT 0 {\n    TICK\n}\n").is_err());
    assert!(Circuit::from_stim_str("REPEAT 2 {\n    TICK\n").is_err());
}

#[test]
fn target_from_str_matches_stim_surface_forms() {
    assert_eq!(
        Target::from_str("rec[-3]").unwrap(),
        Target::measurement_record(MeasureRecordOffset::try_new(-3).unwrap())
    );
    assert_eq!(
        Target::from_str("!5").unwrap(),
        Target::qubit(QubitId::new(5).unwrap(), true)
    );
    assert_eq!(Target::from_str("sweep[7]").unwrap(), Target::sweep_bit(7));
    assert_eq!(
        Target::from_str("Z11").unwrap(),
        Target::pauli(Pauli::Z, QubitId::new(11).unwrap(), false)
    );
    assert!(Target::from_str("4294967295").is_err());
    assert!(Target::from_str("X4294967295").is_err());
    assert!(Target::from_str("rec[0]").is_err());
    assert!(Target::from_str("rec[-1073741824]").is_err());
}

#[test]
fn target_typed_accessors_match_stim_gate_target_boundaries() {
    // Adapted from Stim v1.16.0 src/stim/circuit/gate_target.test.cc.
    let qubit_id = QubitId::new(5).unwrap();
    let qubit = Target::qubit(qubit_id, false);
    assert_eq!(qubit.qubit_id(), Some(qubit_id));
    assert_eq!(qubit.measurement_record_offset(), None);
    assert_eq!(qubit.sweep_bit_id(), None);

    let pauli_id = QubitId::new(7).unwrap();
    let pauli = Target::pauli(Pauli::Y, pauli_id, true);
    assert_eq!(pauli.qubit_id(), Some(pauli_id));
    assert_eq!(pauli.measurement_record_offset(), None);
    assert_eq!(pauli.sweep_bit_id(), None);

    let offset = MeasureRecordOffset::try_new(-5).unwrap();
    let record = Target::measurement_record(offset);
    assert_eq!(record.qubit_id(), None);
    assert_eq!(record.measurement_record_offset(), Some(offset));
    assert_eq!(record.sweep_bit_id(), None);

    let sweep = Target::sweep_bit(11);
    assert_eq!(sweep.qubit_id(), None);
    assert_eq!(sweep.measurement_record_offset(), None);
    assert_eq!(sweep.sweep_bit_id(), Some(11));

    let combiner = Target::combiner();
    assert_eq!(combiner.qubit_id(), None);
    assert_eq!(combiner.measurement_record_offset(), None);
    assert_eq!(combiner.sweep_bit_id(), None);
}

#[test]
fn target_inversion_matches_stim_gate_target() {
    // Adapted from Stim v1.16.0 src/stim/circuit/gate_target.test.cc inverse.
    let qubit_id = QubitId::new(5).unwrap();
    assert_eq!(
        Target::qubit(qubit_id, false).try_inverted().unwrap(),
        Target::qubit(qubit_id, true)
    );
    assert_eq!(
        Target::qubit(qubit_id, true).try_inverted().unwrap(),
        Target::qubit(qubit_id, false)
    );

    let pauli_id = QubitId::new(9).unwrap();
    assert_eq!(
        Target::pauli(Pauli::Z, pauli_id, false)
            .try_inverted()
            .unwrap(),
        Target::pauli(Pauli::Z, pauli_id, true)
    );
    assert_eq!(
        Target::pauli(Pauli::Z, pauli_id, true)
            .try_inverted()
            .unwrap(),
        Target::pauli(Pauli::Z, pauli_id, false)
    );

    assert!(Target::combiner().try_inverted().is_err());
    assert!(
        Target::measurement_record(MeasureRecordOffset::try_new(-3).unwrap())
            .try_inverted()
            .is_err()
    );
    assert!(Target::sweep_bit(6).try_inverted().is_err());
}

#[test]
fn target_classification_matches_stim_gate_target() {
    // Adapted from Stim v1.16.0 src/stim/circuit/gate_target.test.cc.
    let qubit = Target::qubit(QubitId::new(2).unwrap(), false);
    let inverted_qubit = Target::qubit(QubitId::new(3).unwrap(), true);
    let sweep = Target::sweep_bit(5);
    let rec = Target::measurement_record(MeasureRecordOffset::try_new(-7).unwrap());
    let x = Target::pauli(Pauli::X, QubitId::new(11).unwrap(), false);
    let inverted_x = Target::pauli(Pauli::X, QubitId::new(13).unwrap(), true);
    let y = Target::pauli(Pauli::Y, QubitId::new(17).unwrap(), false);
    let inverted_y = Target::pauli(Pauli::Y, QubitId::new(19).unwrap(), true);
    let z = Target::pauli(Pauli::Z, QubitId::new(23).unwrap(), false);
    let inverted_z = Target::pauli(Pauli::Z, QubitId::new(29).unwrap(), true);
    let combiner = Target::combiner();

    for target in [&qubit, &inverted_qubit, &sweep, &rec, &combiner] {
        assert!(!target.is_pauli_target(), "{target}");
    }
    for target in [&x, &inverted_x, &y, &inverted_y, &z, &inverted_z] {
        assert!(target.is_pauli_target(), "{target}");
    }

    assert!(sweep.is_classical_bit_target());
    assert!(rec.is_classical_bit_target());
    for target in [
        &qubit,
        &inverted_qubit,
        &x,
        &inverted_x,
        &y,
        &inverted_y,
        &z,
        &inverted_z,
        &combiner,
    ] {
        assert!(!target.is_classical_bit_target(), "{target}");
    }

    assert!(qubit.is_qubit_target());
    assert!(inverted_qubit.is_qubit_target());
    assert!(sweep.is_sweep_bit_target());
    assert!(rec.is_measurement_record_target());
    assert!(combiner.is_combiner());
    assert!(inverted_qubit.is_inverted_result_target());
    assert!(inverted_x.is_inverted_result_target());
    assert!(!rec.is_inverted_result_target());
    assert!(x.is_x_target());
    assert!(y.is_y_target());
    assert!(z.is_z_target());
    assert_eq!(x.pauli_type(), Some(Pauli::X));
    assert_eq!(qubit.pauli_type(), None);
}

trait CircuitItemExt {
    fn as_instruction(&self) -> Option<&stab_core::CircuitInstruction>;
    fn as_repeat_block(&self) -> Option<&stab_core::RepeatBlock>;
}

impl CircuitItemExt for CircuitItem {
    fn as_instruction(&self) -> Option<&stab_core::CircuitInstruction> {
        match self {
            Self::Instruction(instruction) => Some(instruction),
            Self::RepeatBlock(_) => None,
        }
    }

    fn as_repeat_block(&self) -> Option<&stab_core::RepeatBlock> {
        match self {
            Self::Instruction(_) => None,
            Self::RepeatBlock(repeat) => Some(repeat),
        }
    }
}

fn target_groups(item: &CircuitItem) -> Vec<Vec<Target>> {
    item.as_instruction()
        .expect("instruction")
        .target_groups()
        .into_iter()
        .map(<[Target]>::to_vec)
        .collect()
}
