#![allow(
    clippy::expect_used,
    reason = "PFM-B4 exact parity tests use compact pinned Stim examples"
)]

use std::str::FromStr;

use stab_algebra::Flow;
use stab_analysis::{
    UnsignedStabilizerFlowFailure, check_if_circuit_has_unsigned_stabilizer_flows,
    check_unsigned_stabilizer_flows_with_diagnostics, circuit_flow_generators,
    solve_for_flow_measurements,
};
use stab_model::Circuit;

const GENERATOR_CASES: &[(&str, &[&str])] = &[
    ("", &[]),
    ("X 0\n", &["X -> X", "Z -> -Z"]),
    ("H 0\n", &["X -> Z", "Z -> X"]),
    ("S 0\n", &["X -> Y", "Z -> Z"]),
    ("S_DAG 0\n", &["X -> -Y", "Z -> Z"]),
    (
        "ISWAP 3 1 2 3\n",
        &[
            "___X -> _YZ_",
            "___Z -> _Z__",
            "__X_ -> __ZY",
            "__Z_ -> ___Z",
            "_X__ -> -_ZXZ",
            "_Z__ -> __Z_",
            "X___ -> X___",
            "Z___ -> Z___",
        ],
    ),
    ("SQRT_X 0\nS 0\n", &["X -> Y", "Z -> X"]),
    ("M 0\n", &["1 -> Z xor rec[0]", "Z -> rec[0]"]),
    (
        "M 0 0\n",
        &["1 -> rec[0] xor rec[1]", "1 -> Z xor rec[1]", "Z -> rec[1]"],
    ),
    (
        "MXX 2 0\n",
        &[
            "1 -> X_X xor rec[0]",
            "__X -> __X",
            "_X_ -> _X_",
            "_Z_ -> _Z_",
            "X__ -> __X xor rec[0]",
            "Z_Z -> Z_Z",
        ],
    ),
    (
        "MYY 3 1 2 3\n",
        &[
            "1 -> __YY xor rec[1]",
            "1 -> _Y_Y xor rec[0]",
            "___Y -> ___Y",
            "__Y_ -> ___Y xor rec[1]",
            "_XZZ -> _ZZX xor rec[0]",
            "_ZZZ -> _ZZZ",
            "X___ -> X___",
            "Z___ -> Z___",
        ],
    ),
    (
        "MZZ 3 1 2 3\n",
        &[
            "1 -> __ZZ xor rec[1]",
            "1 -> _Z_Z xor rec[0]",
            "___Z -> ___Z",
            "__Z_ -> ___Z xor rec[1]",
            "_XXX -> _XXX",
            "_Z__ -> ___Z xor rec[0]",
            "X___ -> X___",
            "Z___ -> Z___",
        ],
    ),
    ("SPP Z0\n", &["X -> Y", "Z -> Z"]),
    ("SPP X0 Z0\n", &["X -> Y", "Z -> X"]),
    (
        "SPP X0*X1\n",
        &["_X -> _X", "_Z -> -XY", "X_ -> X_", "Z_ -> -YX"],
    ),
    ("SPP_DAG Z0\n", &["X -> -Y", "Z -> Z"]),
    ("M 0\nCX rec[-1] 0\n", &["1 -> Z", "Z -> rec[0]"]),
    ("R 0\n", &["1 -> Z"]),
    ("MR 0\n", &["1 -> Z", "Z -> rec[0]"]),
    ("M 0\nXCZ 0 rec[-1]\n", &["1 -> Z", "Z -> rec[0]"]),
    (
        "MPAD 0 1 1 0\n",
        &["1 -> rec[0]", "1 -> rec[3]", "1 -> -rec[1]", "1 -> -rec[2]"],
    ),
    (
        "M 0\nCY rec[-1] 1\n",
        &[
            "1 -> Z_ xor rec[0]",
            "_X -> _X xor rec[0]",
            "_Z -> _Z xor rec[0]",
            "Z_ -> rec[0]",
        ],
    ),
    (
        "HERALDED_ERASE(0.04) 1\n\
         HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1\n\
         TICK\n\
         MPP X0*Y1*Z2 Z0*Z1\n",
        &[
            "1 -> rec[0]",
            "1 -> rec[1]",
            "1 -> XYZ xor rec[2]",
            "1 -> ZZ_ xor rec[3]",
            "__Z -> __Z",
            "_ZX -> _ZX",
            "XXX -> _ZY xor rec[2]",
            "Z_X -> _ZX xor rec[3]",
        ],
    ),
    (
        "MPP Y0*Y1 Y2*Y3\n",
        &[
            "1 -> __YY xor rec[1]",
            "1 -> YY__ xor rec[0]",
            "___Y -> ___Y",
            "__XZ -> __ZX xor rec[1]",
            "__ZZ -> __ZZ",
            "_Y__ -> _Y__",
            "XZ__ -> ZX__ xor rec[0]",
            "ZZ__ -> ZZ__",
        ],
    ),
    ("X_ERROR(0.1) 0\n", &["X -> X", "Z -> Z"]),
];

#[test]
fn flow_generation_common_semantic_matrix_matches_stim() {
    for (circuit_text, expected) in GENERATOR_CASES {
        assert_eq!(
            generator_strings(circuit_text),
            expected
                .iter()
                .copied()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            "{circuit_text}"
        );
    }
}

#[test]
fn flow_measurement_solving_common_matrix_matches_stim() {
    assert_flow_solve_empty();
    assert_flow_solve_simple();
    assert_flow_solve_repetition_code();
    assert_python_measurement_solve();
    assert_python_multitarget_solve();
    assert_python_fewer_measurements();
}

#[test]
fn flow_checking_repeats_and_diagnostics_match_stim() {
    assert_bounded_repeat_generation_and_checking();
    assert_flow_diagnostics();
}

fn assert_flow_solve_empty() {
    assert_eq!(
        solve_for_flow_measurements(&circuit(""), &[]).expect("empty solve"),
        Vec::<Option<Vec<i32>>>::new()
    );
}

fn assert_flow_solve_simple() {
    let circuit = circuit("MX 0\n");
    let queries = [
        flow("1 -> X0"),
        flow("1 -> Y0"),
        flow("Y0 -> Y0"),
        flow("X0 -> 1"),
        flow("X0 -> Z0"),
        flow("Y1 -> Y1"),
    ];
    assert_eq!(
        solve_for_flow_measurements(&circuit, &queries).expect("simple solve"),
        vec![Some(vec![0]), None, None, Some(vec![0]), None, Some(vec![]),]
    );
}

fn assert_flow_solve_repetition_code() {
    let circuit = circuit(
        "R 1 3\n\
         CX 0 1 2 3\n\
         CX 4 3 2 1\n\
         M 1 3\n",
    );
    let queries = [
        flow("Z0*Z2 -> 1"),
        flow("1 -> Z2*Z4"),
        flow("1 -> Z0*Z4"),
        flow("Z0*Z4 -> Z0*Z2"),
        flow("Z0 -> Z0"),
        flow("Z0 -> Z1"),
        flow("Z0 -> Z2"),
        flow("X0*X2*X4 -> X0*X2*X4"),
        flow("X0 -> X0"),
        flow("X0 -> Z0"),
    ];
    assert_eq!(
        solve_for_flow_measurements(&circuit, &queries).expect("repetition-code solve"),
        vec![
            Some(vec![0]),
            Some(vec![1]),
            Some(vec![0, 1]),
            Some(vec![1]),
            Some(vec![]),
            None,
            Some(vec![0]),
            Some(vec![]),
            None,
            None,
        ]
    );
    let error = solve_for_flow_measurements(&circuit, &[flow("1 -> 1")])
        .expect_err("empty-Pauli query must be rejected");
    assert!(error.to_string().contains("non-empty Pauli"), "{error}");
}

fn assert_python_measurement_solve() {
    let measured_circuit = circuit("M 2\n");
    let queries = [
        flow("X2 -> X2"),
        flow("Y2 -> Y2"),
        flow("Z2 -> Z2"),
        flow("Z2 -> 1"),
    ];
    assert_eq!(
        solve_for_flow_measurements(&measured_circuit, &queries).expect("measured-idle solve"),
        vec![None, None, Some(vec![]), Some(vec![0])]
    );
    assert_eq!(
        solve_for_flow_measurements(
            &circuit("MXX 0 1\n"),
            &[flow("YY -> ZZ"), flow("YY -> YY"), flow("YZ -> ZY")],
        )
        .expect("Python MXX batch solve"),
        vec![Some(vec![0]), Some(vec![]), Some(vec![0])]
    );
}

fn assert_python_multitarget_solve() {
    let cases = [
        ("M 1 2\n", vec![flow("_Z -> 1")], vec![Some(vec![0])]),
        ("MX 1 2\n", vec![flow("_X -> 1")], vec![Some(vec![0])]),
        (
            "MYY 1 2 3 4\n",
            vec![flow("_YY__ -> 1")],
            vec![Some(vec![0])],
        ),
        (
            "MPP Y1*Y2 Y3*Y4\n",
            vec![flow("_YY__ -> 1")],
            vec![Some(vec![0])],
        ),
    ];
    for (circuit_text, queries, expected) in cases {
        assert_eq!(
            solve_for_flow_measurements(&circuit(circuit_text), &queries)
                .expect("multi-target solve"),
            expected,
            "{circuit_text}"
        );
    }
}

fn assert_python_fewer_measurements() {
    let product = "Z0*Z1*Z2*Z3*Z4*Z5*Z6*Z7*Z8";
    for (circuit_text, expected) in [
        (format!("MPP {product}\nM 0 1 2 3 4 5 6 7 8\n"), vec![0]),
        (format!("M 0 1 2 3 4 5 6 7 8\nMPP {product}\n"), vec![9]),
    ] {
        let queries = [
            flow(&format!("1 -> {product}")),
            flow(&format!("{product} -> 1")),
        ];
        assert_eq!(
            solve_for_flow_measurements(&circuit(&circuit_text), &queries)
                .expect("fewer-measurements solve"),
            vec![Some(expected.clone()), Some(expected)],
            "{circuit_text}"
        );
    }
}

fn assert_bounded_repeat_generation_and_checking() {
    let repeated = circuit("REPEAT 3 {\n    M 0\n}\n");
    let expanded = circuit("M 0\nM 0\nM 0\n");
    let flows = circuit_flow_generators(&repeated).expect("bounded repeat generators");

    assert_eq!(
        flows,
        circuit_flow_generators(&expanded).expect("expanded generators")
    );
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&repeated, &flows),
        vec![true; flows.len()]
    );
}

fn assert_flow_diagnostics() {
    let circuit = circuit("H 0\n");
    let flows = [flow("X -> Z"), flow("X -> X")];
    assert_eq!(
        check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &flows),
        [true, false]
    );

    let diagnostics = check_unsigned_stabilizer_flows_with_diagnostics(&circuit, &flows);
    assert!(diagnostics.first().is_some_and(|check| check.has_flow()));
    assert!(matches!(
        diagnostics.get(1).and_then(|check| check.failure()),
        Some(UnsignedStabilizerFlowFailure::OutputMismatch { .. })
    ));
}

fn generator_strings(text: &str) -> Vec<String> {
    circuit_flow_generators(&circuit(text))
        .expect("flow generators")
        .into_iter()
        .map(|flow| flow.to_string())
        .collect()
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn flow(text: &str) -> Flow {
    Flow::from_str(text).expect("parse flow")
}
