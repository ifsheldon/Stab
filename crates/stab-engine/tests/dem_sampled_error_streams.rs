#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    reason = "DEM stream tests fail immediately on fixture errors and compare small exact counts"
)]

use std::convert::Infallible;

use stab_engine::{DemSamplingCompiler, DemSamplingPlan, RandomPolicy, Seed, ShotCount};
use stab_model::DetectorErrorModel;
use stab_records::{DemSampleBatchView, DemSampleSink, RecordBatchView};

/// Models covering every way the detector stream treats an error: sampled directly, inside a
/// repeat that cannot fire, inside a deterministic repeat applied once or cancelled, inside a
/// folded zero-shift repeat at any nesting, and inside shifted repeats around those.
const MODELS: &[&str] = &[
    "error(0.1) D0\nrepeat 3 {\n    error(0) D1\n}\nerror(0.2) D1 L0\n",
    "error(0.1) D0\nrepeat 5 {\n    error(0.1) D1\n}\nerror(0.2) D2 L0\n",
    "error(0.1) D0\nrepeat 4 {\n    error(1) D1\n}\nrepeat 3 {\n    error(1) D2 L1\n    error(0) D0\n}\nerror(0.2) D2 L0\n",
    "error(0) D0\nerror(1) D1\nerror(0.3) L0\nerror(0.4)\nrepeat 2 {\n    error(0.25) L1\n}\n",
    "repeat 3 {\n    error(0.02) D0\n    repeat 2 {\n        error(0.3) D1 L0\n        repeat 3 {\n            error(1) D2\n        }\n        repeat 2 {\n            error(0.7) D0 D2\n        }\n    }\n    repeat 4 {\n        error(1) L1\n    }\n}\n",
    "repeat 4 {\n    error(0.01) D0 D1\n    repeat 6 {\n        error(0.6) D1\n    }\n    repeat 2 {\n        error(0) D0\n    }\n    shift_detectors 2\n}\nerror(0.5) D3 L0\n",
    "repeat 3 {\n    repeat 100 {\n        error(0.004) D0\n        error(0.9) D1 L0\n    }\n    repeat 7 {\n        error(1) D2\n        error(0.5) D0\n    }\n    shift_detectors 3\n    error(0.15) D0 D2\n}\n",
    "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
];

const SHOT_COUNTS: [u64; 8] = [1, 63, 64, 65, 257, 1_000, 1_025, 2_100];

#[derive(Default)]
struct Rows {
    detectors: Vec<Vec<bool>>,
    observables: Vec<Vec<bool>>,
    errors: Vec<Vec<bool>>,
}

impl DemSampleSink for Rows {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        let detection = batch.detection();
        for shot in 0..detection.shot_count() {
            self.detectors.push(row(detection.detectors(), shot));
            self.observables.push(row(detection.observables(), shot));
            if let Some(errors) = batch.sampled_errors() {
                self.errors.push(row(errors, shot));
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn row(records: RecordBatchView<'_>, shot: usize) -> Vec<bool> {
    (0..records.bits_per_shot())
        .map(|bit| records.get(shot, bit).expect("record bit"))
        .collect()
}

fn compile(text: &str) -> DemSamplingPlan {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM fixture");
    DemSamplingCompiler::new()
        .compile(&model)
        .expect("compile DEM sampler")
}

fn sample(plan: &DemSamplingPlan, seed: u64, shots: u64, record_errors: bool) -> Rows {
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .expect("DEM session");
    let mut rows = Rows::default();
    if record_errors {
        session
            .run_with_sampled_errors(ShotCount::new(shots), &mut rows)
            .expect("sampled-error run");
    } else {
        session
            .run(ShotCount::new(shots), &mut rows)
            .expect("detector-only run");
    }
    rows
}

#[test]
fn recording_sampled_errors_leaves_seeded_detectors_and_replay_unchanged() {
    for text in MODELS {
        let plan = compile(text);
        for seed in [0, 9_001] {
            for shots in SHOT_COUNTS {
                let context = format!("seed={seed} shots={shots} model:\n{text}");
                let plain = sample(&plan, seed, shots, false);
                let recorded = sample(&plan, seed, shots, true);
                assert_eq!(plain.detectors, recorded.detectors, "{context}");
                assert_eq!(plain.observables, recorded.observables, "{context}");
                assert_eq!(recorded.errors.len() as u64, shots, "{context}");

                let mut replay = plan
                    .replay_session(ShotCount::new(shots))
                    .expect("replay session");
                let mut replayed = Rows::default();
                replay
                    .run(&recorded.errors, &mut replayed)
                    .expect("replay recorded errors");
                assert_eq!(replayed.detectors, plain.detectors, "{context}");
                assert_eq!(replayed.observables, plain.observables, "{context}");
                assert_eq!(replayed.errors, recorded.errors, "{context}");
            }
        }
    }
}

/// Copies of one compiled error that the detector stream skips, applies by parity, or folds,
/// given as a model and the probability of each of its `copies` recorded bits.
struct CopyCase {
    model: &'static str,
    probability: f64,
    copies: usize,
}

const COPY_CASES: &[CopyCase] = &[
    CopyCase {
        model: "repeat 2 {\n    error(0.05) D0\n}\n",
        probability: 0.05,
        copies: 2,
    },
    CopyCase {
        model: "repeat 3 {\n    error(0.2) D0 L0\n}\n",
        probability: 0.2,
        copies: 3,
    },
    CopyCase {
        model: "repeat 4 {\n    error(0.85) D0\n}\n",
        probability: 0.85,
        copies: 4,
    },
    CopyCase {
        model: "repeat 5 {\n    error(0.6) L0\n}\n",
        probability: 0.6,
        copies: 5,
    },
    CopyCase {
        model: "repeat 2 {\n    repeat 3 {\n        error(0.1) D0\n    }\n}\n",
        probability: 0.1,
        copies: 6,
    },
    CopyCase {
        model: "repeat 2 {\n    error(0.5) D0\n}\n",
        probability: 0.5,
        copies: 2,
    },
    CopyCase {
        model: "repeat 40 {\n    error(0.005) D0\n}\n",
        probability: 0.005,
        copies: 40,
    },
    CopyCase {
        model: "repeat 3 {\n    error(0) D0\n}\n",
        probability: 0.0,
        copies: 3,
    },
    CopyCase {
        model: "repeat 4 {\n    error(1) D0\n}\n",
        probability: 1.0,
        copies: 4,
    },
];

fn binomial_pmf(trials: usize, probability: f64) -> Vec<f64> {
    let mut pmf = vec![0.0; trials + 1];
    pmf[0] = 1.0;
    for _ in 0..trials {
        for hits in (0..=trials).rev() {
            let stay = pmf[hits] * (1.0 - probability);
            let gain = if hits > 0 {
                pmf[hits - 1] * probability
            } else {
                0.0
            };
            pmf[hits] = stay + gain;
        }
    }
    pmf
}

/// Fails when an observed frequency of `count` in `trials` is more than six standard
/// deviations from `probability`; six sigma keeps the whole fixed-seed test far from chance.
fn assert_frequency(label: &str, count: usize, trials: usize, probability: f64) {
    let expected = trials as f64 * probability;
    let sigma = (trials as f64 * probability * (1.0 - probability)).sqrt();
    if sigma == 0.0 {
        assert_eq!(count as f64, expected, "{label}");
        return;
    }
    let z = (count as f64 - expected) / sigma;
    assert!(z.abs() < 6.0, "{label}: {count}/{trials} gives z={z:.2}");
}

#[test]
fn recorded_copies_of_skipped_and_folded_errors_are_independent_with_their_probability() {
    const SHOTS: usize = 200_000;
    for case in COPY_CASES {
        let plan = compile(case.model);
        let rows = sample(&plan, 77, SHOTS as u64, true);
        let mut hits_histogram = vec![0_usize; case.copies + 1];
        let mut copy_hits = vec![0_usize; case.copies];
        let mut joint_first_last = 0_usize;
        for errors in &rows.errors {
            assert_eq!(errors.len(), case.copies, "{}", case.model);
            let hits = errors.iter().filter(|bit| **bit).count();
            hits_histogram[hits] += 1;
            for (count, bit) in copy_hits.iter_mut().zip(errors) {
                *count += usize::from(*bit);
            }
            joint_first_last += usize::from(errors[0] && errors[case.copies - 1]);
        }
        for (copy, count) in copy_hits.into_iter().enumerate() {
            assert_frequency(
                &format!("copy {copy} of\n{}", case.model),
                count,
                SHOTS,
                case.probability,
            );
        }
        assert_frequency(
            &format!("first and last copies of\n{}", case.model),
            joint_first_last,
            SHOTS,
            case.probability * case.probability,
        );
        for (hits, (count, probability)) in hits_histogram
            .into_iter()
            .zip(binomial_pmf(case.copies, case.probability))
            .enumerate()
        {
            assert_frequency(
                &format!("{hits} hits among the copies of\n{}", case.model),
                count,
                SHOTS,
                probability,
            );
        }
    }
}
