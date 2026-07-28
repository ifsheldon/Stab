use std::path::Path;

use crate::manifest::{BenchmarkRow, Milestone, Runner};
use crate::root::RepoRoot;

use super::{
    OutputWitness, compare_note, frozen_pre_a4_cli_witness, measurement_work,
    run_sample_compare_row,
};

#[test]
fn m8_benchmark_rows_have_stab_compare_runners() {
    let root = RepoRoot::resolve(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("repository root"),
    )
    .expect("resolve repository root");
    for (id, expected_measurements) in [
        (
            "m8-measure-reader-01",
            &["stab_read_01_dense_per10", "stab_read_01_sparse_per10"][..],
        ),
        (
            "m8-measure-reader-b8",
            &["stab_read_b8_dense_per10", "stab_read_b8_sparse_per10"][..],
        ),
        (
            "m8-measure-reader-r8",
            &[
                "stab_read_r8_dense_per10",
                "stab_read_r8_dense_per100",
                "stab_read_r8_sparse_per10",
                "stab_read_r8_sparse_per100",
            ][..],
        ),
        (
            "m8-measure-reader-hits",
            &[
                "stab_read_hits_dense_per10",
                "stab_read_hits_dense_per100",
                "stab_read_hits_sparse_per10",
                "stab_read_hits_sparse_per100",
            ][..],
        ),
        (
            "m8-measure-reader-dets",
            &[
                "stab_read_dets_dense_per10",
                "stab_read_dets_dense_per100",
                "stab_read_dets_sparse_per10",
                "stab_read_dets_sparse_per100",
            ][..],
        ),
        (
            "m8-measure-reader-ptb64-contract",
            &["stab_measure_reader_ptb64_64x10k_contract"][..],
        ),
        (
            "m8-frame-simulator",
            &[
                "stab_frame_compile_depolarize1",
                "stab_frame_sample_depolarize1_b8",
            ][..],
        ),
        (
            "m8-tableau-simulator",
            &["stab_tableau_sample_cx_1shot"][..],
        ),
        (
            "m8-reference-sample-tree",
            &["stab_reference_sample_tree_nested_circuit"][..],
        ),
        (
            "m8-sample-analysis-1shot",
            &[
                "stab_sample_compile_plan_auto_noisy_1q",
                "stab_sample_compile_plan_scalar_noisy_1q",
                "stab_sample_construct_session_noisy_1q",
                "stab_sample_execute_witness_sink_64_continuous_session",
                "stab_sample_consume_typed_batch_64",
                "stab_sample_encode_b8_64",
                "stab_sample_repeated_session_16x4_continuous_session",
            ][..],
        ),
        (
            "m8-sample-throughput-1024",
            &["stab_sample_1024_zero_one"][..],
        ),
        (
            "m8-sample-throughput-1000000",
            &["stab_sample_1000000_zero_one"][..],
        ),
        (
            "m8-probability-util",
            &[
                "stab_biased_random_1024_0point1percent",
                "stab_biased_random_1024_0point01percent",
                "stab_biased_random_1024_1percent",
                "stab_biased_random_1024_40percent",
                "stab_biased_random_1024_50percent",
                "stab_biased_random_1024_90percent",
                "stab_biased_random_1024_99percent",
            ][..],
        ),
        (
            "m8-sample-primary-repetition-contract",
            &["stab_sample_primary_repetition_d3_r3"][..],
        ),
        (
            "m8-sample-primary-rotated-surface-contract",
            &["stab_sample_primary_rotated_surface_d3_r3"][..],
        ),
        (
            "m8-sample-primary-unrotated-surface-contract",
            &["stab_sample_primary_unrotated_surface_d3_r3"][..],
        ),
        (
            "m8-sample-high-repeat-contract",
            &["stab_sample_high_repeat_contract"][..],
        ),
    ] {
        let row = BenchmarkRow {
            id: id.to_string(),
            milestone: Milestone::M8,
            threshold_class: crate::manifest::ThresholdClass::ReportOnly,
            runner: Runner::StimCli,
            upstream_source: "src/stim/cmd/command_sample.test.cc".to_string(),
            stim_perf_filter: String::new(),
            argv: "sample|--shots|1".to_string(),
            stdin_path: "oracle/fixtures/inputs/sample_noisy.stim".to_string(),
            phase: "throughput".to_string(),
            measurement: "sample".to_string(),
            description: "test row".to_string(),
            comparability: crate::comparability::ComparabilityClass::Unspecified,
        };

        let measurements = run_sample_compare_row(&root, "release", &row)
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
fn gated_sample_cli_rows_have_frozen_process_preflight_witnesses() {
    for (row_id, bytes, digest) in [
        (
            "m8-sample-primary-repetition-contract",
            128,
            0xc6a0_1d09_04c3_59a5,
        ),
        (
            "m8-sample-primary-rotated-surface-contract",
            320,
            0x0c81_72cc_5f87_aa84,
        ),
        (
            "m8-sample-primary-unrotated-surface-contract",
            448,
            0x5298_992f_11e2_32d7,
        ),
        ("m8-sample-high-repeat-contract", 64, 0x5e27_5dae_3600_d85b),
    ] {
        assert_eq!(
            frozen_pre_a4_cli_witness(row_id),
            Some(OutputWitness { bytes, digest })
        );
        let note = compare_note(row_id).expect("process comparison note");
        assert!(note.contains("bounded subprocesses"));
        assert!(note.contains("discarded stdout"));
        assert!(note.contains("frozen pre-A4"));
    }
    assert_eq!(frozen_pre_a4_cli_witness("m8-sample-analysis-1shot"), None);
}
