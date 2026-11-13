//! Statistical contract for the sparse categorical noise walk.
//!
//! `visit_sparse_categorical_events` replaces per-target categorical masks whenever a channel's
//! total probability is at most 0.125. These tests drive it through both frame word widths that
//! use it in production: the 256-lane `BitBlock` measurement-sampling frame and the 64-lane `u64`
//! detection frame, including a partial final detection batch.
//!
//! Each data qubit is half of a Bell pair that is decoded after the noise, so its measured flip
//! pair identifies the applied Pauli exactly: a Z component flips the data measurement and an X
//! component flips the ancilla measurement. Expected frequencies come from the circuit text in
//! Stim argument order, never from the engine's own category tables.
//!
//! Every comparison is a two-sided binomial bound at 5 sigma without an absolute floor. The 200
//! comparisons per run have a combined re-seeding false-positive rate below 2e-4; the seeds are
//! fixed, so a failure is deterministic.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "statistical frame tests index fixed-width tallies and fail immediately on fixture errors"
)]

use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_model::Circuit;

use super::{DirectDetectorFramePlan, PauliFrameSamplingPlan};
use crate::detection::DetectionConversionLimits;

const DATA_QUBITS: usize = 50;
const SIGMA: f64 = 5.0;
const PAULI_CHANNEL_1: [f64; 3] = [0.006, 0.012, 0.018];
/// `PAULI_CHANNEL_2` arguments in Stim order (IX, IY, IZ, XI, ..., ZZ), totalling 0.12.
const PAULI_CHANNEL_2: [f64; 15] = [
    0.001, 0.002, 0.003, 0.004, 0.005, 0.006, 0.007, 0.008, 0.009, 0.010, 0.011, 0.012, 0.013,
    0.014, 0.015,
];
const BIT_BLOCK_BATCHES: usize = 32;
const U64_FULL_BATCHES: usize = 128;
const U64_FINAL_LANES: usize = 37;

/// Noise channel under test; categories index Paulis as 0=I, 1=X, 2=Y, 3=Z per qubit, which
/// is Stim's channel argument order once the identity category is dropped.
#[derive(Clone, Copy)]
enum Channel {
    Single,
    Pair,
}

impl Channel {
    const fn item_count(self) -> usize {
        match self {
            Self::Single => DATA_QUBITS,
            Self::Pair => DATA_QUBITS / 2,
        }
    }

    const fn category_count(self) -> usize {
        match self {
            Self::Single => 4,
            Self::Pair => 16,
        }
    }

    fn expected_category_probability(self, category: usize) -> f64 {
        let probabilities: &[f64] = match self {
            Self::Single => &PAULI_CHANNEL_1,
            Self::Pair => &PAULI_CHANNEL_2,
        };
        if category == 0 {
            1.0 - probabilities.iter().sum::<f64>()
        } else {
            probabilities[category - 1]
        }
    }

    fn circuit(self) -> Circuit {
        let data = (0..DATA_QUBITS)
            .map(|qubit| qubit.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let pairs = (0..DATA_QUBITS)
            .map(|qubit| format!("{qubit} {}", qubit + DATA_QUBITS))
            .collect::<Vec<_>>()
            .join(" ");
        let measured = (0..2 * DATA_QUBITS)
            .map(|qubit| qubit.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let arguments = match self {
            Self::Single => PAULI_CHANNEL_1.map(|p| p.to_string()).join(","),
            Self::Pair => PAULI_CHANNEL_2.map(|p| p.to_string()).join(","),
        };
        let gate = match self {
            Self::Single => "PAULI_CHANNEL_1",
            Self::Pair => "PAULI_CHANNEL_2",
        };
        Circuit::from_stim_str(&format!(
            "H {data}\nCX {pairs}\n{gate}({arguments}) {data}\nCX {pairs}\nH {data}\nM {measured}\n"
        ))
        .expect("parse Bell-decoded noise circuit")
    }
}

/// Selects the lanes whose decoded single-qubit Pauli equals `pauli`.
const fn pauli_lanes(pauli: usize, x: u64, z: u64) -> u64 {
    match pauli {
        0 => !x & !z,
        1 => x & !z,
        2 => x & z,
        _ => !x & z,
    }
}

struct Tally {
    channel: Channel,
    categories: Vec<u64>,
    item_errors: Vec<u64>,
    lane_word_errors: Vec<u64>,
    lane_word_trials: Vec<u64>,
    shots: u64,
}

impl Tally {
    fn new(channel: Channel, lane_words: usize) -> Self {
        Self {
            channel,
            categories: vec![0; channel.category_count()],
            item_errors: vec![0; channel.item_count()],
            lane_word_errors: vec![0; lane_words],
            lane_word_trials: vec![0; lane_words],
            shots: 0,
        }
    }

    /// Records one 64-lane slice; `measurement(m)` is that slice of measurement `m`'s flips.
    fn add_lane_word(&mut self, lane_word: usize, active: u64, measurement: impl Fn(usize) -> u64) {
        let x = |qubit: usize| measurement(qubit + DATA_QUBITS) & active;
        let z = |qubit: usize| measurement(qubit) & active;
        for item in 0..self.channel.item_count() {
            let error_lanes = match self.channel {
                Channel::Single => {
                    for (category, count) in self.categories.iter_mut().enumerate() {
                        *count += u64::from(
                            (pauli_lanes(category, x(item), z(item)) & active).count_ones(),
                        );
                    }
                    x(item) | z(item)
                }
                Channel::Pair => {
                    let (left, right) = (2 * item, 2 * item + 1);
                    for (category, count) in self.categories.iter_mut().enumerate() {
                        let lanes = pauli_lanes(category / 4, x(left), z(left))
                            & pauli_lanes(category % 4, x(right), z(right))
                            & active;
                        *count += u64::from(lanes.count_ones());
                    }
                    x(left) | z(left) | x(right) | z(right)
                }
            };
            self.item_errors[item] += u64::from(error_lanes.count_ones());
            self.lane_word_errors[lane_word] += u64::from(error_lanes.count_ones());
            self.lane_word_trials[lane_word] += u64::from(active.count_ones());
        }
    }

    fn assert_matches(&self, path: &str) {
        let items = self.channel.item_count() as u64;
        for (category, &count) in self.categories.iter().enumerate() {
            assert_binomial(
                &format!("{path} category {category}"),
                count,
                items * self.shots,
                self.channel.expected_category_probability(category),
            );
        }
        let error_probability = 1.0 - self.channel.expected_category_probability(0);
        for (item, &count) in self.item_errors.iter().enumerate() {
            assert_binomial(
                &format!("{path} target group {item}"),
                count,
                self.shots,
                error_probability,
            );
        }
        for (lane_word, (&count, &trials)) in self
            .lane_word_errors
            .iter()
            .zip(&self.lane_word_trials)
            .enumerate()
        {
            assert_binomial(
                &format!("{path} lane word {lane_word}"),
                count,
                trials,
                error_probability,
            );
        }
    }
}

#[allow(
    clippy::cast_precision_loss,
    reason = "trial counts stay far below f64's exact integer range"
)]
fn assert_binomial(label: &str, successes: u64, trials: u64, probability: f64) {
    let expected = trials as f64 * probability;
    let tolerance = SIGMA * (trials as f64 * probability * (1.0 - probability)).sqrt();
    assert!(
        (successes as f64 - expected).abs() <= tolerance,
        "{label}: observed {successes} of {trials}, expected {expected:.2} +/- {tolerance:.2}"
    );
}

#[test]
fn bit_block_sampling_frame_sparse_channels_match_category_target_and_lane_marginals() {
    for (channel, seed) in [(Channel::Single, 0x5A11), (Channel::Pair, 0x5A12)] {
        let circuit = channel.circuit();
        let plan = PauliFrameSamplingPlan::try_compile(&circuit, 2 * DATA_QUBITS, 1 << 30)
            .expect("compile Pauli-frame sampling plan");
        let mut state = plan.state().expect("allocate Pauli-frame state");
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut tally = Tally::new(channel, 4);
        for _ in 0..BIT_BLOCK_BATCHES {
            plan.sample_batch(&mut state, &mut rng)
                .expect("sample BitBlock batch");
            for lane_word in 0..4 {
                tally.add_lane_word(lane_word, u64::MAX, |measurement| {
                    plan.measurement_segment(&state, measurement, lane_word * 64, 64)
                        .expect("measurement segment")
                });
            }
            tally.shots += 256;
        }
        tally.assert_matches("BitBlock sampling frame");
    }
}

#[test]
fn u64_detection_frame_sparse_channels_match_marginals_and_never_touch_inactive_lanes() {
    for (channel, seed) in [(Channel::Single, 0xD311), (Channel::Pair, 0xD312)] {
        let circuit = channel.circuit();
        let plan = DirectDetectorFramePlan::compile(&circuit, DetectionConversionLimits::default())
            .expect("compile direct detector-frame plan");
        let mut state = plan.state().expect("allocate detector-frame state");
        let mut rng = SmallRng::seed_from_u64(seed);
        let mut tally = Tally::new(channel, 1);
        for lanes in std::iter::repeat_n(64, U64_FULL_BATCHES).chain([U64_FINAL_LANES]) {
            plan.sample_batch(&mut state, &mut rng, lanes)
                .expect("sample u64 batch");
            let active = super::super::batch_active_mask(lanes);
            let measurements = &state.frame.measurements;
            assert_eq!(measurements.len(), 2 * DATA_QUBITS);
            assert!(
                measurements.iter().all(|word| word & !active == 0),
                "{lanes}-lane batch flipped an inactive lane"
            );
            tally.add_lane_word(0, active, |measurement| measurements[measurement]);
            tally.shots += lanes as u64;
        }
        tally.assert_matches("u64 detection frame");
    }
}
