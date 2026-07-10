#![allow(
    clippy::expect_used,
    reason = "PFM-B4 exact parity tests use compact pinned Stim examples"
)]

use std::str::FromStr;

use stab_core::{Circuit, Flow, circuit_flow_generators, solve_for_flow_measurements};

macro_rules! exact_generator_case {
    ($name:ident, $circuit:expr, [$($expected:expr),* $(,)?]) => {
        #[test]
        fn $name() {
            let expected: Vec<String> = vec![$($expected.to_string()),*];
            assert_eq!(
                generator_strings($circuit),
                expected,
            );
        }
    };
}

exact_generator_case!(pfm_b4_flow_various_empty, "", []);
exact_generator_case!(pfm_b4_flow_various_x, "X 0\n", ["X -> X", "Z -> -Z"]);
exact_generator_case!(pfm_b4_flow_various_h, "H 0\n", ["X -> Z", "Z -> X"]);
exact_generator_case!(pfm_b4_flow_various_s, "S 0\n", ["X -> Y", "Z -> Z"]);
exact_generator_case!(
    pfm_b4_flow_various_s_dag,
    "S_DAG 0\n",
    ["X -> -Y", "Z -> Z"]
);
exact_generator_case!(
    pfm_b4_flow_various_iswap,
    "ISWAP 3 1 2 3\n",
    [
        "___X -> _YZ_",
        "___Z -> _Z__",
        "__X_ -> __ZY",
        "__Z_ -> ___Z",
        "_X__ -> -_ZXZ",
        "_Z__ -> __Z_",
        "X___ -> X___",
        "Z___ -> Z___",
    ]
);
exact_generator_case!(
    pfm_b4_flow_various_composed_unitary,
    "SQRT_X 0\nS 0\n",
    ["X -> Y", "Z -> X"]
);
exact_generator_case!(
    pfm_b4_flow_various_measurement,
    "M 0\n",
    ["1 -> Z xor rec[0]", "Z -> rec[0]"]
);
exact_generator_case!(
    pfm_b4_flow_various_duplicate_measurement,
    "M 0 0\n",
    ["1 -> rec[0] xor rec[1]", "1 -> Z xor rec[1]", "Z -> rec[1]",]
);
exact_generator_case!(
    pfm_b4_flow_various_mxx,
    "MXX 2 0\n",
    [
        "1 -> X_X xor rec[0]",
        "__X -> __X",
        "_X_ -> _X_",
        "_Z_ -> _Z_",
        "X__ -> __X xor rec[0]",
        "Z_Z -> Z_Z",
    ]
);
exact_generator_case!(
    pfm_b4_flow_various_myy,
    "MYY 3 1 2 3\n",
    [
        "1 -> __YY xor rec[1]",
        "1 -> _Y_Y xor rec[0]",
        "___Y -> ___Y",
        "__Y_ -> ___Y xor rec[1]",
        "_XZZ -> _ZZX xor rec[0]",
        "_ZZZ -> _ZZZ",
        "X___ -> X___",
        "Z___ -> Z___",
    ]
);
exact_generator_case!(
    pfm_b4_flow_various_mzz,
    "MZZ 3 1 2 3\n",
    [
        "1 -> __ZZ xor rec[1]",
        "1 -> _Z_Z xor rec[0]",
        "___Z -> ___Z",
        "__Z_ -> ___Z xor rec[1]",
        "_XXX -> _XXX",
        "_Z__ -> ___Z xor rec[0]",
        "X___ -> X___",
        "Z___ -> Z___",
    ]
);
exact_generator_case!(pfm_b4_flow_various_spp_z, "SPP Z0\n", ["X -> Y", "Z -> Z"]);
exact_generator_case!(
    pfm_b4_flow_various_spp_multigroup,
    "SPP X0 Z0\n",
    ["X -> Y", "Z -> X"]
);
exact_generator_case!(
    pfm_b4_flow_various_spp_two_qubit,
    "SPP X0*X1\n",
    ["_X -> _X", "_Z -> -XY", "X_ -> X_", "Z_ -> -YX"]
);
exact_generator_case!(
    pfm_b4_flow_various_spp_dag,
    "SPP_DAG Z0\n",
    ["X -> -Y", "Z -> Z"]
);
exact_generator_case!(
    pfm_b4_flow_various_cx_feedback,
    "M 0\nCX rec[-1] 0\n",
    ["1 -> Z", "Z -> rec[0]"]
);
exact_generator_case!(pfm_b4_flow_various_reset, "R 0\n", ["1 -> Z"]);
exact_generator_case!(
    pfm_b4_flow_various_measure_reset,
    "MR 0\n",
    ["1 -> Z", "Z -> rec[0]"]
);
exact_generator_case!(
    pfm_b4_flow_various_xcz_feedback,
    "M 0\nXCZ 0 rec[-1]\n",
    ["1 -> Z", "Z -> rec[0]"]
);
exact_generator_case!(
    pfm_b4_flow_various_mpad,
    "MPAD 0 1 1 0\n",
    ["1 -> rec[0]", "1 -> rec[3]", "1 -> -rec[1]", "1 -> -rec[2]",]
);
exact_generator_case!(
    pfm_b4_flow_various_cy_feedback,
    "M 0\nCY rec[-1] 1\n",
    [
        "1 -> Z_ xor rec[0]",
        "_X -> _X xor rec[0]",
        "_Z -> _Z xor rec[0]",
        "Z_ -> rec[0]",
    ]
);
exact_generator_case!(
    pfm_b4_flow_various_heralded_mpp,
    "HERALDED_ERASE(0.04) 1\n\
     HERALDED_PAULI_CHANNEL_1(0.01, 0.02, 0.03, 0.04) 1\n\
     TICK\n\
     MPP X0*Y1*Z2 Z0*Z1\n",
    [
        "1 -> rec[0]",
        "1 -> rec[1]",
        "1 -> XYZ xor rec[2]",
        "1 -> ZZ_ xor rec[3]",
        "__Z -> __Z",
        "_ZX -> _ZX",
        "XXX -> _ZY xor rec[2]",
        "Z_X -> _ZX xor rec[3]",
    ]
);
exact_generator_case!(
    pfm_b4_flow_various_mpp_yy,
    "MPP Y0*Y1 Y2*Y3\n",
    [
        "1 -> __YY xor rec[1]",
        "1 -> YY__ xor rec[0]",
        "___Y -> ___Y",
        "__XZ -> __ZX xor rec[1]",
        "__ZZ -> __ZZ",
        "_Y__ -> _Y__",
        "XZ__ -> ZX__ xor rec[0]",
        "ZZ__ -> ZZ__",
    ]
);

#[test]
fn pfm_b4_flow_solve_empty() {
    assert_eq!(
        solve_for_flow_measurements(&circuit(""), &[]).expect("empty solve"),
        Vec::<Option<Vec<i32>>>::new()
    );
}

#[test]
fn pfm_b4_flow_solve_simple() {
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

#[test]
fn pfm_b4_flow_solve_repetition_code() {
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

#[test]
fn pfm_b4_flow_python_measurement_solve() {
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

#[test]
fn pfm_b4_flow_python_multitarget_solve() {
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

#[test]
fn pfm_b4_flow_python_fewer_measurements() {
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
