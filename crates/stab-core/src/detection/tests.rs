#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "detection tests use direct fixture assertions for compact diagnostics"
)]

use super::*;

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
        DetectionConversionOptions {
            skip_reference_sample,
        },
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
        DetectionConversionOptions {
            skip_reference_sample,
        },
    )
    .expect("convert measurements with sweep")
}

#[test]
fn compiled_detection_converter_streams_like_materialized_conversion() {
    let circuit = Circuit::from_stim_str(
        "X 0\nM 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(2) rec[-1]\n",
    )
    .expect("parse circuit");
    let measurements = vec![
        vec![false, false],
        vec![false, true],
        vec![true, false],
        vec![true, true],
    ];
    let materialized = convert_measurements_to_detection_events(
        &circuit,
        &measurements,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("materialized conversion");
    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile converter");
    let mut streamed = Vec::new();
    converter
        .try_for_each_detection_event(measurements.iter().map(Vec::as_slice), |record| {
            streamed.push(record.clone());
            Ok::<(), CircuitError>(())
        })
        .expect("stream conversion");

    assert_eq!(streamed, materialized.records);
}

#[test]
fn sampled_detection_streams_like_materialized_sampling() {
    for circuit_text in [
        "X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n",
        "RX 0\nZ_ERROR(0.25) 0\nOBSERVABLE_INCLUDE(0) X0\n",
    ] {
        let circuit = Circuit::from_stim_str(circuit_text).expect("parse circuit");
        let materialized =
            sample_detection_events(&circuit, 32, Some(11)).expect("materialized sampling");
        let mut streamed = Vec::new();
        try_for_each_sampled_detection_event(&circuit, 32, Some(11), |record| {
            streamed.push(record.clone());
            Ok::<(), CircuitError>(())
        })
        .expect("stream sampling");

        assert_eq!(streamed, materialized.records);
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

    let mut streamed = Vec::new();
    try_for_each_sampled_detection_event(&sweep_circuit, 32, Some(17), |record| {
        streamed.push(record.clone());
        Ok::<(), CircuitError>(())
    })
    .expect("stream sweep sampling");

    assert_eq!(streamed, sweep_output.records);
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
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![false, false, false],
            },
            DetectionEventRecord {
                detectors: vec![true, true],
                observables: vec![false, false, true],
            },
            DetectionEventRecord {
                detectors: vec![false, false],
                observables: vec![false, false, false],
            },
            DetectionEventRecord {
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
            DetectionEventRecord {
                detectors: vec![false, false],
                observables: vec![false, false, false],
            },
            DetectionEventRecord {
                detectors: vec![false, true],
                observables: vec![false, false, true],
            },
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![false, false, false],
            },
            DetectionEventRecord {
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
        vec![DetectionEventRecord {
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
            DetectionEventRecord {
                detectors: Vec::new(),
                observables: Vec::new(),
            },
            DetectionEventRecord {
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
        DetectionConversionOptions {
            skip_reference_sample: true,
        },
    );

    assert!(result.is_err());
}

#[test]
fn detection_conversion_uses_all_false_default_sweep_bits() {
    let circuit =
        Circuit::from_stim_str("CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("parse circuit");
    let output = convert_measurements_to_detection_events(
        &circuit,
        &[vec![false], vec![true]],
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("convert with default sweep");

    assert_eq!(
        output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![false],
                observables: Vec::new(),
            },
            DetectionEventRecord {
                detectors: vec![true],
                observables: Vec::new(),
            },
        ]
    );
}

#[test]
fn detection_conversion_uses_per_shot_sweep_reference_samples() {
    let output = convert_with_sweep(
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
        &[&[false], &[false], &[true]],
        &[&[false], &[true], &[true]],
        false,
    );

    assert_eq!(
        output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![false],
                observables: vec![false],
            },
            DetectionEventRecord {
                detectors: vec![true],
                observables: vec![true],
            },
            DetectionEventRecord {
                detectors: vec![false],
                observables: vec![false],
            },
        ]
    );
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
            DetectionEventRecord {
                detectors: vec![false],
                observables: Vec::new(),
            },
            DetectionEventRecord {
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
            DetectionEventRecord {
                detectors: vec![false],
                observables: Vec::new(),
            },
            DetectionEventRecord {
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
        vec![DetectionEventRecord {
            detectors: vec![true],
            observables: Vec::new(),
        }]
    );
}

#[test]
fn detection_conversion_rejects_bad_sweep_records_and_unsupported_sampling_surfaces() {
    let circuit =
        Circuit::from_stim_str("CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n").expect("parse circuit");
    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile converter");
    let short_sweeps = converter
        .try_for_each_detection_event_with_sweep(
            [vec![false], vec![true]].iter().map(Vec::as_slice),
            [vec![false]].iter().map(Vec::as_slice),
            |_| Ok::<(), CircuitError>(()),
        )
        .expect_err("reject short sweep iterator");
    assert!(
        short_sweeps
            .to_string()
            .contains("measurement records have more shots than sweep records"),
        "{short_sweeps}"
    );
    let long_sweeps = converter
        .try_for_each_detection_event_with_sweep(
            [vec![false]].iter().map(Vec::as_slice),
            [vec![false], vec![true]].iter().map(Vec::as_slice),
            |_| Ok::<(), CircuitError>(()),
        )
        .expect_err("reject long sweep iterator");
    assert!(
        long_sweeps
            .to_string()
            .contains("sweep records have more shots than measurement records"),
        "{long_sweeps}"
    );

    let error = convert_measurements_to_detection_events_with_sweep(
        &circuit,
        &[vec![false]],
        &[Vec::new()],
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect_err("reject wrong sweep width");
    assert!(
        error.to_string().contains("sweep record 0 expected 1 bits"),
        "{error}"
    );

    let unsupported = Circuit::from_stim_str("R 0\nXCZ sweep[0] 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse unsupported sweep circuit");
    let unsupported_error = convert_measurements_to_detection_events_with_sweep(
        &unsupported,
        &[vec![false]],
        &[vec![true]],
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect_err("reject unsupported sweep gate");
    assert!(
        unsupported_error
            .to_string()
            .contains(UNSUPPORTED_SWEEP_DETECTION_MESSAGE),
        "{unsupported_error}"
    );
    let unsupported_shape = Circuit::from_stim_str("CX sweep[0] sweep[1]\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse unsupported sweep shape");
    let unsupported_shape_error = convert_measurements_to_detection_events_with_sweep(
        &unsupported_shape,
        &[vec![false]],
        &[vec![true, true]],
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect_err("reject unsupported sweep target shape");
    assert!(
        unsupported_shape_error
            .to_string()
            .contains("does not support CX"),
        "{unsupported_shape_error}"
    );

    for (source, gate) in [
        ("RX 0\nCX 0 sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n", "CX"),
        (
            "RX 0\nMX 0\nCX rec[-1] sweep[0]\nOBSERVABLE_INCLUDE(0) X0\n",
            "CX",
        ),
        (
            "RX 0\nMX 0\nXCZ 0 rec[-1]\nOBSERVABLE_INCLUDE(0) X0\n",
            "XCZ",
        ),
        (
            "RX 0\nMX 0\nYCZ 0 rec[-1]\nOBSERVABLE_INCLUDE(0) X0\n",
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

    let mut streamed = Vec::new();
    try_for_each_sampled_detection_event(&sweep_circuit, 32, Some(5), |record| {
        streamed.push(record.clone());
        Ok::<(), CircuitError>(())
    })
    .expect("stream frame sweep sampling");
    assert_eq!(streamed, sweep_output.records);
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
            DetectionConversionOptions {
                skip_reference_sample: true,
            },
        )
        .is_err()
    );

    let huge_repeat =
        Circuit::from_stim_str("REPEAT 100001 {\n    M 0\n}\n").expect("parse repeat");
    assert!(measurement_record_count(&huge_repeat).is_err());
}

#[test]
fn detection_record_writers_cover_text_and_bit_packed_formats() {
    let output = DetectionConversionOutput {
        detector_count: 2,
        observable_count: 2,
        records: vec![
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![false, true],
            },
            DetectionEventRecord {
                detectors: vec![false, true],
                observables: vec![true, false],
            },
        ],
    };

    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            SampleFormat::ZeroOne
        )
        .unwrap(),
        b"1001\n0110\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            SampleFormat::Dets
        )
        .unwrap(),
        b"shot D0 L1\nshot D1 L0\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Prepend,
            SampleFormat::Dets
        )
        .unwrap(),
        b"shot L1 D0\nshot L0 D1\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            SampleFormat::Hits
        )
        .unwrap(),
        b"0,3\n1,2\n"
    );
    assert_eq!(
        write_detection_records(
            &output,
            DetectionObservableOutputMode::Append,
            SampleFormat::B8
        )
        .unwrap(),
        [0b0000_1001, 0b0000_0110]
    );
    assert_eq!(
        write_observable_records(&output, SampleFormat::B8).unwrap(),
        [0b0000_0010, 0b0000_0001]
    );

    let ptb64_output = DetectionConversionOutput {
        detector_count: 2,
        observable_count: 1,
        records: vec![
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![true],
            };
            64
        ],
    };
    assert_eq!(
        write_ptb64_detection_records(&ptb64_output, DetectionObservableOutputMode::Append)
            .unwrap(),
        [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        ]
    );
    assert_eq!(
        write_ptb64_observable_records(&ptb64_output).unwrap(),
        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    );
}

#[test]
fn detection_sampling_matches_basic_frame_simulator_utility_semantics() {
    let circuit = Circuit::from_stim_str("X_ERROR(1) 0\nM 0\nDETECTOR rec[-1]\n").expect("parse");
    let output = sample_detection_events(&circuit, 5, Some(5)).expect("sample detections");

    assert_eq!(output.detector_count, 1);
    assert_eq!(
        output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![true],
                observables: Vec::new(),
            };
            5
        ],
    );
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
