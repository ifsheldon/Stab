#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "PF1 gate execution compatibility tests use direct assertions for compact diagnostics"
)]

use stab_analysis::{ErrorAnalyzerOptions, circuit_to_detector_error_model};
use stab_engine::{MeasurementToDetectionCompiler, ReferenceSampleMode};
use stab_model::Circuit;

mod support;
use support::{SamplingFixture, convert_detection_records};

#[test]
fn gate_execution_contract_accepts_supported_spp_execution_paths() {
    for gate_name in ["SPP", "SPP_DAG"] {
        let (sampler, detection_conversion, analyzer) = execution_support_row(gate_name);
        assert_eq!(sampler, "Decomposed", "{gate_name} sampler support");
        assert_eq!(
            detection_conversion, "Decomposed",
            "{gate_name} detection-conversion support"
        );
        assert_eq!(analyzer, "Yes", "{gate_name} analyzer support");

        let circuit = Circuit::from_stim_str(&format!("{gate_name} Z0\nM 0\nDETECTOR rec[-1]\n"))
            .expect("parse SPP");
        let sampler =
            SamplingFixture::compile(&circuit).expect("sampler should accept supported SPP");
        assert_eq!(sampler.sample_zero_one(1), vec![vec![false]]);
        for reference_mode in [
            ReferenceSampleMode::UseReferenceSample,
            ReferenceSampleMode::SkipReferenceSample,
        ] {
            MeasurementToDetectionCompiler::new()
                .reference_sample_mode(reference_mode)
                .compile(&circuit)
                .expect("detection conversion should accept supported SPP");
        }
        convert_detection_records(
            &circuit,
            &[vec![false]],
            None,
            ReferenceSampleMode::SkipReferenceSample,
        )
        .expect("plan conversion should accept supported SPP");
        circuit_to_detector_error_model(&circuit, ErrorAnalyzerOptions::default())
            .expect("analyzer should accept supported SPP");
    }
}

fn execution_support_row(gate_name: &str) -> (&'static str, &'static str, &'static str) {
    let line = include_str!("../../../docs/plans/rpf1-gate-execution-support-contract.md")
        .lines()
        .find(|line| line.starts_with(&format!("| `{gate_name}` |")))
        .expect("gate support contract row");
    let cells = line
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect::<Vec<_>>();
    let [
        _gate,
        _validation,
        _tableau,
        _unitary,
        _flow,
        _decomposition,
        sampler,
        detection_conversion,
        analyzer,
    ] = cells.as_slice()
    else {
        panic!("support contract row shape: {line}");
    };
    (sampler, detection_conversion, analyzer)
}
