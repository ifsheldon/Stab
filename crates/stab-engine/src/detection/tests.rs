#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "detection tests use direct fixture assertions for compact diagnostics"
)]

use super::test_support::{
    DetectionConversionOutput, convert_measurements_to_detection_events,
    convert_measurements_to_detection_events_with_sweep, sample_detection_events,
};
use super::*;

use crate::ReferenceSampleMode;

#[test]
fn conversion_admission_does_not_allocate_detector_term_storage() {
    let circuit =
        Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) X0 rec[-1]\n")
            .expect("parse fixture");
    let detector = circuit
        .items()
        .get(1)
        .and_then(CircuitItem::as_instruction)
        .expect("fixture must contain detector instruction");
    let mut admission = ConversionPlan::new(DetectionConversionLimits::default(), false);
    admission.measurement_count = 1;
    let measured = allocation_counter::measure(|| {
        for _ in 0..4_096 {
            admission
                .record_detector(detector)
                .expect("dry detector admission");
        }
    });
    assert_eq!(measured.count_total, 0, "{measured:?}");

    let observable = circuit
        .items()
        .get(2)
        .and_then(CircuitItem::as_instruction)
        .expect("fixture must contain observable instruction");
    let measured = allocation_counter::measure(|| {
        for _ in 0..4_096 {
            admission
                .record_observable(observable)
                .expect("dry observable admission");
        }
    });
    assert_eq!(measured.count_total, 0, "{measured:?}");
}

fn convert(
    circuit_text: &str,
    measurements: &[&[bool]],
    skip_reference_sample: bool,
) -> DetectionConversionOutput {
    let circuit = Circuit::from_stim_str(circuit_text).expect("parse circuit");
    let measurements = measurements
        .iter()
        .map(|record| record.to_vec())
        .collect::<Vec<_>>();
    convert_measurements_to_detection_events(
        &circuit,
        &measurements,
        reference_mode(skip_reference_sample),
    )
    .expect("convert measurements")
}

fn convert_with_sweep(
    circuit_text: &str,
    measurements: &[&[bool]],
    sweeps: &[&[bool]],
    skip_reference_sample: bool,
) -> DetectionConversionOutput {
    let circuit = Circuit::from_stim_str(circuit_text).expect("parse circuit");
    let measurements = measurements
        .iter()
        .map(|record| record.to_vec())
        .collect::<Vec<_>>();
    let sweeps = sweeps
        .iter()
        .map(|record| record.to_vec())
        .collect::<Vec<_>>();
    convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &measurements,
        &sweeps,
        reference_mode(skip_reference_sample),
    )
    .expect("convert measurements with sweep")
}

fn reference_mode(skip_reference_sample: bool) -> ReferenceSampleMode {
    if skip_reference_sample {
        ReferenceSampleMode::SkipReferenceSample
    } else {
        ReferenceSampleMode::UseReferenceSample
    }
}

#[test]
fn detection_sampling_uses_all_false_default_sweep_bits() {
    let sweep_circuit = Circuit::from_stim_str("H 0\nCX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse sweep-conditioned circuit");
    let explicit_false_circuit =
        Circuit::from_stim_str("H 0\nM 0\nDETECTOR rec[-1]\n").expect("parse explicit circuit");

    validate_detection_sampling_circuit(&sweep_circuit).expect("validate non-frame sweep sampling");
    let sweep_output =
        sample_detection_events(&sweep_circuit, 32, Some(17)).expect("sample sweep circuit");
    let explicit_false_output = sample_detection_events(&explicit_false_circuit, 32, Some(17))
        .expect("sample explicit false circuit");

    assert_eq!(sweep_output.records, explicit_false_output.records);
}

#[test]
fn detection_conversion_uses_reference_sample_for_detectors_and_observables() {
    let output = convert(
        "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
        &[
            &[false, false],
            &[false, true],
            &[true, false],
            &[true, true],
        ],
        false,
    );

    assert_eq!(output.detector_count, 2);
    assert_eq!(output.observable_count, 3);
    assert_eq!(
        output.records,
        vec![
            DetectionRecordBuffer {
                detectors: vec![true, false],
                observables: vec![false, false, false],
            },
            DetectionRecordBuffer {
                detectors: vec![true, true],
                observables: vec![false, false, true],
            },
            DetectionRecordBuffer {
                detectors: vec![false, false],
                observables: vec![false, false, false],
            },
            DetectionRecordBuffer {
                detectors: vec![false, true],
                observables: vec![false, false, true],
            },
        ],
    );
}

#[test]
fn detection_conversion_can_skip_reference_sample() {
    let output = convert(
        "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
        &[
            &[false, false],
            &[false, true],
            &[true, false],
            &[true, true],
        ],
        true,
    );

    assert_eq!(
        output.records,
        vec![
            DetectionRecordBuffer {
                detectors: vec![false, false],
                observables: vec![false, false, false],
            },
            DetectionRecordBuffer {
                detectors: vec![false, true],
                observables: vec![false, false, true],
            },
            DetectionRecordBuffer {
                detectors: vec![true, false],
                observables: vec![false, false, false],
            },
            DetectionRecordBuffer {
                detectors: vec![true, true],
                observables: vec![false, false, true],
            },
        ],
    );
}

#[test]
fn detection_conversion_handles_repeats_coordinates_and_empty_detectors() {
    let output = convert(
        "M 0 !1\nSHIFT_COORDS(2, 3)\nDETECTOR(5) rec[-2]\nDETECTOR rec[-1]\nREPEAT 2 {\n    DETECTOR rec[-2] rec[-1]\n}\nDETECTOR\n",
        &[&[false, true]],
        true,
    );

    assert_eq!(
        output.records,
        vec![DetectionRecordBuffer {
            detectors: vec![false, true, true, true, false],
            observables: Vec::new(),
        }],
    );
}

#[test]
fn detection_conversion_handles_empty_detector_circuits() {
    let output = convert("M 0\n", &[&[false], &[true]], true);

    assert_eq!(output.detector_count, 0);
    assert_eq!(
        output.records,
        vec![
            DetectionRecordBuffer {
                detectors: Vec::new(),
                observables: Vec::new(),
            },
            DetectionRecordBuffer {
                detectors: Vec::new(),
                observables: Vec::new(),
            },
        ],
    );
}

#[test]
fn detection_conversion_rejects_invalid_measurement_references() {
    let circuit = Circuit::from_stim_str("DETECTOR rec[-1]\n").expect("parse circuit");
    let result = convert_measurements_to_detection_events(
        &circuit,
        &[Vec::new()],
        ReferenceSampleMode::SkipReferenceSample,
    );

    assert!(result.is_err());
}

#[test]
fn detection_conversion_skip_reference_sample_ignores_sweep_reference() {
    let output = convert_with_sweep(
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
        &[&[false], &[true]],
        &[&[true], &[true]],
        true,
    );

    assert_eq!(
        output.records,
        vec![
            DetectionRecordBuffer {
                detectors: vec![false],
                observables: Vec::new(),
            },
            DetectionRecordBuffer {
                detectors: vec![true],
                observables: Vec::new(),
            },
        ]
    );
}

#[test]
fn detection_conversion_supports_sweep_controlled_error_propagation_and_repeats() {
    let output = convert_with_sweep(
        "H 0\nCZ sweep[0] 0\nMX 0\nDETECTOR rec[-1]\n",
        &[&[false], &[false]],
        &[&[false], &[true]],
        false,
    );
    assert_eq!(
        output.records,
        vec![
            DetectionRecordBuffer {
                detectors: vec![false],
                observables: Vec::new(),
            },
            DetectionRecordBuffer {
                detectors: vec![true],
                observables: Vec::new(),
            },
        ]
    );

    let repeated = convert_with_sweep(
        "REPEAT 3 {\n    CX sweep[0] 0\n}\nM 0\nDETECTOR rec[-1]\n",
        &[&[false]],
        &[&[true]],
        false,
    );
    assert_eq!(
        repeated.records,
        vec![DetectionRecordBuffer {
            detectors: vec![true],
            observables: Vec::new(),
        }]
    );
}

#[test]
fn detection_conversion_rejects_bad_sweep_records_and_unsupported_sampling_surfaces() {
    let circuit =
        Circuit::from_stim_str("CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("parse circuit");
    let short_sweeps = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &[vec![false], vec![true]],
        &[vec![false]],
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect_err("reject short sweep batch");
    assert!(
        short_sweeps
            .to_string()
            .contains("measurement batch has 2 shots but sweep batch has 1"),
        "{short_sweeps}"
    );
    let long_sweeps = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &[vec![false]],
        &[vec![false], vec![true]],
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect_err("reject long sweep batch");
    assert!(
        long_sweeps
            .to_string()
            .contains("measurement batch has 1 shots but sweep batch has 2"),
        "{long_sweeps}"
    );

    let error = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &[vec![false]],
        &[Vec::new()],
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect_err("reject wrong sweep width");
    assert!(
        error
            .to_string()
            .contains("record 0 has 0 bits but 1 were expected"),
        "{error}"
    );

    let unsupported = Circuit::from_stim_str("R 0\nXCZ sweep[0] 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse unsupported sweep target role");
    let unsupported_error = convert_measurements_to_detection_events_with_sweep(
        &unsupported,
        &[vec![false]],
        &[vec![true]],
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect_err("reject unsupported sweep target role");
    assert!(
        unsupported_error.to_string().contains("XCZ target shape"),
        "{unsupported_error}"
    );
    let unsupported_shape = Circuit::from_stim_str("CX sweep[0] sweep[1]\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse unsupported sweep shape");
    let unsupported_shape_error = convert_measurements_to_detection_events_with_sweep(
        &unsupported_shape,
        &[vec![false]],
        &[vec![true, true]],
        ReferenceSampleMode::UseReferenceSample,
    )
    .expect_err("reject unsupported sweep target shape");
    assert!(
        unsupported_shape_error
            .to_string()
            .contains("CX target shape"),
        "{unsupported_shape_error}"
    );

    for source in [
        "CX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
        "CY 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
    ] {
        let invalid_sweep_order =
            Circuit::from_stim_str(source).expect("parse invalid sweep order");
        let conversion_error = convert_measurements_to_detection_events_with_sweep(
            &invalid_sweep_order,
            &[vec![false]],
            &[vec![true]],
            ReferenceSampleMode::UseReferenceSample,
        )
        .expect_err("reject invalid sampler sweep target order");
        assert!(
            conversion_error.to_string().contains("does not support"),
            "{source}\n{conversion_error}"
        );

        let validation_error = validate_detection_sampling_circuit(&invalid_sweep_order)
            .expect_err("reject invalid sampler sweep target order during validation");
        assert!(
            validation_error.to_string().contains("does not support"),
            "{source}\n{validation_error}"
        );
        let sampling_error = sample_detection_events(&invalid_sweep_order, 1, Some(5))
            .expect_err("reject invalid sampler sweep target order during sampling");
        assert!(
            sampling_error.to_string().contains("does not support"),
            "{source}\n{sampling_error}"
        );

        let skip_reference_error = convert_measurements_to_detection_events_with_sweep(
            &invalid_sweep_order,
            &[vec![false]],
            &[vec![false]],
            ReferenceSampleMode::SkipReferenceSample,
        )
        .expect_err("skip-reference conversion must validate sampler target order");
        assert!(
            skip_reference_error.to_string().contains("target shape"),
            "{source}\n{skip_reference_error}"
        );
    }

    let invalid_feedback = Circuit::from_stim_str("M 0\nCX 1 rec[-1]\nDETECTOR rec[-1]\n")
        .expect("parse invalid feedback order");
    let skip_reference_error = convert_measurements_to_detection_events_with_sweep(
        &invalid_feedback,
        &[vec![false]],
        &[Vec::new()],
        ReferenceSampleMode::SkipReferenceSample,
    )
    .expect_err("skip-reference conversion must validate feedback target order");
    assert!(
        skip_reference_error.to_string().contains("target shape"),
        "{skip_reference_error}"
    );

    for (source, gate) in [
        ("RX 0\nCX 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n", "CX"),
        (
            "RX 0\nMX 0\nCX rec[-1] sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n",
            "CX",
        ),
        (
            "RX 0\nMX 0\nXCZ rec[-1] 0\nOBSERVABLE_INCLUDE(0) X0\n",
            "XCZ",
        ),
        (
            "RX 0\nMX 0\nYCZ rec[-1] 0\nOBSERVABLE_INCLUDE(0) X0\n",
            "YCZ",
        ),
    ] {
        let unsupported_frame_shape =
            Circuit::from_stim_str(source).expect("parse unsupported frame sweep shape");
        let validation_error = validate_detection_sampling_circuit(&unsupported_frame_shape)
            .expect_err("reject frame sweep target during validation");
        assert!(
            validation_error
                .to_string()
                .contains(&format!("M9 detector frame subset does not support {gate}")),
            "{validation_error}"
        );
        let frame_error = sample_detection_events(&unsupported_frame_shape, 1, Some(5))
            .expect_err("reject frame sweep target");
        assert!(
            frame_error
                .to_string()
                .contains(&format!("M9 detector frame subset does not support {gate}")),
            "{frame_error}"
        );
    }
}

#[test]
fn detection_sampling_uses_all_false_default_sweep_bits_frame_path() {
    let sweep_circuit = Circuit::from_stim_str(
        "RX 0\n\
         RX 1\n\
         CX sweep[0] 0\n\
         CY sweep[1] 0\n\
         CZ 0 sweep[2]\n\
         CZ sweep[3] 0\n\
         CZ sweep[4] sweep[5]\n\
         XCZ 0 1 0 sweep[6]\n\
         YCZ 0 1 0 sweep[7]\n\
         MX 0\n\
         CZ rec[-1] sweep[8]\n\
         REPEAT 2 {\n\
             CX sweep[9] 0\n\
             XCZ 0 sweep[10]\n\
         }\n\
         OBSERVABLE_INCLUDE(0) X0\n",
    )
    .expect("parse frame-path sweep-conditioned circuit");
    let explicit_false_circuit =
        Circuit::from_stim_str("RX 0\nRX 1\nXCZ 0 1\nYCZ 0 1\nMX 0\nOBSERVABLE_INCLUDE(0) X0\n")
            .expect("parse explicit circuit");

    validate_detection_sampling_circuit(&sweep_circuit).expect("validate frame sweep sampling");
    assert_eq!(
        measurement_record_count(&sweep_circuit).expect("sweep measurement count"),
        measurement_record_count(&explicit_false_circuit).expect("explicit measurement count")
    );
    assert_eq!(
        detection_record_width(&sweep_circuit).expect("sweep detection width"),
        detection_record_width(&explicit_false_circuit).expect("explicit detection width")
    );
    let sweep_output =
        sample_detection_events(&sweep_circuit, 32, Some(5)).expect("sample frame sweep circuit");
    let explicit_false_output = sample_detection_events(&explicit_false_circuit, 32, Some(5))
        .expect("sample explicit false frame circuit");
    assert_eq!(sweep_output.records, explicit_false_output.records);
}

#[test]
fn detection_sampling_supports_xcz_ycz_measurement_feedback_frame_path() {
    for measured_state in ["M 0", "X_ERROR(1) 0\nM 0"] {
        let feedback_circuit = Circuit::from_stim_str(&format!(
            "R 0 1 2\n\
             {measured_state}\n\
             XCZ 1 rec[-1]\n\
             YCZ 2 rec[-1]\n\
             OBSERVABLE_INCLUDE(0) Z1\n\
             OBSERVABLE_INCLUDE(1) Z2\n"
        ))
        .expect("parse feedback circuit");
        let explicit_circuit = Circuit::from_stim_str(&format!(
            "R 0 1 2\n\
             {measured_state}\n\
             CX rec[-1] 1\n\
             CY rec[-1] 2\n\
             OBSERVABLE_INCLUDE(0) Z1\n\
             OBSERVABLE_INCLUDE(1) Z2\n"
        ))
        .expect("parse equivalent feedback circuit");

        validate_detection_sampling_circuit(&feedback_circuit).expect("validate feedback circuit");
        assert_eq!(
            measurement_record_count(&feedback_circuit).expect("feedback measurement count"),
            measurement_record_count(&explicit_circuit).expect("equivalent measurement count")
        );
        assert_eq!(
            detection_record_width(&feedback_circuit).expect("feedback detection width"),
            detection_record_width(&explicit_circuit).expect("equivalent detection width")
        );
        let feedback_output = sample_detection_events(&feedback_circuit, 16, Some(7))
            .expect("sample feedback circuit");
        let explicit_output = sample_detection_events(&explicit_circuit, 16, Some(7))
            .expect("sample equivalent feedback");
        assert_eq!(feedback_output.records, explicit_output.records);
    }
}

#[test]
fn detection_sampling_supports_pauli_target_observables_like_frame_simulator() {
    // Adapted from Stim v1.16.0 src/stim/simulators/frame_simulator.test.cc
    // observable_include_paulis_rx/ry/rz.
    for (reset, random_pair, stable_observable) in
        [("RZ", (0, 1), 2), ("RY", (0, 2), 1), ("RX", (1, 2), 0)]
    {
        let circuit = Circuit::from_stim_str(&format!(
            "{reset} 0\n\
                 OBSERVABLE_INCLUDE(0) X0\n\
                 OBSERVABLE_INCLUDE(1) Y0\n\
                 OBSERVABLE_INCLUDE(2) Z0\n"
        ))
        .expect("parse");
        let output = sample_detection_events(&circuit, 1024, Some(5)).expect("detect");

        let hits = |observable: usize| {
            output
                .records
                .iter()
                .filter(|record| record.observables[observable])
                .count()
        };
        let first_hits = hits(random_pair.0);
        assert_eq!(first_hits, hits(random_pair.1));
        assert!((300..700).contains(&first_hits));
        assert_eq!(hits(stable_observable), 0);
    }
}

#[test]
fn detection_sampling_supports_product_measurements_with_pauli_observables() {
    for circuit_text in [
        "RX 0 1\nMXX 0 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z0\n",
        "RY 0 1\nMYY 0 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) X0\n",
        "R 0 1\nMZZ 0 1\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) X0\n",
        "RX 0\nRY 1\nR 2\nMPP X0*Y1*Z2\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) Z0\n",
    ] {
        let circuit = Circuit::from_stim_str(circuit_text).expect("parse");
        let output = sample_detection_events(&circuit, 1024, Some(5)).expect("detect");

        assert!(
            output
                .records
                .iter()
                .all(|record| record.detectors.first() == Some(&false))
        );
        let hits = output
            .records
            .iter()
            .filter(|record| record.observables[0])
            .count();
        assert!((300..700).contains(&hits));
    }
}

#[test]
fn product_measurement_collapse_lands_on_the_whole_measured_product() {
    // Each circuit measures an anticommuting product and then a product that stabilizes the
    // prepared state. The second detector stays silent only when the first collapse multiplies
    // the frame by the whole measured product; randomizing a single term multiplies the
    // deviation by a Pauli outside the measured group and fires it roughly half the time.
    for circuit_text in [
        "HERALDED_ERASE(0) 2\nR 0 1\nMXX 0 1\nMZZ 0 1\nDETECTOR rec[-1]\n",
        "HERALDED_ERASE(0) 2\nR 0 1\nMYY 0 1\nMZZ 0 1\nDETECTOR rec[-1]\n",
        "HERALDED_ERASE(0) 2\nR 0 1\nMPP X0*X1\nMPP Z0*Z1\nDETECTOR rec[-1]\n",
    ] {
        for seed in [1_u64, 7, 42, 20260805] {
            let circuit = Circuit::from_stim_str(circuit_text).expect("parse");
            let output = sample_detection_events(&circuit, 4096, Some(seed)).expect("detect");
            let hits = output
                .records
                .iter()
                .filter(|record| record.detectors[0])
                .count();
            assert_eq!(hits, 0, "seed {seed}: {circuit_text:?}");
        }
    }
}

#[test]
fn whole_product_collapse_keeps_commuting_products_deterministic_and_stays_random() {
    // Statistical contract: 4 fixed seeds x 4096 shots. The MXX record is physically random
    // (the Z-basis reset randomizes the frame z-parity it reads) and the observable reads the
    // X-frame bit that the MXX collapse randomizes, so both counts are Binomial(4096, 1/2);
    // the band 1856..=2240 is the mean plus or minus 6 sigma (sigma = 32), a false-positive
    // budget of roughly 2e-9 per assertion. The MZZ zero assertion is an exact invariant.
    let circuit = Circuit::from_stim_str(
        "HERALDED_ERASE(0) 2\nR 0 1\nMXX 0 1\nMZZ 0 1\nDETECTOR rec[-1]\nDETECTOR rec[-2]\nOBSERVABLE_INCLUDE(0) Z0\n",
    )
    .expect("parse");
    for seed in [1_u64, 7, 42, 20260805] {
        let output = sample_detection_events(&circuit, 4096, Some(seed)).expect("detect");
        let mzz_hits = output
            .records
            .iter()
            .filter(|record| record.detectors[0])
            .count();
        let mxx_hits = output
            .records
            .iter()
            .filter(|record| record.detectors[1])
            .count();
        let observable_hits = output
            .records
            .iter()
            .filter(|record| record.observables[0])
            .count();
        assert_eq!(mzz_hits, 0, "seed {seed}: MZZ stabilizer detector fired");
        assert!(
            (1856..=2240).contains(&mxx_hits),
            "seed {seed}: random MXX hits {mxx_hits} outside the 6-sigma band"
        );
        assert!(
            (1856..=2240).contains(&observable_hits),
            "seed {seed}: observable hits {observable_hits} outside the 6-sigma band"
        );
    }
}

#[test]
fn detection_sampling_frame_path_ignores_reference_sample_measurement_bits() {
    let circuit = Circuit::from_stim_str(
        "M !0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) Z0\n",
    )
    .expect("parse");
    let output = sample_detection_events(&circuit, 8, Some(5)).expect("detect");

    assert!(
        output
            .records
            .iter()
            .all(|record| { record.detectors == [false] && record.observables == [false, false] })
    );
}

#[test]
fn detection_sampling_frame_path_rejects_invalid_feedback_measurement_references() {
    let circuit =
        Circuit::from_stim_str("CX rec[-1] 0\nOBSERVABLE_INCLUDE(0) Z0\n").expect("parse");
    let result = sample_detection_events(&circuit, 1, Some(5));

    assert!(result.is_err());
}

#[test]
fn detection_conversion_rejects_unbounded_record_shapes() {
    let huge_observable =
        Circuit::from_stim_str("M 0\nOBSERVABLE_INCLUDE(1000000) rec[-1]\n").expect("parse");
    assert!(
        convert_measurements_to_detection_events(
            &huge_observable,
            &[vec![false]],
            ReferenceSampleMode::SkipReferenceSample,
        )
        .is_err()
    );

    let huge_repeat =
        Circuit::from_stim_str("REPEAT 100001 {\n    M 0\n}\n").expect("parse repeat");
    assert!(measurement_record_count(&huge_repeat).is_err());
}

#[test]
fn detection_sampling_handles_gauge_detectors_structurally() {
    let circuit = Circuit::from_stim_str("MPP Z8*X9\nDETECTOR rec[-1]\n").expect("parse");
    let first = sample_detection_events(&circuit, 1000, Some(5)).expect("sample detections");
    let second = sample_detection_events(&circuit, 1000, Some(5)).expect("sample detections");

    assert_eq!(first, second);
    let hits = first
        .records
        .iter()
        .filter(|record| record.detectors.first().copied().unwrap_or(false))
        .count();
    assert!(
        (350..=650).contains(&hits),
        "expected gauge detector to produce random-looking events, got {hits}"
    );
}
