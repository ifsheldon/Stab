use super::{
    BaselineSummary, compare_incomplete_details, compare_note, measurement_work,
    parse_stim_perf_line, run_stab_compare_row, summarize_baseline_row, validate_baseline_metadata,
};
use crate::manifest::{BenchmarkRow, Milestone, Runner};
use crate::report::{BaselineReport, Measurement};

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
        summarize_baseline_row(&report, "measured-row"),
        BaselineSummary::Present("circuit_parse=0.000001300s".to_string())
    );
    assert_eq!(
        summarize_baseline_row(&report, "contract-row"),
        BaselineSummary::Present("contract-only".to_string())
    );
    assert_eq!(
        summarize_baseline_row(&report, "missing-row"),
        BaselineSummary::Missing
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
fn incomplete_details_names_empty_contract_only_rows() {
    let details = compare_incomplete_details(
        &["future-runner".to_string()],
        &["missing-baseline".to_string()],
        &["contract-placeholder".to_string()],
    );

    assert!(details.contains("pending Stab comparison runner(s): future-runner"));
    assert!(details.contains("missing baseline row(s): missing-baseline"));
    assert!(
        details.contains("contract-only row(s) without Stab measurement(s): contract-placeholder")
    );
}

#[test]
fn m6_benchmark_rows_have_stab_compare_runners() {
    for (id, expected_measurements) in [
        (
            "m6-clifford-string",
            &["stab_clifford_string_multiply_4096"][..],
        ),
        (
            "m6-pauli-string",
            &[
                "stab_pauli_string_multiply_10k",
                "stab_pauli_string_commutes_10k",
            ][..],
        ),
        ("m6-pauli-iter", &["stab_pauli_iter_16q_weight_1_to_3"][..]),
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
            threshold_class: "report-only".to_string(),
            runner: Runner::StimPerf,
            upstream_source: "src/stim/stabilizers/test.perf.cc".to_string(),
            stim_perf_filter: "test".to_string(),
            argv: String::new(),
            stdin_path: String::new(),
            phase: "throughput".to_string(),
            measurement: "stabilizers".to_string(),
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
