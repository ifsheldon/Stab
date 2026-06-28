#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "M12 simulator cross-checks mirror compact upstream graph/vector examples"
)]

use num_complex::Complex32;
use stab_core::{Circuit, Tableau, unitary_to_tableau};

#[test]
fn graph_simulator_normal_form_examples_preserve_tableau_semantics() {
    // Adapted from Stim v1.16.0 src/stim/simulators/graph_simulator.test.cc.
    // Stim's graph simulator emits state-preparation circuits. Stab does not
    // expose a graph-state simulator, so this checks the emitted graph-normal
    // circuits against the equivalent prepared Clifford tableau.
    for case in [
        GraphCase {
            name: "initial graph state",
            qubits: 6,
            input: "",
            graph_normal_form: "
                RX 0 1 2 3 4 5
                TICK
                H 0 1 2 3 4 5
            ",
        },
        GraphCase {
            name: "basis reset after all-H",
            qubits: 6,
            input: "H 0 1 2 3 4 5",
            graph_normal_form: "
                RX 0 1 2 3 4 5
                TICK
            ",
        },
        GraphCase {
            name: "single-qubit graph basis transform",
            qubits: 6,
            input: "
                H 0 1 2 3 4 5
                H 0
                S 1
                C_XYZ 2 3 3
                SQRT_X_DAG 4
            ",
            graph_normal_form: "
                RX 0 1 2 3 4 5
                TICK
                C_XYZ 2
                C_ZYX 3
                H 0
                S 1
                SQRT_X_DAG 4
            ",
        },
        GraphCase {
            name: "single-qubit Pauli graph update",
            qubits: 6,
            input: "
                H 0 1 2 3 4 5
                H 0
                S 1
                C_XYZ 2 3 3
                SQRT_X_DAG 4
                X 0
                S 1
                Y 2
                Z 3
            ",
            graph_normal_form: "
                RX 0 1 2 3 4 5
                TICK
                X 2 3
                Z 0 1
                C_XYZ 2
                C_ZYX 3
                H 0
                SQRT_X_DAG 4
            ",
        },
        GraphCase {
            name: "HS XYZ graph decomposition",
            qubits: 10,
            input: "
                H 0 1 2 3 4 5 6 7 8 9
                I 0
                H 1
                S 2
                SQRT_X_DAG 3
                C_XYZ 4
                C_ZYX 5
                X 6
                Y 7
                Z 8
                H 9
                Z 9
            ",
            graph_normal_form: "
                RX 0 1 2 3 4 5 6 7 8 9
                TICK
                X 6 9
                Y 7
                Z 4 8
                S 2 3 4
                H 1 3 4 5 9
                S 3 5
            ",
        },
    ] {
        assert_eq!(
            graph_prepared_tableau(case.qubits, case.input),
            circuit(case.graph_normal_form)
                .to_tableau(false, true, true)
                .expect(case.name),
            "{}",
            case.name
        );
    }
}

#[test]
fn vector_simulator_small_state_examples_match_stab_tableaux() {
    // Adapted from Stim v1.16.0 src/stim/simulators/vector_simulator.test.cc.
    let h = f32::sqrt(0.5);

    let bell_state = final_state(2, &[Op::H(0), Op::Cx(0, 1)], 0);
    assert_state_close(
        &bell_state,
        &[c(h, 0.0), c(0.0, 0.0), c(0.0, 0.0), c(h, 0.0)],
    );
    assert_vector_unitary_matches_circuit("H 0\nCX 0 1\n", 2, &[Op::H(0), Op::Cx(0, 1)]);

    let h_squared = final_state(1, &[Op::H(0), Op::H(0)], 0);
    assert_state_close(&h_squared, &[c(1.0, 0.0), c(0.0, 0.0)]);
    assert_vector_unitary_matches_circuit("H 0\nH 0\n", 1, &[Op::H(0), Op::H(0)]);

    let sqrt_x_squared = final_state(1, &[Op::SqrtXDag(0), Op::SqrtXDag(0)], 0);
    assert_state_close(&sqrt_x_squared, &[c(0.0, 0.0), c(1.0, 0.0)]);
    assert_vector_unitary_matches_circuit(
        "SQRT_X_DAG 0\nSQRT_X_DAG 0\n",
        1,
        &[Op::SqrtXDag(0), Op::SqrtXDag(0)],
    );

    let y_after_bell = final_state(2, &[Op::H(0), Op::Cx(0, 1), Op::Y(1)], 0);
    assert_state_close(
        &y_after_bell,
        &[c(0.0, 0.0), c(0.0, -h), c(0.0, h), c(0.0, 0.0)],
    );
    assert_vector_unitary_matches_circuit(
        "H 0\nCX 0 1\nY 1\n",
        2,
        &[Op::H(0), Op::Cx(0, 1), Op::Y(1)],
    );
}

struct GraphCase {
    name: &'static str,
    qubits: usize,
    input: &'static str,
    graph_normal_form: &'static str,
}

fn graph_prepared_tableau(qubits: usize, input: &str) -> Tableau {
    let mut text = String::new();
    text.push('H');
    for qubit in 0..qubits {
        text.push(' ');
        text.push_str(&qubit.to_string());
    }
    text.push('\n');
    text.push_str(input);
    circuit(&text)
        .to_tableau(false, true, true)
        .expect("prepared tableau")
}

#[derive(Clone, Copy)]
enum Op {
    H(usize),
    SqrtXDag(usize),
    Y(usize),
    Cx(usize, usize),
}

fn assert_vector_unitary_matches_circuit(circuit_text: &str, qubits: usize, ops: &[Op]) {
    let matrix = unitary_matrix(qubits, ops);
    assert_eq!(
        unitary_to_tableau(&matrix, true).expect(circuit_text),
        circuit(circuit_text)
            .to_tableau(false, false, false)
            .expect(circuit_text)
    );
}

fn final_state(qubits: usize, ops: &[Op], input_basis: usize) -> Vec<Complex32> {
    let dimension = 1_usize << qubits;
    let mut state = vec![c(0.0, 0.0); dimension];
    state[input_basis] = c(1.0, 0.0);
    apply_ops(&mut state, ops);
    state
}

fn unitary_matrix(qubits: usize, ops: &[Op]) -> Vec<Vec<Complex32>> {
    let dimension = 1_usize << qubits;
    let mut matrix = vec![vec![c(0.0, 0.0); dimension]; dimension];
    for column in 0..dimension {
        let state = final_state(qubits, ops, column);
        for (row, amplitude) in matrix.iter_mut().zip(state) {
            row[column] = amplitude;
        }
    }
    matrix
}

fn apply_ops(state: &mut [Complex32], ops: &[Op]) {
    for op in ops {
        match *op {
            Op::H(target) => {
                let h = f32::sqrt(0.5);
                apply_single(
                    state,
                    target,
                    [[c(h, 0.0), c(h, 0.0)], [c(h, 0.0), c(-h, 0.0)]],
                );
            }
            Op::SqrtXDag(target) => {
                apply_single(
                    state,
                    target,
                    [[c(0.5, -0.5), c(0.5, 0.5)], [c(0.5, 0.5), c(0.5, -0.5)]],
                );
            }
            Op::Y(target) => {
                apply_single(
                    state,
                    target,
                    [[c(0.0, 0.0), c(0.0, -1.0)], [c(0.0, 1.0), c(0.0, 0.0)]],
                );
            }
            Op::Cx(control, target) => apply_cx(state, control, target),
        }
    }
}

fn apply_single(state: &mut [Complex32], target: usize, matrix: [[Complex32; 2]; 2]) {
    let mask = 1_usize << target;
    for base in 0..state.len() {
        if base & mask == 0 {
            let paired = base | mask;
            let a0 = state[base];
            let a1 = state[paired];
            state[base] = matrix[0][0] * a0 + matrix[0][1] * a1;
            state[paired] = matrix[1][0] * a0 + matrix[1][1] * a1;
        }
    }
}

fn apply_cx(state: &mut [Complex32], control: usize, target: usize) {
    let control_mask = 1_usize << control;
    let target_mask = 1_usize << target;
    for base in 0..state.len() {
        if base & control_mask != 0 && base & target_mask == 0 {
            state.swap(base, base | target_mask);
        }
    }
}

fn assert_state_close(actual: &[Complex32], expected: &[Complex32]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let delta = *actual - *expected;
        assert!(
            delta.norm() <= 1e-4,
            "state[{index}] expected {expected:?} got {actual:?}"
        );
    }
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn c(real: f32, imag: f32) -> Complex32 {
    Complex32::new(real, imag)
}
