#![allow(
    clippy::expect_used,
    reason = "PF2 QEC inverse reset-measure-detector parity tests mirror compact upstream examples"
)]

use stab_core::{
    Circuit, InverseQecOptions, circuit_inverse_qec, circuit_inverse_qec_with_options,
};

#[test]
fn circuit_inverse_qec_supports_reset_measure_detector_triplet() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_inverse_qec.test.cc.
    let input = circuit(
        "
        R 0
        M 0
        DETECTOR rec[-1]
    ",
    );

    assert_eq!(circuit_inverse_qec(&input).expect("inverse R/M/D"), input);
    assert_eq!(input.inverse_qec().expect("method inverse R/M/D"), input);
}

#[test]
fn circuit_inverse_qec_with_options_keeps_selected_reset_measure_detector_measurements() {
    // Adapted from Stim v1.16.0 src/stim/util_top/circuit_inverse_qec.test.cc
    // r_m_det_keep_m.
    let input = circuit(
        "
        R 0
        M 0
        DETECTOR rec[-1]
    ",
    );
    let expected = circuit(
        "
        M 0
        M 0
        DETECTOR rec[-2] rec[-1]
    ",
    );
    let options = InverseQecOptions {
        keep_measurements: true,
    };

    assert_eq!(
        circuit_inverse_qec_with_options(&input, options).expect("inverse R/M/D keep M"),
        expected
    );
    assert_eq!(
        input
            .inverse_qec_with_options(options)
            .expect("method inverse R/M/D keep M"),
        expected
    );
    assert_eq!(
        circuit_inverse_qec_with_options(&input, InverseQecOptions::default())
            .expect("default options inverse R/M/D"),
        input
    );
}

#[test]
fn circuit_inverse_qec_supports_reset_measure_detector_selected_bases_and_metadata() {
    for circuit_text in [
        "
        RX 1
        MX 1
        DETECTOR[tag](2, 3) rec[-1]
        ",
        "
        RY 2
        MY 2
        DETECTOR(5) rec[-1]
        ",
    ] {
        let input = circuit(circuit_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse selected basis R/M/D"),
            input,
            "{circuit_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_supports_multi_target_reset_measure_detector_parity() {
    // Adapted from Stim v1.16.0 circuit_inverse_qec reset/measure/detector behavior.
    for (input_text, expected_text) in [
        (
            "
            R 0 1
            M 0 1
            DETECTOR rec[-2] rec[-1]
            ",
            "
            R 1 0
            M 1 0
            DETECTOR rec[-2] rec[-1]
            ",
        ),
        (
            "
            RX 0 1
            MX 0 1
            DETECTOR rec[-2] rec[-1]
            ",
            "
            RX 1 0
            MX 1 0
            DETECTOR rec[-2] rec[-1]
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            DETECTOR rec[-2]
            ",
            "
            M 1
            R 0
            M 1 0
            DETECTOR rec[-1]
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            DETECTOR rec[-1]
            ",
            "
            R 1
            M 0 1 0
            DETECTOR rec[-2]
            ",
        ),
        (
            "
            R 0 1 2
            M 0 1 2
            DETECTOR rec[-3] rec[-1]
            ",
            "
            R 2
            M 1
            R 0
            M 2 1 0
            DETECTOR rec[-3] rec[-1]
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse multi-target R/M/D"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_simplifies_reset_measure_detector_record_parity() {
    for (input_text, expected_text) in [
        (
            "
            R 0
            M 0
            DETECTOR[tag](2, 3) rec[-1] rec[-1]
            ",
            "
            R 0
            M 0
            ",
        ),
        (
            "
            R 0
            M 0
            DETECTOR[tag](2, 3) rec[-1] rec[-1] rec[-1]
            ",
            "
            R 0
            M 0
            DETECTOR[tag](2, 3) rec[-1]
            ",
        ),
        (
            "
            R 0
            M 0
            DETECTOR
            ",
            "
            M 0 0
            ",
        ),
        (
            "
            R 0 1
            M 0 1
            DETECTOR
            ",
            "
            M 1 0 1 0
            ",
        ),
    ] {
        let input = circuit(input_text);
        let expected = circuit(expected_text);

        assert_eq!(
            circuit_inverse_qec(&input).expect("inverse detector parity"),
            expected,
            "{input_text}"
        );
    }
}

#[test]
fn circuit_inverse_qec_keep_measurements_rejects_unpromoted_packets() {
    let input = circuit(
        "
        R 0
        M 0
        MR 0
        DETECTOR rec[-1]
    ",
    );
    let error = circuit_inverse_qec_with_options(
        &input,
        InverseQecOptions {
            keep_measurements: true,
        },
    )
    .expect_err("keep_measurements is scoped to reset-measure-detector")
    .to_string();

    assert!(
        error.contains("keep_measurements is currently supported only"),
        "{error}"
    );
    assert!(circuit_inverse_qec(&input).is_ok());
}

#[test]
fn circuit_inverse_qec_keep_measurements_rejects_broader_reset_measure_detector_variants() {
    for circuit_text in [
        "
        RX 0
        MX 0
        DETECTOR rec[-1]
        ",
        "
        R 0 1
        M 0 1
        DETECTOR rec[-2] rec[-1]
        ",
        "
        R 0 1
        M 0 1
        DETECTOR rec[-2]
        ",
        "
        R 0
        M 0
        DETECTOR rec[-1] rec[-1]
        ",
        "
        R 0
        M 0
        DETECTOR
        ",
        "
        R 0
        M 0
        DETECTOR[tag](2, 3) rec[-1]
        ",
    ] {
        let input = circuit(circuit_text);
        let error = circuit_inverse_qec_with_options(
            &input,
            InverseQecOptions {
                keep_measurements: true,
            },
        )
        .expect_err("keep_measurements is exact-scope only")
        .to_string();

        assert!(
            error.contains("keep_measurements is currently supported only"),
            "{circuit_text}\n{error}"
        );
        assert!(circuit_inverse_qec(&input).is_ok(), "{circuit_text}");
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}
