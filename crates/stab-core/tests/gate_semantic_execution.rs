#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "PF3 gate semantic execution tests use compact per-gate diagnostics"
)]

use stab_core::{
    Circuit, CircuitError, CompiledDetectionConverter, CompiledSampler, DetectionConversionOptions,
    DetectionEventRecord, ErrorAnalyzerOptions, Gate, circuit_to_detector_error_model,
    sample_detection_events,
};

#[test]
fn fixed_tableau_gates_execute_across_current_public_surfaces() {
    let cases = fixed_tableau_gate_cases();
    assert_eq!(cases.len(), 46);

    for case in cases {
        let sampler = CompiledSampler::compile(&case.circuit)
            .unwrap_or_else(|error| panic!("sampler rejected {}: {error}", case.gate_name));
        assert_eq!(
            sampler.sample_zero_one(1),
            vec![vec![false]],
            "{} should cancel with its inverse before measurement",
            case.gate_name
        );

        let converter = CompiledDetectionConverter::compile(
            &case.circuit,
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .unwrap_or_else(|error| panic!("detection converter rejected {}: {error}", case.gate_name));
        assert_eq!(converter.detector_count(), 1, "{}", case.gate_name);

        circuit_to_detector_error_model(&case.circuit, ErrorAnalyzerOptions::default())
            .unwrap_or_else(|error| panic!("analyzer rejected {}: {error}", case.gate_name));
    }
}

#[test]
fn mpad_executes_across_current_public_surfaces() {
    let circuit = Circuit::from_stim_str(
        "MPAD 0 1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("parse MPAD circuit");

    let sampler = CompiledSampler::compile(&circuit).expect("compile MPAD sampler");
    assert_eq!(
        sampler.sample_zero_one(2),
        vec![vec![false, true], vec![false, true]]
    );

    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile MPAD detection converter");
    assert_eq!(converter.measurement_count(), 2);
    assert_eq!(converter.detector_count(), 2);
    assert_eq!(converter.observable_count(), 1);
    assert_eq!(
        converter
            .convert_record(&[false, true])
            .expect("convert MPAD reference record"),
        DetectionEventRecord {
            detectors: vec![false, false],
            observables: vec![false],
        }
    );

    let skip_reference_converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: true,
        },
    )
    .expect("compile skip-reference MPAD detection converter");
    assert_eq!(
        skip_reference_converter
            .convert_record(&[false, true])
            .expect("convert skip-reference MPAD record"),
        DetectionEventRecord {
            detectors: vec![false, true],
            observables: vec![true],
        }
    );

    let detection_output =
        sample_detection_events(&circuit, 2, Some(3)).expect("sample MPAD detection events");
    assert_eq!(detection_output.detector_count, 2);
    assert_eq!(detection_output.observable_count, 1);
    assert_eq!(
        detection_output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![false, false],
                observables: vec![false],
            };
            2
        ]
    );

    let frame_circuit = Circuit::from_stim_str(
        "MPAD 0 1\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) Z0\n",
    )
    .expect("parse frame-path MPAD circuit");
    let frame_output = sample_detection_events(&frame_circuit, 2, Some(5))
        .expect("sample frame-path MPAD detection events");
    assert_eq!(frame_output.detector_count, 0);
    assert_eq!(frame_output.observable_count, 2);
    assert_eq!(
        frame_output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![],
                observables: vec![false, false],
            };
            2
        ]
    );

    let analyzer_circuit = Circuit::from_stim_str(
        "M(0.125) 5\nMPAD 0 1\nDETECTOR rec[-1] rec[-2]\nDETECTOR rec[-3]\n",
    )
    .expect("parse analyzer MPAD circuit");
    let dem = circuit_to_detector_error_model(&analyzer_circuit, ErrorAnalyzerOptions::default())
        .expect("analyze MPAD circuit");
    assert_eq!(dem.to_string(), "error(0.125) D1\ndetector D0\n");
}

#[test]
fn stochastic_mpad_executes_across_sampler_and_detection_surfaces() {
    const SHOTS: usize = 4000;

    let circuit = Circuit::from_stim_str("MPAD(0.25) 0 1\n").expect("parse stochastic MPAD");
    let sampler = CompiledSampler::compile(&circuit).expect("compile stochastic MPAD sampler");
    let shots = sampler.sample_zero_one_with_seed(SHOTS, Some(17));

    let first_flips = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    let second_flips = shots
        .iter()
        .filter(|shot| shot.get(1) == Some(&false))
        .count();
    assert_binomial_5_sigma("MPAD(0.25) 0 flips", first_flips, SHOTS, 0.25);
    assert_binomial_5_sigma("MPAD(0.25) 1 flips", second_flips, SHOTS, 0.25);
    let mut sampler_buckets = [0usize; 4];
    for shot in &shots {
        let [first, second] = shot.as_slice() else {
            panic!("expected two MPAD sample bits, got {shot:?}");
        };
        let bucket = ((*first as usize) << 1) | (*second as usize);
        *sampler_buckets
            .get_mut(bucket)
            .expect("two boolean bits produce a four-bucket index") += 1;
    }
    assert_binomial_5_sigma("MPAD outputs 00", sampler_buckets[0], SHOTS, 0.1875);
    assert_binomial_5_sigma("MPAD outputs 01", sampler_buckets[1], SHOTS, 0.5625);
    assert_binomial_5_sigma("MPAD outputs 10", sampler_buckets[2], SHOTS, 0.0625);
    assert_binomial_5_sigma("MPAD outputs 11", sampler_buckets[3], SHOTS, 0.1875);

    let converter_circuit =
        Circuit::from_stim_str("MPAD(0.25) 0\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n")
            .expect("parse stochastic MPAD converter circuit");
    let converter = CompiledDetectionConverter::compile(
        &converter_circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile stochastic MPAD detection converter");
    assert_eq!(
        converter
            .convert_record(&[false])
            .expect("convert unflipped stochastic MPAD record"),
        DetectionEventRecord {
            detectors: vec![false],
            observables: vec![false],
        }
    );
    assert_eq!(
        converter
            .convert_record(&[true])
            .expect("convert flipped stochastic MPAD record"),
        DetectionEventRecord {
            detectors: vec![true],
            observables: vec![true],
        }
    );

    let detection_output = sample_detection_events(&converter_circuit, SHOTS, Some(23))
        .expect("sample stochastic MPAD detection events");
    let detector_flips = detection_output
        .records
        .iter()
        .filter(|record| record.detectors.first() == Some(&true))
        .count();
    let observable_flips = detection_output
        .records
        .iter()
        .filter(|record| record.observables.first() == Some(&true))
        .count();
    assert_binomial_5_sigma(
        "stochastic MPAD detector flips",
        detector_flips,
        SHOTS,
        0.25,
    );
    assert_eq!(
        detector_flips, observable_flips,
        "detector and observable should be sourced by the same stochastic pad record"
    );

    let frame_circuit = Circuit::from_stim_str(
        "MPAD(0.25) 0\nOBSERVABLE_INCLUDE(0) rec[-1]\nOBSERVABLE_INCLUDE(1) Z0\n",
    )
    .expect("parse frame-path stochastic MPAD circuit");
    let frame_output = sample_detection_events(&frame_circuit, SHOTS, Some(29))
        .expect("sample frame-path stochastic MPAD detection events");
    let frame_record_flips = frame_output
        .records
        .iter()
        .filter(|record| record.observables.first() == Some(&true))
        .count();
    let frame_pauli_flips = frame_output
        .records
        .iter()
        .filter(|record| record.observables.get(1) == Some(&true))
        .count();
    assert_binomial_5_sigma(
        "frame-path stochastic MPAD observable flips",
        frame_record_flips,
        SHOTS,
        0.25,
    );
    assert_eq!(
        frame_pauli_flips, 0,
        "stochastic MPAD should not introduce an unrelated Pauli-frame observable flip"
    );
}

#[test]
fn mpp_executes_across_current_public_surfaces() {
    let circuit = Circuit::from_stim_str(
        "H 0\nCX 0 1\nMPP X0*X1 !Z0*Z1\nDETECTOR rec[-2]\nDETECTOR rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    )
    .expect("parse MPP circuit");

    let sampler = CompiledSampler::compile(&circuit).expect("compile MPP sampler");
    assert_eq!(
        sampler.sample_zero_one(2),
        vec![vec![false, true], vec![false, true]]
    );

    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile MPP detection converter");
    assert_eq!(converter.measurement_count(), 2);
    assert_eq!(converter.detector_count(), 2);
    assert_eq!(converter.observable_count(), 1);
    assert_eq!(
        converter
            .convert_record(&[false, true])
            .expect("convert MPP reference record"),
        DetectionEventRecord {
            detectors: vec![false, false],
            observables: vec![false],
        }
    );

    let skip_reference_converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: true,
        },
    )
    .expect("compile skip-reference MPP detection converter");
    assert_eq!(
        skip_reference_converter
            .convert_record(&[false, true])
            .expect("convert skip-reference MPP record"),
        DetectionEventRecord {
            detectors: vec![false, true],
            observables: vec![true],
        }
    );

    let detection_output =
        sample_detection_events(&circuit, 2, Some(3)).expect("sample MPP detection events");
    assert_eq!(detection_output.detector_count, 2);
    assert_eq!(detection_output.observable_count, 1);
    assert_eq!(
        detection_output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![false, false],
                observables: vec![false],
            };
            2
        ]
    );

    let frame_circuit = Circuit::from_stim_str(
        "MPP !Z0 Z0\nOBSERVABLE_INCLUDE(0) rec[-2]\nOBSERVABLE_INCLUDE(1) Z0\n",
    )
    .expect("parse frame-path MPP circuit");
    let frame_output = sample_detection_events(&frame_circuit, 2, Some(5))
        .expect("sample frame-path MPP detection events");
    assert_eq!(frame_output.detector_count, 0);
    assert_eq!(frame_output.observable_count, 2);
    assert_eq!(
        frame_output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![],
                observables: vec![false, false],
            };
            2
        ]
    );

    let analyzer_circuit =
        Circuit::from_stim_str("MPP X0*X1 X0\nTICK\nMPP X0\nDETECTOR rec[-1] rec[-2]\n")
            .expect("parse analyzer MPP circuit");
    let dem = circuit_to_detector_error_model(&analyzer_circuit, ErrorAnalyzerOptions::default())
        .expect("analyze MPP circuit");
    assert_eq!(dem.to_string(), "detector D0\n");
}

#[test]
fn variable_target_spp_execution_matches_decomposed_circuit() {
    let cases = [
        "SPP X0\nM 0\nDETECTOR rec[-1]\n",
        "SPP !X0\nM 0\nDETECTOR rec[-1]\n",
        "SPP X0*X1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
        "SPP_DAG Y0*Y1\nM 0 1\nDETECTOR rec[-1] rec[-2]\n",
        "SPP Z0\nS_DAG 0\nM 0\nDETECTOR rec[-1]\n",
        "SPP_DAG Z0\nS 0\nM 0\nDETECTOR rec[-1]\n",
    ];
    for circuit_text in cases {
        let circuit = Circuit::from_stim_str(circuit_text).expect("parse SPP circuit");
        let decomposed = circuit.decomposed().expect("decompose SPP circuit");

        let sampler = CompiledSampler::compile(&circuit)
            .unwrap_or_else(|error| panic!("sampler rejected {circuit_text}: {error}"));
        let decomposed_sampler = CompiledSampler::compile(&decomposed)
            .unwrap_or_else(|error| panic!("sampler rejected decomposed {circuit_text}: {error}"));
        assert_eq!(
            sampler.sample_zero_one_with_seed(16, Some(5)),
            decomposed_sampler.sample_zero_one_with_seed(16, Some(5)),
            "{circuit_text} should sample like its decomposed form"
        );

        let converter = CompiledDetectionConverter::compile(
            &circuit,
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .unwrap_or_else(|error| panic!("detection converter rejected {circuit_text}: {error}"));
        let decomposed_converter = CompiledDetectionConverter::compile(
            &decomposed,
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .expect("compile decomposed detector conversion");
        assert_eq!(
            converter.detector_count(),
            decomposed_converter.detector_count(),
            "{circuit_text} should keep the decomposed detector count"
        );

        assert_eq!(
            sample_detection_events(&circuit, 16, Some(7)).unwrap_or_else(|error| panic!(
                "detection sampling rejected {circuit_text}: {error}"
            )),
            sample_detection_events(&decomposed, 16, Some(7))
                .expect("sample decomposed detection events"),
            "{circuit_text} should detect like its decomposed form"
        );
    }
}

#[test]
fn variable_target_spp_matches_hard_coded_phase_product_decompositions() {
    let spp_x =
        Circuit::from_stim_str("SPP X0\nM 0\nDETECTOR rec[-1]\n").expect("parse SPP X circuit");
    let explicit_x = Circuit::from_stim_str("H 0\nS 0\nH 0\nM 0\nDETECTOR rec[-1]\n")
        .expect("parse explicit SPP X decomposition");

    let spp_sampler = CompiledSampler::compile(&spp_x).expect("compile SPP X sampler");
    let explicit_sampler =
        CompiledSampler::compile(&explicit_x).expect("compile explicit SPP X sampler");
    assert_eq!(
        spp_sampler.sample_zero_one_with_seed(32, Some(11)),
        explicit_sampler.sample_zero_one_with_seed(32, Some(11))
    );
    assert_eq!(
        sample_detection_events(&spp_x, 32, Some(13)).expect("sample SPP X detection"),
        sample_detection_events(&explicit_x, 32, Some(13))
            .expect("sample explicit SPP X detection")
    );

    let spp_xx = Circuit::from_stim_str("SPP X0*X1\nOBSERVABLE_INCLUDE(0) X0\n")
        .expect("parse frame-path SPP XX circuit");
    let explicit_xx = Circuit::from_stim_str(
        "H 0\nH 1\nCX 1 0\nS 0\nCX 1 0\nH 1\nH 0\nOBSERVABLE_INCLUDE(0) X0\n",
    )
    .expect("parse explicit frame-path SPP XX decomposition");
    assert_eq!(
        sample_detection_events(&spp_xx, 32, Some(17)).expect("sample frame-path SPP XX"),
        sample_detection_events(&explicit_xx, 32, Some(17))
            .expect("sample explicit frame-path SPP XX")
    );
}

#[test]
fn variable_target_spp_executes_in_frame_detection_path() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let circuit =
            Circuit::from_stim_str(&format!("{gate_name} X0*X1\nOBSERVABLE_INCLUDE(0) X0\n"))
                .expect("parse frame-detection SPP circuit");

        let output = sample_detection_events(&circuit, 4, Some(0)).unwrap_or_else(|error| {
            panic!("frame detector sampling rejected {gate_name}: {error}")
        });
        assert_eq!(output.detector_count, 0, "{gate_name}");
        assert_eq!(output.observable_count, 1, "{gate_name}");
        assert_eq!(output.records.len(), 4, "{gate_name}");
    }
}

#[test]
fn anti_hermitian_spp_execution_is_rejected_by_sampler_and_detection_conversion() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let circuit =
            Circuit::from_stim_str(&format!("{gate_name} X0*Z0\nM 0\nDETECTOR rec[-1]\n"))
                .expect("parse anti-Hermitian SPP circuit");

        let sampler_error = CompiledSampler::compile(&circuit)
            .expect_err("sampler should reject anti-Hermitian SPP");
        assert!(
            matches!(
                sampler_error,
                CircuitError::InvalidSamplerCompilation { .. }
            ),
            "{gate_name}: {sampler_error}"
        );
        let sampler_error = sampler_error.to_string();
        assert!(
            sampler_error.contains("anti-Hermitian"),
            "{gate_name}: {sampler_error}"
        );

        for skip_reference_sample in [false, true] {
            let converter_error = CompiledDetectionConverter::compile(
                &circuit,
                DetectionConversionOptions {
                    skip_reference_sample,
                },
            )
            .expect_err("detection conversion should reject anti-Hermitian SPP");
            assert!(
                matches!(
                    converter_error,
                    CircuitError::InvalidSamplerCompilation { .. }
                ),
                "{gate_name}: {converter_error}"
            );
            let converter_error = converter_error.to_string();
            assert!(
                converter_error.contains("anti-Hermitian"),
                "{gate_name}: {converter_error}"
            );
        }

        let frame_circuit =
            Circuit::from_stim_str(&format!("{gate_name} X0*Z0\nOBSERVABLE_INCLUDE(0) X0\n"))
                .expect("parse anti-Hermitian frame SPP circuit");
        let frame_error = sample_detection_events(&frame_circuit, 1, Some(19))
            .expect_err("frame detection should reject anti-Hermitian SPP");
        assert!(
            matches!(frame_error, CircuitError::InvalidSamplerCompilation { .. }),
            "{gate_name}: {frame_error}"
        );
        let frame_error = frame_error.to_string();
        assert!(
            frame_error.contains("anti-Hermitian"),
            "{gate_name}: {frame_error}"
        );
    }
}

struct GateExecutionCase {
    gate_name: &'static str,
    circuit: Circuit,
}

fn assert_binomial_5_sigma(label: &str, count: usize, trials: usize, probability: f64) {
    let mean = trials as f64 * probability;
    let stddev = (trials as f64 * probability * (1.0 - probability)).sqrt();
    let lower = (mean - 5.0 * stddev).floor().max(0.0);
    let upper = (mean + 5.0 * stddev).ceil().min(trials as f64);
    let count = count as f64;
    assert!(
        lower <= count && count <= upper,
        "{label}: expected count in {lower:.0}..={upper:.0} for n={trials}, p={probability}, got {count:.0}"
    );
}

fn fixed_tableau_gate_cases() -> Vec<GateExecutionCase> {
    Gate::all()
        .filter(|gate| gate.has_tableau())
        .map(|gate| {
            let gate_name = gate.canonical_name();
            let inverse_name = gate
                .inverse()
                .expect("fixed-tableau gate has inverse")
                .canonical_name();
            let arity = gate.tableau().expect("fixed-tableau gate").len();
            let targets = match arity {
                1 => "0",
                2 => "0 1",
                other => panic!("{gate_name} has unexpected arity {other}"),
            };
            let circuit = Circuit::from_stim_str(&format!(
                "{gate_name} {targets}\n{inverse_name} {targets}\nM 0\nDETECTOR rec[-1]\n"
            ))
            .unwrap_or_else(|error| {
                panic!("failed to build {gate_name} execution circuit: {error}")
            });
            GateExecutionCase { gate_name, circuit }
        })
        .collect()
}
