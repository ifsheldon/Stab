use super::{
    compare_note, measurement_work, parse_stim_perf_line, run_stab_compare_row,
    selected_baseline_rows, validate_baseline_metadata,
};
use crate::compare::{
    BaselineCompareStatus, BaselineSummary, CompareRowBuild, build_compare_row_result,
    compare_incomplete_details, summarize_baseline_row,
};
use crate::manifest::{BenchmarkManifest, BenchmarkRow, Milestone, Runner};
use crate::report::{BaselineReport, Measurement};

mod pf3;
mod pf5;
mod pfm_b1;
mod runner_smoke;

#[test]
fn parses_stim_perf_measurement_line() {
    let measurement = parse_stim_perf_line(
        "[..................*<|....................] 1.3 us (vs 950 ns) circuit_parse",
    )
    .expect("parse line");

    assert_eq!(
        measurement,
        Measurement {
            name: "circuit_parse".to_string(),
            seconds: 0.0000013,
            variance_seconds: None,
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations: None,
        }
    );
}

#[test]
fn summarizes_present_contract_and_missing_baseline_rows() {
    let report = serde_json::from_str::<BaselineReport>(
        r#"{
            "schema_version": 1,
            "generated_unix_epoch_seconds": 0,
            "machine": {
                "os": "linux",
                "arch": "x86_64",
                "family": "unix",
                "available_parallelism": 1,
                "rustc_version": "rustc test",
                "cmake_version": "cmake test"
            },
            "stim": {
                "source_path": "vendor/stim",
                "expected_tag": "v1.16.0",
                "expected_commit": "expected",
                "actual_tag": "v1.16.0",
                "actual_commit": "actual"
            },
            "command": {
                "target_seconds": 0.001,
                "cli_iterations": 1,
                "filters": []
            },
            "rows": [
                {
                    "id": "measured-row",
                    "milestone": "M4",
                    "threshold_class": "report-only",
                    "runner": "stim-perf",
                    "upstream_source": "src/stim/circuit/circuit.perf.cc",
                    "phase": "analysis",
                    "measurement": "parser-throughput",
                    "status": "measured",
                    "command": {
                        "program": "stim_perf",
                        "args": [],
                        "stdin_path": ""
                    },
                    "measurements": [
                        {
                            "name": "circuit_parse",
                            "seconds": 0.0000013,
                            "iterations": null
                        }
                    ]
                },
                {
                    "id": "contract-row",
                    "milestone": "M4",
                    "threshold_class": "report-only",
                    "runner": "contract-only",
                    "upstream_source": "src/stim/circuit/circuit.test.cc",
                    "phase": "analysis",
                    "measurement": "canonical-print",
                    "status": "contract-only",
                    "command": {
                        "program": "",
                        "args": [],
                        "stdin_path": ""
                    },
                    "measurements": []
                }
            ]
        }"#,
    )
    .expect("baseline report");

    assert_eq!(
        summarize_baseline_row(&report, &benchmark_row("measured-row", Runner::StimPerf)),
        BaselineSummary::Present("circuit_parse=0.000001300s".to_string())
    );
    assert_eq!(
        summarize_baseline_row(
            &report,
            &benchmark_row("contract-row", Runner::ContractOnly)
        ),
        BaselineSummary::Present("contract-only".to_string())
    );
    assert_eq!(
        summarize_baseline_row(&report, &benchmark_row("missing-row", Runner::StimPerf)),
        BaselineSummary::Missing
    );
}

#[test]
fn rejects_placeholder_baseline_for_runnable_row() {
    let report = serde_json::from_str::<BaselineReport>(
        r#"{
            "schema_version": 1,
            "generated_unix_epoch_seconds": 0,
            "machine": {
                "os": "linux",
                "arch": "x86_64",
                "family": "unix",
                "available_parallelism": 1,
                "rustc_version": "rustc test",
                "cmake_version": "cmake test"
            },
            "stim": {
                "source_path": "vendor/stim",
                "expected_tag": "v1.16.0",
                "expected_commit": "expected",
                "actual_tag": "v1.16.0",
                "actual_commit": "actual"
            },
            "command": {
                "target_seconds": 0.001,
                "cli_iterations": 1,
                "filters": []
            },
            "rows": [
                {
                    "id": "measured-row",
                    "milestone": "M4",
                    "threshold_class": "report-only",
                    "runner": "stim-perf",
                    "upstream_source": "src/stim/circuit/circuit.perf.cc",
                    "phase": "analysis",
                    "measurement": "parser-throughput",
                    "status": "contract-only",
                    "command": {
                        "program": "",
                        "args": [],
                        "stdin_path": ""
                    },
                    "measurements": []
                }
            ]
        }"#,
    )
    .expect("baseline report");

    let summary = summarize_baseline_row(&report, &benchmark_row("measured-row", Runner::StimPerf));

    assert_eq!(
        summary,
        BaselineSummary::Invalid("status=contract-only expected measured".to_string())
    );
}

#[test]
fn rejects_baseline_metadata_from_wrong_stim_revision() {
    let report = serde_json::from_str::<BaselineReport>(
        r#"{
            "schema_version": 1,
            "generated_unix_epoch_seconds": 0,
            "machine": {
                "os": "linux",
                "arch": "x86_64",
                "family": "unix",
                "available_parallelism": 1,
                "rustc_version": "rustc test",
                "cmake_version": "cmake test"
            },
            "stim": {
                "source_path": "vendor/stim",
                "expected_tag": "v1.16.0",
                "expected_commit": "e2fc1eca7fd21684d433aa5f10f4504ea4860d07",
                "actual_tag": "v1.17.0",
                "actual_commit": "wrong"
            },
            "command": {
                "target_seconds": 0.001,
                "cli_iterations": 1,
                "filters": []
            },
            "rows": []
        }"#,
    )
    .expect("baseline report");

    let error = validate_baseline_metadata(&report).expect_err("reject wrong metadata");

    assert!(error.to_string().contains("actual_tag=v1.17.0"));
    assert!(error.to_string().contains("actual_commit=wrong"));
}

#[test]
fn primary_baseline_selection_excludes_metadata_and_m12_placeholder_rows() {
    let m4_row = benchmark_row("m4-circuit-parse", Runner::StimPerf);
    let mut metadata_row = benchmark_row("m7-perf-harness", Runner::ContractOnly);
    metadata_row.milestone = Milestone::M7;
    metadata_row.threshold_class = crate::manifest::ThresholdClass::BaselineMetadata;
    let mut non_primary_row =
        benchmark_row("m9-detecting-regions-basic-batch", Runner::ContractOnly);
    non_primary_row.milestone = Milestone::M9;
    non_primary_row.threshold_class = crate::manifest::ThresholdClass::NonPrimaryReportOnly;
    let mut m12_row = benchmark_row("m12-primary-performance-matrix", Runner::ContractOnly);
    m12_row.milestone = Milestone::M12;
    let mut pf_row = benchmark_row("pf1-circuit-coordinate-query", Runner::ContractOnly);
    pf_row.milestone = Milestone::Pf1;
    pf_row.threshold_class = crate::manifest::ThresholdClass::NonPrimaryReportOnly;
    let manifest = BenchmarkManifest {
        rows: vec![m4_row, metadata_row, non_primary_row, m12_row, pf_row],
    };

    let rows = selected_baseline_rows(&manifest, &[], true).expect("primary rows");
    let ids = rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>();

    assert_eq!(ids, ["m4-circuit-parse"]);
}

#[test]
fn m4_gate_lookup_benchmark_splits_lookup_surfaces() {
    let row = BenchmarkRow {
        id: "m4-gate-lookup".to_string(),
        milestone: Milestone::M4,
        threshold_class: crate::manifest::ThresholdClass::ReportOnly,
        runner: Runner::StimPerf,
        upstream_source: "src/stim/gates/gates.perf.cc".to_string(),
        stim_perf_filter: "gate_data_hash_all_gate_names".to_string(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "gate-lookup".to_string(),
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

    assert_eq!(
        names,
        [
            "stab_gate_data_hash_all_gate_names",
            "stab_gate_lookup_aliases_contract",
            "stab_gate_lookup_lowercase_contract",
            "stab_gate_lookup_invalid_contract",
        ]
    );
    assert!(compare_note("m4-gate-lookup").is_some());
    for name in names {
        assert!(
            measurement_work("m4-gate-lookup", name).is_some(),
            "m4-gate-lookup/{name} should report normalized work"
        );
    }
}

#[test]
fn pf1_circuit_coordinate_benchmark_reports_public_query_surfaces() {
    let row = BenchmarkRow {
        id: "pf1-circuit-coordinate-query".to_string(),
        milestone: Milestone::Pf1,
        threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
        runner: Runner::ContractOnly,
        upstream_source: "src/stim/circuit/circuit.perf.cc".to_string(),
        stim_perf_filter: String::new(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "circuit-coordinate-query".to_string(),
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

    assert_eq!(
        names,
        [
            "stab_circuit_counts_nested_repeat",
            "stab_circuit_final_coordinate_shift_nested_repeat",
            "stab_circuit_final_qubit_coordinates_nested_repeat",
            "stab_circuit_detector_coordinates_nested_repeat",
            "stab_circuit_detector_coordinates_late_nested_repeat",
        ]
    );
    assert!(compare_note("pf1-circuit-coordinate-query").is_some());
    for name in names {
        assert!(
            measurement_work("pf1-circuit-coordinate-query", name).is_some(),
            "pf1-circuit-coordinate-query/{name} should report normalized work"
        );
    }
}

#[test]
fn pf1_dem_counts_benchmark_reports_public_query_surfaces() {
    let row = BenchmarkRow {
        id: "pf1-dem-counts-repeat".to_string(),
        milestone: Milestone::Pf1,
        threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
        runner: Runner::ContractOnly,
        upstream_source: "src/stim/dem/detector_error_model.test.cc".to_string(),
        stim_perf_filter: String::new(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "dem-counts-repeat".to_string(),
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

    assert_eq!(
        names,
        [
            "stab_dem_counts_nested_repeat",
            "stab_dem_final_coordinate_shift_nested_repeat",
            "stab_dem_detector_coordinates_nested_repeat",
        ]
    );
    assert!(compare_note("pf1-dem-counts-repeat").is_some());
    for name in names {
        assert!(
            measurement_work("pf1-dem-counts-repeat", name).is_some(),
            "pf1-dem-counts-repeat/{name} should report normalized work"
        );
    }
}

#[test]
fn pf1_dem_without_tags_benchmark_reports_public_query_surface() {
    let row = BenchmarkRow {
        id: "pf1-dem-without-tags".to_string(),
        milestone: Milestone::Pf1,
        threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
        runner: Runner::ContractOnly,
        upstream_source: "src/stim/dem/detector_error_model_pybind_test.py".to_string(),
        stim_perf_filter: String::new(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "dem-without-tags".to_string(),
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

    assert_eq!(names, ["stab_dem_without_tags_nested_repeat"]);
    assert!(compare_note("pf1-dem-without-tags").is_some());
    for name in names {
        assert!(
            measurement_work("pf1-dem-without-tags", name).is_some(),
            "pf1-dem-without-tags/{name} should report normalized work"
        );
    }
}

#[test]
fn pf1_gate_metadata_benchmark_reports_public_metadata_surfaces() {
    let row = BenchmarkRow {
        id: "pf1-gate-metadata-lookup".to_string(),
        milestone: Milestone::Pf1,
        threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
        runner: Runner::ContractOnly,
        upstream_source: "src/stim/gates/gates.perf.cc".to_string(),
        stim_perf_filter: String::new(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "gate-metadata".to_string(),
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

    assert_eq!(
        names,
        [
            "stab_gate_metadata_flags_all_gates",
            "stab_gate_metadata_inverse_all_gates",
            "stab_gate_metadata_tableau_supported_gates",
            "stab_gate_metadata_flows_supported_gates",
            "stab_gate_metadata_unitary_supported_gates",
            "stab_gate_metadata_decomposition_text_supported_gates",
            "stab_gate_metadata_decomposition_parse_supported_gates",
            "stab_gate_metadata_alias_lookup_all_aliases",
        ]
    );
    assert!(compare_note("pf1-gate-metadata-lookup").is_some());
    for name in names {
        assert!(
            measurement_work("pf1-gate-metadata-lookup", name).is_some(),
            "pf1-gate-metadata-lookup/{name} should report normalized work"
        );
    }
}

#[test]
fn primary_baseline_selection_rejects_empty_filtered_primary_rows() {
    let mut m12_row = benchmark_row("m12-primary-performance-matrix", Runner::ContractOnly);
    m12_row.milestone = Milestone::M12;
    let manifest = BenchmarkManifest {
        rows: vec![m12_row],
    };

    let error = selected_baseline_rows(&manifest, &["M12".to_string()], true)
        .expect_err("reject empty primary selection");

    assert!(error.to_string().contains("primary"));
}

#[test]
fn incomplete_details_names_empty_contract_only_rows() {
    let details = compare_incomplete_details(
        &["future-runner".to_string()],
        &["missing-baseline".to_string()],
        &["placeholder-baseline".to_string()],
        &["contract-placeholder".to_string()],
    );

    assert!(details.contains("pending Stab comparison runner(s): future-runner"));
    assert!(details.contains("missing baseline row(s): missing-baseline"));
    assert!(details.contains("invalid baseline row(s): placeholder-baseline"));
    assert!(
        details.contains("contract-only row(s) without Stab measurement(s): contract-placeholder")
    );
}

#[test]
fn compare_row_result_records_ratio_and_beta_gate_status() {
    let row = benchmark_row("measured-row", Runner::StimPerf);
    let result = build_compare_row_result(CompareRowBuild {
        row: &row,
        status: "measured",
        baseline_summary: "stim=0.001s",
        stab_summary: "stab=0.0015s",
        note: Some("same workload".to_string()),
        stim_measurements: vec![Measurement {
            name: "stim".to_string(),
            seconds: 0.001,
            variance_seconds: None,
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations: None,
        }],
        stab_measurements: vec![Measurement {
            name: "stab".to_string(),
            seconds: 0.0015,
            variance_seconds: Some(0.0),
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations: Some(1),
        }],
        baseline_status: BaselineCompareStatus::Comparable,
    });

    assert_eq!(result.stim_median_seconds, Some(0.001));
    assert_eq!(result.stab_median_seconds, Some(0.0015));
    assert_eq!(result.relative_ratio, Some(1.5));
    assert_eq!(result.pass_fail_status, "fail");
    assert_eq!(result.note.as_deref(), Some("same workload"));
}

#[test]
fn compare_row_result_distinguishes_missing_baseline_from_uncomparable_contracts() {
    let row = benchmark_row("contract-row", Runner::ContractOnly);
    let missing = build_compare_row_result(CompareRowBuild {
        row: &row,
        status: "measured",
        baseline_summary: "missing-baseline",
        stab_summary: "stab=0.001s",
        note: None,
        stim_measurements: Vec::new(),
        stab_measurements: vec![Measurement {
            name: "stab".to_string(),
            seconds: 0.001,
            variance_seconds: Some(0.0),
            allocation: None,
            resident_bytes: None,
            resident_delta_bytes: None,
            observations: Vec::new(),
            iterations: Some(1),
        }],
        baseline_status: BaselineCompareStatus::Missing,
    });
    let contract_only = build_compare_row_result(CompareRowBuild {
        row: &row,
        status: "contract-only",
        baseline_summary: "contract-only",
        stab_summary: "no-runner",
        note: None,
        stim_measurements: Vec::new(),
        stab_measurements: Vec::new(),
        baseline_status: BaselineCompareStatus::Comparable,
    });

    assert_eq!(missing.pass_fail_status, "missing-baseline");
    assert_eq!(contract_only.pass_fail_status, "not-comparable");
}

#[test]
fn m6_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurements) in [
        (
            "m6-clifford-string",
            &["stab_clifford_string_multiplication_10K"][..],
        ),
        (
            "m6-pauli-string",
            &[
                "stab_pauli_string_multiplication_1M",
                "stab_pauli_string_multiplication_100K",
                "stab_pauli_string_multiplication_10K",
            ][..],
        ),
        (
            "m6-pauli-iter",
            &[
                "stab_pauli_iter_xz_2_to_5_of_5",
                "stab_pauli_iter_xyz_1_of_1000",
            ][..],
        ),
        (
            "m6-tableau",
            &[
                "stab_tableau_from_circuit_32q",
                "stab_tableau_inverse_32q",
                "stab_tableau_apply_32q",
            ][..],
        ),
        ("m6-tableau-iter", &["stab_tableau_iter_unsigned_2q"][..]),
        (
            "m6-stabilizers-to-tableau",
            &[
                "stab_stabilizers_to_tableau_16q",
                "stab_stabilizers_to_inverse_tableau_16q",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M6,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner: Runner::StimPerf,
            upstream_source: "src/stim/stabilizers/test.perf.cc".to_string(),
            stim_perf_filter: "test".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: "stabilizers".to_string(),
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
        for name in names {
            assert!(
                measurement_work(id, name).is_some(),
                "{id}/{name} should report normalized work"
            );
        }
    }
}

#[test]
fn m9_benchmark_rows_have_stab_compare_runners() {
    for (id, runner, expected_measurements) in [
        (
            "m9-convert-measurements-dets",
            Runner::StimCli,
            &["stab_convert_measurements_to_dets"][..],
        ),
        (
            "m9-detect-text-cli",
            Runner::StimCli,
            &["stab_detect_1024_dets"][..],
        ),
        (
            "m9-detect-bitpacked-cli",
            Runner::StimCli,
            &["stab_detect_1024_b8"][..],
        ),
        (
            "m9-detect-primary-matrix-contract",
            Runner::StimCli,
            &[
                "stab_detect_primary_repetition_d3_r3_dets",
                "stab_detect_primary_repetition_d3_r3_b8",
            ][..],
        ),
        ("m9-m2d-text-cli", Runner::StimCli, &["stab_m2d_dets"][..]),
        (
            "m9-m2d-bitpacked-contract",
            Runner::StimCli,
            &["stab_m2d_b8"][..],
        ),
        (
            "m9-m2d-sweep-01-cli",
            Runner::StimCli,
            &["stab_m2d_sweep_01_dets"][..],
        ),
        (
            "m9-m2d-sweep-b8-cli",
            Runner::StimCli,
            &["stab_m2d_sweep_b8"][..],
        ),
        (
            "m9-m2d-sweep-obs-out-cli",
            Runner::StimCli,
            &["stab_m2d_sweep_obs_out"][..],
        ),
        (
            "m9-m2d-ran-without-feedback-cli",
            Runner::StimCli,
            &["stab_m2d_ran_without_feedback"][..],
        ),
        (
            "m9-detecting-regions-basic-batch",
            Runner::ContractOnly,
            &[
                "stab_detecting_regions_basic_cases",
                "stab_detecting_regions_basic_regions",
            ][..],
        ),
        (
            "m9-missing-detectors-basic-batch",
            Runner::ContractOnly,
            &[
                "stab_missing_detectors_basic_cases",
                "stab_missing_detectors_basic_suggestions",
            ][..],
        ),
        (
            "m9-feedback-inline-mpp-batch",
            Runner::ContractOnly,
            &["stab_feedback_inline_mpp_transforms"][..],
        ),
        (
            "m9-m2d-primary-matrix-contract",
            Runner::StimCli,
            &[
                "stab_m2d_primary_repetition_d3_r3_dets",
                "stab_m2d_primary_repetition_d3_r3_b8",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M9,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner,
            upstream_source: "src/stim/cmd/command_detect.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: "detect|--shots|1024".to_string(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: "detector-conversion".to_string(),
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
        for name in names {
            assert!(
                measurement_work(id, name).is_some(),
                "{id}/{name} should report normalized work"
            );
        }
    }
}

#[test]
fn pf7_cli_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurement, measurement) in [
        (
            "pf7-cli-m2d-sweep-b8",
            "stab_pf7_cli_m2d_sweep_b8",
            "cli-m2d-sweep",
        ),
        (
            "pf7-cli-m2d-feedback-inline",
            "stab_pf7_cli_m2d_feedback_inline",
            "cli-m2d-feedback-inline",
        ),
        (
            "pf7-cli-analyze-errors-decompose",
            "stab_pf7_cli_analyze_errors_decompose",
            "cli-analyze-errors",
        ),
        (
            "pf7-cli-analyze-errors-generated",
            "stab_pf7_cli_analyze_errors_generated",
            "cli-analyze-errors-generated",
        ),
        (
            "pf7-cli-legacy-dispatch-startup",
            "stab_pf7_cli_legacy_gen_d3_r3",
            "cli-legacy-dispatch",
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf7,
            threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim/cmd/command_m2d.test.cc".to_string(),
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

        assert_eq!(names.as_slice(), &[expected_measurement]);
        assert!(
            compare_note(id).is_some(),
            "{id} should explain benchmark comparability"
        );
        assert!(
            measurement_work(id, expected_measurement).is_some(),
            "{id}/{expected_measurement} should report normalized work"
        );
    }
}

#[test]
fn pf2_transform_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurements) in [
        (
            "pf2-circuit-flatten-repeat",
            &["stab_circuit_flatten_repeat_shifted_coords"][..],
        ),
        (
            "pf2-circuit-without-noise",
            &["stab_circuit_without_noise_top_level"][..],
        ),
        (
            "pf2-circuit-decompose-mpp-spp",
            &["stab_circuit_decompose_mpp_spp"][..],
        ),
        (
            "pf2-feedback-inline-batch",
            &[
                "stab_circuit_with_inlined_feedback_mpp",
                "stab_circuit_with_inlined_feedback_repeat_loop",
                "stab_circuit_with_inlined_feedback_xcz_ycz",
            ][..],
        ),
        (
            "pf2-time-reverse-flow",
            &["stab_circuit_time_reversed_for_flows_unitary"][..],
        ),
        (
            "pf2-time-reverse-flow-measurement",
            &["stab_circuit_time_reversed_for_flows_measurement"][..],
        ),
        (
            "pfm-b1-time-reverse-generated-surface",
            &[
                "stab_circuit_time_reversed_for_flows_generated_surface_d3_r2",
                "stab_circuit_time_reversed_for_flows_generated_surface_d5_r2",
                "stab_circuit_time_reversed_for_flows_generated_surface_d7_r2",
            ][..],
        ),
        (
            "pfm-b1-time-reverse-mpad-matrix",
            &[
                "stab_circuit_time_reversed_for_flows_mpad_matrix",
                "stab_circuit_time_reversed_for_flows_mpad_scale_1",
                "stab_circuit_time_reversed_for_flows_mpad_scale_8",
                "stab_circuit_time_reversed_for_flows_mpad_scale_64",
            ][..],
        ),
        (
            "pfm-b1-time-reverse-large-unitary-repeat",
            &[
                "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1",
                "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1024",
                "stab_circuit_time_reversed_for_flows_unitary_repeat_count_1b",
                "stab_circuit_time_reversed_for_flows_unitary_repeat_wide_body_1b",
            ][..],
        ),
        (
            "pfm-b1-time-reverse-sparse-high-qubit",
            &[
                "stab_circuit_time_reversed_for_flows_sparse_qubit_0",
                "stab_circuit_time_reversed_for_flows_sparse_qubit_1000000",
            ][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::Pf2,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner: Runner::ContractOnly,
            upstream_source: "src/stim/circuit/circuit.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: "circuit-transform".to_string(),
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

#[test]
fn convert_benchmark_rows_have_stab_cli_runners() {
    for (id, expected_measurement) in [
        ("m7-convert-01-to-b8", "stab_convert_01_to_b8_128"),
        ("m7-convert-b8-to-01", "stab_convert_b8_to_01_128"),
        ("m7-convert-b8-to-b8-wide", "stab_convert_b8_to_b8_2048"),
        ("m7-convert-dets-to-b8", "stab_convert_dets_to_b8_dl72"),
        ("m7-convert-b8-to-dets", "stab_convert_b8_to_dets_dl72"),
        ("m7-convert-ptb64-to-01", "stab_convert_ptb64_to_01_128"),
        ("m7-convert-01-to-ptb64", "stab_convert_01_to_ptb64_128"),
        (
            "m7-convert-circuit-dl-obs-out",
            "stab_convert_circuit_dl_obs_out",
        ),
        ("m7-convert-dem-dets-to-01", "stab_convert_dem_dets_to_01"),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M7,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner: Runner::StimCli,
            upstream_source: "src/stim/cmd/command_convert.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: "convert-cli".to_string(),
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

        assert_eq!(names, [expected_measurement]);
        assert!(
            compare_note(id).is_some(),
            "{id} should explain benchmark comparability"
        );
        assert!(
            measurement_work(id, expected_measurement).is_some(),
            "{id}/{expected_measurement} should report normalized work"
        );
    }
}

#[test]
fn m10_dem_benchmark_rows_have_stab_compare_runners() {
    for (id, runner, expected_measurements) in [
        (
            "m10-analyze-errors-fold-cli",
            Runner::StimCli,
            &["stab_analyze_errors_fold_repeat"][..],
        ),
        (
            "m10-analyze-errors-high-repeat-contract",
            Runner::StimCli,
            &["stab_analyze_errors_fold_repeat"][..],
        ),
        (
            "m10-graphlike-search",
            Runner::StimPerf,
            &["stab_graphlike_search_chain"][..],
        ),
        (
            "m10-error-analyzer",
            Runner::StimPerf,
            &["stab_error_analyzer_surface_code"][..],
        ),
        (
            "m10-error-decomp",
            Runner::StimPerf,
            &[
                "stab_independent_to_disjoint_xyz_errors",
                "stab_disjoint_to_independent_xyz_errors_approx_exact",
                "stab_disjoint_to_independent_xyz_errors_approx_p10",
                "stab_disjoint_to_independent_xyz_errors_approx_p100",
            ][..],
        ),
        (
            "m10-analyze-errors-decompose-cli",
            Runner::StimCli,
            &["stab_analyze_errors_decompose_basic"][..],
        ),
        (
            "m10-dem-parse-contract",
            Runner::StimCli,
            &["stab_dem_parse_sample"][..],
        ),
        (
            "m10-dem-print-contract",
            Runner::ContractOnly,
            &["stab_dem_print_sample"][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M10,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner,
            upstream_source: "src/stim/dem/detector_error_model.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "analysis".to_string(),
            measurement: "dem-format".to_string(),
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
        for name in names {
            assert!(
                measurement_work(id, name).is_some(),
                "{id}/{name} should report normalized work"
            );
        }
    }
}

fn benchmark_row(id: &str, runner: Runner) -> BenchmarkRow {
    let (upstream_source, measurement) = if runner == Runner::ContractOnly {
        ("src/stim/circuit/circuit.test.cc", "canonical-print")
    } else {
        ("src/stim/circuit/circuit.perf.cc", "parser-throughput")
    };
    BenchmarkRow {
        id: id.to_string(),
        milestone: Milestone::M4,
        threshold_class: crate::manifest::ThresholdClass::ReportOnly,
        runner,
        upstream_source: upstream_source.to_string(),
        stim_perf_filter: "test".to_string(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: measurement.to_string(),
        description: "test row".to_string(),
        comparability: crate::comparability::ComparabilityClass::Unspecified,
    }
}
