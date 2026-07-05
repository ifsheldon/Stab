use super::super::{
    compare_note, measurement_work, pf5::expected_flow_solve_measurement_work_for_test,
    run_stab_compare_row,
};
use crate::manifest::{BenchmarkRow, Milestone, Runner};

#[test]
fn detector_utility_benchmark_rows_have_stab_compare_runners() {
    for (id, upstream_source, measurement, expected_measurements) in [
        (
            "pf5-detecting-regions-repeat",
            "src/stim/util_top/circuit_to_detecting_regions.test.cc",
            "detecting-regions-repeat",
            &["stab_pf5_detecting_regions_repeat_ticks"][..],
        ),
        (
            "pf5-detecting-regions-targets",
            "src/stim/circuit/circuit_pybind_test.py",
            "detecting-regions-targets",
            &["stab_pf5_detecting_regions_target_filters"][..],
        ),
        (
            "pf5-detecting-regions-clifford",
            "doc/python_api_reference_vDev.md",
            "detecting-regions-clifford",
            &["stab_pf5_detecting_regions_clifford_gates"][..],
        ),
        (
            "pf5-detecting-regions-generated-repetition",
            "src/stim/circuit/circuit_pybind_test.py",
            "detecting-regions-generated",
            &["stab_pf5_detecting_regions_generated_repetition"][..],
        ),
        (
            "pf5-missing-detectors-mpp",
            "src/stim/util_top/missing_detectors.test.cc",
            "missing-detectors-mpp",
            &[
                "stab_pf5_missing_detectors_mpp_cases",
                "stab_pf5_missing_detectors_mpp_suggestions",
            ][..],
        ),
        (
            "pf5-missing-detectors-generated-code",
            "src/stim/util_top/missing_detectors.test.cc",
            "missing-detectors-generated",
            &[
                "stab_pf5_missing_detectors_generated_cases",
                "stab_pf5_missing_detectors_generated_suggestions",
            ][..],
        ),
        (
            "pf5-has-all-flows-batch",
            "src/stim/util_top/has_flow.test.cc",
            "has-all-flows",
            &[
                "stab_pf5_has_flows_batch_cases",
                "stab_pf5_has_flows_batch_flows",
            ][..],
        ),
        (
            "pf5-flow-generators-measurement-rich",
            "src/stim/util_top/circuit_flow_generators.test.cc",
            "flow-generators-measurement",
            &[
                "stab_pf5_flow_generators_measurement_cases",
                "stab_pf5_flow_generators_measurement_flows",
            ][..],
        ),
        (
            "pf5-flow-solve-measurement-rich",
            "src/stim/util_top/circuit_flow_generators.test.cc",
            "flow-solve",
            &[
                "stab_pf5_flow_solve_measurement_cases",
                "stab_pf5_flow_solve_measurement_queries",
            ][..],
        ),
        (
            "pf5-flow-solve-measurement-python",
            "src/stim/util_top/circuit_flow_generators_test.py",
            "flow-solve-python",
            &[
                "stab_pf5_flow_solve_measurement_python_cases",
                "stab_pf5_flow_solve_measurement_python_queries",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf5,
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

        let measurements = run_stab_compare_row(&row)
            .expect("run compare row")
            .expect("Stab runner");
        let names = measurements
            .iter()
            .map(|measurement| measurement.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(names.as_slice(), expected_measurements);
        assert!(compare_note(id).is_some());
        for name in names {
            assert!(measurement_work(id, name).is_some());
        }
    }
}

#[test]
fn flow_solve_measurement_work_tracks_promoted_corpus() {
    for (row_id, case_name, query_name) in [
        (
            "pf5-flow-solve-measurement-rich",
            "stab_pf5_flow_solve_measurement_cases",
            "stab_pf5_flow_solve_measurement_queries",
        ),
        (
            "pf5-flow-solve-measurement-python",
            "stab_pf5_flow_solve_measurement_python_cases",
            "stab_pf5_flow_solve_measurement_python_queries",
        ),
    ] {
        let (expected_cases, expected_queries) =
            expected_flow_solve_measurement_work_for_test(row_id).expect("flow-solve work");
        assert_eq!(
            measurement_work(row_id, case_name),
            Some((expected_cases, "cases/s"))
        );
        assert_eq!(
            measurement_work(row_id, query_name),
            Some((expected_queries, "queries/s"))
        );
    }
}
