#![allow(
    clippy::expect_used,
    reason = "sampling unit tests use direct fixture parsing assertions for compact diagnostics"
)]

use super::execute::{
    ReferenceExecutionStats, execute_reference_operations, reset_reference_execution_stats,
    take_reference_execution_stats,
};
use super::small_frame::SmallStabilizerFrame;
use super::stabilizer_frame::StabilizerStateSnapshot;
use super::*;
use stab_model::Gate;
use stab_records::{MeasurementBatchView, MeasurementSink};
use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
struct TestSampler {
    plan: SamplingPlan,
}

impl TestSampler {
    fn compile(circuit: &Circuit) -> Result<Self, SamplingCompileError> {
        SamplingCompiler::new()
            .compile(circuit)
            .map(|plan| Self { plan })
    }

    fn sample_zero_one_with_seed(&self, shots: usize, seed: Option<u64>) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed_and_reference_mode(shots, seed, false)
    }

    fn sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<Vec<bool>> {
        let random_policy = seed.map_or(RandomPolicy::Entropy, |seed| {
            RandomPolicy::Seeded(Seed::new(seed))
        });
        let reference_mode = if skip_reference_sample {
            ReferenceSampleMode::SkipReferenceSample
        } else {
            ReferenceSampleMode::UseReferenceSample
        };
        let mut session = self
            .plan
            .session_with_reference_mode(random_policy, reference_mode)
            .expect("construct sampling session");
        let mut sink = RecordSink::default();
        session
            .run(
                ShotCount::new(u64::try_from(shots).expect("shot count")),
                &mut sink,
            )
            .expect("run sampling session");
        sink.records
    }

    fn reference_sample(&self) -> Vec<bool> {
        self.plan.try_reference_sample().expect("reference sample")
    }
}

#[derive(Default)]
struct RecordSink {
    records: Vec<Vec<bool>>,
}

impl MeasurementSink for RecordSink {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            let record = (0..batch.width().get())
                .map(|bit_index| {
                    batch
                        .get(shot_index, bit_index)
                        .expect("validated batch dimensions")
                })
                .collect();
            self.records.push(record);
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn samples(input: &str, shots: usize) -> Vec<Vec<bool>> {
    let circuit = Circuit::from_stim_str(input).expect("parse circuit");
    TestSampler::compile(&circuit)
        .expect("compile sampler")
        .sample_zero_one_with_seed(shots, None)
}

fn count_determined(input: &str, unknown_input: bool) -> u64 {
    let circuit = Circuit::from_stim_str(input).expect("parse circuit");
    count_determined_measurements(&circuit, unknown_input).expect("count determined measurements")
}

fn reference_with_stats(plan: &SamplingPlan) -> (Vec<bool>, ReferenceExecutionStats) {
    let mut rng = SmallRng::seed_from_u64(0);
    let mut frame = StabilizerFrame::try_new(plan.inner.qubit_count).expect("reference frame");
    let mut snapshot =
        StabilizerStateSnapshot::try_new(plan.inner.qubit_count).expect("reference state snapshot");
    let mut record = Vec::with_capacity(plan.inner.measurement_count);
    let mut output = Vec::with_capacity(plan.inner.measurement_count);
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame: &mut frame,
        record: &mut record,
        output: &mut output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    reset_reference_execution_stats();
    execute_reference_operations(
        &plan.inner.operations,
        &mut buffers,
        &mut rng,
        &[],
        plan.reference_sample_loop_policy(),
        Some(&mut snapshot),
    )
    .expect("execute reference program");
    let stats = take_reference_execution_stats();
    (output, stats)
}

#[test]
fn reference_loop_policy_reuses_only_proven_invariant_work() {
    let invariant = Circuit::from_stim_str("REPEAT 512 {\n    H 0\n    M 0\n    R 0\n}\n")
        .expect("parse invariant loop");
    let folded = SamplingCompiler::new()
        .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Fold)
        .compile(&invariant)
        .expect("compile folded reference plan");
    let iterated = SamplingCompiler::new()
        .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
        .compile(&invariant)
        .expect("compile iterative reference plan");
    let (folded_output, folded_stats) = reference_with_stats(&folded);
    let (iterated_output, iterated_stats) = reference_with_stats(&iterated);

    assert_eq!(folded_output, iterated_output);
    assert_eq!(folded_output.len(), 512);
    assert_eq!(folded_stats.folded_repeats, 1);
    assert_eq!(folded_stats.reused_iterations, 511);
    assert_eq!(folded_stats.reused_operation_dispatches, 1_533);
    assert_eq!(iterated_stats, ReferenceExecutionStats::default());
    assert_ne!(folded.fingerprint(), iterated.fingerprint());

    for source in [
        "REPEAT 128 {\n    H 0\n}\nM 0\n",
        "M 0\nREPEAT 128 {\n    CX rec[-1] 0\n    M 0\n}\n",
    ] {
        let circuit = Circuit::from_stim_str(source).expect("parse non-foldable loop");
        let folded = SamplingCompiler::new()
            .compile(&circuit)
            .expect("compile non-foldable loop");
        let iterated = SamplingCompiler::new()
            .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
            .compile(&circuit)
            .expect("compile iterative non-foldable loop");
        let (folded_output, folded_stats) = reference_with_stats(&folded);
        let (iterated_output, _) = reference_with_stats(&iterated);
        assert_eq!(folded_output, iterated_output, "{source}");
        assert_eq!(folded_stats, ReferenceExecutionStats::default(), "{source}");
    }

    let body = std::iter::repeat_n("    H 0\n", 64).collect::<String>();
    let narrow = Circuit::from_stim_str(&format!("REPEAT 2 {{\n{body}}}\nM 0\n"))
        .expect("parse narrow fold-profitability circuit");
    let wide = Circuit::from_stim_str(&format!("REPEAT 2 {{\n{body}}}\nM 127\n"))
        .expect("parse wide fold-profitability circuit");
    let narrow = SamplingCompiler::new()
        .compile(&narrow)
        .expect("compile narrow fold-profitability circuit");
    let wide = SamplingCompiler::new()
        .compile(&wide)
        .expect("compile wide fold-profitability circuit");
    assert!(narrow.inner.has_reference_state_snapshot_candidate());
    assert!(!wide.inner.has_reference_state_snapshot_candidate());
    assert!(!wide.inner.uses_reference_state_snapshot());
}

#[test]
fn reference_snapshot_storage_is_limited_to_fold_candidates() {
    let ordinary = SamplingCompiler::new()
        .compile(&Circuit::from_stim_str("H 0\nM 0\n").expect("parse ordinary reference"))
        .expect("compile ordinary reference");
    assert!(!ordinary.inner.has_reference_state_snapshot_candidate());
    assert!(!ordinary.inner.uses_reference_state_snapshot());
    assert_eq!(
        ordinary.estimated_reference_work_storage_bytes(),
        general_frame_work_storage_bytes(
            ordinary.inner.qubit_count,
            ordinary.inner.measurement_count,
            false,
        )
    );

    let repeated = Circuit::from_stim_str("REPEAT 512 {\n    H 0\n    M 0\n    R 0\n}\n")
        .expect("parse reusable reference repeat");
    let folded = SamplingCompiler::new()
        .compile(&repeated)
        .expect("compile folded reference repeat");
    let iterated = SamplingCompiler::new()
        .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
        .compile(&repeated)
        .expect("compile iterated reference repeat");

    assert!(folded.inner.has_reference_state_snapshot_candidate());
    assert!(folded.inner.uses_reference_state_snapshot());
    assert!(!iterated.inner.has_reference_state_snapshot_candidate());
    assert!(!iterated.inner.uses_reference_state_snapshot());
    assert_eq!(
        folded
            .estimated_reference_work_storage_bytes()
            .saturating_sub(iterated.estimated_reference_work_storage_bytes()),
        StabilizerStateSnapshot::storage_bytes(folded.inner.qubit_count)
    );

    let base = general_frame_work_storage_bytes(
        folded.inner.qubit_count,
        folded.inner.measurement_count,
        false,
    );
    assert!(reference_state_snapshot_fits(
        folded.inner.qubit_count,
        folded.inner.measurement_count,
        u64::try_from(folded.estimated_reference_work_storage_bytes())
            .expect("small folded reference estimate"),
    ));
    assert!(!reference_state_snapshot_fits(
        folded.inner.qubit_count,
        folded.inner.measurement_count,
        u64::try_from(base).expect("small base reference estimate"),
    ));

    let fallback_qubits = (SmallStabilizerFrame::MAX_QUBITS + 1..20_000)
        .find(|qubits| {
            let base = general_frame_work_storage_bytes(*qubits, 1, false);
            base <= u128::from(api::MAX_SAMPLING_SESSION_STORAGE_BYTES)
                && !reference_state_snapshot_fits(
                    *qubits,
                    1,
                    api::MAX_SAMPLING_SESSION_STORAGE_BYTES,
                )
        })
        .expect("reference snapshot has a fallback-only storage interval");
    let fallback_repetitions = fallback_qubits.saturating_mul(2).saturating_add(1);
    let fallback_circuit = Circuit::from_stim_str(&format!(
        "REPEAT {fallback_repetitions} {{\n    H {}\n}}\nM {}\n",
        fallback_qubits - 1,
        fallback_qubits - 1
    ))
    .expect("parse snapshot-fallback circuit");
    let fallback_folded = SamplingCompiler::new()
        .compile(&fallback_circuit)
        .expect("compile snapshot-fallback circuit");
    let fallback_iterated = SamplingCompiler::new()
        .reference_sample_loop_policy(ReferenceSampleLoopPolicy::Iterate)
        .compile(&fallback_circuit)
        .expect("compile explicit-iteration fallback circuit");
    assert!(
        fallback_folded
            .inner
            .has_reference_state_snapshot_candidate()
    );
    assert!(!fallback_folded.inner.uses_reference_state_snapshot());
    assert_eq!(
        fallback_folded.estimated_reference_work_storage_bytes(),
        fallback_iterated.estimated_reference_work_storage_bytes()
    );
}

#[test]
fn expanded_sampling_work_is_bounded_while_zero_width_repeats_are_constant_work() {
    let exact = Circuit::from_stim_str("REPEAT 999999 {\n    H 0\n}\nM 0\n")
        .expect("parse exact expanded-work boundary");
    SamplingCompiler::new()
        .compile(&exact)
        .expect("accept exact expanded-work boundary");

    let first_excess = Circuit::from_stim_str("REPEAT 1000000 {\n    H 0\n}\nM 0\n")
        .expect("parse first expanded-work excess");
    assert_eq!(
        SamplingCompiler::new().compile(&first_excess),
        Err(SamplingCompileError::ExpandedOperationLimit {
            actual: crate::ResourceAmount::exact(1_000_001),
            limit: 1_000_000,
        })
    );
    assert_eq!(
        SamplingCompiler::new()
            .compile(&first_excess)
            .expect_err("expanded work must be rejected")
            .code(),
        SamplingCompileErrorCode::ResourceLimit
    );
    let detection_error = crate::DetectionError::from(
        SamplingCompiler::new()
            .compile(&first_excess)
            .expect_err("detection sampling must preserve the resource failure"),
    );
    assert!(matches!(
        detection_error,
        crate::DetectionError::ResourceLimit(_)
    ));
    if let crate::DetectionError::ResourceLimit(detection_limit) = detection_error {
        assert_eq!(
            detection_limit.kind(),
            crate::DetectionResourceKind::SamplingExpandedOperations
        );
        assert_eq!(detection_limit.actual(), 1_000_001);
        assert_eq!(detection_limit.limit(), 1_000_000);
    }

    let zero_width = Circuit::from_stim_str("REPEAT 1000000000000 {\n    H 0\n    H 0\n}\n")
        .expect("parse zero-width huge repeat");
    let sampler = TestSampler::compile(&zero_width).expect("compile zero-width huge repeat");
    assert_eq!(
        sampler.sample_zero_one_with_seed(3, Some(17)),
        vec![Vec::<bool>::new(); 3]
    );
    assert!(sampler.reference_sample().is_empty());

    let nested = Circuit::from_stim_str(
        "REPEAT 64 {\n    REPEAT 1000000000000 {\n        H 0\n        H 0\n    }\n    M 0\n    R 0\n}\n",
    )
    .expect("parse nested excessive work");
    assert_eq!(
        SamplingCompiler::new().compile(&nested),
        Err(SamplingCompileError::ExpandedOperationLimit {
            actual: crate::ResourceAmount::exact(128_000_000_000_128),
            limit: 1_000_000,
        })
    );

    let above_u64 = Circuit::from_stim_str(
        "REPEAT 1000000000000 {\n    REPEAT 1000000000000 {\n        H 0\n    }\n    M 0\n}\n",
    )
    .expect("parse work above u64");
    let error = SamplingCompiler::new()
        .compile(&above_u64)
        .expect_err("work above u64 must retain a truthful lower-bound diagnostic");
    assert_eq!(
        error,
        SamplingCompileError::ExpandedOperationLimit {
            actual: crate::ResourceAmount::from_u128(u128::MAX),
            limit: 1_000_000,
        }
    );
    let detection_error = crate::DetectionError::from(error);
    assert!(matches!(
        detection_error,
        crate::DetectionError::ResourceLimit(_)
    ));
    if let crate::DetectionError::ResourceLimit(detection_limit) = detection_error {
        assert_eq!(detection_limit.actual(), u64::MAX);
        assert!(detection_limit.actual_is_lower_bound());
    }
}

#[test]
fn warmed_fixed_tableau_gate_execution_does_not_allocate_per_dispatch() {
    let mut circuit_text = String::new();
    for gate in Gate::all().filter(|gate| stab_analysis::gate_has_tableau(*gate)) {
        let inverse = gate.inverse().expect("tableau gate inverse");
        let arity = stab_analysis::gate_tableau(gate)
            .expect("gate tableau")
            .len();
        let targets = [(1, "0"), (2, "0 1")]
            .into_iter()
            .find_map(|(candidate, targets)| (candidate == arity).then_some(targets))
            .expect("fixed-tableau gate must have supported arity");
        writeln!(circuit_text, "{} {targets}", gate.canonical_name()).expect("write gate circuit");
        writeln!(circuit_text, "{} {targets}", inverse.canonical_name())
            .expect("write inverse circuit");
    }
    let circuit = Circuit::from_stim_str(&circuit_text).expect("parse gate corpus");
    let sampler = TestSampler::compile(&circuit).expect("compile gate corpus");
    let mut rng = SmallRng::seed_from_u64(29);
    let mut frame = StabilizerFrame::new(sampler.plan.inner.qubit_count);
    let mut record = Vec::with_capacity(sampler.plan.inner.measurement_count);
    let mut output = Vec::with_capacity(sampler.plan.inner.measurement_count);
    sampler
        .plan
        .sample_shot_in_mode_into(
            &mut rng,
            ExecutionMode::Sample,
            &[],
            &mut frame,
            &mut record,
            &mut output,
        )
        .expect("warm gate corpus");

    let one = allocation_counter::measure(|| {
        sampler
            .plan
            .sample_shot_in_mode_into(
                &mut rng,
                ExecutionMode::Sample,
                &[],
                &mut frame,
                &mut record,
                &mut output,
            )
            .expect("execute one warmed gate corpus");
    });
    let many = allocation_counter::measure(|| {
        for _ in 0..256 {
            sampler
                .plan
                .sample_shot_in_mode_into(
                    &mut rng,
                    ExecutionMode::Sample,
                    &[],
                    &mut frame,
                    &mut record,
                    &mut output,
                )
                .expect("execute warmed gate corpus");
        }
    });

    assert_eq!(
        many.count_total, one.count_total,
        "gate dispatch allocation count scaled with repetitions: one={one:?}, many={many:?}"
    );
    assert_eq!(
        many.bytes_total, one.bytes_total,
        "gate dispatch allocation bytes scaled with repetitions: one={one:?}, many={many:?}"
    );
    assert_eq!(
        many.bytes_max, one.bytes_max,
        "gate dispatch peak allocation scaled with repetitions: one={one:?}, many={many:?}"
    );
}

#[test]
fn samples_m8_basic_measurements_as_zeroes() {
    assert_eq!(
        samples(
            include_str!("../../../../oracle/fixtures/inputs/sample_basic.stim"),
            2
        ),
        vec![vec![false, false], vec![false, false]]
    );
}

#[test]
fn samples_single_qubit_clifford_measurements() {
    assert_eq!(samples("H 0\nS 0\nS 0\nH 0\nM 0\n", 3), vec![vec![true]; 3]);

    let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    let first = sampler.sample_zero_one_with_seed(1000, Some(5));
    let second = sampler.sample_zero_one_with_seed(1000, Some(5));
    assert_eq!(first, second);

    let hits = first.iter().filter(|shot| shot == &&vec![true]).count();
    assert!(
        (400..=600).contains(&hits),
        "expected roughly 500 H-basis measurement hits, got {hits}"
    );
}

#[test]
fn can_sample_against_zero_reference_sample() {
    let circuit = Circuit::from_stim_str("H 0\nS 0\nS 0\nH 0\nM 0\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");

    assert_eq!(
        sampler.sample_zero_one_with_seed_and_reference_mode(3, Some(5), false),
        vec![vec![true]; 3]
    );
    assert_eq!(
        sampler.sample_zero_one_with_seed_and_reference_mode(3, Some(5), true),
        vec![vec![false]; 3]
    );
}

#[test]
fn samples_x_and_y_basis_measurements_deterministically() {
    assert_eq!(samples("H 0\nMX 0\n", 1), vec![vec![false]]);
    assert_eq!(samples("X 0\nH 0\nMX 0\n", 1), vec![vec![true]]);
    assert_eq!(samples("H 0\nS 0\nMY 0\n", 1), vec![vec![false]]);
    assert_eq!(samples("H 0\nZ 0\nS 0\nMY 0\n", 1), vec![vec![true]]);
}

#[test]
fn random_basis_measurement_collapses_to_the_measured_basis() {
    let circuit = Circuit::from_stim_str("MX 0\nMX 0\nMY 1\nMY 1\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");

    for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
        assert_eq!(shot.first(), shot.get(1));
        assert_eq!(shot.get(2), shot.get(3));
    }
}

#[test]
fn reset_and_measure_reset_use_their_measurement_basis() {
    assert_eq!(
        samples("RX 0\nMX 0\nRY 1\nMY 1\n", 1),
        vec![vec![false, false]]
    );

    let circuit = Circuit::from_stim_str("MRX 0\nMX 0\nMRY 1\nMY 1\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
        assert_eq!(
            shot.get(1),
            Some(&false),
            "MRX should reset to +X after reporting"
        );
        assert_eq!(
            shot.get(3),
            Some(&false),
            "MRY should reset to +Y after reporting"
        );
    }
}

#[test]
fn measurement_record_feedback_applies_local_paulis() {
    assert_eq!(
        samples("X 0\nM 0\nCX rec[-1] 1\nM 1\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("M 0\nCX rec[-1] 1\nM 1\n", 1),
        vec![vec![false, false]]
    );
    assert_eq!(
        samples("X 0\nM 0\nCY rec[-1] 1\nM 1\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("H 1\nX 0\nM 0\nCZ rec[-1] 1\nMX 1\n", 1),
        vec![vec![true, true]]
    );
}

#[test]
fn active_feedback_crosses_inner_and_outer_repeat_boundaries() {
    let sweep_source = "R 0\nREPEAT 2 {\n    REPEAT 2 {\n        CX sweep[0] 0\n        M 0\n        R 0\n    }\n}\n";
    let sweep_folded = Circuit::from_stim_str(sweep_source).expect("parse nested sweep feedback");
    let sweep_unrolled =
        stab_analysis::flattened_circuit(&sweep_folded).expect("unroll nested sweep feedback");
    for (sweep, expected) in [([false], vec![false; 4]), ([true], vec![true; 4])] {
        let mut folded_record = Vec::new();
        SamplingCompiler::new()
            .compile(&sweep_folded)
            .expect("compile folded sweep feedback")
            .reference_measurement_record_with_sweep_into(&sweep, &mut folded_record)
            .expect("execute folded sweep feedback");
        let mut unrolled_record = Vec::new();
        SamplingCompiler::new()
            .compile(&sweep_unrolled)
            .expect("compile unrolled sweep feedback")
            .reference_measurement_record_with_sweep_into(&sweep, &mut unrolled_record)
            .expect("execute unrolled sweep feedback");
        assert_eq!(folded_record, expected);
        assert_eq!(folded_record, unrolled_record);
    }

    let record_source = "X 0\nM 0\nREPEAT 2 {\n    REPEAT 2 {\n        R 1\n        CX rec[-1] 1\n        M 1\n    }\n}\n";
    let record_folded =
        Circuit::from_stim_str(record_source).expect("parse nested record feedback");
    let record_unrolled =
        stab_analysis::flattened_circuit(&record_folded).expect("unroll nested record feedback");
    let expected = vec![vec![true; 5]];
    assert_eq!(samples(record_source, 1), expected);
    assert_eq!(
        TestSampler::compile(&record_unrolled)
            .expect("compile unrolled record feedback")
            .sample_zero_one_with_seed(1, None),
        expected
    );

    for (source, missing) in [
        (
            "M 0\nREPEAT 2 {\n    REPEAT 2 {\n        CX rec[-2] 1\n        M 1\n    }\n}\n",
            "rec[-2]",
        ),
        (
            "M 0\nREPEAT 2 {\n    REPEAT 2 {\n        M 1\n    }\n    CX rec[-4] 2\n}\n",
            "rec[-4]",
        ),
    ] {
        let error = SamplingCompiler::new()
            .compile(&Circuit::from_stim_str(source).expect("parse invalid nested feedback"))
            .expect_err("first unavailable nested lookback must fail");
        assert!(error.to_string().contains(missing), "{error}");
    }
}

#[test]
fn entangling_clifford_measurements_preserve_bell_correlations() {
    let circuit = Circuit::from_stim_str("H 0\nCX 0 1\nM 0 1\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    let hits = shots
        .iter()
        .filter(|shot| shot.first() == Some(&true))
        .count();
    assert!(
        (400..=600).contains(&hits),
        "expected roughly balanced Bell-pair measurements, got {hits}"
    );
    assert!(
        shots
            .iter()
            .all(|shot| shot.first().copied() == shot.get(1).copied()),
        "Bell-pair measurements should be perfectly correlated"
    );
}

#[test]
fn entangling_measure_reset_collapses_then_resets_only_measured_qubit() {
    let circuit = Circuit::from_stim_str("H 0\nCX 0 1\nMR 0\nM 0 1\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    assert!(
        shots.iter().all(|shot| {
            shot.get(1) == Some(&false) && shot.first().copied() == shot.get(2).copied()
        }),
        "MR should record the Bell collapse, reset qubit 0, and leave qubit 1 collapsed"
    );
}

#[test]
fn qubit_cx_and_feedback_cx_can_coexist() {
    let circuit =
        Circuit::from_stim_str("H 0\nCX 0 1\nM 0\nCX rec[-1] 2\nM 1 2\n").expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    let shots = sampler.sample_zero_one_with_seed(1000, Some(5));

    assert!(
        shots.iter().all(|shot| {
            let Some(measured) = shot.first() else {
                return false;
            };
            shot.get(1) == Some(measured) && shot.get(2) == Some(measured)
        }),
        "qubit CX should create a Bell correlation and feedback CX should read the measurement record"
    );
}

#[test]
fn two_qubit_tableau_gates_act_on_stabilizer_frame() {
    assert_eq!(
        samples("X 0\nSWAP 0 1\nM 0 1\n", 1),
        vec![vec![false, true]]
    );
}

#[test]
fn pair_measurement_inversions_flip_product_results() {
    for shot in samples("MXX 0 1 0 !1 !0 1 !0 !1\n", 100) {
        let first = shot.first().copied().expect("first MXX result");
        assert_eq!(shot, vec![first, !first, !first, first]);
    }
}

#[test]
fn mpp_measures_pauli_products_with_inversions() {
    assert_eq!(
        samples("H 0\nCX 0 1\nMPP X0*X1 Z0*Z1 !Y0*Y1\n", 1),
        vec![vec![false, false, false]]
    );
}

#[test]
fn heralded_pauli_channel_records_and_applies_local_paulis() {
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 0, 0, 0) 0\n", 1),
        vec![vec![false]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(1, 0, 0, 0) 0\nM 0\n", 1),
        vec![vec![true, false]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 1, 0, 0) 0\nM 0\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("HERALDED_PAULI_CHANNEL_1(0, 0, 1, 0) 0\nM 0\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples("H 0\nHERALDED_PAULI_CHANNEL_1(0, 0, 0, 1) 0\nMX 0\n", 1),
        vec![vec![true, true]]
    );
    assert_eq!(
        samples(
            "HERALDED_PAULI_CHANNEL_1(0, 1, 0, 0) 0\nCX rec[-1] 1\nM 0 1\n",
            1
        ),
        vec![vec![true, true, true]]
    );
}

#[test]
fn public_sampler_outputs_include_heralded_measurement_records() {
    let circuit = Circuit::from_stim_str(
        "HERALDED_PAULI_CHANNEL_1(1, 0, 0, 0) 0\nM 0\nHERALDED_ERASE(0) 1\n",
    )
    .expect("parse circuit");
    let sampler = TestSampler::compile(&circuit).expect("compile sampler");
    let expected = vec![true, false, false];

    assert_eq!(sampler.reference_sample(), vec![false, false, false]);
    assert_eq!(
        sampler.sample_zero_one_with_seed(1, Some(5)),
        vec![expected.clone()]
    );
}

#[test]
fn anti_hermitian_mpp_products_are_rejected() {
    let circuit = Circuit::from_stim_str("MPP X0*Z0\n").expect("parse circuit");

    assert_eq!(
        TestSampler::compile(&circuit),
        Err(SamplingCompileError::invalid_circuit(
            "MPP Pauli product is anti-Hermitian"
        ))
    );
}

#[test]
fn correlated_error_branches_match_stim_else_semantics() {
    assert_eq!(
        samples("E(1)\nELSE_CORRELATED_ERROR(1) X0\nM 0\n", 1),
        vec![vec![false]],
        "an empty successful correlated-error branch must suppress its ELSE branch"
    );
    assert_eq!(
        samples(
            "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(0) X1 X2\nELSE_CORRELATED_ERROR(0) X2 X3\nM 0 1 2 3\n",
            1,
        ),
        vec![vec![false, false, false, false]]
    );
    assert_eq!(
        samples(
            "E(1) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\nE(1) X3 X4\nM 0 1 2 3 4\n",
            1,
        ),
        vec![vec![true, true, false, true, true]]
    );
    assert_eq!(
        samples(
            "CORRELATED_ERROR(0) X0 X1\nELSE_CORRELATED_ERROR(1) X1 X2\nELSE_CORRELATED_ERROR(1) X2 X3\nM 0 1 2 3\n",
            1,
        ),
        vec![vec![false, true, true, false]]
    );
}

#[test]
fn rejects_feedback_that_reads_missing_measurements() {
    let circuit = Circuit::from_stim_str("CX rec[-1] 0\n").expect("parse circuit");

    assert_eq!(
        TestSampler::compile(&circuit),
        Err(SamplingCompileError::invalid_circuit(
            "measurement record target rec[-1] is not available while compiling CX feedback"
        ))
    );
}

#[test]
fn count_determined_measurements_matches_unknown_input_subset() {
    assert_eq!(count_determined("MZZ 0 1", false), 1);
    assert_eq!(count_determined("MZZ 0 1", true), 0);
    assert_eq!(count_determined("MPP Z0*Z1 X2*X3", false), 1);
    assert_eq!(count_determined("MPP Z0*Z1 X2*X3", true), 0);
    assert_eq!(
        count_determined(
            "
            MPP Z0*Z1 X2*X3
            TICK
            MPP Z0*Z1 X2*X3
            ",
            true,
        ),
        2
    );
    assert_eq!(
        count_determined(
            "
            MPP Z0*Z1 X2*X3
            TICK
            MPP Z0*Z1 X2*X3
            ",
            false,
        ),
        3
    );
}

#[test]
fn count_determined_measurements_matches_basis_measurement_subset() {
    for (input, expected) in [
        ("", 0),
        ("RX 0\nMX 0", 1),
        ("RX 0\nMRX 0", 1),
        ("RZ 0\nMX 0", 0),
        ("RZ 0\nMRX 0", 0),
        ("RY 0\nMY 0", 1),
        ("RY 0\nMRY 0", 1),
        ("RX 0\nMY 0", 0),
        ("RX 0\nMRY 0", 0),
        ("RZ 0\nMZ 0", 1),
        ("RZ 0\nMRZ 0", 1),
        ("RX 0\nMZ 0", 0),
        ("RX 0\nMRZ 0", 0),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_matches_pair_and_mpp_subset() {
    for (input, expected) in [
        ("RX 0 1\nMXX 0 1", 1),
        ("RY 0 1\nMXX 0 1", 0),
        ("RY 0 1\nMYY 0 1", 1),
        ("RX 0 1\nMYY 0 1", 0),
        ("RZ 0 1\nMZZ 0 1", 1),
        ("RY 0 1\nMZZ 0 1", 0),
        ("RX 0\nMPP X0", 1),
        ("RY 0\nMPP X0", 0),
        ("RY 0\nMPP Y0", 1),
        ("RX 0\nMPP Y0", 0),
        ("RZ 0\nMPP Z0", 1),
        ("RX 0\nMPP Z0", 0),
        ("RX 0\nRY 1\nRZ 2\nMPP X0*Y1*Z2", 1),
        ("RX 0\nRX 1\nRZ 2\nMPP X0*Y1*Z2", 0),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_matches_convergence_subset() {
    for (input, expected) in [
        ("MX 0 0", 1),
        ("MY 0 0", 1),
        ("RX 0\nMZ 0 0", 1),
        ("MRX 0 0", 1),
        ("MRY 0 0", 1),
        ("RX 0\nMRZ 0 0", 1),
        ("MXX 0 1 0 1", 1),
        ("MYY 0 1 0 1", 1),
        ("RX 0 1\nMZZ 0 1 0 1", 1),
        ("MXX 0 1\nMYY 0 1", 1),
        ("MPP X0*X1 Y0*Y1", 1),
        ("MPP X0*X1 X1*X2 !X0*X2", 1),
        ("REPEAT 3 {\nMPP X0*X1\n}", 2),
        ("MXX 0 1\nMX 0 1", 1),
        ("MYY 0 1\nMY 0 1", 1),
        ("RX 0 1\nMZZ 0 1\nMZ 0 1", 1),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_ignores_flip_arguments_like_stim() {
    // Stim strips measurement arguments before counting (count_determined_measurements.inl
    // re-dispatches every measurement with empty args), so noisy flips never change
    // determinism.
    for (input, expected) in [
        ("M(0.5) 0", 1),
        ("R 0\nM(1) 0", 1),
        ("MXX(0.25) 0 1 0 1", 1),
        ("H 0\nM(1) 0", 0),
    ] {
        assert_eq!(count_determined(input, false), expected, "{input}");
    }
}

#[test]
fn count_determined_measurements_rejects_pad_and_heralded_records_like_stim() {
    // Stim's count_determined_measurements throws "unhandled measurement type" for every
    // result-producing gate outside its M/MR/MXX/MPP families, which covers MPAD and the
    // heralded channels.
    for input in [
        "MPAD 1 0 1 0",
        "HERALDED_ERASE(0.25) 0",
        "HERALDED_PAULI_CHANNEL_1(0.1, 0.05, 0, 0) 0",
    ] {
        let circuit = Circuit::from_stim_str(input).expect("parse circuit");
        let error = count_determined_measurements(&circuit, false)
            .expect_err("pad and heralded records must be rejected");
        assert!(
            matches!(
                error,
                CountDeterminedMeasurementsError::Execution(
                    SamplingExecutionError::UnsupportedDeterminedMeasurementGate { .. }
                )
            ),
            "{input}: {error}"
        );
    }
}
