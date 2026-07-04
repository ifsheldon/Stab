#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "PF3 gate semantic execution tests use compact per-gate diagnostics"
)]

use stab_core::{
    Circuit, CompiledDetectionConverter, CompiledSampler, DetectionConversionOptions,
    ErrorAnalyzerOptions, Gate, circuit_to_detector_error_model,
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
fn variable_target_spp_execution_stays_explicitly_rejected() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let circuit =
            Circuit::from_stim_str(&format!("{gate_name} X0*X1\nM 0\nDETECTOR rec[-1]\n"))
                .expect("parse SPP circuit");

        let sampler_error = CompiledSampler::compile(&circuit)
            .expect_err("sampler rejects variable-target SPP execution")
            .to_string();
        assert!(
            sampler_error.contains("sampler subset does not support"),
            "{gate_name}: {sampler_error}"
        );

        let converter_error = CompiledDetectionConverter::compile(
            &circuit,
            DetectionConversionOptions {
                skip_reference_sample: true,
            },
        )
        .expect_err("detection conversion rejects variable-target SPP execution")
        .to_string();
        assert!(
            converter_error.contains("detection conversion does not yet support"),
            "{gate_name}: {converter_error}"
        );

        let analyzer_error =
            circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
                .expect_err("analyzer rejects variable-target SPP execution")
                .to_string();
        assert!(
            analyzer_error.contains("analyze_errors does not yet support"),
            "{gate_name}: {analyzer_error}"
        );
    }
}

struct GateExecutionCase {
    gate_name: &'static str,
    circuit: Circuit,
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
