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
            ][..],
        ),
        (
            "pf4-dem-folded-traversal",
            &[
                "stab_pf4_dem_hyper_capped_repeat",
                "stab_pf4_dem_sat_capped_repeat",
                "stab_pf4_dem_analyzer_capped_repeat",
                "stab_pf4_error_matcher_capped_repeat",
            ][..],
        ),
        (
            "pf4-dem-folded-graphlike-traversal",
            &["stab_pf4_dem_graphlike_capped_repeat"][..],
        ),
        (
            "pf4-dem-sampler-folded-repeat",
            &[
                "stab_pf4_dem_sampler_compile_capped_repeat",
                "stab_pf4_dem_sampler_sample_capped_repeat",
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
    let row = BenchmarkRow {
        id: "pf6-analyze-errors-generated-surface".to_string(),
        milestone: Milestone::Pf6,
        threshold_class: "non-primary-report-only".to_string(),
        runner: Runner::ContractOnly,
        upstream_source: "src/stim/simulators/error_analyzer.perf.cc".to_string(),
        stim_perf_filter: String::new(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "analyze-errors-generated".to_string(),
        description: "test row".to_string(),
    };

    assert_benchmark_measurements(
        "pf6-analyze-errors-generated-surface",
        row,
        &["stab_pf6_analyze_errors_generated_surface"],
    );
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
