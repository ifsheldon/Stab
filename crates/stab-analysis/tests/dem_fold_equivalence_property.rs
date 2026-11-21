#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "property tests index fixed generator tables with in-range picks and report generated circuits through direct assertion diagnostics"
)]

//! Loop folding must be a pure representation change: for any circuit, the folded DEM flattens to
//! the same independent-error distribution, detector coordinates, and detector/observable counts
//! as the unfolded DEM, and both modes accept or reject the same circuits.

use std::fmt::Write as _;

use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, RngAlgorithm, RngSeed};
use stab_analysis::{ErrorAnalyzerOptions, LoopFolding, circuit_to_detector_error_model};
use stab_model::Circuit;

#[path = "support/dem_semantics.rs"]
mod dem_semantics;

use dem_semantics::{ResolvedDem, assert_same_distribution};

const QUBITS: usize = 4;
const PROBABILITIES: [&str; 4] = ["0.001", "0.01", "0.125", "0.25"];
const OUTER_REPEATS: [u64; 4] = [2, 9, 23, 40];
const INNER_REPEATS: [u64; 4] = [1, 2, 4, 9];

/// Deterministic choice source; an exhausted tape yields the simplest choices so shrinking the
/// tape shrinks the circuit.
struct Tape<'a> {
    bytes: &'a [u8],
}

impl Tape<'_> {
    fn pick(&mut self, choices: usize) -> usize {
        match self.bytes.split_first() {
            Some((&byte, rest)) => {
                self.bytes = rest;
                usize::from(byte) % choices
            }
            None => 0,
        }
    }

    fn pick_from<'b, T>(&mut self, choices: &'b [T]) -> &'b T {
        &choices[self.pick(choices.len())]
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Basis {
    Z,
    X,
}

/// Generates circuits whose every measurement is deterministic in the noiseless circuit.
///
/// Each qubit is prepared in an eigenstate of a fixed basis and only ever measured in that basis
/// (single-qubit or two-qubit `MPP` products of same-basis qubits); all other Clifford gates
/// appear as `U ... U^-1` sandwiches with noise inside. Every detector or observable over any set
/// of in-range records is therefore deterministic, so analyzer rejection can only come from
/// deliberately out-of-range record lookbacks.
struct Generator<'a> {
    tape: Tape<'a>,
    basis: [Basis; QUBITS],
    text: String,
    /// Measurements guaranteed to exist at this point during the first pass.
    measurements: u64,
}

impl Generator<'_> {
    fn line(&mut self, indent: usize, line: &str) {
        let _ = writeln!(self.text, "{:indent$}{line}", "", indent = indent * 4);
    }

    fn qubit(&mut self) -> usize {
        self.tape.pick(QUBITS)
    }

    fn noise(&mut self, indent: usize) {
        let probability = *self.tape.pick_from(&PROBABILITIES);
        let a = self.qubit();
        let line = match self.tape.pick(4) {
            0 => format!("X_ERROR({probability}) {a}"),
            1 => format!("Z_ERROR({probability}) {a}"),
            2 => format!("DEPOLARIZE1({probability}) {a}"),
            _ => {
                let b = (a + 1 + self.tape.pick(QUBITS - 1)) % QUBITS;
                format!("DEPOLARIZE2({probability}) {a} {b}")
            }
        };
        self.line(indent, &line);
    }

    fn clifford_sandwich(&mut self, indent: usize) {
        let mut gates = Vec::new();
        for _ in 0..=self.tape.pick(3) {
            let a = self.qubit();
            gates.push(match self.tape.pick(3) {
                0 => (format!("H {a}"), format!("H {a}")),
                1 => (format!("S {a}"), format!("S_DAG {a}")),
                _ => {
                    let b = (a + 1 + self.tape.pick(QUBITS - 1)) % QUBITS;
                    (format!("CX {a} {b}"), format!("CX {a} {b}"))
                }
            });
        }
        for (forward, _) in &gates {
            self.line(indent, forward);
        }
        self.noise(indent);
        for (_, inverse) in gates.iter().rev() {
            self.line(indent, inverse);
        }
    }

    fn measurement(&mut self, indent: usize) {
        let a = self.qubit();
        if self.tape.pick(2) == 0 {
            let probability = *self.tape.pick_from(&PROBABILITIES);
            let flip = if self.basis[a] == Basis::Z { 'X' } else { 'Z' };
            self.line(indent, &format!("{flip}_ERROR({probability}) {a}"));
        }
        let line = match (self.tape.pick(3), self.basis[a]) {
            (0, Basis::Z) => format!("MR {a}"),
            (0, Basis::X) => format!("MRX {a}"),
            (1, basis) => {
                let partner = (0..QUBITS).find(|&b| b != a && self.basis[b] == basis);
                let pauli = if basis == Basis::Z { 'Z' } else { 'X' };
                match partner {
                    Some(b) => format!("MPP {pauli}{a}*{pauli}{b}"),
                    None => format!("MPP {pauli}{a}"),
                }
            }
            (_, Basis::Z) => format!("M {a}"),
            (_, Basis::X) => format!("MX {a}"),
        };
        self.line(indent, &line);
        self.measurements += 1;
    }

    /// A record lookback that is in range on the first pass, except for a rare deliberate
    /// out-of-range lookback that both fold modes must reject.
    fn lookback(&mut self) -> Option<u64> {
        if self.measurements == 0 {
            return None;
        }
        if self.tape.pick(32) == 31 {
            return Some(self.measurements + 1);
        }
        let range = usize::try_from(self.measurements.min(12)).expect("small lookback range");
        Some(1 + u64::try_from(self.tape.pick(range)).expect("small lookback"))
    }

    fn detector(&mut self, indent: usize) {
        let mut records = String::new();
        for _ in 0..=self.tape.pick(2) {
            if let Some(lookback) = self.lookback() {
                let _ = write!(records, " rec[-{lookback}]");
            }
        }
        let x = self.tape.pick(3);
        let y = self.tape.pick(3);
        self.line(indent, &format!("DETECTOR({x}, {y}){records}"));
    }

    fn observable(&mut self, indent: usize) {
        if let Some(lookback) = self.lookback() {
            let observable = self.tape.pick(2);
            self.line(
                indent,
                &format!("OBSERVABLE_INCLUDE({observable}) rec[-{lookback}]"),
            );
        }
    }

    fn body(&mut self, indent: usize, depth: usize) {
        let first_measurement = self.measurements;
        for _ in 0..2 + self.tape.pick(8) {
            match self.tape.pick(9) {
                0 | 1 => self.noise(indent),
                2 => self.clifford_sandwich(indent),
                3 | 4 => self.measurement(indent),
                5 => self.detector(indent),
                6 => self.observable(indent),
                7 => {
                    let dx = self.tape.pick(2);
                    self.line(indent, &format!("SHIFT_COORDS({dx}, 1)"));
                }
                _ if depth < 2 => self.repeat(indent, depth + 1, &INNER_REPEATS),
                _ => self.noise(indent),
            }
        }
        // Close every iteration with a measurement compared against the previous iteration's
        // record, so each loop carries a cross-iteration detector.
        self.measurement(indent);
        let per_iteration = self.measurements - first_measurement;
        if self.measurements > per_iteration {
            self.line(
                indent,
                &format!("DETECTOR rec[-1] rec[-{}]", per_iteration + 1),
            );
        }
    }

    fn repeat(&mut self, indent: usize, depth: usize, counts: &[u64]) {
        let count = *self.tape.pick_from(counts);
        let before = self.measurements;
        self.line(indent, &format!("REPEAT {count} {{"));
        self.body(indent + 1, depth);
        self.line(indent, "}");
        let per_iteration = self.measurements - before;
        self.measurements = before + per_iteration * count;
    }

    fn circuit(bytes: &[u8]) -> String {
        let mut generator = Generator {
            tape: Tape { bytes },
            basis: [Basis::Z; QUBITS],
            text: String::new(),
            measurements: 0,
        };
        for qubit in 0..QUBITS {
            let basis = if generator.tape.pick(2) == 0 {
                Basis::Z
            } else {
                Basis::X
            };
            generator.basis[qubit] = basis;
            let reset = if basis == Basis::Z { "R" } else { "RX" };
            generator.line(0, &format!("{reset} {qubit}"));
        }
        generator.measurement(0);
        generator.repeat(0, 0, &OUTER_REPEATS);
        for _ in 0..generator.tape.pick(3) {
            generator.measurement(0);
            generator.detector(0);
        }
        generator.observable(0);
        generator.text
    }
}

fn assert_fold_modes_agree(text: &str) {
    let circuit = Circuit::from_stim_str(text).expect("generated circuit parses");
    let analyze = |fold_loops| {
        circuit_to_detector_error_model(
            &circuit,
            &ErrorAnalyzerOptions {
                loops: if fold_loops {
                    LoopFolding::Fold
                } else {
                    LoopFolding::Unroll
                },
                ..ErrorAnalyzerOptions::default()
            },
        )
    };
    match (analyze(true), analyze(false)) {
        (Ok(folded), Ok(unfolded)) => {
            let folded_view = ResolvedDem::new(&folded);
            let unfolded_view = ResolvedDem::new(&unfolded);
            assert_same_distribution(
                &folded_view.symptom_distribution(),
                &unfolded_view.symptom_distribution(),
                1e-12,
                &format!("folded vs unfolded\n{text}"),
            );
            assert_eq!(
                folded.count_detectors().expect("folded detector count"),
                unfolded.count_detectors().expect("unfolded detector count"),
                "{text}"
            );
            assert_eq!(
                folded.count_observables().expect("folded observable count"),
                unfolded
                    .count_observables()
                    .expect("unfolded observable count"),
                "{text}"
            );
            // Coordinate-free `detector` declarations are a representation choice (pinned Stim
            // declares error-free detectors per folded period), so only coordinates are compared.
            let coordinates = |view: &ResolvedDem| {
                view.detector_coordinates
                    .iter()
                    .filter(|(_, coordinates)| !coordinates.is_empty())
                    .map(|(detector, coordinates)| (*detector, coordinates.clone()))
                    .collect::<Vec<_>>()
            };
            let folded_coordinates = coordinates(&folded_view);
            let unfolded_coordinates = coordinates(&unfolded_view);
            assert_eq!(
                folded_coordinates.len(),
                unfolded_coordinates.len(),
                "{text}"
            );
            for ((detector, actual), (expected_detector, expected)) in
                folded_coordinates.iter().zip(&unfolded_coordinates)
            {
                assert_eq!(detector, expected_detector, "{text}");
                assert_eq!(actual.len(), expected.len(), "D{detector}\n{text}");
                for (actual, expected) in actual.iter().zip(expected) {
                    assert!((actual - expected).abs() <= 1e-9, "D{detector}\n{text}");
                }
            }
        }
        (Err(folded), Err(unfolded)) => assert_eq!(
            std::mem::discriminant(&folded),
            std::mem::discriminant(&unfolded),
            "{folded} vs {unfolded}\n{text}"
        ),
        (folded, unfolded) => panic!(
            "fold modes disagree on acceptance: folded {:?}, unfolded {:?}\n{text}",
            folded.map(|dem| dem.to_dem_string()),
            unfolded.map(|dem| dem.to_dem_string())
        ),
    }
}

proptest! {
    // Fixed seed: the case set is reproducible across runs and machines.
    #![proptest_config(ProptestConfig {
        cases: 64,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(0xf01d_100b_5eed_0001),
        failure_persistence: Some(Box::new(FileFailurePersistence::Off)),
        ..ProptestConfig::default()
    })]

    #[test]
    fn folded_dem_flattens_to_unfolded_dem(tape in prop::collection::vec(any::<u8>(), 0..160)) {
        assert_fold_modes_agree(&Generator::circuit(&tape));
    }
}
