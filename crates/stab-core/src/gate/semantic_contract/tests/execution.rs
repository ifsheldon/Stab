#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "PFM-B2 generated semantic tests use direct assertions for compact diagnostics"
)]

use std::collections::{BTreeMap, BTreeSet};

use super::super::{
    GateSemanticFamily, GateSurface, GateSurfaceBehavior, GateTargetPattern,
    gate_contract_statistical_plan,
};
use crate::{
    Circuit, CircuitResult, CompiledDetectionConverter, CompiledSampler,
    DetectionConversionOptions, ErrorAnalyzerOptions, Gate, Probability, circuit_flow_generators,
    circuit_to_detector_error_model, sample_detection_events,
};

mod statistical;

#[test]
fn gate_surface_contract_fixed_tableau() {
    let gates = gates_in_families(&[GateSemanticFamily::FixedTableau]);
    assert!(!gates.is_empty());
    for gate in gates {
        let inverse = gate.inverse().expect("fixed-tableau inverse");
        let targets = match gate.tableau().expect("fixed tableau").len() {
            1 => "0",
            2 => "0 1",
            arity => panic!("{} has unexpected arity {arity}", gate.canonical_name()),
        };
        let measured_qubits = if targets == "0" { "0" } else { "0 1" };
        let circuit_text = format!(
            "{} {targets}\n{} {targets}\nM {measured_qubits}\n",
            gate.canonical_name(),
            inverse.canonical_name()
        );
        assert_all_semantic_surfaces_execute(&circuit_text);
        let circuit = circuit(&circuit_text);
        let sampler = CompiledSampler::compile(&circuit).expect("compile inverse pair");
        assert!(
            sampler
                .sample_zero_one_with_seed(8, Some(11))
                .iter()
                .all(|record| record.iter().all(|bit| !bit)),
            "{} followed by {} must preserve |0>",
            gate.canonical_name(),
            inverse.canonical_name()
        );
        assert_empty_target_semantic_noop(gate.canonical_name(), "");
    }
}

#[test]
fn gate_surface_contract_fixed_tableau_general_circuit() {
    let mut text = String::new();
    for gate in gates_in_families(&[GateSemanticFamily::FixedTableau]) {
        let inverse = gate.inverse().expect("fixed-tableau inverse");
        let targets = match gate.tableau().expect("fixed tableau").len() {
            1 => "0",
            2 => "0 1",
            arity => panic!("{} has unexpected arity {arity}", gate.canonical_name()),
        };
        text.push_str(&format!(
            "{} {targets}\n{} {targets}\n",
            gate.canonical_name(),
            inverse.canonical_name()
        ));
    }
    text.push_str("M 0 1\n");
    assert_exact_reference_and_samples(&text, &[false, false]);
    assert_all_semantic_surfaces_execute(&text);
}

#[test]
fn gate_surface_contract_measure_reset() {
    assert_family_names(
        &[
            GateSemanticFamily::Measurement,
            GateSemanticFamily::MeasureReset,
            GateSemanticFamily::Reset,
        ],
        &["M", "MR", "MRX", "MRY", "MX", "MY", "R", "RX", "RY"],
    );

    for (prepare, measure, expected) in [
        ("R 0 1", "M !0 1", [true, false]),
        ("RX 0 1", "MX !0 1", [true, false]),
        ("RY 0 1", "MY !0 1", [true, false]),
        ("R 0 1", "M(1) !0 1", [false, true]),
        ("RX 0 1", "MX(1) !0 1", [false, true]),
        ("RY 0 1", "MY(1) !0 1", [false, true]),
    ] {
        let text = format!("{prepare}\n{measure}\n");
        assert_exact_reference_and_samples(&text, &expected);
        assert_all_semantic_surfaces_execute(&text);
    }

    for (prepare, reset, verify) in [
        ("X 0", "R 0", "M 0"),
        ("Z 0", "RX 0", "MX 0"),
        ("X 0", "RY 0", "MY 0"),
    ] {
        let text = format!("{prepare}\n{reset}\n{verify}\n");
        assert_exact_reference_and_samples(&text, &[false]);
        assert_all_semantic_surfaces_execute(&text);
    }

    for gate in ["M", "MX", "MY", "MR", "MRX", "MRY", "R", "RX", "RY"] {
        assert_empty_target_semantic_noop(gate, "");
    }
}

#[test]
fn gate_surface_contract_measure_reset_x() {
    assert_noisy_measure_reset_basis("pfm3-contract-measure-reset-x", "RX 0\nZ 0", "MRX", "MX");
}

#[test]
fn gate_surface_contract_measure_reset_y() {
    assert_noisy_measure_reset_basis("pfm3-contract-measure-reset-y", "RY 0\nX 0", "MRY", "MY");
}

#[test]
fn gate_surface_contract_measure_reset_z() {
    assert_noisy_measure_reset_basis("pfm3-contract-measure-reset-z", "X 0", "MR", "M");
}

#[test]
fn gate_surface_contract_pair_measurement() {
    assert_family_names(
        &[GateSemanticFamily::PairMeasurement],
        &["MXX", "MYY", "MZZ"],
    );
    for (prepare, gate) in [("RX 0 1", "MXX"), ("RY 0 1", "MYY"), ("R 0 1", "MZZ")] {
        let text = format!("{prepare}\n{gate} 0 1\n");
        assert_exact_reference_and_samples(&text, &[false]);
        assert_all_semantic_surfaces_execute(&text);
        assert_empty_target_semantic_noop(gate, "");
    }
}

#[test]
fn gate_surface_contract_pair_measurement_inversion() {
    for (prepare, gate) in [("RX 0 1", "MXX"), ("RY 0 1", "MYY"), ("R 0 1", "MZZ")] {
        let text = format!("{prepare}\n{gate} !0 1\n");
        assert_exact_reference_and_samples(&text, &[true]);
        assert_all_semantic_surfaces_execute(&text);

        let probability_flipped = format!("{prepare}\n{gate}(1) !0 1\n");
        assert_exact_reference_and_samples(&probability_flipped, &[false]);
        assert_all_semantic_surfaces_execute(&probability_flipped);
    }
}

#[test]
fn gate_surface_contract_mpp_deterministic() {
    assert_family_names(&[GateSemanticFamily::PauliProductMeasurement], &["MPP"]);
    let bell = "H 0\nCX 0 1\nMPP X0*X1 !Z0*Z1 X0*X0\n";
    assert_exact_reference_and_samples(bell, &[false, true, false]);
    assert_all_semantic_surfaces_execute(bell);

    let flipped = "H 0\nCX 0 1\nMPP(1) X0*X1 !Z0*Z1\n";
    assert_exact_reference_and_samples(flipped, &[true, false]);
    assert_all_semantic_surfaces_execute(flipped);
    assert_empty_target_semantic_noop("MPP", "");
}

#[test]
fn gate_surface_contract_mpp_anti_hermitian_rejection() {
    let text = "MPP X0*Z0\nM 0\n";
    let parsed = circuit(text);
    assert_eq!(
        Gate::from_name("MPP")
            .expect("MPP")
            .surface_contract()
            .classify_target_groups(
                parsed
                    .iter_flattened_instructions()
                    .next()
                    .expect("MPP instruction")
                    .targets()
            ),
        Some(vec![GateTargetPattern::AntiHermitianPauliProduct])
    );
    for surface in GateSurface::ALL {
        let result = run_surface(text, surface);
        if surface == GateSurface::Parser {
            result.expect("parser accepts anti-Hermitian product syntax");
        } else {
            let error = result.expect_err("semantic surface must reject anti-Hermitian MPP");
            assert!(
                error.to_string().contains("anti-Hermitian")
                    || error.to_string().contains("not Hermitian"),
                "{surface:?}: {error}"
            );
        }
    }
}

#[test]
fn gate_surface_contract_mpad_deterministic() {
    assert_family_names(&[GateSemanticFamily::MeasurementPad], &["MPAD"]);
    let text = "MPAD 0 1 0 1\n";
    assert_exact_reference_and_samples(text, &[false, true, false, true]);
    assert_all_semantic_surfaces_execute(text);

    let flipped = "MPAD(1) 0 1\n";
    assert_exact_reference_and_samples(flipped, &[true, false]);
    assert_all_semantic_surfaces_execute(flipped);
    assert_empty_target_semantic_noop("MPAD", "");
}

#[test]
fn gate_surface_contract_spp() {
    assert_family_names(
        &[GateSemanticFamily::PauliProductPhase],
        &["SPP", "SPP_DAG"],
    );
    for text in ["SPP X0\nM 0\n", "SPP_DAG Y0\nM 0\n"] {
        let original = circuit(text);
        let decomposed = original.decomposed().expect("decompose SPP");
        assert_circuit_semantics_equal(&original, &decomposed, text);
        assert_all_semantic_surfaces_execute(text);
    }
    for gate in ["SPP", "SPP_DAG"] {
        assert_empty_target_semantic_noop(gate, "");
    }
}

#[test]
fn gate_surface_contract_spp_multiple() {
    for text in [
        "SPP !X0*X1 Z2\nM 0 1 2\n",
        "SPP_DAG Y0*Y1 X2*X3\nM 0 1 2 3\n",
        "SPP Z0*Z0\nM 0\n",
    ] {
        let original = circuit(text);
        let decomposed = original.decomposed().expect("decompose grouped SPP");
        assert_circuit_semantics_equal(&original, &decomposed, text);
        assert_all_semantic_surfaces_execute(text);
    }
}

#[test]
fn gate_surface_contract_spp_rejection() {
    for gate in ["SPP", "SPP_DAG"] {
        let text = format!("{gate} X0*Z0\nM 0\n");
        for surface in GateSurface::ALL {
            let result = run_surface(&text, surface);
            if surface == GateSurface::Parser {
                result.expect("parser accepts anti-Hermitian phase product syntax");
            } else {
                let error = result.expect_err("semantic surface rejects anti-Hermitian SPP");
                assert!(
                    error.to_string().contains("anti-Hermitian")
                        || error.to_string().contains("not Hermitian"),
                    "{surface:?}: {error}"
                );
            }
        }
    }
}

#[test]
fn gate_surface_contract_identity_noise() {
    assert_family_names(
        &[GateSemanticFamily::IdentityNoise],
        &["II_ERROR", "I_ERROR"],
    );
    for instruction in [
        "I_ERROR 0",
        "I_ERROR(0.1,0.2,0.3) 0",
        "II_ERROR 0 1",
        "II_ERROR(0.25,0.5) 0 1",
    ] {
        let with_noise = format!("H 0\nH 0\n{instruction}\nM 0 1\n");
        let without_noise = "H 0\nH 0\nM 0 1\n";
        assert_circuit_semantics_equal(&circuit(&with_noise), &circuit(without_noise), instruction);
        assert_all_semantic_surfaces_execute(&with_noise);
    }
    assert_empty_target_semantic_noop("I_ERROR", "(0.25)");
    assert_empty_target_semantic_noop("II_ERROR", "(0.25)");
}

#[test]
fn gate_surface_contract_annotations() {
    assert_family_names(
        &[GateSemanticFamily::Annotation],
        &[
            "DETECTOR",
            "OBSERVABLE_INCLUDE",
            "QUBIT_COORDS",
            "SHIFT_COORDS",
            "TICK",
        ],
    );
    let text = "X_ERROR(0.25) 0\nM 0\nDETECTOR rec[-1]\n";
    assert_all_semantic_surfaces_execute(text);
    let dem = circuit_to_detector_error_model(&circuit(text), ErrorAnalyzerOptions::default())
        .expect("analyze vacuous detector declaration")
        .to_string();
    assert_eq!(dem, "error(0.25) D0\n");

    for text in [
        "M 0\nDETECTOR\n",
        "M 0\nOBSERVABLE_INCLUDE(0)\n",
        "QUBIT_COORDS(1,2)\nM 0\n",
    ] {
        assert_all_semantic_surfaces_execute(text);
    }
}

#[test]
fn gate_surface_contract_annotation_coordinates() {
    let text = "QUBIT_COORDS(1,2) 0\nTICK\nM 0\nDETECTOR(3) rec[-1]\nOBSERVABLE_INCLUDE(0) rec[-1]\nSHIFT_COORDS(4,5)\nDETECTOR(6) rec[-1]\n";
    assert_exact_reference_and_samples(text, &[false]);
    assert_all_semantic_surfaces_execute(text);
    let dem = circuit_to_detector_error_model(&circuit(text), ErrorAnalyzerOptions::default())
        .expect("analyze annotation coordinates")
        .to_string();
    assert_eq!(
        dem,
        "detector(3) D0\nlogical_observable L0\nshift_detectors(4, 5) 0\ndetector(6) D1\n"
    );
}

#[test]
fn gate_surface_contract_annotation_tags() {
    let text = "R[reset-tag] 0\nX_ERROR[error-tag](0.25) 0\nM[measure-tag] 0\nDETECTOR[detector-tag] rec[-1]\nOBSERVABLE_INCLUDE[observable-tag](0) rec[-1]\nSHIFT_COORDS[shift-tag](1)\n";
    assert_all_semantic_surfaces_execute(text);
    let dem = circuit_to_detector_error_model(&circuit(text), ErrorAnalyzerOptions::default())
        .expect("analyze tagged annotations")
        .to_string();
    assert_eq!(
        dem,
        "error[error-tag](0.25) D0 L0\ndetector[detector-tag] D0\nlogical_observable[observable-tag] L0\nshift_detectors[shift-tag](1) 0\n"
    );
}

#[test]
fn gate_surface_contract_classical_controls() {
    assert_family_names(
        &[
            GateSemanticFamily::ForwardClassicalControl,
            GateSemanticFamily::SymmetricClassicalControl,
            GateSemanticFamily::ReverseClassicalControl,
        ],
        &["CX", "CY", "CZ", "XCZ", "YCZ"],
    );

    for gate_name in ["CX", "CY", "CZ", "XCZ", "YCZ"] {
        let gate = Gate::from_name(gate_name).expect("controlled Pauli gate");
        let contract = gate.surface_contract();
        for pattern in contract.target_patterns() {
            let text = classical_control_circuit(gate_name, *pattern);
            let baseline = classical_control_baseline();
            for surface in GateSurface::ALL {
                let decision = contract
                    .decision(surface, *pattern)
                    .expect("declared classical-control decision");
                let result = run_surface(&text, surface);
                match decision.behavior {
                    GateSurfaceBehavior::UnsupportedShape => {
                        result.expect_err("unsupported classical-control role must reject");
                    }
                    GateSurfaceBehavior::SemanticNoop => {
                        let actual = result.expect("semantic no-op must execute");
                        let expected = run_surface(&baseline, surface)
                            .expect("classical-control baseline must execute");
                        assert_eq!(actual, expected, "{gate_name} {pattern:?} on {surface:?}");
                    }
                    GateSurfaceBehavior::Execute => {
                        result.unwrap_or_else(|error| {
                            panic!("{gate_name} {pattern:?} on {surface:?}: {error}")
                        });
                    }
                    other => panic!(
                        "classical-control contract unexpectedly uses {other:?} for {gate_name} {pattern:?} on {surface:?}"
                    ),
                }
            }
        }
    }
}

#[test]
fn gate_surface_contract_classical_control_rejection() {
    for text in [
        "M 0\nCX 1 rec[-1]\n",
        "M 0\nCY 1 rec[-1]\n",
        "M 0\nXCZ rec[-1] 1\n",
        "M 0\nYCZ rec[-1] 1\n",
    ] {
        let parsed = Circuit::from_stim_str(text).expect("parser accepts target syntax");
        CompiledSampler::compile(&parsed)
            .expect_err("frame-style sampling must reject quantum control of a classical target");
    }
}

#[test]
fn gate_surface_contract_classical_control_feedback() {
    for text in [
        "MPAD 1\nCX rec[-1] 0\nM 0\n",
        "MPAD 1\nCY rec[-1] 0\nM 0\n",
        "MPAD 1\nXCZ 0 rec[-1]\nM 0\n",
        "MPAD 1\nYCZ 0 rec[-1]\nM 0\n",
    ] {
        assert_exact_reference_and_samples(text, &[true, true]);
        assert_all_semantic_surfaces_execute(text);
    }
}

#[test]
fn gate_surface_contract_classical_control_no_sweep_data() {
    let text = "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n";
    let sampler = CompiledSampler::compile_allowing_sweep(&circuit(text))
        .expect("compile omitted sweep-data circuit");
    assert_eq!(
        sampler.sample_zero_one_with_seed(32, Some(17)),
        vec![vec![false]; 32]
    );
    assert_all_semantic_surfaces_execute(text);
}

#[test]
fn gate_surface_contract_control_flow() {
    assert_family_names(&[GateSemanticFamily::ControlFlow], &["REPEAT"]);
    let folded = circuit("REPEAT 3 {\n    H 0\n    H 0\n}\nM 0\n");
    let unrolled = circuit("H 0\nH 0\nH 0\nH 0\nH 0\nH 0\nM 0\n");
    assert_circuit_semantics_equal(&folded, &unrolled, "REPEAT 3");
    assert_all_semantic_surfaces_execute(&folded.to_stim_string());

    let nested = circuit("REPEAT 2 {\n    REPEAT 3 {\n        X 0\n    }\n}\nM 0\n");
    assert_exact_reference_and_samples(&nested.to_stim_string(), &[false]);
    assert_all_semantic_surfaces_execute(&nested.to_stim_string());
}

#[derive(Debug, Eq, PartialEq)]
enum SurfaceFingerprint {
    Parsed(String),
    Samples(Vec<Vec<bool>>),
    Reference(Vec<bool>),
    Converted(crate::DetectionEventRecord),
    Detected(crate::DetectionConversionOutput),
    Analyzed(String),
    Flows(Vec<crate::Flow>),
}

fn run_surface(text: &str, surface: GateSurface) -> CircuitResult<SurfaceFingerprint> {
    let circuit = Circuit::from_stim_str(text)?;
    match surface {
        GateSurface::Parser => Ok(SurfaceFingerprint::Parsed(circuit.to_stim_string())),
        GateSurface::MeasurementSampler => {
            let sampler = CompiledSampler::compile_allowing_sweep(&circuit)?;
            Ok(SurfaceFingerprint::Samples(
                sampler.sample_zero_one_with_seed(4, Some(7)),
            ))
        }
        GateSurface::ReferenceSampler => {
            Ok(SurfaceFingerprint::Reference(circuit.reference_sample()?))
        }
        GateSurface::DetectionConverter => {
            let converter = CompiledDetectionConverter::compile(
                &circuit,
                DetectionConversionOptions {
                    skip_reference_sample: false,
                },
            )?;
            let sampler = CompiledSampler::compile_allowing_sweep(&circuit)?;
            let sweep_record = vec![false; converter.sweep_bit_count()];
            let mut reference = Vec::with_capacity(converter.measurement_count());
            sampler.reference_measurement_record_with_sweep_into(&sweep_record, &mut reference)?;
            let record = if converter.sweep_bit_count() == 0 {
                converter.convert_record(&reference)?
            } else {
                let mut output = converter.reusable_detection_record();
                let mut reference_sample = converter.reusable_reference_sample();
                converter.convert_record_with_sweep_into(
                    &reference,
                    &sweep_record,
                    &mut reference_sample,
                    &mut output,
                )?;
                output
            };
            Ok(SurfaceFingerprint::Converted(record))
        }
        GateSurface::DetectorFrame => {
            let frame = Circuit::from_stim_str(&format!(
                "{}OBSERVABLE_INCLUDE(100) Z0\n",
                circuit.to_stim_string()
            ))?;
            Ok(SurfaceFingerprint::Detected(sample_detection_events(
                &frame,
                4,
                Some(7),
            )?))
        }
        GateSurface::DetectionSampler => Ok(SurfaceFingerprint::Detected(sample_detection_events(
            &circuit,
            4,
            Some(7),
        )?)),
        GateSurface::ErrorAnalyzer => Ok(SurfaceFingerprint::Analyzed(
            circuit_to_detector_error_model(
                &circuit,
                ErrorAnalyzerOptions {
                    approximate_disjoint_errors_threshold: Some(
                        Probability::try_new(1.0).expect("unit probability"),
                    ),
                    ..ErrorAnalyzerOptions::default()
                },
            )?
            .to_string(),
        )),
        GateSurface::FlowGenerator => Ok(SurfaceFingerprint::Flows(circuit_flow_generators(
            &circuit,
        )?)),
    }
}

fn assert_all_semantic_surfaces_execute(text: &str) {
    for surface in GateSurface::ALL {
        run_surface(text, surface)
            .unwrap_or_else(|error| panic!("{surface:?} rejected {text:?}: {error}"));
    }
}

fn assert_exact_reference_and_samples(text: &str, expected: &[bool]) {
    let circuit = circuit(text);
    assert_eq!(
        circuit.reference_sample().expect("reference sample"),
        expected
    );
    let sampler = CompiledSampler::compile(&circuit).expect("compile deterministic sampler");
    assert_eq!(
        sampler.sample_zero_one_with_seed(4, Some(5)),
        vec![expected.to_vec(); 4]
    );
}

fn assert_noisy_measure_reset_basis(case_id: &str, prepare: &str, gate: &str, verify: &str) {
    let plan = statistical_plan(case_id);
    let text = format!("{prepare}\n{gate}(0.05) !0\n{verify} 0\n");
    let samples = CompiledSampler::compile(&circuit(&text))
        .expect("compile noisy measurement-reset circuit")
        .sample_zero_one_with_seed(statistical_shot_count(plan), Some(plan.seed));
    let mut counts = BTreeMap::from([("measurement-zero", 0), ("measurement-one", 0)]);
    for record in samples {
        let [measurement, reset_verification] = record.as_slice() else {
            panic!("expected noisy measurement and reset verification: {record:?}");
        };
        assert!(
            !reset_verification,
            "measurement-reset must reset its basis"
        );
        *counts
            .get_mut(if *measurement {
                "measurement-one"
            } else {
                "measurement-zero"
            })
            .expect("measurement-reset bucket") += 1;
    }
    assert_statistical_counts(case_id, &counts);
    assert_all_semantic_surfaces_execute(&text);
}

fn assert_empty_target_semantic_noop(gate_name: &str, arguments: &str) {
    let with_gate = circuit(&format!("{gate_name}{arguments}\nM 0\n"));
    let without_gate = circuit("M 0\n");
    assert_circuit_semantics_equal(&with_gate, &without_gate, gate_name);
}

fn assert_circuit_semantics_equal(actual: &Circuit, expected: &Circuit, label: &str) {
    let actual_sampler = CompiledSampler::compile_allowing_sweep(actual)
        .unwrap_or_else(|error| panic!("compile {label}: {error}"));
    let expected_sampler = CompiledSampler::compile_allowing_sweep(expected)
        .unwrap_or_else(|error| panic!("compile expected {label}: {error}"));
    assert_eq!(
        actual_sampler.sample_zero_one_with_seed(16, Some(29)),
        expected_sampler.sample_zero_one_with_seed(16, Some(29)),
        "measurement sampler: {label}"
    );
    assert_eq!(
        actual.reference_sample().expect("actual reference"),
        expected.reference_sample().expect("expected reference"),
        "reference sampler: {label}"
    );
    assert_eq!(
        sample_detection_events(actual, 16, Some(31)).expect("actual detection samples"),
        sample_detection_events(expected, 16, Some(31)).expect("expected detection samples"),
        "detection sampler: {label}"
    );
    let actual_frame = circuit(&format!(
        "{}OBSERVABLE_INCLUDE(100) Z0\n",
        actual.to_stim_string()
    ));
    let expected_frame = circuit(&format!(
        "{}OBSERVABLE_INCLUDE(100) Z0\n",
        expected.to_stim_string()
    ));
    assert_eq!(
        sample_detection_events(&actual_frame, 16, Some(37)).expect("actual frame samples"),
        sample_detection_events(&expected_frame, 16, Some(37)).expect("expected frame samples"),
        "detector frame: {label}"
    );
    assert_eq!(
        circuit_to_detector_error_model(actual, ErrorAnalyzerOptions::default())
            .expect("actual analysis"),
        circuit_to_detector_error_model(expected, ErrorAnalyzerOptions::default())
            .expect("expected analysis"),
        "error analyzer: {label}"
    );
    assert_eq!(
        circuit_flow_generators(actual).expect("actual flows"),
        circuit_flow_generators(expected).expect("expected flows"),
        "flow generator: {label}"
    );
}

fn assert_family_names(families: &[GateSemanticFamily], expected: &[&str]) {
    let actual = gates_in_families(families)
        .into_iter()
        .map(Gate::canonical_name)
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, expected.iter().copied().collect::<BTreeSet<_>>());
}

fn gates_in_families(families: &[GateSemanticFamily]) -> Vec<Gate> {
    Gate::all()
        .filter(|gate| families.contains(&gate.info.semantic_family))
        .collect()
}

fn classical_control_circuit(gate_name: &str, pattern: GateTargetPattern) -> String {
    let targets = match pattern {
        GateTargetPattern::EmptyTargetList => "",
        GateTargetPattern::QubitQubit => "0 1",
        GateTargetPattern::RecordQubit => "rec[-1] 0",
        GateTargetPattern::SweepQubit => "sweep[0] 0",
        GateTargetPattern::QubitRecord => "0 rec[-1]",
        GateTargetPattern::QubitSweep => "0 sweep[0]",
        GateTargetPattern::RecordRecord => "rec[-1] rec[-2]",
        GateTargetPattern::RecordSweep => "rec[-1] sweep[0]",
        GateTargetPattern::SweepRecord => "sweep[0] rec[-1]",
        GateTargetPattern::SweepSweep => "sweep[0] sweep[1]",
        other => panic!("unexpected classical-control pattern {other:?}"),
    };
    format!("M 2 3\n{gate_name} {targets}\nM 0 1\nDETECTOR rec[-1]\n")
}

fn classical_control_baseline() -> String {
    "M 2 3\nM 0 1\nDETECTOR rec[-1]\n".to_string()
}

fn assert_sweep_reference(text: &str, expected_false: &[bool], expected_true: &[bool]) {
    let circuit = circuit(text);
    let sampler = CompiledSampler::compile_allowing_sweep(&circuit).expect("compile sweep sampler");
    let mut false_reference = Vec::new();
    sampler
        .reference_sample_with_sweep_into(&[false], &mut false_reference)
        .expect("false sweep reference");
    let mut true_reference = Vec::new();
    sampler
        .reference_sample_with_sweep_into(&[true], &mut true_reference)
        .expect("true sweep reference");
    assert_eq!(false_reference, expected_false, "false sweep: {text}");
    assert_eq!(true_reference, expected_true, "true sweep: {text}");

    let converter = CompiledDetectionConverter::compile(
        &circuit,
        DetectionConversionOptions {
            skip_reference_sample: false,
        },
    )
    .expect("compile sweep converter");
    let mut converted = converter.reusable_detection_record();
    let mut reference = converter.reusable_reference_sample();
    converter
        .convert_record_with_sweep_into(expected_true, &[true], &mut reference, &mut converted)
        .expect("convert true sweep reference");
    assert!(converted.detectors.iter().all(|bit| !bit));
    assert!(converted.observables.iter().all(|bit| !bit));
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).unwrap_or_else(|error| panic!("parse {text:?}: {error}"))
}

fn statistical_plan(case_id: &str) -> &'static super::super::GateContractStatisticalPlan {
    gate_contract_statistical_plan(case_id)
        .unwrap_or_else(|| panic!("missing statistical plan for {case_id}"))
}

fn statistical_shot_count(plan: &super::super::GateContractStatisticalPlan) -> usize {
    usize::try_from(plan.shots).expect("statistical shot count fits usize")
}

fn assert_statistical_counts(case_id: &str, counts: &BTreeMap<&str, usize>) {
    let plan = statistical_plan(case_id);
    assert_eq!(counts.len(), plan.buckets.len(), "{case_id} bucket count");
    for bucket in plan.buckets {
        let count = counts
            .get(bucket.name)
            .copied()
            .unwrap_or_else(|| panic!("{case_id} missing bucket {}", bucket.name));
        let observed = count as f64 / plan.shots as f64;
        let standard_deviation =
            (bucket.expected_probability * (1.0 - bucket.expected_probability) / plan.shots as f64)
                .sqrt();
        let allowed_delta = plan
            .absolute_probability_floor
            .max(plan.sigma_multiplier * standard_deviation);
        assert!(
            (observed - bucket.expected_probability).abs() <= allowed_delta,
            "{case_id} bucket {} observed {observed:.6}, expected {:.6} +/- {allowed_delta:.6}",
            bucket.name,
            bucket.expected_probability
        );
    }
}
