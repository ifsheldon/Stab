#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "unit tests build small fixed circuits and index generated choices directly"
)]

use std::fmt::Write as _;

use proptest::prelude::*;
use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString};
use stab_model::Circuit;

use super::FlowCheckAlgorithm;
use crate::circuit_flow::checker::{
    UnsignedFlowCheckOptions, UnsignedStabilizerFlowFailure, check_unsigned_flows_using,
    diagnose_unsigned_flows_using,
};
use crate::circuit_inverse::{TimeReversedForFlowsOptions, time_reversed_for_flows_using};
use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, CircuitToTableauOptions, circuit_to_tableau,
    circuit_to_tableau_with_options,
};
use stab_model::{Cancellation, Cancelled, MemoryBudget};

const ALGORITHMS: [FlowCheckAlgorithm; 2] = [
    FlowCheckAlgorithm::DenseTableau,
    FlowCheckAlgorithm::SparseTracker,
];

fn flow(text: &str) -> Flow {
    text.parse().expect("test flow")
}

fn pauli(text: &str) -> PauliString {
    text.parse().expect("test Pauli string")
}

fn dense(_: &Circuit, _: &[Flow]) -> FlowCheckAlgorithm {
    FlowCheckAlgorithm::DenseTableau
}

fn sparse(_: &Circuit, _: &[Flow]) -> FlowCheckAlgorithm {
    FlowCheckAlgorithm::SparseTracker
}

/// The public result of time reversal, with errors compared by message.
fn time_reversed(
    circuit: &Circuit,
    flows: &[Flow],
    select: fn(&Circuit, &[Flow]) -> FlowCheckAlgorithm,
) -> Result<(String, Vec<Flow>), String> {
    time_reversed_for_flows_using(
        circuit,
        flows,
        TimeReversedForFlowsOptions::default(),
        select,
    )
    .map(|(inverse, reversed)| (inverse.to_stim_string(), reversed))
    .map_err(|error| error.to_string())
}

#[test]
fn failing_unitary_flows_report_identical_output_mismatches_on_both_algorithms() {
    let circuit = Circuit::from_stim_str(
        "H 0\nCX 0 1\nTICK\nSWAP 1 2\nDEPOLARIZE1(0.25) 0\nQUBIT_COORDS(1) 2\n",
    )
    .expect("test circuit");
    let flows = [
        flow("X__ -> Z__"),
        flow("X__ -> X__"),
        flow("Z -> Z"),
        flow("_Z_ -> ZZZ"),
    ];
    let [dense_checks, sparse_checks] = ALGORITHMS.map(|algorithm| {
        diagnose_unsigned_flows_using(
            algorithm,
            &circuit,
            &flows,
            &UnsignedFlowCheckOptions::default(),
        )
        .expect("default options diagnose every flow")
    });
    assert_eq!(dense_checks, sparse_checks);
    let [dense_bools, sparse_bools] = ALGORITHMS.map(|algorithm| {
        check_unsigned_flows_using(
            algorithm,
            &circuit,
            &flows,
            &UnsignedFlowCheckOptions::default(),
        )
        .expect("default options check every flow")
    });
    assert_eq!(dense_bools, vec![true, false, false, false]);
    assert_eq!(sparse_bools, dense_bools);

    let failures = dense_checks
        .iter()
        .map(|check| check.failure().cloned())
        .collect::<Vec<_>>();
    assert_eq!(
        failures,
        vec![
            None,
            Some(UnsignedStabilizerFlowFailure::OutputMismatch {
                expected_output: pauli("X__"),
                actual_output: pauli("Z__"),
            }),
            Some(UnsignedStabilizerFlowFailure::OutputMismatch {
                expected_output: pauli("Z"),
                actual_output: pauli("X_X"),
            }),
            Some(UnsignedStabilizerFlowFailure::OutputMismatch {
                expected_output: pauli("ZZZ"),
                actual_output: pauli("Z_Z"),
            }),
        ]
    );
}

#[test]
fn time_reversal_rejects_the_same_failing_flow_on_both_algorithms() {
    let circuit = Circuit::from_stim_str("H 0\nISWAP 0 1\nSQRT_XX 1 2\nXCZ 2 0\nSPP X0*Z2\n")
        .expect("test circuit");
    let flows = [flow("Z__ -> Z__"), flow("X__ -> X__")];
    let dense_result = time_reversed(&circuit, &flows, dense);
    assert!(dense_result.is_err());
    assert_eq!(dense_result, time_reversed(&circuit, &flows, sparse));
}

/// One generated instruction: a gate index, its qubits, and whether its first target is inverted.
type GeneratedInstruction = (usize, Vec<usize>, bool);

const SINGLE_QUBIT_GATES: [&str; 11] = [
    "H",
    "S",
    "S_DAG",
    "SQRT_X",
    "SQRT_Y_DAG",
    "C_XYZ",
    "H_XY",
    "X",
    "Y",
    "I",
    "Z",
];
const TWO_QUBIT_GATES: [&str; 16] = [
    "CX",
    "CY",
    "CZ",
    "XCZ",
    "YCX",
    "XCY",
    "SWAP",
    "ISWAP",
    "ISWAP_DAG",
    "CXSWAP",
    "CZSWAP",
    "SQRT_XX",
    "SQRT_YY_DAG",
    "SQRT_ZZ",
    "II",
    "SWAPCX",
];
const PRODUCT_GATES: [&str; 2] = ["SPP", "SPP_DAG"];
const FLOW_NEUTRAL_LINES: [&str; 4] = [
    "TICK",
    "QUBIT_COORDS(1, 2) 0",
    "DEPOLARIZE1(0.125) 0",
    "SHIFT_COORDS(1)",
];

fn write_instruction(
    text: &mut String,
    (gate, qubits, inverted): &GeneratedInstruction,
    qubit_count: usize,
    flow_neutral: bool,
) -> bool {
    let gate_count = SINGLE_QUBIT_GATES.len() + TWO_QUBIT_GATES.len() + PRODUCT_GATES.len();
    let gate = gate % (gate_count + usize::from(flow_neutral));
    let qubit = |index: usize| qubits[index % qubits.len()] % qubit_count;
    if gate < SINGLE_QUBIT_GATES.len() {
        let _ = writeln!(text, "{} {}", SINGLE_QUBIT_GATES[gate], qubit(0));
    } else if gate < SINGLE_QUBIT_GATES.len() + TWO_QUBIT_GATES.len() {
        let (first, second) = (qubit(0), qubit(1));
        if first == second || qubit_count < 2 {
            return false;
        }
        let name = TWO_QUBIT_GATES[gate - SINGLE_QUBIT_GATES.len()];
        let _ = writeln!(text, "{name} {first} {second}");
    } else if gate < gate_count {
        let name = PRODUCT_GATES[gate - SINGLE_QUBIT_GATES.len() - TWO_QUBIT_GATES.len()];
        let mut product = Vec::new();
        for (position, raw) in qubits.iter().enumerate() {
            let target = raw % qubit_count;
            if product.iter().any(|(existing, _)| *existing == target) {
                continue;
            }
            let basis = ["X", "Y", "Z"][(raw / qubit_count) % 3];
            let bang = if position == 0 && *inverted { "!" } else { "" };
            product.push((target, format!("{bang}{basis}{target}")));
        }
        let joined = product
            .into_iter()
            .map(|(_, term)| term)
            .collect::<Vec<_>>()
            .join("*");
        let _ = writeln!(text, "{name} {joined}");
    } else {
        let _ = writeln!(text, "{}", FLOW_NEUTRAL_LINES[qubits[0] % 4]);
    }
    true
}

fn random_circuit(
    qubit_count: usize,
    instructions: &[GeneratedInstruction],
    repeat: Option<(u64, usize)>,
    flow_neutral: bool,
) -> Circuit {
    let mut text = format!("I {}\n", qubit_count - 1);
    let split = repeat.map_or(instructions.len(), |(_, start)| {
        start % (instructions.len() + 1)
    });
    for instruction in &instructions[..split] {
        write_instruction(&mut text, instruction, qubit_count, flow_neutral);
    }
    if let Some((count, _)) = repeat {
        let mut body = String::new();
        for instruction in &instructions[split..] {
            write_instruction(&mut body, instruction, qubit_count, flow_neutral);
        }
        if !body.is_empty() {
            let _ = write!(text, "REPEAT {count} {{\n{body}}}\n");
        }
    }
    Circuit::from_stim_str(&text).expect("generated circuit")
}

fn random_pauli(bases: &[u8], width: usize) -> PauliString {
    let bases = bases.iter().take(width).map(|basis| match basis % 4 {
        0 => PauliBasis::I,
        1 => PauliBasis::X,
        2 => PauliBasis::Y,
        _ => PauliBasis::Z,
    });
    PauliString::from_bases(PauliSign::Plus, bases).expect("generated Pauli string")
}

/// Builds valid flows (the circuit's image of the input) and invalid ones (a random output), no
/// wider than the circuit so that both algorithms of the flow checker apply.
fn random_flows(
    circuit: &Circuit,
    qubit_count: usize,
    specs: &[(Vec<u8>, Vec<u8>, usize, bool)],
) -> AnalysisResult<Vec<Flow>> {
    let tableau = circuit_to_tableau(circuit, true, false, false)?;
    specs
        .iter()
        .map(|(input, output, width, valid)| {
            let width = width % (qubit_count + 1);
            let input = random_pauli(input, width);
            let output = if *valid {
                let padded = PauliString::from_bases(
                    PauliSign::Plus,
                    (0..qubit_count).map(|index| input.get(index).unwrap_or(PauliBasis::I)),
                )
                .map_err(crate::circuit_tableau::map_tableau_error)?;
                tableau
                    .apply(&padded)
                    .map_err(crate::circuit_tableau::map_tableau_error)?
            } else {
                random_pauli(output, width)
            };
            Ok(Flow::from_paulis(input, output))
        })
        .collect()
}

fn instruction_strategy() -> impl Strategy<Value = GeneratedInstruction> {
    (
        any::<usize>(),
        prop::collection::vec(0..60_usize, 1..4),
        any::<bool>(),
    )
}

fn flow_spec_strategy() -> impl Strategy<Value = (Vec<u8>, Vec<u8>, usize, bool)> {
    (
        prop::collection::vec(any::<u8>(), 6),
        prop::collection::vec(any::<u8>(), 6),
        any::<usize>(),
        any::<bool>(),
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(192))]

    #[test]
    fn flow_check_algorithms_return_identical_public_results(
        qubit_count in 1..=6_usize,
        instructions in prop::collection::vec(instruction_strategy(), 0..24),
        repeat in prop::option::of((1..6_u64, any::<usize>())),
        flow_neutral in any::<bool>(),
        specs in prop::collection::vec(flow_spec_strategy(), 1..8),
    ) {
        let circuit = random_circuit(qubit_count, &instructions, repeat, flow_neutral);
        let flows = random_flows(&circuit, qubit_count, &specs).expect("generated flows");

        let [dense_bools, sparse_bools] =
            ALGORITHMS.map(|algorithm| check_unsigned_flows_using(algorithm, &circuit, &flows, &UnsignedFlowCheckOptions::default())
                .expect("default options check every flow"));
        prop_assert_eq!(&dense_bools, &sparse_bools);
        let [dense_checks, sparse_checks] =
            ALGORITHMS.map(|algorithm| diagnose_unsigned_flows_using(algorithm, &circuit, &flows, &UnsignedFlowCheckOptions::default())
                .expect("default options diagnose every flow"));
        prop_assert_eq!(&dense_checks, &sparse_checks);
        for (check, has_flow) in dense_checks.iter().zip(&dense_bools) {
            prop_assert_eq!(check.has_flow(), *has_flow);
            let output_mismatch = matches!(
                check.failure(),
                None | Some(UnsignedStabilizerFlowFailure::OutputMismatch { .. })
            );
            prop_assert!(output_mismatch, "unexpected failure {:?}", check.failure());
        }
        for (flow, (has_flow, spec)) in flows.iter().zip(dense_bools.iter().zip(&specs)) {
            prop_assert!(!spec.3 || *has_flow, "valid flow {} was rejected", flow);
        }

        prop_assert_eq!(
            time_reversed(&circuit, &flows, dense),
            time_reversed(&circuit, &flows, sparse)
        );
    }
}

#[test]
fn a_refused_tableau_falls_back_to_the_sparse_tracker_but_cancellation_does_not() {
    // 71 qubits: the Tableau needs 4568 bytes, while no extracted region needs more than 103.
    let circuit = Circuit::from_stim_str("H 0\nCX 0 1\nS 1\nSWAP 1 70\n").unwrap();
    let flows = [
        flow("X_ -> Z_"),
        flow("_Z -> Z"),
        flow("_Z -> ZZ"),
        flow("Y -> Y"),
    ];
    let small = UnsignedFlowCheckOptions {
        memory_budget: MemoryBudget::from_bytes(1024),
        ..UnsignedFlowCheckOptions::default()
    };
    let refusal = circuit_to_tableau_with_options(
        &circuit,
        &CircuitToTableauOptions {
            ignore_noise: true,
            memory_budget: small.memory_budget,
            ..CircuitToTableauOptions::default()
        },
    )
    .expect_err("the budget refuses the dense Tableau");
    assert_eq!(
        refusal
            .resource_limit_error()
            .map(|error| error.operation()),
        Some(AnalysisOperation::TableauConversion)
    );

    let defaults = UnsignedFlowCheckOptions::default();
    let dense_bools = check_unsigned_flows_using(
        FlowCheckAlgorithm::DenseTableau,
        &circuit,
        &flows,
        &defaults,
    )
    .unwrap();
    let dense_checks = diagnose_unsigned_flows_using(
        FlowCheckAlgorithm::DenseTableau,
        &circuit,
        &flows,
        &defaults,
    )
    .unwrap();
    assert!(dense_bools.iter().any(|has_flow| *has_flow));
    assert!(dense_bools.iter().any(|has_flow| !*has_flow));
    let cancelled = Cancellation::new();
    cancelled.cancel();
    let set = UnsignedFlowCheckOptions {
        cancellation: cancelled,
        ..UnsignedFlowCheckOptions::default()
    };
    for algorithm in ALGORITHMS {
        assert_eq!(
            check_unsigned_flows_using(algorithm, &circuit, &flows, &small).unwrap(),
            dense_bools
        );
        assert_eq!(
            diagnose_unsigned_flows_using(algorithm, &circuit, &flows, &small).unwrap(),
            dense_checks
        );
        // A set signal is reported, never answered with flows that failed closed.
        assert_eq!(
            check_unsigned_flows_using(algorithm, &circuit, &flows, &set),
            Err(AnalysisError::Cancelled(Cancelled))
        );
        assert_eq!(
            diagnose_unsigned_flows_using(algorithm, &circuit, &flows, &set),
            Err(AnalysisError::Cancelled(Cancelled))
        );
    }
}
