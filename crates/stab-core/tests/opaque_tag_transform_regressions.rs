#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "regression tests use compact exact model fixtures and public item inspection"
)]

use stab_core::{Circuit, CircuitInstruction, CircuitItem, DemItem, DetectorErrorModel};

fn circuit_from_bytes(bytes: &[u8]) -> Circuit {
    Circuit::from_stim_bytes(bytes).expect("parse opaque-tag circuit")
}

fn dem_from_bytes(bytes: &[u8]) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_bytes(bytes).expect("parse opaque-tag DEM")
}

fn circuit_instructions(circuit: &Circuit) -> Vec<&CircuitInstruction> {
    circuit
        .items()
        .iter()
        .filter_map(CircuitItem::as_instruction)
        .collect()
}

fn circuit_instruction_tags(circuit: &Circuit) -> Vec<Option<&[u8]>> {
    circuit_instructions(circuit)
        .into_iter()
        .map(CircuitInstruction::tag_bytes)
        .collect()
}

fn dem_top_level_tags(model: &DetectorErrorModel) -> Vec<Option<&[u8]>> {
    model
        .items()
        .iter()
        .map(|item| match item {
            DemItem::Instruction(instruction) => instruction.tag_bytes(),
            DemItem::RepeatBlock(repeat) => repeat.tag_bytes(),
        })
        .collect()
}

#[test]
fn flattened_circuit_preserves_opaque_instruction_tags() {
    let circuit = circuit_from_bytes(b"REPEAT[\xfc] 2 {\n    H[\xff] 0\n    S[\xfe] 0\n}\n");
    let flattened = circuit.flattened().expect("flatten tagged circuit");

    assert_eq!(
        flattened.to_stim_bytes(),
        b"H[\xff] 0\nS[\xfe] 0\nH[\xff] 0\nS[\xfe] 0\n"
    );
    assert_eq!(
        circuit_instruction_tags(&flattened),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xff".as_slice()),
            Some(b"\xfe".as_slice()),
        ]
    );
}

#[test]
fn circuit_without_noise_preserves_opaque_tags_on_surviving_records() {
    let circuit = circuit_from_bytes(
        b"H[\xff] 0\n\
          X_ERROR[\xfc](0.125) 0\n\
          M[\xfe](0.25) 0\n\
          DEPOLARIZE1[\xfb](0.25) 0\n\
          MR[\xfd](0.5) 1\n",
    );
    let noiseless = circuit.without_noise().expect("strip noisy behavior");

    assert_eq!(
        noiseless.to_stim_bytes(),
        b"H[\xff] 0\nM[\xfe] 0\nMR[\xfd] 1\n"
    );
    assert_eq!(
        circuit_instruction_tags(&noiseless),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xfd".as_slice()),
        ]
    );
}

#[test]
fn simplified_and_decomposed_circuits_preserve_opaque_tags_on_expanded_operations() {
    let simplified = circuit_from_bytes(b"H_XY[\xff] 0\nCZ[\xfe] 0 1\n")
        .simplified()
        .expect("simplify tagged circuit");

    assert_eq!(
        simplified.to_stim_bytes(),
        b"H[\xff] 0\nS[\xff] 0 0\nH[\xff] 0\nS[\xff] 0\n\
          H[\xfe] 1\nCX[\xfe] 0 1\nH[\xfe] 1\n"
    );
    assert_eq!(
        circuit_instruction_tags(&simplified),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xff".as_slice()),
            Some(b"\xff".as_slice()),
            Some(b"\xff".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xfe".as_slice()),
        ]
    );

    let decomposed = circuit_from_bytes(b"SQRT_X[\xfd] 0\n")
        .decomposed()
        .expect("decompose tagged circuit");

    assert_eq!(
        decomposed.to_stim_bytes(),
        b"H[\xfd] 0\nS[\xfd] 0\nH[\xfd] 0\n"
    );
    assert!(
        circuit_instruction_tags(&decomposed)
            .into_iter()
            .all(|tag| tag == Some(b"\xfd".as_slice()))
    );
}

#[test]
fn inverse_circuits_preserve_opaque_tags_in_reversed_models() {
    let circuit = circuit_from_bytes(
        b"H[\xff] 0\n\
          S[\xfe] 0\n\
          CX[\xfd] 0 1\n\
          REPEAT[\xfc] 2 {\n    H[\xfb] 2\n    S[\xfa] 2\n}\n",
    );
    let inverse = circuit.inverse_unitary().expect("invert tagged circuit");

    assert_eq!(
        inverse.to_stim_bytes(),
        b"REPEAT[\xfc] 2 {\n    S_DAG[\xfa] 2\n    H[\xfb] 2\n}\n\
          CX[\xfd] 0 1\nS_DAG[\xfe] 0\nH[\xff] 0\n"
    );
    let repeat = inverse
        .items()
        .first()
        .and_then(CircuitItem::as_repeat_block)
        .expect("inverted repeat block");
    assert_eq!(repeat.tag_bytes(), Some(b"\xfc".as_slice()));
    assert_eq!(
        circuit_instruction_tags(repeat.body()),
        vec![Some(b"\xfa".as_slice()), Some(b"\xfb".as_slice())]
    );
    assert_eq!(
        circuit_instruction_tags(&inverse),
        vec![
            Some(b"\xfd".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xff".as_slice()),
        ]
    );

    let inverse_qec = circuit_from_bytes(b"M[\xff](0.125) 0 1\nMX[\xfe](0.25) 2\n")
        .inverse_qec()
        .expect("QEC-invert tagged measurements");
    assert_eq!(
        inverse_qec.to_stim_bytes(),
        b"MX[\xfe](0.25) 2\nM[\xff](0.125) 1 0\n"
    );
    assert_eq!(
        circuit_instruction_tags(&inverse_qec),
        vec![Some(b"\xfe".as_slice()), Some(b"\xff".as_slice())]
    );
}

#[test]
fn feedback_inlining_preserves_opaque_tags_on_surviving_and_introduced_operations() {
    let circuit = circuit_from_bytes(
        b"CX[\xff] 0 1\n\
          M[\xfe] 1\n\
          CX[\xfd] rec[-1] 1\n\
          CX[\xfc] 0 1\n\
          M[\xfb] 1\n\
          DETECTOR[\xfa] rec[-1] rec[-2]\n\
          OBSERVABLE_INCLUDE[\xf9](0) rec[-1]\n",
    );
    let inlined = circuit
        .with_inlined_feedback()
        .expect("inline supported feedback");

    assert_eq!(
        inlined.to_stim_bytes(),
        b"CX[\xff] 0 1\n\
          M[\xfe] 1\n\
          OBSERVABLE_INCLUDE[\xfd](0) rec[-1]\n\
          CX[\xfc] 0 1\n\
          M[\xfb] 1\n\
          DETECTOR[\xfa] rec[-1]\n\
          OBSERVABLE_INCLUDE[\xf9](0) rec[-1]\n"
    );
    assert_eq!(
        circuit_instruction_tags(&inlined),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xfe".as_slice()),
            Some(b"\xfd".as_slice()),
            Some(b"\xfc".as_slice()),
            Some(b"\xfb".as_slice()),
            Some(b"\xfa".as_slice()),
            Some(b"\xf9".as_slice()),
        ]
    );
}

#[test]
fn rounded_dem_preserves_opaque_instruction_and_repeat_tags() {
    let model = dem_from_bytes(
        b"error[\xff](0.49) D0\n\
          repeat[\xfe] 2 {\n    error[\xfd](0.51) D1\n}\n",
    );
    let rounded = model.rounded(0).expect("round tagged DEM");

    assert_eq!(
        rounded.to_dem_bytes(),
        b"error[\xff](0) D0\nrepeat[\xfe] 2 {\n    error[\xfd](1) D1\n}\n"
    );
    assert_eq!(
        dem_top_level_tags(&rounded),
        vec![Some(b"\xff".as_slice()), Some(b"\xfe".as_slice())]
    );
    let repeat = rounded
        .items()
        .iter()
        .find_map(|item| match item {
            DemItem::Instruction(_) => None,
            DemItem::RepeatBlock(repeat) => Some(repeat),
        })
        .expect("rounded repeat block");
    assert_eq!(
        dem_top_level_tags(repeat.body()),
        vec![Some(b"\xfd".as_slice())]
    );
}

#[test]
fn flattened_dem_preserves_opaque_tags_on_materialized_instructions() {
    let model = dem_from_bytes(
        b"error[\xff](0.5) D0\n\
          shift_detectors[\xfc] 3\n\
          repeat[\xfe] 2 {\n    error[\xfd](0.25) D0 L1\n    detector[\xfb] D1\n    shift_detectors[\xfa] 2\n}\n",
    );
    let flattened = model.flattened().expect("flatten tagged DEM");

    assert_eq!(
        flattened.to_dem_bytes(),
        b"error[\xff](0.5) D0\n\
          error[\xfd](0.25) D3 L1\n\
          detector[\xfb] D4\n\
          error[\xfd](0.25) D5 L1\n\
          detector[\xfb] D6\n"
    );
    assert_eq!(
        dem_top_level_tags(&flattened),
        vec![
            Some(b"\xff".as_slice()),
            Some(b"\xfd".as_slice()),
            Some(b"\xfb".as_slice()),
            Some(b"\xfd".as_slice()),
            Some(b"\xfb".as_slice()),
        ]
    );
}
