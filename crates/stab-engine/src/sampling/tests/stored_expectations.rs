//! Stored expectations recorded from the earlier implementation, which compiled a separate
//! per-target `SampleProgram` beside the frame program.
//!
//! The reference-sample and general-frame digests below were computed by that implementation over
//! the corpus built here, and keep both falsifiable after the per-target program was retired.
//!
//! The determined-measurement digest is the one exception. That implementation's generator-list count was
//! wrong in unknown-input mode for three corpus circuits that reset every qubit before measuring
//! (it reported 52, 15, and 12 where the known-input counts are 57, 22, and 17), although a
//! circuit that resets every qubit first cannot depend on its input state. Pinned Stim entangles
//! each qubit with a fresh partner (`count_determined_measurements.inl`), as the inverse-tableau
//! count now does, so the digest below records the corrected counts and
//! `unknown_input_counts_equal_known_input_counts_after_full_resets` pins the property.

use rand::RngExt as _;
use sha2::{Digest as _, Sha256};
use stab_analysis::{ColorCodeTask, RepetitionCodeTask};

use super::reference_differential::random_clifford_circuit;
use super::*;

const NOISE_AND_FEEDBACK: &str = "\
R 0 1 2 3 4 5
RX 6
RY 7
H 0 2
S 1
CX 0 1 2 3
X_ERROR(0.1) 0 1
Y_ERROR(0.05) 2
Z_ERROR(0.2) 3 4
DEPOLARIZE1(0.15) 5 6 7
DEPOLARIZE2(0.1) 0 1 2 3
PAULI_CHANNEL_1(0.01, 0.02, 0.03) 4 5
PAULI_CHANNEL_2(0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01, 0.01) 6 7
E(0.2) X0 Z1
ELSE_CORRELATED_ERROR(0.3) Y2 X3
ELSE_CORRELATED_ERROR(0.4) Z4
HERALDED_ERASE(0.25) 0 5
HERALDED_PAULI_CHANNEL_1(0.05, 0.1, 0.15, 0.2) 1 6
M(0.05) 0 !1
MX(0.1) 2
MY 3
MR !4
MRX(0.02) 5
MRY 6
MXX 0 1
MYY(0.1) !2 3
MZZ 4 5
MPP(0.05) X0*Z1 !Y2*Y3
MPAD 0 1
MPAD(0.3) 1
CX rec[-1] 0
CY rec[-3] 1
CZ rec[-2] 2
XCZ 3 rec[-4]
YCZ 4 rec[-5]
SPP X0*Y1
SPP_DAG Z2*Z3
SQRT_XX 0 1
ISWAP 2 3
REPEAT 7 {
    DEPOLARIZE2(0.05) 0 1
    CX 0 1
    H 2
    M 0 1 2
    CX rec[-1] 3
    DETECTOR rec[-1] rec[-4]
}
M 0 1 2 3 4 5 6 7
";

fn corpus() -> Vec<String> {
    let mut circuits = vec![NOISE_AND_FEEDBACK.to_owned()];
    let probability = Probability::try_new(0.01).expect("probability");
    let rounds = RoundCount::try_new(7).expect("rounds");
    let distance = CodeDistance::try_new(3).expect("distance");
    for task in [
        SurfaceCodeTask::RotatedMemoryX,
        SurfaceCodeTask::RotatedMemoryZ,
        SurfaceCodeTask::UnrotatedMemoryX,
    ] {
        let params = CodeCircuitParams::try_new(CodeTask::Surface(task), distance, rounds)
            .expect("surface parameters")
            .with_before_round_data_depolarization(probability)
            .with_before_measure_flip_probability(probability)
            .with_after_reset_flip_probability(probability)
            .with_after_clifford_depolarization(probability);
        circuits.push(
            params
                .generate()
                .expect("surface circuit")
                .circuit()
                .to_stim_string(),
        );
    }
    let color =
        CodeCircuitParams::try_new(CodeTask::Color(ColorCodeTask::MemoryXyz), distance, rounds)
            .expect("color parameters")
            .with_after_clifford_depolarization(probability);
    circuits.push(
        color
            .generate()
            .expect("color circuit")
            .circuit()
            .to_stim_string(),
    );
    let repetition = CodeCircuitParams::try_new(
        CodeTask::Repetition(RepetitionCodeTask::Memory),
        distance,
        rounds,
    )
    .expect("repetition parameters")
    .with_before_measure_flip_probability(probability);
    circuits.push(
        repetition
            .generate()
            .expect("repetition circuit")
            .circuit()
            .to_stim_string(),
    );
    let mut rng = SmallRng::seed_from_u64(0x0E3E_0E3E);
    for qubits in [1_usize, 2, 3, 7, 64, 65, 130] {
        for _ in 0..2 {
            let mut text = random_clifford_circuit(&mut rng, qubits);
            if rng.random_bool(0.5) {
                text.push_str("X_ERROR(0.5) 0\nM 0\n");
            }
            circuits.push(text);
        }
    }
    circuits
}

fn update_bits(hasher: &mut Sha256, label: &str, bits: &[bool]) {
    hasher.update(label.as_bytes());
    hasher.update((bits.len() as u64).to_le_bytes());
    hasher.update(bits.iter().map(|bit| u8::from(*bit)).collect::<Vec<_>>());
}

#[test]
fn reference_samples_match_the_pre_e3_stored_digest() {
    let mut hasher = Sha256::new();
    for text in corpus() {
        let circuit = Circuit::from_stim_str(&text).expect("parse corpus circuit");
        for policy in [
            ReferenceSampleLoopPolicy::Fold,
            ReferenceSampleLoopPolicy::Iterate,
        ] {
            let plan = SamplingCompiler::new()
                .reference_sample_loop_policy(policy)
                .compile(&circuit)
                .expect("compile corpus circuit");
            let reference = bits(&plan.try_reference_sample().expect("reference sample"));
            update_bits(&mut hasher, "reference", &reference);
        }
    }
    assert_eq!(
        hex::encode(hasher.finalize()),
        "0dd4755ce48b9056c688c7a584d1fdd967c18904a3977d0ae40daad469358e48"
    );
}

#[test]
fn determined_measurement_counts_match_the_pre_e3_stored_digest() {
    let mut hasher = Sha256::new();
    for text in corpus() {
        let circuit = Circuit::from_stim_str(&text).expect("parse corpus circuit");
        for initial_state in [InitialState::Zero, InitialState::Unknown] {
            let outcome = match count_determined_measurements(&circuit, initial_state) {
                Ok(count) => format!("ok:{count}"),
                Err(error) => format!("err:{error}"),
            };
            hasher.update(outcome.as_bytes());
        }
    }
    assert_eq!(
        hex::encode(hasher.finalize()),
        "c381f5f28d84cf9dc7a8c8bb24d5171d96a316d3218e569726978c309a500178"
    );
}

#[test]
fn unknown_input_counts_equal_known_input_counts_after_full_resets() {
    for (index, text) in corpus().into_iter().enumerate().skip(1).take(6) {
        let circuit = Circuit::from_stim_str(&text).expect("parse generated circuit");
        let known =
            count_determined_measurements(&circuit, InitialState::Zero).expect("known-input count");
        let unknown = count_determined_measurements(&circuit, InitialState::Unknown)
            .expect("unknown-input count");
        assert!(known > 0, "corpus circuit {index}");
        assert_eq!(unknown, known, "corpus circuit {index}:\n{text}");
    }
}
