#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    CompiledDemSampler, DetectionEventRecord, DetectionObservableOutputMode, DetectorErrorModel,
    SampleFormat, write_detection_records,
};

fn compile_dem(text: &str) -> CompiledDemSampler {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM");
    CompiledDemSampler::compile(&model).expect("compile DEM sampler")
}

fn detector_hits(output: &[DetectionEventRecord], detector: usize) -> usize {
    output
        .iter()
        .filter(|record| record.detectors.get(detector).copied().unwrap_or(false))
        .count()
}

fn observable_hits(output: &[DetectionEventRecord], observable: usize) -> usize {
    output
        .iter()
        .filter(|record| record.observables.get(observable).copied().unwrap_or(false))
        .count()
}

#[test]
fn dem_sampler_basic_sizing_matches_upstream_semantics() {
    let empty = compile_dem("");
    let output = empty
        .sample_detection_events_with_seed(3, Some(5))
        .expect("sample empty");
    assert_eq!(output.detector_count, 0);
    assert_eq!(output.observable_count, 0);
    assert_eq!(
        output.records,
        vec![
            DetectionEventRecord {
                detectors: vec![],
                observables: vec![],
            };
            3
        ]
    );

    let sparse = compile_dem(
        "
        logical_observable L2000
        detector D1000
        ",
    );
    let output = sparse
        .sample_detection_events_with_seed(2, Some(5))
        .expect("sample sparse declaration model");
    assert_eq!(output.detector_count, 1001);
    assert_eq!(output.observable_count, 2001);
    assert!(output.records.iter().all(|record| {
        record.detectors.len() == 1001
            && record.observables.len() == 2001
            && !record.detectors.iter().any(|bit| *bit)
            && !record.observables.iter().any(|bit| *bit)
    }));
}

#[test]
fn dem_sampler_samples_deterministic_detector_error_each_shot() {
    let sampler = compile_dem("error(1) D0\n");
    let output = sampler
        .sample_detection_events_with_seed(3, Some(5))
        .expect("sample");

    assert_eq!(output.detector_count, 1);
    assert_eq!(output.observable_count, 0);
    assert_eq!(output.records.len(), 3);
    assert!(
        output
            .records
            .iter()
            .all(|record| record.detectors == [true])
    );
    assert!(
        output
            .records
            .iter()
            .all(|record| record.observables.is_empty())
    );

    let bytes = write_detection_records(
        &output,
        DetectionObservableOutputMode::DetectorsOnly,
        SampleFormat::ZeroOne,
    )
    .expect("write output");
    assert_eq!(bytes, b"1\n1\n1\n");
}

#[test]
fn dem_sampler_samples_observables_only_errors() {
    let sampler = compile_dem("error(1) L0\n");
    let output = sampler
        .sample_detection_events_with_seed(2, Some(5))
        .expect("sample");

    assert_eq!(output.detector_count, 0);
    assert_eq!(output.observable_count, 1);
    assert!(
        output
            .records
            .iter()
            .all(|record| { record.detectors.is_empty() && record.observables == [true] })
    );
}

#[test]
fn dem_sampler_records_sampled_errors_and_replays_them() {
    let sampler = compile_dem(
        "
        error(1) D0 L0
        error(0) D1
        error(1) D1
        ",
    );
    assert_eq!(sampler.error_count(), 3);

    let (sampled_output, error_records) = sampler
        .sample_detection_events_and_errors_with_seed(2, Some(5))
        .expect("sample with errors");
    assert_eq!(
        error_records,
        vec![vec![true, false, true], vec![true, false, true]]
    );
    assert!(
        sampled_output
            .records
            .iter()
            .all(|record| { record.detectors == [true, true] && record.observables == [true] })
    );

    let replayed = sampler
        .sample_detection_events_from_error_records(&[
            vec![true, false, false],
            vec![false, true, false],
            vec![true, true, true],
        ])
        .expect("replay errors");
    assert_eq!(
        replayed.records,
        vec![
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![true],
            },
            DetectionEventRecord {
                detectors: vec![false, true],
                observables: vec![false],
            },
            DetectionEventRecord {
                detectors: vec![true, false],
                observables: vec![true],
            },
        ]
    );

    let error = sampler
        .sample_detection_events_from_error_records(&[vec![true]])
        .expect_err("reject wrong replay width");
    assert!(error.to_string().contains("expected 3 bits"), "{error}");
}

#[test]
fn dem_sampler_writes_dense_bit_packed_detector_and_observable_output() {
    let sampler = compile_dem("error(1) D0 D1 D2 D3 D4 D5 D6 D7 D8 L0\n");
    let output = sampler
        .sample_detection_events_with_seed(2, Some(5))
        .expect("sample");

    assert_eq!(output.detector_count, 9);
    assert_eq!(output.observable_count, 1);
    assert!(output.records.iter().all(|record| {
        record.detectors.len() == 9
            && record.detectors.iter().all(|bit| *bit)
            && record.observables == [true]
    }));

    let bytes = write_detection_records(
        &output,
        DetectionObservableOutputMode::Append,
        SampleFormat::B8,
    )
    .expect("write bit-packed output");
    assert_eq!(bytes, [0xff, 0x03, 0xff, 0x03]);
}

#[test]
fn dem_sampler_basic_probabilities_match_upstream_semantics() {
    let sampler = compile_dem(
        "
        error(0) D0
        error(0.25) D1 L0
        error(0.5) D2
        error(0.75) D3
        error(1) D4 ^ D5
        ",
    );
    let output = sampler
        .sample_detection_events_with_seed(1000, Some(5))
        .expect("sample");
    let records = &output.records;

    assert_eq!(detector_hits(records, 0), 0);
    assert!((150..=350).contains(&detector_hits(records, 1)));
    assert!((350..=650).contains(&detector_hits(records, 2)));
    assert!((650..=850).contains(&detector_hits(records, 3)));
    assert_eq!(detector_hits(records, 4), 1000);
    assert_eq!(detector_hits(records, 5), 1000);
    assert_eq!(detector_hits(records, 1), observable_hits(records, 0));
    assert!(records.iter().all(|record| {
        record.detectors.get(4).copied().unwrap_or(false)
            == record.detectors.get(5).copied().unwrap_or(true)
    }));
}

#[test]
fn dem_sampler_correlated_combinations_match_upstream_semantics() {
    let sampler = compile_dem(
        "
        error(0.1) D0 D1
        error(0.2) D1 D2
        error(0.3) D2 D0
        ",
    );
    let output = sampler
        .sample_detection_events_with_seed(1000, Some(5))
        .expect("sample");
    let records = &output.records;

    assert!((240..=440).contains(&detector_hits(records, 0)));
    assert!((160..=360).contains(&detector_hits(records, 1)));
    assert!((280..=480).contains(&detector_hits(records, 2)));
    assert!(
        records
            .iter()
            .all(|record| { !record.detectors.iter().fold(false, |acc, bit| acc ^ *bit) })
    );
}

#[test]
fn dem_sampler_respects_detector_shifts_repeats_and_observables() {
    let sampler = compile_dem(
        "
        error(1) D0 L1
        shift_detectors 1
        repeat 2 {
            error(1) D0
            shift_detectors 1
        }
        error(0) D0 L0
        ",
    );
    let output = sampler
        .sample_detection_events_with_seed(1, Some(5))
        .expect("sample");

    assert_eq!(output.detector_count, 4);
    assert_eq!(output.observable_count, 2);
    assert_eq!(
        output.records,
        vec![DetectionEventRecord {
            detectors: vec![true, true, true, false],
            observables: vec![false, true],
        }]
    );
}

#[test]
fn dem_sampler_seeded_noisy_error_is_reproducible_and_statistical() {
    let sampler = compile_dem("error(0.25) D0\n");
    let first = sampler
        .sample_detection_events_with_seed(1000, Some(5))
        .expect("sample");
    let second = sampler
        .sample_detection_events_with_seed(1000, Some(5))
        .expect("sample again");

    assert_eq!(first, second);
    let hits = first
        .records
        .iter()
        .filter(|record| record.detectors == [true])
        .count();
    assert!(
        (180..=320).contains(&hits),
        "expected noisy DEM hits near p=0.25, got {hits}"
    );
}

#[test]
fn dem_sampler_rejects_repeat_expansion_before_counting_detectors() {
    let too_large_repeat = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            error(1) D0
        }
        ",
    )
    .expect("parse oversized repeat");
    let error = CompiledDemSampler::compile(&too_large_repeat).expect_err("reject repeat count");
    assert!(
        error
            .to_string()
            .contains("supports repeat counts up to 100000"),
        "{error}"
    );

    let nested_explosion = DetectorErrorModel::from_dem_str(
        "
        repeat 100000 {
            repeat 100000 {
                error(1) D0
            }
        }
        ",
    )
    .expect("parse nested repeat");
    let error =
        CompiledDemSampler::compile(&nested_explosion).expect_err("reject nested expansion");
    assert!(
        error.to_string().contains("expanded repeat iterations"),
        "{error}"
    );
}

#[test]
fn dem_sampler_rejects_excessive_buffered_outputs_before_sampling() {
    let empty = compile_dem("");
    let error = empty
        .sample_detection_events_with_seed(64_000_001, Some(5))
        .expect_err("reject excessive empty records");
    assert!(
        error
            .to_string()
            .contains("would require 64000001 buffered units"),
        "{error}"
    );

    let high_detector = compile_dem("detector D64000000\n");
    let error = high_detector
        .sample_detection_events_with_seed(1, Some(5))
        .expect_err("reject excessive detector width");
    assert!(
        error
            .to_string()
            .contains("would require 64000001 buffered units"),
        "{error}"
    );

    let high_observable = compile_dem("logical_observable L64000000\n");
    let error = high_observable
        .sample_detection_events_with_seed(1, Some(5))
        .expect_err("reject excessive observable width");
    assert!(
        error
            .to_string()
            .contains("would require 64000001 buffered units"),
        "{error}"
    );

    let sampler_with_error_records = compile_dem("error(1) D0\n");
    let error = sampler_with_error_records
        .sample_detection_events_and_errors_with_seed(32_000_001, Some(5))
        .expect_err("reject excessive detector plus error records");
    assert!(
        error
            .to_string()
            .contains("would require 64000002 buffered units"),
        "{error}"
    );

    let wide_replay_sampler = compile_dem(
        "
        repeat 100000 {
            error(1) D0
        }
        ",
    );
    let replay_records = vec![vec![false; wide_replay_sampler.error_count()]; 641];
    let error = wide_replay_sampler
        .sample_detection_events_from_error_records(&replay_records)
        .expect_err("reject excessive replayed error records");
    assert!(
        error
            .to_string()
            .contains("would require 64100641 buffered units"),
        "{error}"
    );
}
