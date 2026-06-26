#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M6 stabilizers-to-tableau parity tests mirror compact upstream examples"
)]

use std::str::FromStr;

use stab_core::{
    PauliBasis, PauliSign, PauliString, Tableau, TableauIterator, stabilizers_to_tableau,
};

#[test]
fn stabilizers_to_tableau_bell_pair_matches_stim() {
    // Adapted from Stim v1.16.0 src/stim/util_top/stabilizers_to_tableau.test.cc.
    let mut input = vec![pauli("XX"), pauli("ZZ")];
    let expected = Tableau::gate2("+Z_", "+XX", "+_X", "+ZZ").expect("expected Bell tableau");

    let actual = stabilizers_to_tableau(&input, false, false, false).expect("convert Bell pair");
    assert_eq!(actual, expected);
    assert!(actual.satisfies_invariants().expect("invariants"));

    input.push(pauli("-YY"));
    assert!(stabilizers_to_tableau(&input, false, false, false).is_err());
    assert_eq!(
        stabilizers_to_tableau(&input, true, false, false).expect("ignore redundant stabilizer"),
        expected
    );

    input[2] = pauli("+YY");
    assert!(stabilizers_to_tableau(&input, true, true, false).is_err());

    input[2] = pauli("+Z_");
    assert!(stabilizers_to_tableau(&input, true, true, false).is_err());
}

#[test]
fn stabilizers_to_tableau_detects_anticommutation() {
    let input = vec![pauli("YY"), pauli("YX")];
    assert!(stabilizers_to_tableau(&input, false, false, false).is_err());
}

#[test]
fn stabilizers_to_tableau_handles_size_affecting_redundancy() {
    let mut input = vec![pauli("X_"), pauli("_X")];
    for _ in 0..150 {
        input.push(pauli("__"));
    }

    let tableau =
        stabilizers_to_tableau(&input, true, true, false).expect("convert redundant input");
    assert_eq!(tableau.len(), 2);
    assert_eq!(tableau.z_output(0).expect("z0"), &pauli("X_"));
    assert_eq!(tableau.z_output(1).expect("z1"), &pauli("_X"));
    assert!(tableau.satisfies_invariants().expect("invariants"));
}

#[test]
fn stabilizers_to_tableau_underconstrained_requires_opt_in_and_can_invert() {
    let input = vec![pauli("Z_")];
    assert!(stabilizers_to_tableau(&input, false, false, false).is_err());

    let actual = stabilizers_to_tableau(&input, false, true, false).expect("underconstrained");
    assert_eq!(actual.z_output(0).expect("z0"), &pauli("Z_"));
    assert!(actual.satisfies_invariants().expect("invariants"));

    let inverted = stabilizers_to_tableau(&input, false, true, true).expect("inverse");
    assert_eq!(actual.inverse().expect("actual inverse"), inverted);
}

#[test]
fn stabilizers_to_tableau_preserves_z_outputs_from_valid_tableaus() {
    // Deterministic miniature of Stim's stabilizers_to_tableau_fuzz coverage.
    for source in TableauIterator::new(2, true)
        .expect("tableau iterator")
        .take(50)
    {
        let stabilizers = (0..source.len())
            .map(|index| source.z_output(index).expect("source z").clone())
            .collect::<Vec<_>>();
        let actual =
            stabilizers_to_tableau(&stabilizers, false, false, false).expect("convert stabilizers");

        for index in 0..source.len() {
            assert_eq!(
                actual.z_output(index).expect("actual z"),
                source.z_output(index).expect("source z")
            );
        }
        assert!(actual.satisfies_invariants().expect("invariants"));
    }
}

#[test]
fn stabilizers_to_tableau_empty_input_is_empty_identity() {
    let actual = stabilizers_to_tableau(&[], false, false, false).expect("empty conversion");
    assert_eq!(actual, Tableau::identity(0));
}

#[test]
fn stabilizers_to_tableau_accepts_inputs_past_iterator_limit() {
    let input = (0..70)
        .map(|index| single_pauli(70, index, PauliBasis::Z))
        .collect::<Vec<_>>();

    let actual = stabilizers_to_tableau(&input, false, false, false).expect("large conversion");
    assert_eq!(actual.len(), 70);
    assert_eq!(
        actual.x_output(69).expect("x69"),
        &single_pauli(70, 69, PauliBasis::X)
    );
    assert_eq!(
        actual.z_output(69).expect("z69"),
        &single_pauli(70, 69, PauliBasis::Z)
    );
    assert!(actual.satisfies_invariants().expect("invariants"));
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse PauliString")
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}
