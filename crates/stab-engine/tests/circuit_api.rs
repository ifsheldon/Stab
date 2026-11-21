#![allow(
    clippy::expect_used,
    reason = "circuit execution tests use direct assertions for compact diagnostics"
)]

mod support;

use std::assert_matches;

use stab_engine::{
    DetectionConversionLimits, DetectionError, InitialState, MeasurementToDetectionCompiler,
    ReferenceSample, ReferenceSampleTree, SamplingCompiler, circuit_reference_signs,
    detection_record_width,
};
use stab_model::Circuit;

use support::SamplingFixture;

/// The reference sample of `circuit`'s default-compiled sampling plan, with a failure of either
/// step reported as its message.
fn circuit_reference_sample(circuit: &Circuit) -> Result<ReferenceSample, String> {
    SamplingCompiler::default()
        .compile(circuit)
        .map_err(|error| error.to_string())?
        .try_reference_sample()
        .map_err(|error| error.to_string())
}

/// The determined-measurement count of `circuit`'s default-compiled sampling plan.
fn count_determined_measurements(
    circuit: &Circuit,
    initial_state: InitialState,
) -> Result<u64, String> {
    SamplingCompiler::default()
        .compile(circuit)
        .map_err(|error| error.to_string())?
        .try_count_determined_measurements(initial_state)
        .map_err(|error| error.to_string())
}

fn reference_bits(circuit: &Circuit) -> Vec<bool> {
    circuit_reference_sample(circuit)
        .expect("reference sample")
        .iter()
        .collect()
}

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
        MeasurementToDetectionCompiler::default()
            .compile(&circuit)
            .expect("compile conversion")
            .measurement_width()
            .get(),
        2
    );
    assert_eq!(
        detection_record_width(&circuit, &DetectionConversionLimits::default())
            .expect("detection record width"),
        5
    );
}

#[test]
fn pf1_circuit_reference_determined_reference_sample_matches_compiled_sampler() {
    let empty_measurement_circuit = Circuit::from_stim_str("H 0\nCX 0 1\n").expect("parse");
    assert_eq!(
        reference_bits(&empty_measurement_circuit),
        Vec::<bool>::new()
    );

    let simple_reference = Circuit::from_stim_str("X 0\nM 0\n").expect("parse");
    assert_eq!(reference_bits(&simple_reference), vec![true]);

    let sweep_controlled = Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse");
    assert_eq!(reference_bits(&sweep_controlled), vec![true]);

    for source in ["X 0\nCX 0 sweep[0]\nM 0\n", "X 0\nCY 0 sweep[0]\nM 0\n"] {
        let invalid_sweep_order =
            Circuit::from_stim_str(source).expect("parse invalid sweep order");
        let error = circuit_reference_sample(&invalid_sweep_order)
            .expect_err("reject invalid sampler sweep target order");
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

    assert_eq!(reference_bits(&circuit), expected);
    assert_eq!(
        expected.len(),
        usize::try_from(circuit.count_measurements().expect("measurements"))
            .expect("measurement count fits usize")
    );
    assert!(expected.iter().any(|bit| *bit));
}

#[test]
fn circuit_reference_signs_match_stim_ordering_and_reference_semantics() {
    let cases: &[(&str, &str, &[bool], &[bool])] = &[
        (
            "documented sparse observable",
            "X 1\nM 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\n\
             OBSERVABLE_INCLUDE(3) rec[-1] rec[-2]\n",
            &[true, false],
            &[false, false, false, true],
        ),
        (
            "folded order and Pauli term",
            "X 0\nREPEAT 2 {\nM 0\nDETECTOR rec[-1]\n\
             OBSERVABLE_INCLUDE(2) X0 rec[-1]\nX 0\n}\n",
            &[true, false],
            &[false, false, true],
        ),
        (
            "duplicate cancellation and noiseless reference",
            "X 0\nX_ERROR(1) 0\nM(1) 0\nDETECTOR rec[-1] rec[-1]\n\
             OBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
            &[false],
            &[false],
        ),
        (
            "all-zero sweep",
            "X 0\nCX sweep[2] 0\nM 0\nDETECTOR rec[-1]\n",
            &[true],
            &[],
        ),
        (
            "XCZ Pauli observable with sweep",
            "RX 0\nXCZ 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n",
            &[],
            &[false],
        ),
        (
            "YCZ Pauli observable with sweep",
            "RX 0\nYCZ 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n",
            &[],
            &[false],
        ),
        ("empty circuit", "", &[], &[]),
    ];
    for (name, source, expected_detectors, expected_observables) in cases {
        let circuit = Circuit::from_stim_str(source).expect("parse reference-sign case");
        let signs = circuit_reference_signs(&circuit, &DetectionConversionLimits::default())
            .expect("compute reference-sign case");
        let detectors = signs.detector_signs().iter().collect::<Vec<_>>();
        let observables = signs.observable_signs().iter().collect::<Vec<_>>();
        assert_eq!(detectors, *expected_detectors, "{name}");
        assert_eq!(observables, *expected_observables, "{name}");
    }

    // Signs straddling word boundaries pack in declaration order with a masked tail word.
    let mut wide = String::new();
    for qubit in 0..130 {
        if qubit % 3 == 0 {
            wide.push_str(&format!("X {qubit}\n"));
        }
        wide.push_str(&format!(
            "M {qubit}\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE({qubit}) rec[-1]\n"
        ));
    }
    let circuit = Circuit::from_stim_str(&wide).expect("parse word-straddling case");
    let signs = circuit_reference_signs(&circuit, &DetectionConversionLimits::default())
        .expect("compute word-straddling case");
    let expected = (0..130).map(|qubit| qubit % 3 == 0).collect::<Vec<_>>();
    for slice in [signs.detector_signs(), signs.observable_signs()] {
        assert_eq!(slice.len(), 130);
        assert_eq!(slice.iter().collect::<Vec<_>>(), expected);
        assert_eq!(slice.words().len(), 3);
        assert_eq!(slice.popcount(), 44);
        assert_eq!(
            slice.words().last().map(|word| word >> 2),
            Some(0),
            "bits past the last sign stay clear"
        );
    }

    let invalid =
        Circuit::from_stim_str("DETECTOR rec[-1]\n").expect("parse unavailable record target");
    assert_matches!(
        circuit_reference_signs(&invalid, &DetectionConversionLimits::default()),
        Err(DetectionError::InvalidResultFormat { .. })
    );
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
            .expect("decompress repeated reference sample tree")
            .iter()
            .collect::<Vec<_>>(),
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
        count_determined_measurements(&tagged, InitialState::Zero).expect("count determined"),
        1
    );

    let unknown_input = Circuit::from_stim_str(
        "MPP Z0*Z1 X2*X3\n\
         TICK\n\
         MPP Z0*Z1 X2*X3\n",
    )
    .expect("parse unknown-input circuit");
    assert_eq!(
        count_determined_measurements(&unknown_input, InitialState::Unknown)
            .expect("count with unknown input"),
        2
    );
    assert_eq!(
        count_determined_measurements(&unknown_input, InitialState::Zero)
            .expect("count with known zero input"),
        3
    );

    let sweep_controlled =
        Circuit::from_stim_str("X 0\nCX sweep[0] 0\nM 0\n").expect("parse sweep circuit");
    assert_eq!(
        count_determined_measurements(&sweep_controlled, InitialState::Zero)
            .expect("count deterministic sweep circuit"),
        1
    );
    assert_eq!(
        count_determined_measurements(&sweep_controlled, InitialState::Unknown)
            .expect("count unknown-input sweep circuit"),
        0
    );
}
