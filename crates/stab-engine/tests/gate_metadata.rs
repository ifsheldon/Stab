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
fn supported_spp_execution_paths_preserve_semantics() {
    for gate_name in ["SPP", "SPP_DAG"] {
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
