#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "PF3 sampler sweep target-order tests use direct fixture assertions for compact diagnostics"
)]

mod support;

use stab_engine::{DetectionSamplingCompiler, ReferenceSampleMode, SamplingCompiler};
use stab_model::Circuit;
use support::{convert_detection_records, sample_detections};

/// Computes the reference sample of a default-compiled sampling plan, reporting a failure of
/// either step as its message.
fn circuit_reference_sample(circuit: &Circuit) -> Result<Vec<bool>, String> {
    let plan = SamplingCompiler::default()
        .compile(circuit)
        .map_err(|error| error.to_string())?;
    let sample = plan
        .try_reference_sample()
        .map_err(|error| error.to_string())?;
    Ok(sample.iter().collect())
}

#[test]
fn pf3_sampler_sweep_target_order_rejects_cx_cy_second_sweep() {
    for source in [
        "CX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
        "CY 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse invalid sweep order");

        let reference_error =
            circuit_reference_sample(&circuit).expect_err("reject reference sampling");
        assert!(
            reference_error.contains("does not support"),
            "{reference_error}"
        );

        let conversion_error = convert_detection_records(
            &circuit,
            &[vec![false]],
            Some(&[vec![true]]),
            ReferenceSampleMode::UseReferenceSample,
        )
        .expect_err("reject detection conversion")
        .to_string();
        assert!(
            conversion_error.contains("does not support"),
            "{conversion_error}"
        );

        let validation_error = DetectionSamplingCompiler::default()
            .compile(&circuit)
            .expect_err("reject detection sampling validation")
            .to_string();
        assert!(
            validation_error.contains("does not support"),
            "{validation_error}"
        );

        let sampling_error = sample_detections(&circuit, 1, Some(5))
            .expect_err("reject detection sampling")
            .to_string();
        assert!(
            sampling_error.contains("does not support"),
            "{sampling_error}"
        );
    }
}

#[test]
fn pf3_sampler_sweep_target_order_keeps_accepted_orders() {
    for source in [
        "CX sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
        "CY sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
        "CZ sweep[0] 0\nM 0\nDETECTOR rec[-1]\n",
        "CZ 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse accepted sweep order");
        circuit_reference_sample(&circuit).expect("reference sample accepted order");
        DetectionSamplingCompiler::default()
            .compile(&circuit)
            .expect("validate accepted order");
        convert_detection_records(
            &circuit,
            &[vec![false]],
            Some(&[vec![false]]),
            ReferenceSampleMode::UseReferenceSample,
        )
        .expect("convert accepted order");
    }
}
