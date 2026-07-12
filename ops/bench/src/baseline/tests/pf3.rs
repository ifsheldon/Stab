use super::*;

#[test]
fn pf3_sweep_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurements, measurement) in [
        (
            "pf3-m2d-sweep-b8",
            &["stab_pf3_m2d_sweep_b8"][..],
            "m2d-sweep",
        ),
        (
            "pf3-m2d-sweep-ptb64-input",
            &["stab_pf3_m2d_sweep_ptb64"][..],
            "m2d-sweep-ptb64",
        ),
        (
            "pf3-detect-sweep-sampling",
            &[
                "stab_detect_sweep_default_false",
                "stab_detect_frame_sweep_default_false",
            ][..],
            "detect-sweep",
        ),
        (
            "pf3-analyze-errors-sweep",
            &[
                "stab_analyze_errors_sweep_control",
                "stab_analyze_errors_sweep_id_low",
                "stab_analyze_errors_sweep_id_max",
            ][..],
            "analyze-errors-sweep",
        ),
        (
            "pf3-gate-semantic-wide",
            &[
                "stab_pf3_gate_sampler_execution",
                "stab_pf3_gate_reference_sampling",
                "stab_pf3_gate_converter_compilation",
                "stab_pf3_gate_detection_sampling",
                "stab_pf3_gate_detector_frame_sampling",
                "stab_pf3_gate_error_analysis",
                "stab_pf3_gate_flow_generation",
            ][..],
            "gate-semantic-wide",
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf3,
            threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim/simulators/frame_simulator.perf.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: measurement.to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        };

        let measurements = run_stab_compare_row(&row)
            .expect("run compare row")
            .expect("Stab runner");
        let names = measurements
            .iter()
            .map(|measurement| measurement.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names.as_slice(), expected_measurements);
        assert!(
            compare_note(id).is_some(),
            "{id} should explain benchmark comparability"
        );
        for expected_measurement in expected_measurements {
            assert!(
                measurement_work(id, expected_measurement).is_some(),
                "{id}/{expected_measurement} should report normalized work"
            );
        }
    }
}

#[cfg(feature = "count-allocations")]
#[test]
fn pf3_analyzer_sweep_allocation_is_index_magnitude_independent() {
    use std::hint::black_box;

    use stab_core::{Circuit, ErrorAnalyzerOptions, circuit_to_detector_error_model};

    fn analyze(circuit: &Circuit) {
        let model = circuit_to_detector_error_model(circuit, ErrorAnalyzerOptions::default())
            .expect("sweep resource fixture must analyze");
        black_box(model);
    }

    let low = Circuit::from_stim_str(&super::super::m10::analyze_sweep_id_fixture(0))
        .expect("low sweep-id fixture");
    let high = Circuit::from_stim_str(&super::super::m10::analyze_sweep_id_fixture(16_777_215))
        .expect("maximum sweep-id fixture");
    analyze(&low);
    analyze(&high);

    let low_allocations = allocation_counter::measure(|| analyze(&low));
    let high_allocations = allocation_counter::measure(|| analyze(&high));
    const ALLOWED_COUNT_DELTA: u64 = 2;
    const ALLOWED_BYTE_DELTA: u64 = 1_024;
    assert!(
        high_allocations.count_total
            <= low_allocations
                .count_total
                .saturating_add(ALLOWED_COUNT_DELTA),
        "maximum sweep id increased allocation count: low={low_allocations:?}, high={high_allocations:?}"
    );
    assert!(
        high_allocations.bytes_total
            <= low_allocations
                .bytes_total
                .saturating_add(ALLOWED_BYTE_DELTA),
        "maximum sweep id increased total allocated bytes: low={low_allocations:?}, high={high_allocations:?}"
    );
    assert!(
        high_allocations.bytes_max <= low_allocations.bytes_max.saturating_add(ALLOWED_BYTE_DELTA),
        "maximum sweep id increased peak live bytes: low={low_allocations:?}, high={high_allocations:?}"
    );
}
