#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "admission tests use direct fixture setup assertions for focused failures"
)]

//! `sample` and `detect` admit expanded work from the same count: circuit instructions executed
//! by one unrolled shot. Both accept a circuit at exactly one million and reject the next one,
//! whatever the per-instruction target count.

use std::fmt::Write as _;

use stab_engine::{
    DetectionCompileError, DetectionError, DetectionResourceKind, DetectionSamplingCompiler,
    ResourceAmount, SamplingCompileError, SamplingCompiler,
};
use stab_model::Circuit;

const LIMIT: u64 = 1_000_000;

/// `REPEAT iterations { body }` followed by one measurement and its detector, which execute
/// `iterations * body_instructions + 2` instructions.
fn circuit(iterations: u64, body: &str) -> Circuit {
    Circuit::from_stim_str(&format!(
        "REPEAT {iterations} {{\n{body}}}\nM 0\nDETECTOR rec[-1]\n"
    ))
    .expect("parse admission fixture")
}

fn assert_sample_rejects(circuit: &Circuit, actual: u64) {
    assert_eq!(
        SamplingCompiler::new().compile(circuit),
        Err(SamplingCompileError::ExpandedOperationLimit {
            actual: ResourceAmount::exact(actual),
            limit: LIMIT,
        })
    );
}

fn assert_detect_rejects(circuit: &Circuit) {
    match DetectionSamplingCompiler::new().compile(circuit) {
        Err(DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource))) => {
            assert_eq!(resource.kind(), DetectionResourceKind::ExpandedInstructions);
            assert_eq!(resource.limit(), LIMIT);
        }
        other => panic!("detect must reject expanded instructions, got {other:?}"),
    }
}

#[test]
fn sample_and_detect_share_the_expanded_instruction_boundary() {
    let exact = circuit(LIMIT - 2, "H 0\n");
    SamplingCompiler::new()
        .compile(&exact)
        .expect("sample admits exactly one million instructions");
    DetectionSamplingCompiler::new()
        .compile(&exact)
        .expect("detect admits exactly one million instructions");

    let first_excess = circuit(LIMIT - 1, "H 0\n");
    assert_sample_rejects(&first_excess, LIMIT + 1);
    assert_detect_rejects(&first_excess);
}

#[test]
fn wide_instructions_count_once_for_sample_and_detect() {
    // Two thousand targets per instruction: 2,000,000 per-target dispatches, which the retired
    // per-target basis refused for `sample` although `detect` ran the circuit.
    let mut wide = String::from("H");
    for qubit in 0..2000 {
        write!(wide, " {qubit}").expect("write target");
    }
    wide.push('\n');
    let admitted = circuit(1000, &wide);
    let plan = SamplingCompiler::new()
        .compile(&admitted)
        .expect("sample admits a wide circuit within the instruction boundary");
    assert_eq!(plan.measurement_width().get(), 1);
    DetectionSamplingCompiler::new()
        .compile(&admitted)
        .expect("detect admits the same wide circuit");

    // Two instructions per iteration, the first with two thousand targets.
    let rejected = circuit(LIMIT / 2, &format!("{wide}X 0\n"));
    assert_sample_rejects(&rejected, LIMIT + 2);
    assert_detect_rejects(&rejected);
}
