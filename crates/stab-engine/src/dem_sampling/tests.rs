#![allow(
    clippy::expect_used,
    reason = "DEM sampling tests use compact fixture assertions"
)]

use std::convert::Infallible;
use std::sync::Arc;

use stab_model::DetectorErrorModel;
use stab_records::{DemSampleBatchView, DemSampleSink};

use super::plan::DemSamplingPlanInner;
use super::program::{DemSampleBlock, DemSampleError, DemSampleOperation, odd_parity_probability};
use super::*;
use crate::{DetectionRecordBuffer, RandomPolicy, Seed, ShotCount};

#[derive(Default)]
struct CollectingSink {
    records: Vec<DetectionRecordBuffer>,
    error_records: Vec<Vec<bool>>,
    writes: usize,
    finishes: usize,
}

impl DemSampleSink for CollectingSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        self.writes += 1;
        let detection = batch.detection();
        for shot_index in 0..detection.shot_count() {
            let detectors = (0..detection.detector_width().get())
                .map(|bit_index| {
                    detection
                        .detectors()
                        .get(shot_index, bit_index)
                        .expect("detector bit")
                })
                .collect();
            let observables = (0..detection.observable_width().get())
                .map(|bit_index| {
                    detection
                        .observables()
                        .get(shot_index, bit_index)
                        .expect("observable bit")
                })
                .collect();
            self.records.push(DetectionRecordBuffer {
                detectors,
                observables,
            });
            if let Some(sampled_errors) = batch.sampled_errors() {
                self.error_records.push(
                    (0..sampled_errors.bits_per_shot())
                        .map(|bit_index| {
                            sampled_errors
                                .get(shot_index, bit_index)
                                .expect("sampled-error bit")
                        })
                        .collect(),
                );
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        self.finishes += 1;
        Ok(())
    }
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
fn replay_work_arithmetic_overflow_rejects_before_execution() {
    let plan = DemSamplingPlan {
        inner: Arc::new(DemSamplingPlanInner {
            detector_count: 1,
            observable_count: 1,
            operations: DemSampleBlock {
                error_count: 1,
                ..DemSampleBlock::default()
            },
        }),
    };
    let error = plan
        .validate_replay_work_units_with_limits(usize::MAX, DemSamplerLimits::default())
        .expect_err("replay work multiplication must be checked");
    assert!(
        error
            .to_string()
            .contains("DEM sampler replay work overflowed")
    );
}

#[test]
fn seeded_partitioning_and_replay_preserve_exact_dem_streams() {
    for dem_text in [
        "error(1) D0\n",
        "error(0.25) D0\n",
        "error(0.25) L2\n",
        "error(0.25) D0 D2\nerror(0.25) D2 D3\n",
        "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
    ] {
        let model = DetectorErrorModel::from_dem_str(dem_text).expect("parse DEM");
        let plan = DemSamplingCompiler::new()
            .compile(&model)
            .expect("compile DEM sampler");

        let mut whole = plan
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("whole session");
        let mut whole_sink = CollectingSink::default();
        whole
            .run_with_sampled_errors(ShotCount::new(65), &mut whole_sink)
            .expect("whole run");

        let mut partitioned = plan
            .session(RandomPolicy::Seeded(Seed::new(7)))
            .expect("partitioned session");
        let mut partitioned_sink = CollectingSink::default();
        partitioned
            .run_with_sampled_errors(ShotCount::new(1), &mut partitioned_sink)
            .expect("first partition");
        partitioned
            .run_with_sampled_errors(ShotCount::new(64), &mut partitioned_sink)
            .expect("second partition");

        assert_eq!(partitioned_sink.records, whole_sink.records);
        assert_eq!(partitioned_sink.error_records, whole_sink.error_records);

        let mut replay = plan
            .replay_session(ShotCount::new(65))
            .expect("replay session");
        let mut replay_sink = CollectingSink::default();
        replay
            .run(&whole_sink.error_records, &mut replay_sink)
            .expect("replay sampled errors");
        assert_eq!(replay_sink.records, whole_sink.records);
        assert_eq!(replay_sink.error_records, whole_sink.error_records);
    }
}

#[test]
fn execution_failure_preserves_first_error_progress_and_poisons_session() {
    let plan = DemSamplingPlan {
        inner: Arc::new(DemSamplingPlanInner {
            detector_count: 1,
            observable_count: 0,
            operations: DemSampleBlock {
                operations: vec![DemSampleOperation::Error(DemSampleError {
                    probability: 1.0,
                    detectors: vec![1],
                    observables: Vec::new(),
                })],
                error_count: 1,
                direct_sample_effect_count: 1,
                direct_sample_work_count: 1,
                ..DemSampleBlock::default()
            },
        }),
    };
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(3)))
        .expect("construct malformed-program test session");
    let mut sink = CollectingSink::default();
    let error = session
        .run(ShotCount::new(1), &mut sink)
        .expect_err("out-of-range lowered detector must fail execution");
    assert!(
        error
            .to_string()
            .contains("detector index 1 is out of range"),
        "{error}"
    );
    assert_eq!(error.progress().committed_shots().get(), 0);
    assert_eq!(error.progress().attempted_batch_shots().get(), 1);
    assert_eq!(sink.writes, 0);
    assert_eq!(sink.finishes, 0);
    assert!(session.is_poisoned());
}
