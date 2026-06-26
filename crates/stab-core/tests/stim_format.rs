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
