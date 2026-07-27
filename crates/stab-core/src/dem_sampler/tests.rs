#![allow(
    clippy::expect_used,
    reason = "DEM sampler tests use direct fixture assertions for compact diagnostics"
)]

use super::*;

fn collect_streamed_samples(
    sampler: &CompiledDemSampler,
    shots: usize,
    seed: Option<u64>,
) -> CircuitResult<(Vec<DetectionEventRecord>, Vec<Vec<bool>>)> {
    let mut records = Vec::new();
    let mut errors = Vec::new();
    sampler.try_for_each_detection_event_and_error_with_seed(
        shots,
        seed,
        |record, error_record| {
            records.push(record.clone());
            errors.push(error_record.to_vec());
            Ok::<(), CircuitError>(())
        },
    )?;
    Ok((records, errors))
}

#[test]
fn odd_parity_probability_matches_repeated_independent_error_parity() {
    assert_eq!(odd_parity_probability(0.0, 1_000_000), 0.0);
    assert_eq!(odd_parity_probability(1.0, 4), 0.0);
    assert_eq!(odd_parity_probability(1.0, 5), 1.0);
    assert!((odd_parity_probability(0.25, 2) - 0.375).abs() < 1e-12);
    assert!((odd_parity_probability(0.5, 64_000_001) - 0.5).abs() < 1e-12);

    let tiny_probability = odd_parity_probability(1e-18, 1_000_000_000_000_000_000);
    let expected_tiny_probability = -0.5 * (-2.0_f64).exp_m1();
    assert!((tiny_probability - expected_tiny_probability).abs() < 1e-12);

    let near_one = 1.0 - 1e-12;
    let near_one_probability = odd_parity_probability(near_one, 1_000_000_000_001);
    let expected_near_one_probability =
        1.0 - odd_parity_probability(1.0 - near_one, 1_000_000_000_001);
    assert!((near_one_probability - expected_near_one_probability).abs() < 1e-12);
}

#[test]
fn replay_work_arithmetic_overflow_rejects_before_iteration() {
    let sampler = CompiledDemSampler {
        detector_count: 1,
        observable_count: 1,
        operations: DemSampleBlock {
            error_count: 1,
            ..DemSampleBlock::default()
        },
    };
    let error = sampler
        .validate_replay_work_units_with_limits(usize::MAX, DemSamplerLimits::default())
        .expect_err("replay work multiplication must be checked");
    assert!(
        error
            .to_string()
            .contains("DEM sampler replay work overflowed")
    );
}

#[test]
fn dem_streaming_samples_match_materialized_seeded_samples() {
    for dem_text in [
        "error(1) D0\n",
        "error(0.25) D0\n",
        "error(0.25) L2\n",
        "error(0.25) D0 D2\nerror(0.25) D2 D3\n",
        "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
    ] {
        let model = DetectorErrorModel::from_dem_str(dem_text).expect("parse DEM");
        let sampler = CompiledDemSampler::compile(&model).expect("compile DEM sampler");
        let (materialized, materialized_errors) = sampler
            .sample_detection_events_and_errors_with_seed(65, Some(7))
            .expect("materialized samples");
        let (streamed, streamed_errors) =
            collect_streamed_samples(&sampler, 65, Some(7)).expect("streamed samples");

        assert_eq!(streamed, materialized.records);
        assert_eq!(streamed_errors, materialized_errors);
        let replayed = sampler
            .sample_detection_events_from_error_records(&streamed_errors)
            .expect("materialized replay");
        let mut streamed_replay = Vec::new();
        sampler
            .try_for_each_detection_event_from_error_records(
                streamed_errors.iter().map(Vec::as_slice),
                |record, _error_record| {
                    streamed_replay.push(record.clone());
                    Ok::<(), CircuitError>(())
                },
            )
            .expect("streamed replay");
        assert_eq!(streamed_replay, replayed.records);
    }
}
