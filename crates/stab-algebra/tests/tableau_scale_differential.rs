#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "seeded differential fixtures index fixed sample positions and make setup failures test bugs"
)]

//! Differential and algebraic-identity tests of the word-parallel Tableau beyond 1000 qubits.
//!
//! The reference for Pauli actions is [`reference_apply`], a test-only conjugation that multiplies
//! the generator images one checked [`FlexPauliString`] product at a time and shares no code with
//! the Tableau's packed action. Composition, inversion, and powers are checked against that
//! reference on sampled rows and random Paulis, and against algebraic identities where a scalar
//! reference over every row would be too slow in a debug build.

use std::assert_matches;

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use stab_algebra::{
    FlexPauliString, PauliBasis, PauliPhase, PauliSign, PauliString, StabilizerError, Tableau,
    stabilizers_to_tableau,
};

/// Wider than 1000 qubits and not a multiple of 64, so every packed row has a partial word.
const WIDTH: usize = 1_030;
/// Rows on both sides of word boundaries and at the ends.
const SAMPLE_ROWS: [usize; 8] = [0, 1, 63, 64, 65, 517, 1_023, WIDTH - 1];

/// Test-only scalar reference for `tableau(input)`.
fn reference_apply(tableau: &Tableau, input: &PauliString) -> PauliString {
    let width = tableau.len();
    let flex = |pauli: &PauliString| {
        FlexPauliString::from_phase_and_bases(
            pauli.phase(),
            (0..pauli.len()).map(|qubit| pauli.get(qubit).expect("in-range basis")),
        )
        .expect("flex Pauli")
    };
    let mut accumulated = FlexPauliString::from_phase_and_bases(
        input.phase(),
        std::iter::repeat_n(PauliBasis::I, width),
    )
    .expect("identity accumulator");
    for (qubit, basis) in input.active_terms() {
        let image = match basis {
            PauliBasis::X => tableau.x_output(qubit),
            PauliBasis::Y => tableau.y_output(qubit),
            PauliBasis::Z => tableau.z_output(qubit),
            PauliBasis::I => continue,
        }
        .expect("generator image");
        accumulated = accumulated.multiply(&flex(&image)).expect("image product");
    }
    assert_matches!(accumulated.phase(), PauliPhase::Plus | PauliPhase::Minus);
    accumulated.try_into_real().expect("real image")
}

fn random_pauli(width: usize, weight_per_mille: u32, rng: &mut SmallRng) -> PauliString {
    let sign = if rng.random_bool(0.5) {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    };
    let bases = (0..width).map(|_| {
        if rng.random_range(0..1000) >= weight_per_mille {
            PauliBasis::I
        } else {
            [PauliBasis::X, PauliBasis::Y, PauliBasis::Z][rng.random_range(0..3)]
        }
    });
    PauliString::from_bases(sign, bases).expect("random Pauli")
}

fn generator(width: usize, qubit: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..width).map(|index| if index == qubit { basis } else { PauliBasis::I }),
    )
    .expect("canonical generator")
}

#[test]
fn composition_and_action_match_the_scalar_reference_beyond_1000_qubits() {
    let mut rng = SmallRng::seed_from_u64(0x000c_05e5_1030);
    let first = Tableau::random(WIDTH, &mut rng).expect("first random Tableau");
    let second = Tableau::random(WIDTH, &mut rng).expect("second random Tableau");
    let composed = first.then(&second).expect("composition");

    for row in SAMPLE_ROWS {
        assert_eq!(
            composed.x_output(row).expect("composed X image"),
            reference_apply(&second, &first.x_output(row).expect("first X image")),
            "X_{row}"
        );
        assert_eq!(
            composed.z_output(row).expect("composed Z image"),
            reference_apply(&second, &first.z_output(row).expect("first Z image")),
            "Z_{row}"
        );
    }
    for weight in [1, 20, 500, 1000] {
        let input = random_pauli(WIDTH, weight, &mut rng);
        let expected = reference_apply(&second, &reference_apply(&first, &input));
        assert_eq!(
            first.apply(&input).expect("first action"),
            reference_apply(&first, &input)
        );
        assert_eq!(composed.apply(&input).expect("composed action"), expected);
    }
}

#[test]
fn inverse_round_trips_beyond_1000_qubits() {
    let mut rng = SmallRng::seed_from_u64(0x1_4e5e_1030);
    let tableau = Tableau::random(WIDTH, &mut rng).expect("random Tableau");
    let inverse = tableau.inverse().expect("inverse");
    assert!(tableau.then(&inverse).expect("T then T^-1").is_identity());
    assert!(inverse.then(&tableau).expect("T^-1 then T").is_identity());
    for row in SAMPLE_ROWS {
        for basis in [PauliBasis::X, PauliBasis::Z] {
            let preimage = match basis {
                PauliBasis::X => inverse.x_output(row),
                _ => inverse.z_output(row),
            }
            .expect("inverse image");
            assert_eq!(
                reference_apply(&tableau, &preimage),
                generator(WIDTH, row, basis),
                "{basis:?}_{row}"
            );
        }
    }

    let unsigned = tableau.inverse_skipping_signs().expect("unsigned inverse");
    for row in SAMPLE_ROWS {
        assert_eq!(
            unsigned.x_output(row).expect("unsigned X"),
            inverse
                .x_output(row)
                .expect("signed X")
                .with_sign(PauliSign::Plus)
        );
    }
    assert_eq!(
        tableau.pow(-1).expect("negative first power"),
        inverse,
        "negative powers start from the signed inverse"
    );
}

#[test]
fn random_tableaux_beyond_1000_qubits_satisfy_the_commutation_relations() {
    let mut rng = SmallRng::seed_from_u64(0x5a7_1030);
    let tableau = Tableau::random(WIDTH, &mut rng).expect("random Tableau");
    assert!(tableau.satisfies_invariants().expect("invariant check"));
    let checked = Tableau::from_conjugated_generators(
        (0..WIDTH)
            .map(|row| tableau.x_output(row).expect("X image"))
            .collect(),
        (0..WIDTH)
            .map(|row| tableau.z_output(row).expect("Z image"))
            .collect(),
    )
    .expect("checked reconstruction");
    assert_eq!(checked, tableau);
}

#[test]
fn power_by_squaring_matches_repeated_composition_and_huge_exponents() {
    let mut rng = SmallRng::seed_from_u64(0x9_0e5);
    let tableau = Tableau::random(70, &mut rng).expect("random Tableau");
    let inverse = tableau.inverse().expect("inverse");
    let identity = Tableau::identity(70).expect("identity");
    let mut forward = identity.clone();
    let mut backward = identity;
    for exponent in 0..=9_i64 {
        assert_eq!(tableau.pow(exponent).expect("power"), forward, "{exponent}");
        assert_eq!(
            tableau.pow(-exponent).expect("power"),
            backward,
            "-{exponent}"
        );
        forward = forward.then(&tableau).expect("forward step");
        backward = backward.then(&inverse).expect("backward step");
    }

    let half = tableau.pow(1 << 61).expect("2^61 power");
    assert_eq!(
        tableau.pow(1 << 62).expect("2^62 power"),
        half.then(&half).expect("square of 2^61 power")
    );
    // i64::MIN is -(i64::MAX + 1), whose magnitude has no positive i64 form.
    assert_eq!(
        tableau.pow(i64::MIN).expect("minimum power"),
        tableau
            .pow(i64::MAX)
            .expect("maximum power")
            .then(&tableau)
            .expect("2^63 power")
            .inverse()
            .expect("inverse of 2^63 power")
    );
}

#[test]
fn stabilizer_solver_reproduces_stabilizers_beyond_1000_qubits() {
    let mut rng = SmallRng::seed_from_u64(0x501_7e5);
    let source = Tableau::random(WIDTH, &mut rng).expect("random Tableau");
    let stabilizers = (0..WIDTH)
        .map(|row| source.z_output(row).expect("stabilizer"))
        .collect::<Vec<_>>();

    let solved = stabilizers_to_tableau(&stabilizers, false, false, false).expect("solve");
    for (row, stabilizer) in stabilizers.iter().enumerate() {
        assert_eq!(
            &solved.z_output(row).expect("solved Z"),
            stabilizer,
            "Z_{row}"
        );
    }
    for row in SAMPLE_ROWS {
        let destabilizer = solved.x_output(row).expect("destabilizer");
        for (other, stabilizer) in stabilizers.iter().enumerate() {
            assert_eq!(
                destabilizer.commutes(stabilizer).expect("commutation"),
                other != row,
                "X_{row} against Z_{other}"
            );
        }
    }
    assert_eq!(
        stabilizers_to_tableau(&stabilizers, false, false, true).expect("inverse solve"),
        solved.inverse().expect("inverse of solved")
    );

    // A product of earlier stabilizers is redundant, and its negation contradicts them.
    let product = stabilizers[3]
        .multiply_real(&stabilizers[700])
        .expect("commuting product");
    let mut redundant = stabilizers.clone();
    redundant.insert(701, product.clone());
    assert_eq!(
        stabilizers_to_tableau(&redundant, true, false, false).expect("redundant solve"),
        solved
    );
    // Error paths stop at the offending generator, so keep it early.
    let mut early = stabilizers[..20].to_vec();
    let early_product = stabilizers[3]
        .multiply_real(&stabilizers[7])
        .expect("commuting product");
    early.insert(8, early_product.clone());
    assert_matches!(
        stabilizers_to_tableau(&early, false, false, false),
        Err(StabilizerError::RedundantStabilizer { .. })
    );
    early[8] = early_product.with_sign(match early_product.sign() {
        PauliSign::Plus => PauliSign::Minus,
        PauliSign::Minus => PauliSign::Plus,
    });
    assert_matches!(
        stabilizers_to_tableau(&early, true, false, false),
        Err(StabilizerError::InconsistentStabilizer { .. })
    );
    // Like Stim, the first anticommuting pair in input order is reported, later member first.
    early[8] = solved.x_output(10).expect("destabilizer of Z_10");
    assert_eq!(
        stabilizers_to_tableau(&early, false, false, false),
        Err(StabilizerError::AntiCommutingStabilizer {
            stabilizer: stabilizers[10].to_string(),
            conflict: early[8].to_string(),
        })
    );

    // Too few generators are completed only on request, keeping the given ones in order.
    let kept = &stabilizers[..WIDTH - 5];
    let completed = stabilizers_to_tableau(kept, false, true, false).expect("completion");
    for row in SAMPLE_ROWS.into_iter().filter(|row| *row < WIDTH - 5) {
        assert_eq!(&completed.z_output(row).expect("completed Z"), &kept[row]);
    }
    for row in WIDTH - 5..WIDTH {
        let completion = completed.z_output(row).expect("completion Z");
        assert!(
            kept.iter()
                .all(|stabilizer| completion.commutes(stabilizer).expect("commutes"))
        );
    }
}
