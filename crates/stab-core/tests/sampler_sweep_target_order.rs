#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "PF3 sampler sweep target-order tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    Circuit, DetectionConversionOptions, convert_measurements_to_detection_events_with_sweep,
    sample_detection_events, validate_detection_sampling_circuit,
};

#[test]
fn pf3_sampler_sweep_target_order_rejects_cx_cy_second_sweep() {
    for source in [
        "CX 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
        "CY 0 sweep[0]\nM 0\nDETECTOR rec[-1]\n",
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse invalid sweep order");

        let reference_error = circuit
            .reference_sample()
            .expect_err("reject reference sampling")
            .to_string();
        assert!(
            reference_error.contains("does not support"),
            "{reference_error}"
        );

        let conversion_error = convert_measurements_to_detection_events_with_sweep(
            &circuit,
            &[vec![false]],
            &[vec![true]],
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .expect_err("reject detection conversion")
        .to_string();
        assert!(
            conversion_error.contains("does not support"),
            "{conversion_error}"
        );

        let validation_error = validate_detection_sampling_circuit(&circuit)
            .expect_err("reject detection sampling validation")
            .to_string();
        assert!(
            validation_error.contains("does not support"),
            "{validation_error}"
        );

        let sampling_error = sample_detection_events(&circuit, 1, Some(5))
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
        circuit
            .reference_sample()
            .expect("reference sample accepted order");
        validate_detection_sampling_circuit(&circuit).expect("validate accepted order");
        convert_measurements_to_detection_events_with_sweep(
            &circuit,
            &[vec![false]],
            &[vec![false]],
            DetectionConversionOptions {
                skip_reference_sample: false,
            },
        )
        .expect("convert accepted order");
    }
}
