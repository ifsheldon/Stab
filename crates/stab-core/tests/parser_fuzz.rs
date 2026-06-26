#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "fuzz smoke uses direct assertions to keep failing generated cases readable"
)]

use stab_core::Circuit;

#[test]
#[ignore = "local long-running M4 parser fuzz smoke"]
fn parser_fuzz_smoke_round_trips_generated_circuits() {
    let mut rng = DeterministicRng::new(0x5a17_5eed_cafe_f00d);

    for case_index in 0..10_000 {
        let input = generated_circuit(&mut rng);
        let circuit = Circuit::from_stim_str(&input)
            .unwrap_or_else(|error| panic!("case {case_index} failed to parse:\n{input}\n{error}"));
        let printed = circuit.to_stim_string();
        let reparsed = Circuit::from_stim_str(&printed).unwrap_or_else(|error| {
            panic!("case {case_index} failed to reparse:\ninput:\n{input}\nprinted:\n{printed}\n{error}")
        });

        assert_eq!(reparsed, circuit, "case {case_index}\n{input}");
    }
}

#[derive(Clone, Debug)]
struct DeterministicRng {
    state: u64,
}

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        u32::try_from((self.state >> 32) & u64::from(u32::MAX)).expect("masked to u32")
    }

    fn below(&mut self, limit: u32) -> u32 {
        self.next_u32() % limit
    }
}

fn generated_circuit(rng: &mut DeterministicRng) -> String {
    let instruction_count = 1 + usize::try_from(rng.below(32)).expect("instruction count fits");
    let mut out = String::new();
    for _ in 0..instruction_count {
        push_generated_instruction(rng, &mut out);
    }
    out
}

fn push_generated_instruction(rng: &mut DeterministicRng, out: &mut String) {
    match rng.below(9) {
        0 => out.push_str(&format!("H {}\n", qubit(rng))),
        1 => {
            let left = qubit(rng);
            let right = different_qubit(rng, left);
            out.push_str(&format!("CX {left} {right}\n"));
        }
        2 => out.push_str(&format!("M({}) {}\n", probability(rng), qubit(rng))),
        3 => out.push_str(&format!("X_ERROR({}) {}\n", probability(rng), qubit(rng))),
        4 => out.push_str(&format!(
            "QUBIT_COORDS({}, {}) {}\n",
            small_coord(rng),
            small_coord(rng),
            qubit(rng)
        )),
        5 => out.push_str(&format!(
            "MPP {}{}*{}{} {}{}\n",
            pauli(rng),
            qubit(rng),
            pauli(rng),
            qubit(rng),
            pauli(rng),
            qubit(rng)
        )),
        6 => {
            out.push_str(&format!("OBSERVABLE_INCLUDE({}) rec[-1]\n", rng.below(4)));
        }
        7 => {
            out.push_str(&format!(
                "REPEAT {} {{\n    H {}\n    TICK\n}}\n",
                1 + rng.below(8),
                qubit(rng)
            ));
        }
        _ => out.push_str("TICK\n"),
    }
}

fn qubit(rng: &mut DeterministicRng) -> u32 {
    rng.below(16)
}

fn different_qubit(rng: &mut DeterministicRng, first: u32) -> u32 {
    let candidate = rng.below(15);
    if candidate >= first {
        candidate + 1
    } else {
        candidate
    }
}

fn probability(rng: &mut DeterministicRng) -> &'static str {
    match rng.below(5) {
        0 => "0",
        1 => "0.001",
        2 => "0.125",
        3 => "0.5",
        _ => "1",
    }
}

fn small_coord(rng: &mut DeterministicRng) -> i32 {
    i32::try_from(rng.below(17)).expect("coord fits") - 8
}

fn pauli(rng: &mut DeterministicRng) -> &'static str {
    match rng.below(3) {
        0 => "X",
        1 => "Y",
        _ => "Z",
    }
}
