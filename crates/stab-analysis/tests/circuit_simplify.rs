#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "decomposition parity tests use fixed validated fixtures"
)]

use stab_analysis::{AnalysisError, decomposed_circuit};
use stab_model::{Circuit, CircuitItem};
use std::ops::ControlFlow;

#[test]
fn decomposed_circuit_reports_typed_anti_hermitian_failures() {
    assert_eq!(
        decomposed_circuit(&circuit("MPP X0*X0 X0*!X0\n"))
            .expect("constant products")
            .to_stim_string(),
        "MPAD 0 1\n"
    );

    let error = decomposed_circuit(&circuit("SPP X0*Z0\n")).expect_err("anti-Hermitian product");
    assert!(matches!(
        error,
        AnalysisError::InvalidCircuitSimplification { ref message }
            if message.contains("anti-Hermitian")
    ));
}

#[test]
fn decomposed_template_and_classical_control_matrix_matches_stim() {
    // Exact output from Stim v1.16.0 Circuit.decomposed(). Empty H/S instructions
    // are part of the pinned canonical output when a basis buffer has no qubits.
    let cases = [
        ("H_XY 0 1\n", "H 0 1\nS 0 1 0 1\nH 0 1\nS 0 1\n"),
        ("CX rec[-1] 0\n", "CX rec[-1] 0\n"),
        ("CX sweep[0] 0\n", "CX sweep[0] 0\n"),
        ("CX 0 rec[-1]\n", "CX 0 rec[-1]\n"),
        ("CX rec[-1] sweep[0]\n", "CX rec[-1] sweep[0]\n"),
        ("CY rec[-1] 0\n", "S 0 0 0\nCX rec[-1] 0\nS 0\n"),
        ("CY 0 sweep[0]\n", "S\nCX 0 sweep[0]\nS\n"),
        ("CY rec[-1] sweep[0]\n", "S\nCX rec[-1] sweep[0]\nS\n"),
        ("CZ rec[-1] 0\n", "H 0\nCX rec[-1] 0\nH 0\n"),
        ("CZ 0 sweep[0]\n", "H\nCX 0 sweep[0]\nH\n"),
        ("CZ rec[-1] sweep[0]\n", "H\nCX rec[-1] sweep[0]\nH\n"),
        ("CZ rec[-1] rec[-2]\n", "H\nCX rec[-1] rec[-2]\nH\n"),
        ("XCZ 0 rec[-1]\n", "CX rec[-1] 0\n"),
        ("XCZ sweep[0] 0\n", "CX 0 sweep[0]\n"),
        ("XCZ rec[-1] sweep[0]\n", "CX sweep[0] rec[-1]\n"),
        ("YCZ 0 rec[-1]\n", "S 0 0 0\nCX rec[-1] 0\nS 0\n"),
        ("YCZ sweep[0] 0\n", "S\nCX 0 sweep[0]\nS\n"),
        ("YCZ rec[-1] sweep[0]\n", "S\nCX sweep[0] rec[-1]\nS\n"),
        (
            "CY rec[-1] 0 1 sweep[0] rec[-2] 2\n",
            "S 0 2 0 2 0 2\n\
             CX rec[-1] 0 1 sweep[0] rec[-2] 2\n\
             S 0 2\n",
        ),
        (
            "CZ rec[-1] 0 1 sweep[0] rec[-2] rec[-3]\n",
            "H 0\n\
             CX rec[-1] 0 1 sweep[0] rec[-2] rec[-3]\n\
             H 0\n",
        ),
        (
            "XCZ 0 rec[-1] sweep[0] 1 rec[-2] sweep[1]\n",
            "CX rec[-1] 0 1 sweep[0] sweep[1] rec[-2]\n",
        ),
        (
            "YCZ 0 rec[-1] sweep[0] 1 2 sweep[1]\n",
            "S 0 2 0 2 0 2\n\
             CX rec[-1] 0 1 sweep[0] sweep[1] 2\n\
             S 0 2\n",
        ),
    ];

    for (source, expected) in cases {
        assert_eq!(
            decomposed_circuit(&circuit(source))
                .expect(source)
                .to_stim_string(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn streamed_spp_lowering_matches_decomposition_and_stops_before_later_work() {
    let source = circuit("SPP[tag] X0*Y1 Z2\n");
    let instruction = source
        .items()
        .first()
        .and_then(|item| match item {
            CircuitItem::Instruction(instruction) => Some(instruction),
            CircuitItem::RepeatBlock(_) => None,
        })
        .expect("SPP fixture must contain one instruction");
    let mut streamed = Circuit::new();
    let completion: ControlFlow<()> =
        stab_analysis::advanced::visit_decomposed_spp_instructions(instruction, |lowered| {
            streamed.append_instruction(lowered);
            ControlFlow::Continue(())
        })
        .expect("stream SPP decomposition");
    assert!(completion.is_continue());
    assert_eq!(
        streamed,
        decomposed_circuit(&source).expect("materialize SPP decomposition")
    );

    let source = circuit("SPP X0 X1*Z1\n");
    let instruction = source
        .items()
        .first()
        .and_then(|item| match item {
            CircuitItem::Instruction(instruction) => Some(instruction),
            CircuitItem::RepeatBlock(_) => None,
        })
        .expect("SPP fixture must contain one instruction");
    let mut visits = 0;
    let completion =
        stab_analysis::advanced::visit_decomposed_spp_instructions(instruction, |_| {
            visits += 1;
            ControlFlow::Break(())
        })
        .expect("early stop must not reduce the later anti-Hermitian product");
    assert!(completion.is_break());
    assert_eq!(visits, 1);
}

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}
