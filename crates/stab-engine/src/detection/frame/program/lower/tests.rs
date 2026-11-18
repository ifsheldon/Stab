#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "lowering tests use direct fixture assertions"
)]

use stab_model::CircuitItem;

use super::*;

/// Every instruction the plain-Clifford shortcut accepts lowers through the general path to the
/// same Clifford entry, with the same qubit count, payload, and record effects, so admission and
/// materialization do not depend on which path an instruction takes.
#[test]
fn plain_clifford_shortcut_matches_general_lowering() {
    let target_lists = [
        "",
        "0",
        "5 2",
        "0 1 2",
        "3 1 4 1",
        "!0 1",
        "X0 Z1",
        "rec[-1] 0",
        "sweep[0] 1",
        "0 rec[-1]",
    ];
    let mut shortcut_gates = 0_usize;
    for gate in Gate::all() {
        for targets in target_lists {
            let text = format!("M 0 1\n{} {targets}\n", gate.canonical_name());
            let Ok(circuit) = Circuit::from_stim_str(&text) else {
                continue;
            };
            for item in circuit.items() {
                let CircuitItem::Instruction(instruction) = item else {
                    continue;
                };
                check_shortcut(gate, instruction, &mut shortcut_gates);
            }
        }
    }
    // `H`, `CX`, and every other fixed-arity unitary gate take the shortcut on qubit targets.
    assert!(
        shortcut_gates > 40,
        "only {shortcut_gates} instructions took the shortcut"
    );
}

fn check_shortcut(gate: Gate, instruction: &CircuitInstruction, shortcut_gates: &mut usize) {
    let mut actions = FrameCliffordActionCache::default();
    let Some(plain) = plain_clifford(instruction, &mut actions).expect("shortcut lowering") else {
        return;
    };
    *shortcut_gates += 1;
    let name = gate.canonical_name();
    for options in [ProgramOptions::SAMPLING, ProgramOptions::DETECTION] {
        let mut general_actions = FrameCliffordActionCache::default();
        let lowering = lower(instruction, options, &mut general_actions).expect("general lowering");
        let Lowering::Entry(kind @ EntryKind::Clifford(frame)) = lowering else {
            panic!("{name} took the shortcut but lowers to {lowering:?}");
        };
        assert_eq!(frame, plain.frame, "{name} frame action");
        assert_eq!(
            arena_demand(instruction, kind),
            ArenaCounts {
                qubits: plain.targets,
                ..ArenaCounts::default()
            },
            "{name} payload"
        );
        let mut counts = CountState {
            measurements: 2,
            sweep_bits: 0,
        };
        record_effects(instruction, lowering, &mut counts).expect("record effects");
        assert_eq!(
            (counts.measurements, counts.sweep_bits),
            (2, 0),
            "{name} records nothing"
        );
    }
    assert_eq!(
        instruction_qubit_count(instruction).expect("qubit count"),
        plain.qubits,
        "{name} qubit count"
    );
    assert!(
        kept_by_noiseless_circuit(gate),
        "{name} is kept by the noiseless circuit"
    );
}
