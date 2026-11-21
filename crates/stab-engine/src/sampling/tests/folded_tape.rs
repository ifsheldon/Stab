//! Folded and unrolled repeat blocks must be indistinguishable through the unified program tape.
//!
//! Every executor walks repeats through one cursor, so a circuit and its textual unrolling must
//! admit the same counts and produce the same seeded Pauli-frame stream, reference sample,
//! general-frame stream, and detection stream. Only the folded form may retain fewer entries.

use rand::RngExt as _;
use stab_records::{DetectionBatchView, DetectionSink};

use super::reference_differential::RandomCliffordEmitter;
use super::*;
use crate::{
    DetectionSamplingCompiler, DetectionSamplingSessionOptions, detection::frame::ProgramOptions,
};

#[derive(Default)]
struct DetectionBits {
    bits: Vec<bool>,
}

impl DetectionSink for DetectionBits {
    type Error = std::convert::Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        for shot in 0..batch.shot_count() {
            for bit in 0..batch.detector_width().get() {
                self.bits.push(
                    batch
                        .detectors()
                        .get(ShotIndex(shot), BitIndex(bit))
                        .expect("detector bit"),
                );
            }
            for bit in 0..batch.observable_width().get() {
                self.bits.push(
                    batch
                        .observables()
                        .get(ShotIndex(shot), BitIndex(bit))
                        .expect("observable bit"),
                );
            }
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A random noisy body with record feedback, detectors, and an observable.
fn random_body(rng: &mut SmallRng, emitter: &mut RandomCliffordEmitter, qubits: usize) -> String {
    let mut body = String::new();
    emitter.emit(&mut body, rng, qubits, 12);
    let qubit = rng.random_range(0..qubits);
    writeln!(body, "DEPOLARIZE1(0.1) {qubit}").expect("write noise");
    writeln!(body, "PAULI_CHANNEL_2(0.01, 0.02, 0.03, 0.01, 0.02, 0.03, 0.01, 0.02, 0.03, 0.01, 0.02, 0.03, 0.01, 0.02, 0.03) {qubit} {}", (qubit + 1) % qubits).expect("write noise");
    writeln!(body, "MR(0.05) {qubit}").expect("write measurement");
    body.push_str("DETECTOR rec[-1]\n");
    writeln!(body, "HERALDED_ERASE(0.1) {qubit}").expect("write herald");
    writeln!(body, "E(0.2) X{qubit}\nELSE_CORRELATED_ERROR(0.3) Z{qubit}").expect("write error");
    body.push_str("OBSERVABLE_INCLUDE(0) rec[-1]\n");
    body
}

fn general_stream(plan: &SamplingPlan, seed: u64) -> Vec<bool> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut frame = StabilizerFrame::try_new(plan.qubit_count()).expect("general frame");
    let (mut record, mut output, mut stream) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..5 {
        plan.sample_shot_in_mode_into(
            &mut rng,
            ExecutionMode::Sample,
            &[],
            &mut frame,
            &mut record,
            &mut output,
        )
        .expect("general-frame shot");
        stream.extend_from_slice(&output);
    }
    stream
}

fn detection_stream(circuit: &Circuit, seed: u64) -> Vec<bool> {
    let plan = DetectionSamplingCompiler::default()
        .compile(circuit)
        .expect("compile detection plan");
    let mut session = plan
        .session(&DetectionSamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(seed)),
            ..Default::default()
        })
        .expect("detection session");
    let mut sink = DetectionBits::default();
    session
        .run(ShotCount::new(130), &mut sink)
        .expect("run detection");
    sink.bits
}

#[test]
fn folded_and_unrolled_repeats_execute_identically_on_every_program_walker() {
    let mut rng = SmallRng::seed_from_u64(0xF01D);
    for (case, (qubits, outer, inner)) in [(3_usize, 4_u64, 3_u64), (6, 2, 5), (9, 3, 2)]
        .into_iter()
        .enumerate()
    {
        let mut emitter = RandomCliffordEmitter::new();
        let mut prefix = String::new();
        emitter.emit(&mut prefix, &mut rng, qubits, 10);
        prefix.push_str("M 0\n");
        let head = random_body(&mut rng, &mut emitter, qubits);
        let nested = random_body(&mut rng, &mut emitter, qubits);
        let folded = format!(
            "{prefix}REPEAT {outer} {{\n{head}REPEAT {inner} {{\n{nested}}}\nTICK\n}}\nM 0 1\n"
        );
        let mut unrolled = prefix.clone();
        for _ in 0..outer {
            unrolled.push_str(&head);
            for _ in 0..inner {
                unrolled.push_str(&nested);
            }
            unrolled.push_str("TICK\n");
        }
        unrolled.push_str("M 0 1\n");

        let folded_circuit = Circuit::from_stim_str(&folded).expect("parse folded circuit");
        let unrolled_circuit = Circuit::from_stim_str(&unrolled).expect("parse unrolled circuit");
        let label = format!("case {case}:\n{folded}");

        for options in [ProgramOptions::SAMPLING, ProgramOptions::DETECTION] {
            let folded_program =
                CircuitProgram::compile(&folded_circuit, options).expect("compile folded program");
            let unrolled_program = CircuitProgram::compile(&unrolled_circuit, options)
                .expect("compile unrolled program");
            assert_eq!(
                folded_program.counts(),
                unrolled_program.counts(),
                "{label}"
            );
            assert_eq!(
                folded_program.frame_qubit_count(),
                unrolled_program.frame_qubit_count(),
                "{label}"
            );
            assert!(folded_program.entry_count() < unrolled_program.entry_count());
        }

        let folded_plan = SamplingCompiler::default()
            .compile(&folded_circuit)
            .expect("compile folded plan");
        let unrolled_plan = SamplingCompiler::default()
            .compile(&unrolled_circuit)
            .expect("compile unrolled plan");
        let seed = 0xE3 + case as u64;
        for skip in [false, true] {
            let folded_sampler = TestSampler {
                plan: folded_plan.clone(),
            };
            let unrolled_sampler = TestSampler {
                plan: unrolled_plan.clone(),
            };
            assert_eq!(
                folded_sampler.sample_zero_one_with_seed_and_reference_mode(130, Some(seed), skip),
                unrolled_sampler.sample_zero_one_with_seed_and_reference_mode(
                    130,
                    Some(seed),
                    skip
                ),
                "Pauli-frame stream, skip={skip}, {label}"
            );
        }
        assert_eq!(
            folded_plan
                .try_reference_sample()
                .expect("folded reference"),
            unrolled_plan
                .try_reference_sample()
                .expect("unrolled reference"),
            "{label}"
        );
        assert_eq!(
            general_stream(&folded_plan, seed),
            general_stream(&unrolled_plan, seed),
            "{label}"
        );
        assert_eq!(
            detection_stream(&folded_circuit, seed),
            detection_stream(&unrolled_circuit, seed),
            "{label}"
        );
        for initial_state in [InitialState::Zero, InitialState::Unknown] {
            // Heralded noise makes Stim's count reject the circuit; both forms must agree.
            assert_eq!(
                folded_plan.try_count_determined_measurements(initial_state),
                unrolled_plan.try_count_determined_measurements(initial_state),
                "{label}"
            );
        }
    }
}
