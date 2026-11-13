//! Differential and structural contracts for compiled Clifford frame kernels.
//!
//! Every unitary gate in the catalog is compiled through the production frame program and
//! compared against a per-bit reference that conjugates each lane's Pauli through the gate's
//! tableau. Targets sit on both sides of 64- and 256-qubit boundaries and overlap between
//! groups, so sequential group application is checked as well.

#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "differential frame tests fail immediately on fixture errors"
)]

use rand::rngs::SmallRng;
use rand::{RngExt as _, SeedableRng as _};
use stab_algebra::{PauliBasis, PauliString};
use stab_bits::BitBlock;
use stab_model::{Circuit, Gate, GateTargetGroupKind};

use super::BitPlaneDetectionFrame;
use super::noise::FrameExecutionMode;
use super::program::{CircuitProgram, ProgramOptions};
use super::word::FrameWord;

const FRAME_QUBITS: usize = 300;
const SINGLE_QUBIT_TARGETS: [usize; 10] = [0, 63, 64, 65, 127, 128, 255, 256, 299, 64];
const TWO_QUBIT_TARGETS: [usize; 12] = [0, 63, 63, 64, 128, 1, 255, 256, 299, 127, 64, 0];

fn compile(text: &str) -> CircuitProgram {
    let circuit = Circuit::from_stim_str(text).expect("parse frame fixture");
    CircuitProgram::compile(&circuit, ProgramOptions::DETECTION).expect("compile frame fixture")
}

fn random_word<W: FrameWord>(rng: &mut SmallRng) -> W {
    let mut word = W::default();
    for lane in 0..W::LANE_COUNT {
        if rng.random::<bool>() {
            word ^= W::one_hot(lane);
        }
    }
    word
}

fn lane(word: impl FrameWord, lane: usize) -> bool {
    word.extract_u64(lane, 1) != 0
}

fn set_lane<W: FrameWord>(word: &mut W, lane_index: usize, value: bool) {
    if lane(*word, lane_index) != value {
        *word ^= W::one_hot(lane_index);
    }
}

/// Applies `gate` to each target group in order, one lane at a time, through its tableau.
fn reference_apply<W: FrameWord>(gate: Gate, targets: &[usize], xs: &mut [W], zs: &mut [W]) {
    let tableau = stab_analysis::gate_tableau(gate).expect("unitary gate tableau");
    for group in targets.chunks_exact(tableau.len()) {
        for lane_index in 0..W::LANE_COUNT {
            let mut input = PauliString::identity(group.len()).expect("reference Pauli");
            for (position, &qubit) in group.iter().enumerate() {
                let basis =
                    PauliBasis::from_xz(lane(xs[qubit], lane_index), lane(zs[qubit], lane_index));
                input.set(position, basis).expect("set reference Pauli");
            }
            let output = tableau.apply(&input).expect("conjugate reference Pauli");
            for (position, &qubit) in group.iter().enumerate() {
                let basis = output.get(position).expect("reference output basis");
                set_lane(&mut xs[qubit], lane_index, basis.x_bit());
                set_lane(&mut zs[qubit], lane_index, basis.z_bit());
            }
        }
    }
}

fn assert_gate_matches_reference<W: FrameWord + std::fmt::Debug>(gate: Gate, seed: u64) {
    let arity = stab_analysis::gate_tableau(gate)
        .expect("unitary gate tableau")
        .len();
    let targets: &[usize] = match arity {
        1 => &SINGLE_QUBIT_TARGETS,
        2 => &TWO_QUBIT_TARGETS,
        _ => panic!("{} has unsupported arity {arity}", gate.canonical_name()),
    };
    let text = format!(
        "{} {}\nI {}\n",
        gate.canonical_name(),
        targets
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        FRAME_QUBITS - 1
    );
    let program = compile(&text);
    assert_eq!(program.frame_qubit_count(), FRAME_QUBITS);

    let mut rng = SmallRng::seed_from_u64(seed);
    let mut frame =
        BitPlaneDetectionFrame::<W>::try_reusable(FRAME_QUBITS, 0, 0).expect("frame fixture");
    for (x, z) in frame.xs.iter_mut().zip(frame.zs.iter_mut()) {
        *x = random_word(&mut rng);
        *z = random_word(&mut rng);
    }
    let mut expected_xs = frame.xs.clone();
    let mut expected_zs = frame.zs.clone();
    reference_apply(gate, targets, &mut expected_xs, &mut expected_zs);

    let mode = FrameExecutionMode::Sample {
        active_lanes: W::LANE_COUNT,
    };
    frame
        .execute_program(&program, &mut rng, mode)
        .expect("execute compiled gate");
    assert!(
        frame.xs == expected_xs && frame.zs == expected_zs,
        "{} compiled frame action disagrees with its tableau",
        gate.canonical_name()
    );
}

/// Every fixed-arity unitary gate. `SPP` and `SPP_DAG` take Pauli-product targets and are
/// decomposed into these gates before frame lowering.
fn unitary_gates() -> Vec<Gate> {
    let gates = Gate::all()
        .filter(|gate| {
            gate.is_unitary() && gate.target_group_kind() != GateTargetGroupKind::PauliProducts
        })
        .collect::<Vec<_>>();
    for gate in &gates {
        assert!(
            stab_analysis::gate_has_tableau(*gate),
            "unitary gate {} has no tableau for frame lowering",
            gate.canonical_name()
        );
    }
    for gate in Gate::all().filter(|gate| stab_analysis::gate_has_tableau(*gate)) {
        assert!(
            gates.contains(&gate),
            "tableau gate {} is not lowered as a fixed-arity unitary gate",
            gate.canonical_name()
        );
    }
    assert!(gates.len() > 40, "the unitary gate catalog looks truncated");
    gates
}

#[test]
fn every_unitary_gate_kernel_matches_its_per_bit_tableau_reference() {
    for (seed, gate) in (0_u64..).zip(unitary_gates()) {
        assert_gate_matches_reference::<u64>(gate, seed);
        assert_gate_matches_reference::<BitBlock>(gate, seed);
    }
}

#[test]
fn every_unitary_gate_compiles_to_one_program_operation() {
    for gate in unitary_gates() {
        let arity = stab_analysis::gate_tableau(gate)
            .expect("unitary gate tableau")
            .len();
        let text = format!(
            "{} {}\n",
            gate.canonical_name(),
            if arity == 1 { "0" } else { "0 1" }
        );
        let program = compile(&text);
        assert_eq!(
            program.entry_count(),
            1,
            "{} compiled to the wrong number of program operations",
            gate.canonical_name()
        );
        let frame_identity = matches!(gate.canonical_name(), "I" | "X" | "Y" | "Z" | "II");
        assert!(
            matches!(
                program.entries().first(),
                Some(super::program::ProgramEntry::Instruction(
                    super::program::ProgramInstruction::Clifford { frame, .. }
                )) if (*frame == super::program::FrameGate::Identity) == frame_identity
            ),
            "{} must keep its reference action and drop only its frame action",
            gate.canonical_name()
        );
    }
}

#[test]
fn pauli_and_identity_gates_have_no_frame_action_but_size_the_frame() {
    let program = compile("X 0 1\nY 2\nZ 3\nI 4\nII 5 6\nREPEAT 3 {\n    X 7\n    Z 8 9\n}\n");
    // The gates stay for the reference sample, whose signs they change, but none acts on the
    // frame.
    assert_eq!(program.entry_count(), 9);
    assert!(program.entries().iter().all(|entry| !matches!(
        entry,
        super::program::ProgramEntry::Instruction(super::program::ProgramInstruction::Clifford {
            frame: super::program::FrameGate::Single(_) | super::program::FrameGate::Two(_),
            ..
        })
    )));
    // The frame width still covers every qubit, because each batch reset draws one random
    // word per frame qubit and the seeded random stream must not change.
    assert_eq!(program.frame_qubit_count(), 10);

    let mixed = compile("X 0\nH 0\nZ 0\nCZ 0 1\nY 1\n");
    assert_eq!(mixed.entry_count(), 5);
}

#[test]
fn warmed_pauli_product_execution_does_not_allocate() {
    let program = compile(
        "MPP X0*Z1 Y2 X3*X3*Z4 Z0*Z1*Z2\nSPP X0*Y1\nSPP_DAG Z2*Z3 X4\nMPP(0.125) X0*X1 !Y4\nSPP !X1*X2\n",
    );
    assert_warmed_execution_does_not_allocate::<u64>(&program);
    assert_warmed_execution_does_not_allocate::<BitBlock>(&program);
}

fn assert_warmed_execution_does_not_allocate<W: FrameWord>(program: &CircuitProgram) {
    let mut rng = SmallRng::seed_from_u64(7);
    let mode = FrameExecutionMode::Sample {
        active_lanes: W::LANE_COUNT,
    };
    let mut frame = BitPlaneDetectionFrame::<W>::try_reusable(program.frame_qubit_count(), 16, 0)
        .expect("frame fixture");
    frame.reset(&mut rng, mode);
    frame
        .execute_program(program, &mut rng, mode)
        .expect("warm Pauli product execution");
    let measured = allocation_counter::measure(|| {
        for _ in 0..64 {
            frame.reset(&mut rng, mode);
            frame
                .execute_program(program, &mut rng, mode)
                .expect("execute warmed Pauli products");
        }
    });
    assert_eq!(measured.count_total, 0, "{measured:?}");
}
