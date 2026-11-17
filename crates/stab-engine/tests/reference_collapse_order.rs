#![allow(
    clippy::expect_used,
    reason = "integration tests use deterministic valid circuits and exact assertions"
)]

//! Regression pins for the outcome Stim v1.16.0's reference sample forces on random X- and
//! Y-basis collapses.
//!
//! Stim forces every random outcome to false, but `collapse_x` and `collapse_y` collapse the
//! distinct targets of one instruction of its noiseless circuit (where adjacent instructions of
//! one gate and tag fuse once noise is dropped) in ascending qubit order, and `MXX`/`MYY`
//! collapse each disjoint segment by ascending first qubit. When targets are correlated, that
//! order decides which outcome is forced, and with it `m2d` output on non-deterministic
//! detectors. Each expected record is `stim m2d` output of pinned Stim v1.16.0 on an all-zero
//! measurement record with one detector per measurement. The target-order reference that
//! preceded this contract disagrees with every case except the two that pin where fusion stops.

use stab_engine::{ReferenceSampleLoopPolicy, SamplingCompiler};
use stab_model::Circuit;

fn reference(text: &str, policy: ReferenceSampleLoopPolicy) -> String {
    let circuit = Circuit::from_stim_str(text).expect("test circuit should parse");
    SamplingCompiler::new()
        .reference_sample_loop_policy(policy)
        .compile(&circuit)
        .expect("compile sampler")
        .try_reference_sample()
        .expect("reference sample")
        .into_iter()
        .map(|bit| if bit { '1' } else { '0' })
        .collect()
}

#[test]
fn random_x_and_y_collapses_force_the_outcome_pinned_stim_forces() {
    let repeated = format!("0{}", "010".repeat(200));
    let cases: &[(&str, &str)] = &[
        // `MY 1` is random; Stim collapses qubit 0 first, which decides qubit 1.
        ("MPP X0*X1\nMY 1 0\n", "010"),
        // Noise and flip arguments vanish from the noiseless circuit, which fuses both MY.
        ("MPP X0*X1\nMY(0.1) 1\nDEPOLARIZE1(0.1) 0\nMY 0\n", "010"),
        // A TICK, or a differing tag, keeps the instructions apart and the order sequential.
        ("MPP X0*X1\nMY 1\nTICK\nMY 0\n", "001"),
        ("MPP X0*X1\nMY 1\nMY[t] 0\n", "001"),
        // Pair segments collapse by ascending first qubit, across fused instructions.
        ("H 0\nCX 0 1 0 2 0 3\nZ 0\nMXX 2 3 0 1\n", "10"),
        ("H 0\nCX 0 1 0 2 0 3\nZ 0\nMXX 2 3\nMXX 0 1\n", "10"),
        (
            "H 0\nCX 0 1 0 2 0 3\nZ 0\nH_XY 0 1 2 3\nMYY 2 3 0 1\n",
            "10",
        ),
        // Measure-resets collapse every target before recording or resetting any.
        ("H 0\nCX 0 1 0 2 0 3\nZ 0\nMRX 3 2 1 0\n", "1000"),
        // A reset's collapse order shows through a later measurement.
        (
            "SQRT_X 0\nCY 0 1\nSQRT_X 0\nCZ 0 2\nCY 0 2\nZ 2\nCY 1 2\nRX 1 0\nMX 2\n",
            "1",
        ),
        // The order holds in every iteration of a folded repeat.
        (
            "MPP X0*X1\nREPEAT 200 {\n    MPP X0*X1\n    MY 1 0\n}\n",
            &repeated,
        ),
    ];
    for policy in [
        ReferenceSampleLoopPolicy::Fold,
        ReferenceSampleLoopPolicy::Iterate,
    ] {
        for (text, expected) in cases {
            assert_eq!(reference(text, policy), *expected, "{policy:?} {text:?}");
        }
    }
}
