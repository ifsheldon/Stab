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
