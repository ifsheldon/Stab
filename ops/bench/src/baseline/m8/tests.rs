use std::path::Path;

use crate::comparability::ComparabilityClass;
use crate::compare::{BaselineCompareStatus, CompareRowBuild, build_compare_row_result};
use crate::manifest::{BenchmarkRow, Milestone, Runner};
use crate::report::Measurement;
use crate::root::RepoRoot;

use super::{
    MeasureReaderMode, OutputWitness, PROBABILITY_UTIL_CASES, PROBABILITY_UTIL_WORDS,
    TABLEAU_SIMULATOR_OUTPUT, compare_note, frozen_pre_a4_cli_witness, measure_reader_record,
    measurement_work, run_sample_compare_row, validate_measure_reader_input_digest,
    validate_measure_reader_preflight, validate_probability_words, validate_ptb64_reader_preflight,
    validate_tableau_simulator_preflight,
};
use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_core::Probability;
use stab_core::RecordFormat;
use stab_engine::biased_randomize_bits;
use stab_records::{write_ptb64_records_checked, write_records};

#[test]
fn probability_preflight_rejects_same_width_wrong_content() {
    for (name, probability, expected_digest) in PROBABILITY_UTIL_CASES {
        let probability = Probability::try_new(probability).expect("probability");
        let mut rng = SmallRng::seed_from_u64(0);
        let mut words = [0_u64; PROBABILITY_UTIL_WORDS];
        biased_randomize_bits(probability, &mut words, &mut rng);
        validate_probability_words("m8-probability-util", name, &words, expected_digest)
            .expect("frozen probability output");

        words[0] ^= 1;
        let error =
            validate_probability_words("m8-probability-util", name, &words, expected_digest)
                .expect_err("same-width probability mutation must fail");
        assert!(error.to_string().contains("wrong content"));
    }
}

#[test]
fn tableau_preflight_rejects_same_width_wrong_content() {
    validate_tableau_simulator_preflight(
        "m8-tableau-simulator",
        TABLEAU_SIMULATOR_OUTPUT.as_slice(),
    )
    .expect("frozen tableau output");

    let mut changed = TABLEAU_SIMULATOR_OUTPUT;
    changed[0] ^= 1;
    let error = validate_tableau_simulator_preflight("m8-tableau-simulator", &changed)
        .expect_err("same-width tableau mutation must fail");
    assert!(error.to_string().contains("wrong content"));
}

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
            &["stab_reference_sample_tree_flat_20x20"][..],
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
        if id.starts_with("m8-sample-primary-") || id == "m8-sample-high-repeat-contract" {
            assert!(
                measurements
                    .iter()
                    .all(|measurement| measurement.iterations == Some(1)),
                "{id} must contribute one process launch to each outer recorded run"
            );
        }
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
fn measure_reader_preflights_reject_same_width_wrong_content() {
    let expected = measure_reader_record(10);
    let mut changed = expected.clone();
    let first = changed.first_mut().expect("reader fixture bit");
    *first = !*first;

    for mode in [MeasureReaderMode::Packed, MeasureReaderMode::Sparse] {
        let input = write_records(std::slice::from_ref(&changed), RecordFormat::ZeroOne)
            .expect("encode mutated reader fixture");
        let digest_error = validate_measure_reader_input_digest(
            "m8-measure-reader-01",
            &input,
            RecordFormat::ZeroOne,
            10,
        )
        .expect_err("same-width encoded mutation must change the frozen digest");
        assert!(digest_error.to_string().contains("wrong content"));
        let error = validate_measure_reader_preflight(
            "m8-measure-reader-01",
            &input,
            RecordFormat::ZeroOne,
            mode,
            &expected,
        )
        .expect_err("same-width reader mutation must fail");
        assert!(error.to_string().contains("wrong content"));
    }
}

#[test]
fn measure_reader_preflight_rejects_missing_and_extra_records() {
    let expected = measure_reader_record(10);
    let empty_error = validate_measure_reader_preflight(
        "m8-measure-reader-01",
        b"",
        RecordFormat::ZeroOne,
        MeasureReaderMode::Packed,
        &expected,
    )
    .expect_err("missing record must fail");
    assert!(empty_error.to_string().contains("wrong content"));

    let input = write_records(&[expected.clone(), expected.clone()], RecordFormat::ZeroOne)
        .expect("encode duplicate reader fixture");
    let extra_error = validate_measure_reader_preflight(
        "m8-measure-reader-01",
        &input,
        RecordFormat::ZeroOne,
        MeasureReaderMode::Sparse,
        &expected,
    )
    .expect_err("extra record must fail");
    assert!(extra_error.to_string().contains("wrong content"));
}

#[test]
fn ptb64_reader_preflight_rejects_same_width_wrong_content() {
    let expected = measure_reader_record(10);
    let mut changed = expected.clone();
    let first = changed.first_mut().expect("reader fixture bit");
    *first = !*first;
    let records = (0..64).map(|_| changed.clone()).collect::<Vec<_>>();
    let input = write_ptb64_records_checked(&records).expect("PTB64 fixture");

    let error =
        validate_ptb64_reader_preflight("m8-measure-reader-ptb64-contract", &input, &expected)
            .expect_err("same-width PTB64 mutation must fail");
    assert!(error.to_string().contains("wrong content"));
}

#[test]
fn reference_sample_tree_diagnostic_cannot_form_a_stim_ratio() {
    let row = BenchmarkRow {
        id: "m8-reference-sample-tree".to_string(),
        milestone: Milestone::M8,
        threshold_class: crate::manifest::ThresholdClass::NonPrimaryReportOnly,
        runner: Runner::StimPerf,
        upstream_source: "src/stim/util_top/reference_sample_tree.perf.cc".to_string(),
        stim_perf_filter: "reference_sample_tree_*".to_string(),
        argv: String::new(),
        stdin_path: String::new(),
        phase: "analysis".to_string(),
        measurement: "reference-sample".to_string(),
        description: "reference sample tree workloads".to_string(),
        comparability: ComparabilityClass::ReportOnly,
    };
    let result = build_compare_row_result(CompareRowBuild {
        row: &row,
        status: "measured",
        baseline_summary: "stim",
        stab_summary: "stab",
        note: compare_note(&row.id).map(str::to_owned),
        stim_measurements: vec![
            measurement("reference_sample_tree_surface_code_d31_r1000000000", 1.0),
            measurement("reference_sample_tree_nested_circuit", 1.0),
        ],
        stab_measurements: vec![measurement("stab_reference_sample_tree_flat_20x20", 1.0)],
        baseline_status: BaselineCompareStatus::Comparable,
    });

    assert_eq!(result.threshold_class, "non-primary-report-only");
    assert_eq!(result.comparability, ComparabilityClass::ReportOnly);
    assert!(result.relative_ratio.is_none());
    assert!(result.measurement_ratios.is_empty());
    assert_eq!(result.pass_fail_status, "not-comparable");
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

fn measurement(name: &str, seconds: f64) -> Measurement {
    Measurement {
        name: name.to_string(),
        seconds,
        variance_seconds: Some(0.0),
        allocation: None,
        resident_bytes: None,
        resident_delta_bytes: None,
        observations: Vec::new(),
        iterations: Some(1),
    }
}
