#![allow(
    clippy::expect_used,
    reason = "PFM-B4 solver tests use compact failure diagnostics"
)]

use std::str::FromStr;

use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use stab_core::{
    Circuit, CircuitError, Flow, PauliBasis, PauliSign, PauliString,
    check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators,
    solve_for_flow_measurements,
};

const GENERATED_CASES: u32 = 64;
const GENERATED_SEED: [u8; 32] = [0xB4; 32];

#[test]
fn pfm_b4_flow_solve_over_sixteen() {
    let qubits = (0..32)
        .map(|qubit| qubit.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let product = (0..32)
        .map(|qubit| format!("Z{qubit}"))
        .collect::<Vec<_>>()
        .join("*");
    let circuit = circuit(&format!("M {qubits}\nMPP {product}\n"));
    let queries = [
        flow(&format!("1 -> {product}")),
        flow(&format!("{product} -> 1")),
    ];

    let solved = solve_for_flow_measurements(&circuit, &queries).expect("solve 33 measurements");
    assert_eq!(solved, vec![Some(vec![32]), Some(vec![32])]);
    for (query, measurements) in queries.iter().zip(solved) {
        let measurements = measurements.expect("selected flow is solvable");
        let candidate = Flow::new(
            query.input().clone(),
            query.output().clone(),
            measurements,
            [],
        );
        assert_eq!(
            check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[candidate]),
            vec![true]
        );
    }
}

#[test]
fn pfm_b4_flow_solver_rejects_invalid_circuits_without_exhaustive_fallback() {
    for circuit_text in [
        "MPP X0*Y0\n".to_string(),
        format!(
            "M {}\nSPP X20*Z20\n",
            (0..17)
                .map(|qubit| qubit.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        ),
    ] {
        let error = solve_for_flow_measurements(&circuit(&circuit_text), &[flow("Z0 -> Z0")])
            .expect_err("invalid generator shape must fail closed");
        assert!(
            matches!(
                error,
                CircuitError::InvalidTableauConversion { .. }
                    | CircuitError::InvalidCircuitSimplification { .. }
            ),
            "{error:?}"
        );
        assert!(error.to_string().contains("Hermitian"), "{error}");
        assert!(!error.to_string().contains("fallback"), "{error}");
    }
}

#[test]
fn pfm_b4_flow_solver_preserves_idle_qubit_flows_through_resets() {
    for circuit_text in ["R 5\n", "MR 5\n"] {
        let circuit = circuit(circuit_text);
        let queries = [flow("X0 -> X0"), flow("Z0 -> Z0")];
        assert_eq!(
            solve_for_flow_measurements(&circuit, &queries).expect("solve idle-qubit flows"),
            vec![Some(vec![]), Some(vec![])],
            "{circuit_text}"
        );
    }
}

#[test]
fn pfm_b4_flow_solver_treats_mpad_values_as_non_qubit_records() {
    assert_eq!(
        solve_for_flow_measurements(&circuit("MPAD 0\n"), &[flow("X0 -> X0")])
            .expect("solve identity flow through MPAD"),
        vec![Some(vec![])]
    );
}

#[test]
fn pfm_b4_flow_solver_uses_implicit_sparse_query_identity_rows() {
    const HIGH_QUBIT: usize = 65_535;
    let mut high_x = PauliString::identity(HIGH_QUBIT + 1).expect("wide Pauli identity");
    high_x
        .set(HIGH_QUBIT, PauliBasis::X)
        .expect("set high sparse query term");
    let identity = PauliString::identity(HIGH_QUBIT + 1).expect("wide Pauli identity");
    let queries = [
        Flow::new(high_x.clone(), high_x.clone(), [], []),
        Flow::new(identity, high_x, [], []),
    ];

    assert_eq!(
        solve_for_flow_measurements(&circuit(""), &queries).expect("solve sparse query suffix"),
        vec![Some(vec![]), None]
    );
}

#[test]
fn pfm_b4_flow_solver_repeats_match_expansion_and_preserve_caps() {
    let repeated = circuit("REPEAT 2 {\n    M 0\n}\n");
    let expanded = circuit("M 0\nM 0\n");
    let queries = [flow("Z -> 1"), flow("1 -> Z"), flow("X -> X")];
    assert_eq!(
        solve_for_flow_measurements(&repeated, &queries).expect("solve repeated circuit"),
        solve_for_flow_measurements(&expanded, &queries).expect("solve expanded circuit")
    );

    let error = solve_for_flow_measurements(
        &circuit("REPEAT 1000000 {\n    M 0\n}\n"),
        &[flow("Z -> 1")],
    )
    .expect_err("solver must preserve flow-generator repeat cap");
    assert!(error.to_string().contains("current limit 4096"), "{error}");
}

#[test]
fn pfm_b4_flow_solver_small_arbitrary_queries_match_exhaustive_checker() {
    for circuit_text in [
        "I 0 1\n",
        "I 0 1\nR 1\n",
        "I 0 1\nMR 1\n",
        "I 0 1\nM 0\n",
        "I 0 1\nMX 1\n",
        "I 0 1\nMXX 0 1\n",
        "I 0 1\nH 0\nCX 0 1\nM 0 1\n",
        "I 0 1\nM 0\nCX rec[-1] sweep[0]\n",
        "I 0 1\nM 0\nCX rec[-1] 0 0 1\n",
        "I 0 1\nREPEAT 2 {\n    M 0\n}\n",
    ] {
        let circuit = circuit(circuit_text);
        let queries = two_qubit_unsigned_queries();
        let solved = solve_for_flow_measurements(&circuit, &queries)
            .expect("solve arbitrary PFM-B4 query corpus");
        for (query, actual) in queries.iter().zip(solved) {
            let expected = exhaustive_checker_solution(&circuit, query);
            assert_eq!(
                actual.is_some(),
                expected.is_some(),
                "solver/checker disagreement for {circuit_text:?} and {query}"
            );
            if let Some(measurements) = actual {
                let candidate = Flow::new(
                    query.input().clone(),
                    query.output().clone(),
                    measurements,
                    [],
                );
                assert_eq!(
                    check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[candidate]),
                    vec![true],
                    "invalid solver answer for {circuit_text:?} and {query}"
                );
            }
        }
    }
}

#[test]
fn pfm_b4_flow_solver_rank_deficient_inconsistent_and_underdetermined() {
    let duplicate = circuit("M 0 0\n");
    assert_eq!(
        solve_for_flow_measurements(
            &duplicate,
            &[flow("1 -> Z"), flow("Z -> 1"), flow("1 -> Y")],
        )
        .expect("solve duplicate measurement rows"),
        vec![Some(vec![1]), Some(vec![1]), None]
    );

    let product = "Z0*Z1*Z2*Z3";
    let underdetermined = circuit(&format!("M 0 1 2 3\nMPP {product}\n"));
    assert_eq!(
        solve_for_flow_measurements(
            &underdetermined,
            &[
                flow(&format!("1 -> {product}")),
                flow(&format!("{product} -> 1")),
            ],
        )
        .expect("solve underdetermined product"),
        vec![Some(vec![4]), Some(vec![4])]
    );
}

#[test]
fn pfm_b4_flow_solver_handles_sparse_high_qubits_and_uses_pauli_projection() {
    let circuit = circuit("M 1023\n");
    let mut z = PauliString::identity(1024).expect("Pauli identity");
    z.set(1023, PauliBasis::Z).expect("set sparse Z term");
    let identity = PauliString::identity(1024).expect("Pauli identity");
    let queries = [
        Flow::new(identity.clone(), z.clone(), [], []),
        Flow::new(identity.clone(), z.clone(), [], [7]),
        Flow::new(identity, z, [999], [7]),
    ];

    assert_eq!(
        solve_for_flow_measurements(&circuit, &queries).expect("solve sparse high qubit"),
        vec![Some(vec![0]), Some(vec![0]), Some(vec![0])]
    );
}

#[test]
fn pfm_b4_flow_solver_generated_cross_engine_corpus() {
    let config = Config {
        cases: GENERATED_CASES,
        failure_persistence: None,
        rng_algorithm: RngAlgorithm::ChaCha,
        ..Config::default()
    };
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &GENERATED_SEED);
    let mut runner = TestRunner::new_with_rng(config, rng);
    runner
        .run(&prop::collection::vec(0u8..9, 1..14), |operations| {
            let text = generated_circuit_text(&operations);
            let circuit = Circuit::from_stim_str(&text).map_err(|error| {
                TestCaseError::fail(format!("generated circuit did not parse: {error}\n{text}"))
            })?;
            let generators = circuit_flow_generators(&circuit).map_err(|error| {
                TestCaseError::fail(format!(
                    "generated circuit did not produce flows: {error}\n{text}"
                ))
            })?;
            let checks = check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &generators);
            prop_assert!(
                checks.iter().all(|check| *check),
                "generated flow failed checker: {:?}\n{}\n{:?}",
                checks,
                text,
                generators
            );

            for generator in generators.iter().filter(|flow| {
                !flow.input().has_no_pauli_terms() || !flow.output().has_no_pauli_terms()
            }) {
                let query = Flow::new(
                    generator.input().clone(),
                    generator.output().clone(),
                    [],
                    [],
                );
                let solved = solve_for_flow_measurements(&circuit, std::slice::from_ref(&query))
                    .map_err(|error| {
                        TestCaseError::fail(format!(
                            "generated query failed to solve: {error}\n{text}\n{query}"
                        ))
                    })?;
                let Some(measurements) = solved.into_iter().next().flatten() else {
                    return Err(TestCaseError::fail(format!(
                        "generated query was unexpectedly unsolved\n{text}\n{query}"
                    )));
                };
                let candidate = Flow::new(
                    query.input().clone(),
                    query.output().clone(),
                    measurements,
                    [],
                );
                prop_assert_eq!(
                    check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &[candidate]),
                    vec![true],
                    "generated solution failed checker\n{}\n{}",
                    text,
                    query
                );
            }
            Ok(())
        })
        .expect("fixed-seed PFM-B4 flow corpus");
}

fn generated_circuit_text(operations: &[u8]) -> String {
    let mut text = String::new();
    for operation in operations {
        text.push_str(match operation {
            0 => "H 0\n",
            1 => "S 1\n",
            2 => "CX 0 1\n",
            3 => "M 0\n",
            4 => "MX 1\n",
            5 => "MZZ 1 2\n",
            6 => "MPP X0*Z1\n",
            7 => "SPP X1*Z2\n",
            _ => "MPAD 0\n",
        });
    }
    text
}

fn two_qubit_unsigned_queries() -> Vec<Flow> {
    let bases = [PauliBasis::I, PauliBasis::X, PauliBasis::Y, PauliBasis::Z];
    let mut paulis = Vec::with_capacity(16);
    for left in bases {
        for right in bases {
            paulis.push(
                PauliString::from_bases(PauliSign::Plus, [left, right]).expect("two-qubit Pauli"),
            );
        }
    }
    let mut queries = Vec::with_capacity(255);
    for input in &paulis {
        for output in &paulis {
            if input.has_no_pauli_terms() && output.has_no_pauli_terms() {
                continue;
            }
            queries.push(Flow::new(input.clone(), output.clone(), [], []));
        }
    }
    queries
}

fn exhaustive_checker_solution(circuit: &Circuit, query: &Flow) -> Option<Vec<i32>> {
    let measurement_count = usize::try_from(
        circuit
            .count_measurements()
            .expect("count small-corpus measurements"),
    )
    .expect("small-corpus measurement count fits usize");
    assert!(measurement_count <= 8, "bounded exhaustive test corpus");
    for mask in 0usize..(1usize << measurement_count) {
        let measurements = (0..measurement_count)
            .filter(|index| mask & (1usize << index) != 0)
            .map(|index| i32::try_from(index).expect("small measurement index"))
            .collect::<Vec<_>>();
        let candidate = Flow::new(
            query.input().clone(),
            query.output().clone(),
            measurements.clone(),
            [],
        );
        if check_if_circuit_has_unsigned_stabilizer_flows(circuit, &[candidate]) == vec![true] {
            return Some(measurements);
        }
    }
    None
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
