use super::super::{compare_note, measurement_rate_work, measurement_work};
use super::run_stab_compare_row;
use crate::manifest::{BenchmarkRow, Milestone, Runner};

#[test]
fn pf4_dem_transform_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurements) in [
        (
            "pf4-dem-flatten-repeat",
            &["stab_pf4_dem_flatten_repeat"][..],
        ),
        ("pf4-dem-rounded", &["stab_pf4_dem_rounded"][..]),
        (
            "pf4-dem-coordinate-map",
            &[
                "stab_pf4_dem_coordinate_map_all_bounded",
                "stab_pf4_dem_coordinate_map_selected_huge_repeat",
                "stab_pf4_dem_coordinate_map_sparse_overlap",
                "stab_pf4_dem_coordinate_map_nested_sparse_overlap",
                "stab_pf4_dem_coordinate_map_flat_overlap_all",
            ][..],
        ),
        (
            "pf4-dem-folded-traversal",
            &[
                "stab_pf4_dem_hyper_capped_repeat",
                "stab_pf4_dem_hyper_zero_probability_repeat_skip",
                "stab_pf4_dem_hyper_flat_repeat_fold",
                "stab_pf4_dem_sat_capped_repeat",
                "stab_pf4_dem_weighted_sat_zero_probability_repeat_skip",
                "stab_pf4_dem_analyzer_capped_repeat",
                "stab_pf4_error_matcher_capped_repeat",
            ][..],
        ),
        (
            "pf4-dem-folded-graphlike-traversal",
            &[
                "stab_pf4_dem_graphlike_capped_repeat",
                "stab_pf4_dem_graphlike_zero_probability_repeat_skip",
                "stab_pf4_dem_graphlike_flat_repeat_fold",
                "stab_pf4_dem_graphlike_logical_only_flat_repeat_fold",
                "stab_pf4_dem_graphlike_no_target_repeat_skip",
            ][..],
        ),
        (
            "pf4-dem-hypergraph-logical-repeat",
            &["stab_pf4_dem_hyper_logical_only_flat_repeat_fold"][..],
        ),
        (
            "pf4-dem-hypergraph-no-target-repeat",
            &["stab_pf4_dem_hyper_no_target_repeat_skip"][..],
        ),
        (
            "pf4-dem-search-zero-shift-repeat",
            &[
                "stab_pf4_dem_graphlike_zero_shift_repeat_fold",
                "stab_pf4_dem_hyper_zero_shift_repeat_fold",
            ][..],
        ),
        (
            "pf4-dem-search-annotation-repeat",
            &[
                "stab_pf4_dem_graphlike_annotation_repeat_fold",
                "stab_pf4_dem_hyper_annotation_repeat_fold",
            ][..],
        ),
        (
            "pf4-dem-search-mixed-zero-probability-repeat",
            &[
                "stab_pf4_dem_graphlike_mixed_zero_probability_repeat_fold",
                "stab_pf4_dem_hyper_mixed_zero_probability_repeat_fold",
            ][..],
        ),
        (
            "pf4-dem-search-nested-repeat",
            &[
                "stab_pf4_dem_graphlike_nested_repeat_fold",
                "stab_pf4_dem_hyper_nested_repeat_fold",
            ][..],
        ),
        (
            "pf4-dem-sat-flat-repeat-fold",
            &[
                "stab_pf4_dem_sat_flat_repeat_fold",
                "stab_pf4_dem_sat_zero_probability_flat_repeat_fold",
                "stab_pf4_dem_sat_nested_repeat_fold",
                "stab_pf4_dem_weighted_sat_flat_repeat_fold",
                "stab_pf4_dem_weighted_sat_nested_repeat_fold",
            ][..],
        ),
        (
            "pf4-error-matcher-filter-flat-repeat",
            &["stab_pf4_error_matcher_filter_flat_repeat_fold"][..],
        ),
        (
            "pf4-error-matcher-filter-nested-repeat",
            &["stab_pf4_error_matcher_filter_nested_repeat_fold"][..],
        ),
        (
            "pf4-error-matcher-filter-logical-repeat",
            &["stab_pf4_error_matcher_filter_logical_repeat_fold"][..],
        ),
        (
            "pf4-error-matcher-filter-annotation-repeat",
            &["stab_pf4_error_matcher_filter_annotation_repeat_fold"][..],
        ),
        (
            "pf4-dem-sampler-folded-repeat",
            &[
                "stab_pf4_dem_sampler_compile_folded_repeat",
                "stab_pf4_dem_sampler_sample_folded_repeat",
                "stab_pf4_dem_sampler_sample_zero_probability_folded_repeat",
                "stab_pf4_dem_sampler_sample_deterministic_parity_repeat",
                "stab_pf4_dem_sampler_sample_single_stochastic_parity_repeat",
                "stab_pf4_dem_sampler_sample_flat_stochastic_parity_repeat",
                "stab_pf4_dem_sampler_sample_nested_stochastic_parity_repeat",
            ][..],
        ),
        (
            "pfm-b3-dem-traversal-core",
            &[
                "stab_pfm_b3_dem_traversal_flat_equivalent",
                "stab_pfm_b3_dem_traversal_nested_large_repeat",
                "stab_pfm_b3_dem_traversal_sparse_selected_coordinate",
                "stab_pfm_b3_dem_traversal_wide_coordinate_irrelevant",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf4,
            threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim/dem/detector_error_model.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: "dem-transform".to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        };

        assert_benchmark_measurements(id, row, expected_measurements);
    }
}

#[test]
fn pf6_analyzer_benchmark_rows_have_stab_compare_runners() {
    for (id, upstream_source, measurement, expected_measurements) in [
        (
            "pfm-b5-analyzer-cycle-folding",
            "src/stim/simulators/error_analyzer.test.cc",
            "analyzer-cycle-folding",
            &[
                "stab_pfm_b5_analyzer_transient",
                "stab_pfm_b5_analyzer_short_period",
                "stab_pfm_b5_analyzer_long_period",
                "stab_pfm_b5_analyzer_nested",
                "stab_pfm_b5_analyzer_gauge",
                "stab_pfm_b5_analyzer_coordinate",
            ][..],
        ),
        (
            "pfm-b5-analyzer-generated-qec",
            "src/stim/simulators/error_analyzer.perf.cc",
            "analyzer-generated-qec",
            &[
                "stab_pfm_b5_analyzer_repetition_qec",
                "stab_pfm_b5_analyzer_surface_qec",
            ][..],
        ),
        (
            "pfm-b5-graphlike-search-direct-dem",
            "src/stim/search/graphlike/algo.test.cc",
            "graphlike-direct-dem",
            &["stab_pfm_b5_graphlike_direct_dem"][..],
        ),
        (
            "pfm-b5-graphlike-generated-d25",
            "src/stim/search/graphlike/algo.perf.cc",
            "graphlike-generated-d25",
            &["stab_pfm_b5_graphlike_generated_d25"][..],
        ),
        (
            "pfm-b5-graphlike-generated-d11-r1000",
            "src/stim/search/graphlike/algo.perf.cc",
            "graphlike-generated-d11-r1000",
            &["stab_pfm_b5_graphlike_generated_d11_r1000"][..],
        ),
        (
            "pfm-b5-hypergraph-search-direct-dem",
            "src/stim/search/hyper/algo.test.cc",
            "hypergraph-direct-dem",
            &["stab_pfm_b5_hypergraph_direct_dem"][..],
        ),
        (
            "pfm-b5-hypergraph-search-generated-qec",
            "src/stim/search/hyper/algo.test.cc",
            "hypergraph-generated-qec",
            &["stab_pfm_b5_hypergraph_generated_qec"][..],
        ),
        (
            "pfm-b5-wcnf-direct-dem",
            "src/stim/search/sat/wcnf.test.cc",
            "wcnf-direct-dem",
            &[
                "stab_pfm_b5_wcnf_shortest_direct",
                "stab_pfm_b5_wcnf_likeliest_direct",
            ][..],
        ),
        (
            "pfm-b5-wcnf-generated-qec",
            "src/stim/search/sat/wcnf.test.cc",
            "wcnf-generated-qec",
            &[
                "stab_pfm_b5_wcnf_shortest_generated",
                "stab_pfm_b5_wcnf_likeliest_generated",
            ][..],
        ),
        (
            "pf6-error-decomp-loop-folded",
            "src/stim/simulators/error_analyzer.test.cc",
            "error-decomp-loop-folded",
            &["stab_pf6_error_decomp_loop_folded"][..],
        ),
        (
            "pf6-sparse-rev-frame-loop",
            "src/stim/simulators/sparse_rev_frame_tracker.test.cc",
            "sparse-rev-frame",
            &[
                "stab_pf6_sparse_rev_unitary_repeat_flow",
                "stab_pf6_sparse_rev_unitary_repeat_high_idle_flow",
                "stab_pf6_sparse_rev_shifted_measurement_flow",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf6,
            threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: upstream_source.to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: measurement.to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        };

        assert_benchmark_measurements(id, row, expected_measurements);
    }
}

fn assert_benchmark_measurements(id: &str, row: BenchmarkRow, expected_measurements: &[&str]) {
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
    if id.starts_with("pfm-b5-") {
        for measurement in &measurements {
            assert!(
                !measurement.observations.is_empty(),
                "{id}/{} should record algorithm observations",
                measurement.name
            );
        }
    }
    if id == "pfm-b5-analyzer-cycle-folding" {
        let short_period = measurements
            .iter()
            .find(|measurement| measurement.name == "stab_pfm_b5_analyzer_short_period")
            .expect("short-period analyzer measurement");
        let long_period = measurements
            .iter()
            .find(|measurement| measurement.name == "stab_pfm_b5_analyzer_long_period")
            .expect("long-period analyzer measurement");
        assert_eq!(observation_value(short_period, "max_recurrence_period"), 8);
        assert_eq!(observation_value(long_period, "max_recurrence_period"), 127);
        assert!(observation_value(short_period, "folded_repeat_iterations") > 0);
        assert!(observation_value(long_period, "folded_repeat_iterations") > 0);
    }
    for measurement in &measurements {
        assert!(
            measurement_rate_work(id, measurement).is_some(),
            "{id}/{} should report normalized work",
            measurement.name
        );
    }
    if id == "pfm-b5-wcnf-generated-qec" {
        for measurement in &measurements {
            assert_eq!(
                measurement_rate_work(id, measurement),
                Some((
                    observation_value(measurement, "clauses") as f64,
                    "clauses/s"
                ))
            );
        }
    }
}

fn observation_value(measurement: &crate::report::Measurement, name: &str) -> u64 {
    measurement
        .observations
        .iter()
        .find(|observation| observation.name == name)
        .map(|observation| observation.value)
        .expect("named benchmark observation")
}

#[test]
fn sparse_reverse_high_idle_work_excludes_the_active_qubit() {
    assert_eq!(
        measurement_work(
            "pf6-sparse-rev-frame-loop",
            "stab_pf6_sparse_rev_unitary_repeat_high_idle_flow"
        ),
        Some((255.0, "idle-qubits/s"))
    );
}
