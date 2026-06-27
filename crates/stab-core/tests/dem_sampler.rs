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
