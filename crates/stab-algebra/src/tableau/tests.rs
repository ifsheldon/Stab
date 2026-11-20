#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing,
    reason = "seeded differential fixtures make setup failures test bugs"
)]

//! Differential tests of the table-driven append, the batched appender, and prepend.
//!
//! The references are the per-bit append that preceded the table kernels and, for prepend,
//! composition with the embedded gate. Every one- and two-qubit Clifford, signs included, is
//! checked at widths around the 64-bit word and tile boundaries, with random targets and random
//! signed base tableaux, plus random three- to five-qubit Cliffords for the general paths.

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};

use super::{Tableau, TableauAppender, reference};
use crate::{SignIteration, StabilizerError, TableauIterator};

const WIDTHS: [usize; 10] = [1, 2, 3, 63, 64, 65, 127, 128, 129, 200];
/// Widths at which every two-qubit gate is also checked alone on one fixed base tableau,
/// including prepend against an `O(n^3)` composition.
const EXHAUSTIVE_PREPEND_WIDTHS: [usize; 3] = [2, 3, 5];

fn all_gates(num_qubits: usize) -> Vec<Tableau> {
    TableauIterator::new(num_qubits, SignIteration::AllSigns)
        .unwrap()
        .collect()
}

fn random_targets(rng: &mut SmallRng, count: usize, num_qubits: usize) -> Vec<usize> {
    let mut targets = Vec::with_capacity(count);
    while targets.len() < count {
        let target = rng.random_range(0..num_qubits);
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

fn reference_append(tableau: &Tableau, gate: &Tableau, targets: &[usize]) -> Tableau {
    let mut expected = tableau.clone();
    reference::append_per_bit(&mut expected.rows, gate, targets).unwrap();
    expected
}

fn check_append_routes(base: &Tableau, gate: &Tableau, targets: &[usize]) {
    let expected = reference_append(base, gate, targets);
    let mut actual = base.clone();
    actual.append(gate, targets).unwrap();
    assert_eq!(
        actual,
        expected,
        "append {targets:?} at width {}",
        base.len()
    );
    let mut appender = TableauAppender::new(base.clone()).unwrap();
    appender.append(gate, targets).unwrap();
    assert_eq!(
        appender.finish().unwrap(),
        expected,
        "appender {targets:?} at width {}",
        base.len()
    );
}

fn check_prepend(base: &Tableau, gate: &Tableau, targets: &[usize]) {
    let expected = gate
        .embedded(base.len(), targets)
        .unwrap()
        .then(base)
        .unwrap();
    let mut actual = base.clone();
    actual.prepend(gate, targets).unwrap();
    assert_eq!(
        actual,
        expected,
        "prepend {targets:?} at width {}",
        base.len()
    );
}

/// Checks every one- and two-qubit Clifford at `widths`; the widths are split over several tests
/// so that the debug-build reference runs in parallel.
fn check_every_small_gate(widths: &[usize], seed: u64) {
    let one = all_gates(1);
    let two = all_gates(2);
    assert_eq!((one.len(), two.len()), (24, 11_520));
    let mut rng = SmallRng::seed_from_u64(seed);
    for width in widths.iter().copied() {
        let base = Tableau::random(width, &mut rng).unwrap();
        // Every gate in turn on one evolving tableau, so each gate meets a fresh signed state.
        let mut expected = base.clone();
        let mut fast = base.clone();
        let mut appender = TableauAppender::new(base.clone()).unwrap();
        let gates = one
            .iter()
            .chain(if width >= 2 { two.iter() } else { [].iter() });
        for (index, gate) in gates.enumerate() {
            let targets = random_targets(&mut rng, gate.len(), width);
            reference::append_per_bit(&mut expected.rows, gate, &targets).unwrap();
            fast.append(gate, &targets).unwrap();
            appender.append(gate, &targets).unwrap();
            if index % 1024 == 0 {
                assert_eq!(fast, expected, "append gate {index} at width {width}");
            }
        }
        assert_eq!(fast, expected, "append at width {width}");
        assert_eq!(
            appender.finish().unwrap(),
            expected,
            "appender at width {width}"
        );
        for gate in &one {
            check_prepend(&base, gate, &random_targets(&mut rng, 1, width));
        }
        if width >= 2 {
            for _ in 0..32 {
                let gate = &two[rng.random_range(0..two.len())];
                check_prepend(&base, gate, &random_targets(&mut rng, 2, width));
            }
        }
    }
}

#[test]
fn every_small_gate_matches_the_references_at_narrow_widths() {
    check_every_small_gate(&WIDTHS[..5], 0x7ab1e);
}

#[test]
fn every_small_gate_matches_the_references_past_one_word() {
    check_every_small_gate(&WIDTHS[5..7], 0x7ab1f);
}

#[test]
fn every_small_gate_matches_the_references_past_two_words() {
    check_every_small_gate(&WIDTHS[7..9], 0x7ab20);
}

#[test]
fn every_small_gate_matches_the_references_at_200_qubits() {
    check_every_small_gate(&WIDTHS[9..], 0x7ab21);
}

#[test]
fn every_two_qubit_gate_alone_matches_the_references() {
    let mut rng = SmallRng::seed_from_u64(0x2_9a7e);
    let two = all_gates(2);
    for width in EXHAUSTIVE_PREPEND_WIDTHS {
        let base = Tableau::random(width, &mut rng).unwrap();
        for gate in &two {
            let targets = random_targets(&mut rng, 2, width);
            check_append_routes(&base, gate, &targets);
            check_prepend(&base, gate, &targets);
        }
    }
}

/// The two-qubit gates with dedicated prepend paths (CX both ways, CZ, XCX, SWAP) and gates whose
/// compile-time programs use only row products (SQRT_XX, SQRT_ZZ, CY) or products and swaps
/// (ISWAP), each with all sixteen output-sign choices, at multi-word widths where the exhaustive
/// test cannot afford every gate.
#[test]
fn dedicated_two_qubit_prepends_match_composition_with_every_sign_at_wide_widths() {
    let gates: [[&str; 4]; 9] = [
        ["XX", "Z_", "_X", "ZZ"],
        ["X_", "ZZ", "XX", "_Z"],
        ["XZ", "Z_", "ZX", "_Z"],
        ["X_", "ZX", "_X", "XZ"],
        ["_X", "_Z", "X_", "Z_"],
        ["X_", "YX", "_X", "XY"],
        ["YZ", "Z_", "ZY", "_Z"],
        ["ZY", "_Z", "YZ", "Z_"],
        ["XY", "Z_", "ZX", "ZZ"],
    ];
    let mut rng = SmallRng::seed_from_u64(0x5_19e5);
    for width in [64, 65, 130, 200] {
        let base = Tableau::random(width, &mut rng).unwrap();
        for images in gates {
            for signs in 0..16_u8 {
                let [x0, z0, x1, z1] = std::array::from_fn(|output| {
                    let sign = if signs >> output & 1 == 0 { '+' } else { '-' };
                    format!("{sign}{}", images[output])
                });
                let gate = Tableau::gate2(&x0, &z0, &x1, &z1).unwrap();
                for targets in [[3, width - 2], [width - 1, 0]] {
                    check_prepend(&base, &gate, &targets);
                }
            }
        }
    }
}

#[test]
fn wider_random_cliffords_match_the_references() {
    let mut rng = SmallRng::seed_from_u64(0x3_5eed);
    for width in WIDTHS.into_iter().filter(|width| *width >= 5) {
        let base = Tableau::random(width, &mut rng).unwrap();
        for arity in 3..=5 {
            for _ in 0..4 {
                let gate = Tableau::random(arity, &mut rng).unwrap();
                let targets = random_targets(&mut rng, arity, width);
                check_append_routes(&base, &gate, &targets);
                check_prepend(&base, &gate, &targets);
            }
        }
    }
}

#[test]
fn appender_sequences_match_repeated_per_bit_appends() {
    let two = all_gates(2);
    let one = all_gates(1);
    let mut rng = SmallRng::seed_from_u64(0xa99e);
    for width in WIDTHS {
        let base = Tableau::random(width, &mut rng).unwrap();
        let mut expected = base.clone();
        let mut fast = base.clone();
        let mut appender = TableauAppender::new(base).unwrap();
        for step in 0..300 {
            let gate = match step % 7 {
                0 if width >= 3 => Tableau::random(3, &mut rng).unwrap(),
                1..=3 if width >= 2 => two[rng.random_range(0..two.len())].clone(),
                _ => one[rng.random_range(0..one.len())].clone(),
            };
            let targets = random_targets(&mut rng, gate.len(), width);
            expected = reference_append(&expected, &gate, &targets);
            fast.append(&gate, &targets).unwrap();
            appender.append(&gate, &targets).unwrap();
        }
        assert_eq!(fast, expected, "append sequence at width {width}");
        assert_eq!(
            appender.finish().unwrap(),
            expected,
            "appender sequence at width {width}"
        );
    }
}

#[test]
fn invalid_targets_are_refused_before_any_change() {
    let mut rng = SmallRng::seed_from_u64(9);
    let base = Tableau::random(4, &mut rng).unwrap();
    let cnot = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").unwrap();
    let cases: [(&[usize], StabilizerError); 3] = [
        (
            &[1],
            StabilizerError::TableauTargetCountMismatch {
                tableau_qubits: 2,
                target_count: 1,
            },
        ),
        (
            &[1, 1],
            StabilizerError::DuplicateTableauTarget { target: 1 },
        ),
        (
            &[1, 4],
            StabilizerError::TableauTargetOutOfRange {
                target: 4,
                num_qubits: 4,
            },
        ),
    ];
    for (targets, error) in cases {
        let mut prepended = base.clone();
        assert_eq!(prepended.prepend(&cnot, targets), Err(error.clone()));
        assert_eq!(prepended, base);
        let mut appender = TableauAppender::new(base.clone()).unwrap();
        assert_eq!(appender.append(&cnot, targets), Err(error));
        assert_eq!(appender.finish().unwrap(), base);
    }
}

#[test]
fn appender_identity_matches_the_tableau_identity_and_its_admission() {
    for width in [0, 1, 31, 32, 33, 64, 65] {
        let exact = u64::try_from(crate::budget::tableau_bytes(width)).unwrap();
        let budget = crate::AlgebraMemoryBudget::from_bytes(exact);
        let appender = TableauAppender::identity_with_budget(width, budget).unwrap();
        assert_eq!(
            appender.finish().unwrap(),
            Tableau::identity(width).unwrap()
        );
        if let Some(short) = exact.checked_sub(1) {
            let budget = crate::AlgebraMemoryBudget::from_bytes(short);
            assert_eq!(
                TableauAppender::identity_with_budget(width, budget).err(),
                Tableau::identity_with_budget(width, budget).err(),
            );
        }
    }
}

#[test]
fn appender_entry_and_sign_queries_match_tableau_outputs() {
    let mut rng = SmallRng::seed_from_u64(23);
    for width in [0, 1, 31, 32, 33, 64, 65, 130] {
        let tableau = Tableau::random(width, &mut rng).unwrap();
        let appender = TableauAppender::new(tableau.clone()).unwrap();
        for input in 0..width {
            let x_out = tableau.x_output(input).unwrap();
            let z_out = tableau.z_output(input).unwrap();
            assert_eq!(appender.x_output_sign(input).unwrap(), x_out.sign());
            assert_eq!(appender.z_output_sign(input).unwrap(), z_out.sign());
            for output in 0..width {
                assert_eq!(
                    appender.x_output_basis(input, output).unwrap(),
                    x_out.get(output).unwrap(),
                );
                assert_eq!(
                    appender.z_output_basis(input, output).unwrap(),
                    z_out.get(output).unwrap(),
                );
            }
        }
        let oob = Some(StabilizerError::TableauIndexOutOfRange {
            index: width,
            len: width,
        });
        assert_eq!(appender.x_output_sign(width).err(), oob);
        assert_eq!(appender.z_output_sign(width).err(), oob);
        assert_eq!(appender.x_output_basis(width, 0).err(), oob);
        assert_eq!(appender.z_output_basis(0, width).err(), oob);
    }
}
