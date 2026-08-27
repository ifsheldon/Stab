#![allow(
    clippy::expect_used,
    reason = "PF1 circuit execution tests use direct assertions for compact diagnostics"
)]

mod support;

use stab_engine::{
    ReferenceSampleTree, circuit_reference_sample, count_determined_measurements,
    detection_record_width, measurement_record_count,
};
use stab_model::Circuit;

use support::SamplingFixture;

#[test]
fn pf1_circuit_stats_detection_width_helpers_match_owned_upstream_semantics() {
    let circuit = Circuit::from_stim_str(
        "H 5\n\
         M 0 1\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(3) rec[-2]\n",
    )
    .expect("parse circuit");

    assert_eq!(
        measurement_record_count(&circuit).expect("measurement record count"),
        2
    );
    assert_eq!(
        detection_record_width(&circuit).expect("detection record width"),
        5
    );
}

#[test]
fn pf1_circuit_reference_determined_reference_sample_matches_compiled_sampler() {
    let empty_measurement_circuit = Circuit::from_stim_str("H 0\nCX 0 1\n").expect("parse");
    assert_eq!(
        circuit_reference_sample(&empty_measurement_circuit).expect("reference sample"),
        Vec::<bool>::new()
    );

    let simple_reference = Circuit::from_stim_str("X 0\nM 0\n").expect("parse");
    assert_eq!(
        circuit_reference_sample(&simple_reference).expect("reference sample"),
        vec![true]
    );

    let sweep_controlled = Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse");
    assert_eq!(
        circuit_reference_sample(&sweep_controlled).expect("reference sample"),
        vec![true]
    );

    for source in ["X 0\nCX 0 sweep[0]\nM 0\n", "X 0\nCY 0 sweep[0]\nM 0\n"] {
        let invalid_sweep_order =
            Circuit::from_stim_str(source).expect("parse invalid sweep order");
        let error = circuit_reference_sample(&invalid_sweep_order)
            .expect_err("reject invalid sampler sweep target order")
            .to_string();
        assert!(error.contains("does not support"), "{source}\n{error}");
    }

    let circuit = Circuit::from_stim_str(
        "H 0 1\n\
         CX 0 2 1 3\n\
         MPP X0*X1 Y0*Y1 Z0*Z1\n\
         X 0 2 4 6\n\
         M 0 1 2 3 4 5 6 7\n",
    )
    .expect("parse circuit");
    let expected = SamplingFixture::compile(&circuit)
        .expect("compile sampler")
        .reference_sample()
        .expect("compiled reference sample");

    assert_eq!(
        circuit_reference_sample(&circuit).expect("reference sample"),
        expected
    );
    assert_eq!(
        expected.len(),
        usize::try_from(circuit.count_measurements().expect("measurements"))
            .expect("measurement count fits usize")
    );
    assert!(expected.iter().any(|bit| *bit));
}

#[test]
fn pf1_circuit_reference_determined_reference_sample_tree_decompresses_reference_sample() {
    let circuit = Circuit::from_stim_str("M 0\nX 0\nM 0\n").expect("parse circuit");
    let tree = ReferenceSampleTree::from_circuit_reference_sample(&circuit)
        .expect("reference sample tree");

    assert_eq!(
        tree.decompress().expect("decompress reference sample tree"),
        circuit_reference_sample(&circuit).expect("reference sample")
    );
    assert_eq!(tree.size(), 2);
    assert_eq!(tree.get(0), Some(false));
    assert_eq!(tree.get(1), Some(true));
    assert_eq!(tree.get(2), None);

    let repeated = Circuit::from_stim_str(
        "REPEAT 3 {\n\
             R 0\n\
             M 0\n\
             X 0\n\
             M 0\n\
         }\n",
    )
    .expect("parse repeated circuit");
    let repeated_tree = ReferenceSampleTree::from_circuit_reference_sample(&repeated)
        .expect("reference sample tree");
    assert_eq!(
        repeated_tree
            .decompress()
            .expect("decompress repeated reference sample tree"),
        vec![false, true, false, true, false, true]
    );
    assert_eq!(repeated_tree.size(), 6);
}

#[test]
fn pf1_circuit_reference_determined_count_determined_measurements_matches_public_helper_subset() {
    let tagged = Circuit::from_stim_str(
        "R[test1] 0\n\
         M[test3] 0\n\
         DETECTOR[test4](1, 2) rec[-1]\n",
    )
    .expect("parse tagged circuit");
    assert_eq!(
        count_determined_measurements(&tagged, false).expect("count determined"),
        1
    );

    let unknown_input = Circuit::from_stim_str(
        "MPP Z0*Z1 X2*X3\n\
         TICK\n\
         MPP Z0*Z1 X2*X3\n",
    )
    .expect("parse unknown-input circuit");
    assert_eq!(
        count_determined_measurements(&unknown_input, true).expect("count with unknown input"),
        2
    );
    assert_eq!(
        count_determined_measurements(&unknown_input, false).expect("count with known zero input"),
        3
    );

    let sweep_controlled =
        Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse sweep circuit");
    assert_eq!(
        count_determined_measurements(&sweep_controlled, false)
            .expect("count deterministic sweep circuit"),
        1
    );
    assert_eq!(
        count_determined_measurements(&sweep_controlled, true)
            .expect("count unknown-input sweep circuit"),
        0
    );
}
