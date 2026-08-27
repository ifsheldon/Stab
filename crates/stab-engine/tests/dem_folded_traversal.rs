#![allow(
    clippy::expect_used,
    reason = "integration tests use direct assertions for compact parity diagnostics"
)]

use std::convert::Infallible;

use stab_engine::{DemSamplingCompiler, DemSamplingPlan, RandomPolicy, Seed, ShotCount};
use stab_model::DetectorErrorModel;
use stab_records::{DemSampleBatchView, DemSampleSink};

#[derive(Clone, Debug, Eq, PartialEq)]
struct SampleRecord {
    detectors: Vec<bool>,
    observables: Vec<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SampleOutput {
    records: Vec<SampleRecord>,
}

#[derive(Default)]
struct CollectSamples {
    records: Vec<SampleRecord>,
}

impl DemSampleSink for CollectSamples {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DemSampleBatchView<'_>) -> Result<(), Self::Error> {
        let detection = batch.detection();
        for shot in 0..detection.shot_count() {
            self.records.push(SampleRecord {
                detectors: (0..detection.detector_width().get())
                    .map(|bit| {
                        detection
                            .detectors()
                            .get(shot, bit)
                            .expect("validated detector coordinate")
                    })
                    .collect(),
                observables: (0..detection.observable_width().get())
                    .map(|bit| {
                        detection
                            .observables()
                            .get(shot, bit)
                            .expect("validated observable coordinate")
                    })
                    .collect(),
            });
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn dem(text: &str) -> DetectorErrorModel {
    DetectorErrorModel::from_dem_str(text).expect("valid test DEM")
}

fn compile_dem_sampling(model: &DetectorErrorModel) -> Result<DemSamplingPlan, String> {
    DemSamplingCompiler::new()
        .compile(model)
        .map_err(|error| error.to_string())
}

fn collect_dem_samples(
    plan: &DemSamplingPlan,
    shots: usize,
    seed: u64,
) -> Result<SampleOutput, String> {
    let shots = u64::try_from(shots).map_err(|_| "shot count does not fit u64".to_owned())?;
    let mut session = plan
        .session(RandomPolicy::Seeded(Seed::new(seed)))
        .map_err(|error| error.to_string())?;
    let mut sink = CollectSamples::default();
    session
        .run(ShotCount::new(shots), &mut sink)
        .map_err(|error| error.to_string())?;
    Ok(SampleOutput {
        records: sink.records,
    })
}

fn assert_folded_sampling_matches_materialized_reference() {
    let cases = [
        (
            "repeat 3 {\n    error(1) D0 L0\n    shift_detectors 0\n}\n",
            "error(1) D0 L0\nerror(1) D0 L0\nerror(1) D0 L0\n",
        ),
        (
            "repeat[outer] 2 {\n    error(1) D0 ^ D1 L0\n    repeat[inner] 2 {\n        error(0) D1\n    }\n}\n",
            "error(1) D0 ^ D1 L0\nerror(0) D1\nerror(0) D1\nerror(1) D0 ^ D1 L0\nerror(0) D1\nerror(0) D1\n",
        ),
        (
            "repeat 4 {\n    error(1) D2 L1\n}\n",
            "error(1) D2 L1\nerror(1) D2 L1\nerror(1) D2 L1\nerror(1) D2 L1\n",
        ),
    ];

    for (compact, materialized) in cases {
        let compact = compile_dem_sampling(&dem(compact)).expect("compact sampler compilation");
        let materialized =
            compile_dem_sampling(&dem(materialized)).expect("materialized sampler compilation");
        assert_eq!(
            collect_dem_samples(&compact, 64, 0xB3).expect("compact seeded samples"),
            collect_dem_samples(&materialized, 64, 0xB3).expect("materialized seeded samples")
        );
    }
}

#[test]
fn pfm_b3_folded_traversal_sampler() {
    const SHOTS: usize = 100_000;

    assert_folded_sampling_matches_materialized_reference();

    let stochastic = dem("repeat 1000001 {\n\
             error(0.1) D0 L0\n\
             shift_detectors 0\n\
         }\n");
    let plan = compile_dem_sampling(&stochastic).expect("folded sampler compilation");
    assert_eq!(plan.error_count(), 1_000_001);
    let output = collect_dem_samples(&plan, SHOTS, 12_648_437).expect("seeded folded sampling");
    assert_eq!(output.records.len(), SHOTS, "folded statistical shots");
    let mut all_zero = 0_usize;
    let mut joint_nonzero = 0_usize;
    let mut unexpected = 0_usize;
    for record in &output.records {
        match (record.detectors.as_slice(), record.observables.as_slice()) {
            ([false], [false]) => all_zero += 1,
            ([true], [true]) => joint_nonzero += 1,
            _ => unexpected += 1,
        }
    }
    assert_eq!(unexpected, 0, "unexpected joint detector-observable bucket");

    let combinations = compile_dem_sampling(&dem(
        "error(0.1) D0 D1\nerror(0.2) D1 D2\nerror(0.3) D2 D0\n",
    ))
    .expect("pinned combination sampler");
    let combination_output =
        collect_dem_samples(&combinations, SHOTS, 12_648_437).expect("seeded combination sampling");
    assert_eq!(
        combination_output.records.len(),
        SHOTS,
        "combination statistical shots"
    );
    let mut detector_hits = [0_usize; 3];
    for record in &combination_output.records {
        for (index, detector) in record.detectors.iter().copied().enumerate() {
            if detector {
                *detector_hits
                    .get_mut(index)
                    .expect("combination sampler has exactly three detectors") += 1;
            }
        }
        assert!(
            !record
                .detectors
                .iter()
                .copied()
                .fold(false, |parity, bit| parity ^ bit),
            "the three pair mechanisms must preserve even detector parity"
        );
    }
    emit_statistical_completion(
        "pfm4-traversal-sampler",
        12_648_437,
        u64::try_from(SHOTS * 2).expect("folded statistical shots fit u64"),
    );
    assert_probability(all_zero, SHOTS, 0.5);
    assert_probability(joint_nonzero, SHOTS, 0.5);
    assert_probability(detector_hits[0], SHOTS, 0.34);
    assert_probability(detector_hits[1], SHOTS, 0.26);
    assert_probability(detector_hits[2], SHOTS, 0.38);

    let deterministic = compile_dem_sampling(&dem("repeat 1000000000 {\n\
             repeat 3 {\n\
                 error(1) D1 L1\n\
             }\n\
         }\n"))
    .expect("deterministic folded sampler compilation");
    let record = collect_dem_samples(&deterministic, 1, 12_648_437)
        .expect("deterministic folded sample")
        .records
        .into_iter()
        .next()
        .expect("one record");
    assert_eq!(record.detectors, vec![false, false]);
    assert_eq!(record.observables, vec![false, false]);
}

fn assert_probability(observed: usize, shots: usize, expected: f64) {
    let sigma = (expected * (1.0 - expected) / shots as f64).sqrt();
    let tolerance = 0.01_f64.max(6.0 * sigma);
    let observed = observed as f64 / shots as f64;
    assert!(
        (observed - expected).abs() <= tolerance,
        "observed={observed} expected={expected} tolerance={tolerance}"
    );
}

fn emit_statistical_completion(case_id: &str, seed: u64, completed_shots: u64) {
    println!("STAB_CQ1_STATISTICAL\t1\t{case_id}\t{seed}\t0\t{completed_shots}");
}
