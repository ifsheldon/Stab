#![allow(
    clippy::expect_used,
    reason = "compatibility tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    CircuitError, CompiledDemSampler, DemRepeatBlock, DetectionEventRecord,
    DetectionObservableOutputMode, DetectorErrorModel, RepeatCount, SampleFormat,
    write_detection_records,
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
fn pf4_dem_sampler_compiles_repeats_without_flat_operation_cap() {
    let formerly_too_large_repeat = DetectorErrorModel::from_dem_str(
        "
        repeat 100001 {
            error(1) D0
        }
        ",
    )
    .expect("parse folded repeat");
    let sampler =
        CompiledDemSampler::compile(&formerly_too_large_repeat).expect("compile folded repeat");
    assert_eq!(sampler.error_count(), 100_001);
    let sampled = sampler
        .sample_detection_events_with_seed(1, Some(5))
        .expect("sample folded repeat");
    let sampled_record = sampled.records.first().expect("one sampled record");
    assert_eq!(sampled_record.detectors, [true]);

    let formerly_nested_explosion = DetectorErrorModel::from_dem_str(
        "
        repeat 1001 {
            repeat 1000 {
                error(1) D0
            }
        }
        ",
    )
    .expect("parse nested folded repeat");
    let nested_sampler =
        CompiledDemSampler::compile(&formerly_nested_explosion).expect("compile nested repeat");
    assert_eq!(nested_sampler.error_count(), 1_001_000);
}

#[test]
fn pf4_dem_sampler_preserves_flat_error_order_through_nested_repeats() {
    let sampler = compile_dem(
        "
        error(1) D0 L0
        shift_detectors 1
        repeat 2 {
            error(1) D0
            error(0) D1 L1
            shift_detectors 2
            repeat 2 {
                error(1) D0 L2
                shift_detectors 1
            }
        }
        error(1) D0 L3
        ",
    );
    assert_eq!(sampler.error_count(), 10);

    let (sampled_output, error_records) = sampler
        .sample_detection_events_and_errors_with_seed(1, Some(5))
        .expect("sample nested repeated errors");
    assert_eq!(
        error_records,
        vec![vec![
            true, true, false, true, true, true, false, true, true, true,
        ]]
    );
    assert_eq!(
        sampled_output.records,
        vec![DetectionEventRecord {
            detectors: vec![true, true, false, true, true, true, false, true, true, true,],
            observables: vec![true, false, false, true],
        }]
    );

    let replayed = sampler
        .sample_detection_events_from_error_records(&[vec![
            false, false, false, false, false, false, true, false, false, false,
        ]])
        .expect("replay nested flat error");
    assert_eq!(
        replayed.records,
        vec![DetectionEventRecord {
            detectors: vec![
                false, false, false, false, false, false, true, false, false, false,
            ],
            observables: vec![false, true, false, false],
        }]
    );
}

#[test]
fn pf4_dem_sampler_deterministic_repeat_folding_preserves_rng_and_error_order() {
    let folded = compile_dem(
        "
        repeat 101 {
            error(1) D0 L0
        }
        error(0.25) D1
        ",
    );
    let expanded_equivalent = compile_dem(
        "
        error(1) D0 L0
        error(0.25) D1
        ",
    );
    let folded_output = folded
        .sample_detection_events_with_seed(64, Some(11))
        .expect("sample folded deterministic repeat");
    let expanded_output = expanded_equivalent
        .sample_detection_events_with_seed(64, Some(11))
        .expect("sample expanded deterministic equivalent");
    assert_eq!(folded_output.records, expanded_output.records);

    let flat_error_order = compile_dem(
        "
        repeat 3 {
            error(1) D0
            error(0) D0
        }
        ",
    );
    let (_output, error_records) = flat_error_order
        .sample_detection_events_and_errors_with_seed(1, Some(11))
        .expect("sample repeated deterministic error records");
    assert_eq!(
        error_records,
        vec![vec![true, false, true, false, true, false]]
    );
}

#[test]
fn pf4_dem_sampler_folded_repeat_sampling_and_materialized_error_caps() {
    let sampler = compile_dem(
        "
        repeat 100001 {
            error(1) D0 L0
            shift_detectors 1
        }
        ",
    );
    assert_eq!(sampler.error_count(), 100_001);

    let output = sampler
        .sample_detection_events_with_seed(1, Some(5))
        .expect("sample shifted folded repeat");
    assert_eq!(output.detector_count, 100_001);
    assert_eq!(output.observable_count, 1);
    assert_eq!(output.records.len(), 1);
    let record = output.records.first().expect("one shifted folded record");
    assert!(record.detectors.iter().all(|bit| *bit));
    assert_eq!(record.observables, [true]);

    let huge_no_op_error_record = DetectorErrorModel::from_dem_str(
        "
        repeat 64000001 {
            error(0) D0
        }
        ",
    )
    .expect("parse huge flat error-record DEM");
    let huge_sampler = CompiledDemSampler::compile(&huge_no_op_error_record)
        .expect("compile folded error-record DEM");
    assert_eq!(huge_sampler.error_count(), 64_000_001);
    let output = huge_sampler
        .sample_detection_events_with_seed(3, Some(5))
        .expect("skip huge detector-only no-op repeat");
    assert_eq!(output.detector_count, 1);
    assert_eq!(output.observable_count, 0);
    assert_eq!(output.records.len(), 3);
    assert!(
        output
            .records
            .iter()
            .all(|record| record.detectors == [false] && record.observables.is_empty())
    );

    let huge_stochastic_record = DetectorErrorModel::from_dem_str(
        "
        repeat 64000001 {
            error(0.5) D0
        }
        ",
    )
    .expect("parse huge stochastic DEM");
    let huge_stochastic_sampler =
        CompiledDemSampler::compile(&huge_stochastic_record).expect("compile stochastic DEM");
    assert_eq!(huge_stochastic_sampler.error_count(), 64_000_001);
    let huge_deterministic_odd_record = DetectorErrorModel::from_dem_str(
        "
        repeat 64000001 {
            error(1) D0 L0
            error(0) D0
        }
        ",
    )
    .expect("parse huge deterministic odd repeat DEM");
    let huge_deterministic_odd_sampler =
        CompiledDemSampler::compile(&huge_deterministic_odd_record)
            .expect("compile deterministic odd repeat DEM");
    assert_eq!(huge_deterministic_odd_sampler.error_count(), 128_000_002);
    let output = huge_deterministic_odd_sampler
        .sample_detection_events_with_seed(2, Some(5))
        .expect("fold odd deterministic repeat by parity");
    assert_eq!(output.detector_count, 1);
    assert_eq!(output.observable_count, 1);
    assert_eq!(output.records.len(), 2);
    assert!(
        output
            .records
            .iter()
            .all(|record| { record.detectors == [true] && record.observables == [true] })
    );

    let huge_deterministic_even_record = DetectorErrorModel::from_dem_str(
        "
        repeat 64000000 {
            error(1) D0 L0
        }
        ",
    )
    .expect("parse huge deterministic even repeat DEM");
    let huge_deterministic_even_sampler =
        CompiledDemSampler::compile(&huge_deterministic_even_record)
            .expect("compile deterministic even repeat DEM");
    assert_eq!(huge_deterministic_even_sampler.error_count(), 64_000_000);
    let output = huge_deterministic_even_sampler
        .sample_detection_events_with_seed(2, Some(5))
        .expect("fold even deterministic repeat by parity");
    assert_eq!(output.records.len(), 2);
    assert!(
        output
            .records
            .iter()
            .all(|record| { record.detectors == [false] && record.observables == [false] })
    );

    let mixed_zero_and_stochastic_record = DetectorErrorModel::from_dem_str(
        "
        repeat 32000001 {
            error(0.5) D0
            error(0) D0
        }
        ",
    )
    .expect("parse mixed stochastic and zero-probability DEM");
    let mixed_zero_and_stochastic_sampler =
        CompiledDemSampler::compile(&mixed_zero_and_stochastic_record).expect("compile mixed DEM");
    assert_eq!(mixed_zero_and_stochastic_sampler.error_count(), 64_000_002);
    let error = huge_sampler
        .sample_detection_events_and_errors_with_seed(1, Some(5))
        .expect_err("reject materialized sampled-error record");
    assert!(
        error
            .to_string()
            .contains("would require 64000002 buffered units"),
        "{error}"
    );

    let error = huge_stochastic_sampler
        .sample_detection_events_with_seed(1, Some(5))
        .expect_err("reject excessive detector-only sampled work");
    assert!(
        error
            .to_string()
            .contains("would apply 64000001 sampled errors"),
        "{error}"
    );

    let error = huge_deterministic_odd_sampler
        .sample_detection_events_and_errors_with_seed(1, Some(5))
        .expect_err("preserve flat sampled-error materialization cap for deterministic repeats");
    assert!(
        error
            .to_string()
            .contains("would require 128000004 buffered units"),
        "{error}"
    );

    let error = mixed_zero_and_stochastic_sampler
        .sample_detection_events_with_seed(1, Some(5))
        .expect_err("reject mixed zero-probability traversal work");
    assert!(
        error
            .to_string()
            .contains("would apply 64000002 sampled errors"),
        "{error}"
    );

    let error = huge_sampler
        .try_for_each_detection_event_and_error_with_seed(1, Some(5), |_record, _error_record| {
            Ok::<(), CircuitError>(())
        })
        .expect_err("reject excessive streamed sampled-error record");
    assert!(
        error
            .to_string()
            .contains("would require 64000002 buffered units"),
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

#[test]
fn dem_sampler_rejects_materialized_heap_pressure_before_sampling() {
    let empty = compile_dem("");
    let error = empty
        .sample_detection_events_with_seed(3_000_000, Some(5))
        .expect_err("reject excessive materialized record overhead");
    assert!(error.to_string().contains("materialized bytes"), "{error}");
}

#[test]
fn pf4_dem_sampler_rejects_programmatic_deep_repeat_nesting() {
    let mut model = DetectorErrorModel::new();
    for _ in 0..257 {
        let mut outer = DetectorErrorModel::new();
        outer.push_repeat_block(DemRepeatBlock::new(
            RepeatCount::try_new(1).expect("repeat count"),
            model,
            None,
        ));
        model = outer;
    }

    let error = CompiledDemSampler::compile(&model).expect_err("reject deep repeat nesting");
    assert!(
        error.to_string().contains("repeat nesting exceeds"),
        "{error}"
    );
}
