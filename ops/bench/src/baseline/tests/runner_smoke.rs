use super::super::{compare_note, measurement_work, run_stab_compare_row};
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
            ][..],
        ),
        (
            "pf4-dem-sat-flat-repeat-fold",
            &[
                "stab_pf4_dem_sat_flat_repeat_fold",
                "stab_pf4_dem_sat_zero_probability_flat_repeat_fold",
                "stab_pf4_dem_weighted_sat_flat_repeat_fold",
            ][..],
        ),
        (
            "pf4-error-matcher-filter-flat-repeat",
            &["stab_pf4_error_matcher_filter_flat_repeat_fold"][..],
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
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf4,
            threshold_class: "non-primary-report-only".to_string(),
            runner: Runner::ContractOnly,
            upstream_source: "src/stim/dem/detector_error_model.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: "dem-transform".to_string(),
            description: "test row".to_string(),
        };

        assert_benchmark_measurements(id, row, expected_measurements);
    }
}

#[test]
fn pf6_analyzer_benchmark_rows_have_stab_compare_runners() {
    for (id, upstream_source, measurement, expected_measurements) in [
        (
            "pf6-analyze-errors-generated-surface",
            "src/stim/simulators/error_analyzer.perf.cc",
            "analyze-errors-generated",
            &["stab_pf6_analyze_errors_generated_surface"][..],
        ),
        (
            "pf6-error-decomp-loop-folded",
            "src/stim/simulators/error_analyzer.test.cc",
            "error-decomp-loop-folded",
            &["stab_pf6_error_decomp_loop_folded"][..],
        ),
        (
            "pf6-graphlike-search-generated",
            "src/stim/search/graphlike/algo.perf.cc",
            "graphlike-search-generated",
            &["stab_pf6_graphlike_search_generated_surface"][..],
        ),
        (
            "pf6-hypergraph-search-generated",
            "src/stim/search/hyper/algo.test.cc",
            "hypergraph-search-generated",
            &["stab_pf6_hypergraph_search_generated_surface"][..],
        ),
        (
            "pf6-generated-sat-wcnf",
            "src/stim/search/sat/wcnf.test.cc",
            "sat-wcnf-generated",
            &[
                "stab_pf6_shortest_sat_generated_surface",
                "stab_pf6_likeliest_sat_generated_surface",
            ][..],
        ),
        (
            "pf6-sparse-rev-frame-loop",
            "src/stim/simulators/sparse_rev_frame_tracker.test.cc",
            "sparse-rev-frame",
            &[
                "stab_pf6_sparse_rev_unitary_repeat_flow",
                "stab_pf6_sparse_rev_shifted_measurement_flow",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf6,
            threshold_class: "non-primary-report-only".to_string(),
            runner: Runner::ContractOnly,
            upstream_source: upstream_source.to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: measurement.to_string(),
            description: "test row".to_string(),
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
    for name in names {
        assert!(
            measurement_work(id, name).is_some(),
            "{id}/{name} should report normalized work"
        );
    }
}
