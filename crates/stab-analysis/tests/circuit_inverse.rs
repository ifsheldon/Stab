#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "M6 inverse-circuit parity tests mirror compact upstream examples"
)]

use stab_algebra::Tableau;
use stab_analysis::{CircuitToTableauOptions, circuit_inverse_unitary, circuit_to_tableau};
use stab_model::Circuit;

#[test]
fn circuit_inverse_unitary_matches_stim_example() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_inverse_unitary.test.cc.
    let input = circuit(
        "
        H 0
        ISWAP 0 1 1 2 3 2
        S 0 3 4
    ",
    );
    let expected = circuit(
        "
        S_DAG 4 3 0
        ISWAP_DAG 3 2 1 2 0 1
        H 0
    ",
    );

    assert_eq!(circuit_inverse_unitary(&input).expect("inverse"), expected);
}

#[test]
fn circuit_inverse_unitary_matches_stim_spp_and_annotation_contract() {
    let input = circuit(
        "
        QUBIT_COORDS[q](1, 2) 0 1
        TICK[t]
        SPP[s] X0*Y1 Z2
        SHIFT_COORDS[shift](4, -5)
        REPEAT[loop] 2 {
            QUBIT_COORDS[inner](3) 3 4
            TICK[nested]
            SPP_DAG[p] X3*Z4 Y5
            SHIFT_COORDS[inner_shift](1, 2)
        }
        ",
    );
    let expected = circuit(
        "
        QUBIT_COORDS[q](1, 2) 1 0
        REPEAT[loop] 2 {
            QUBIT_COORDS[inner](3) 4 3
            SHIFT_COORDS[inner_shift](-1, -2)
            SPP[p] Y5 Z4*X3
            TICK[nested]
        }
        SHIFT_COORDS[shift](-4, 5)
        SPP_DAG[s] Z2 Y1*X0
        TICK[t]
        ",
    );

    assert_eq!(circuit_inverse_unitary(&input).expect("inverse"), expected);
}

#[test]
fn circuit_inverse_unitary_rejects_measurements_like_stim() {
    assert!(circuit_inverse_unitary(&circuit("M 0")).is_err());
}

#[test]
fn circuit_inverse_unitary_rejects_nonleading_coordinates_and_noninvertible_operations() {
    for text in [
        "H 0\nQUBIT_COORDS(1) 0\n",
        "DETECTOR\n",
        "X_ERROR(0.125) 0\n",
    ] {
        assert!(circuit_inverse_unitary(&circuit(text)).is_err(), "{text}");
    }
}

#[test]
fn circuit_inverse_unitary_composes_to_identity_tableau() {
    let input = circuit(
        "
        SQRT_Y_DAG 1
        CZ 0 1
        SQRT_Y 1
        S 0
    ",
    );
    let inverse = circuit_inverse_unitary(&input).expect("inverse");
    let input_tableau =
        circuit_to_tableau(&input, &CircuitToTableauOptions::default()).expect("input tableau");
    let inverse_tableau =
        circuit_to_tableau(&inverse, &CircuitToTableauOptions::default()).expect("inverse tableau");
    assert_eq!(
        input_tableau
            .then(&inverse_tableau)
            .expect("compose inverse"),
        Tableau::identity(2).expect("Tableau identity")
    );
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

/// Inversion rebuilds repeat bodies with its own stack, so nesting far beyond the parser's
/// envelope costs heap rather than native stack.
#[test]
fn circuit_inverse_unitary_handles_deep_nesting_without_native_recursion() {
    const DEPTH: usize = 2048;
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let mut nested = circuit("QUBIT_COORDS(1) 0\nS 0\nCX 0 1\n");
            for _ in 0..DEPTH {
                let mut outer = circuit("H 0\n");
                outer.append_repeat_block(stab_model::RepeatBlock::new(
                    stab_model::RepeatCount::try_new(3).expect("count"),
                    nested,
                    None,
                ));
                nested = outer;
            }
            let inverse = circuit_inverse_unitary(&nested).expect("deep inverse");
            let mut body = &inverse;
            for _ in 0..DEPTH {
                let [
                    stab_model::CircuitItem::RepeatBlock(repeat),
                    stab_model::CircuitItem::Instruction(h),
                ] = body.items()
                else {
                    panic!("each level inverts to its repeat followed by H");
                };
                assert_eq!(repeat.repeat_count().get(), 3);
                assert_eq!(h.gate().canonical_name(), "H");
                body = repeat.body();
            }
            assert_eq!(
                body,
                &circuit("QUBIT_COORDS(1) 0\nCX 0 1\nS_DAG 0\n"),
                "leading coordinates stay first in the innermost body"
            );
        })
        .expect("spawn")
        .join()
        .expect("deep inverse thread");
}
