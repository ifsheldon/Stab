#![allow(
    clippy::expect_used,
    reason = "sampling unit tests use direct fixture parsing assertions for compact diagnostics"
)]

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
    sampler.plan.sample_shot_in_mode_into(
        &mut rng,
        ExecutionMode::Sample,
        &[],
        &mut frame,
        &mut record,
        &mut output,
    );

    let one = allocation_counter::measure(|| {
        sampler.plan.sample_shot_in_mode_into(
            &mut rng,
            ExecutionMode::Sample,
            &[],
            &mut frame,
            &mut record,
            &mut output,
        );
    });
    let many = allocation_counter::measure(|| {
        for _ in 0..256 {
            sampler.plan.sample_shot_in_mode_into(
                &mut rng,
                ExecutionMode::Sample,
                &[],
                &mut frame,
                &mut record,
                &mut output,
            );
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
