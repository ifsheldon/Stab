#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "folded-summary contract tests use compact fixture setup and direct assertions"
)]

use std::assert_matches;

use stab_model::advanced::{
    FoldedCircuitEvent, FoldedVisitError, repeat_block_with_tag_bytes, visit_folded_circuit,
};
use stab_model::{
    Cancellation, Circuit, FoldedCircuitSummary, FoldedWalkError, GateKind, ParseLimits,
    RepeatCount, RepeatNestingLimit,
};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit fixture")
}

fn summary(circuit: &Circuit) -> FoldedCircuitSummary {
    FoldedCircuitSummary::of(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
    )
    .expect("summarize circuit")
}

#[test]
fn folded_counts_match_the_unrolled_circuit() {
    let folded = circuit(
        "H 0 1\nREPEAT 3 {\n    CX 0 1 2 3\n    MPP X0*Z1 Y2\n    REPEAT 2 {\n        M(0.1) 0 !1\n        DETECTOR(1, 2) rec[-1] rec[-2]\n        TICK\n    }\n    HERALDED_ERASE(0.1) 4\n}\nOBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let unrolled = folded.iter_flattened_instructions().collect::<Vec<_>>();
    let summary = summary(&folded);

    let count = |predicate: &dyn Fn(&stab_model::CircuitInstruction) -> usize| -> u64 {
        unrolled
            .iter()
            .map(|instruction| predicate(instruction) as u64)
            .sum()
    };
    assert_eq!(summary.instructions().exact(), Some(unrolled.len() as u64));
    assert_eq!(
        summary.targets().exact(),
        Some(count(&|i| i.targets().len()))
    );
    assert_eq!(
        summary.arguments().exact(),
        Some(count(&|i| i.args().len()))
    );
    assert_eq!(
        summary.measurements().exact(),
        Some(folded.count_measurements().expect("count"))
    );
    assert_eq!(
        summary.detectors().exact(),
        Some(folded.count_detectors().expect("count"))
    );
    assert_eq!(
        summary.ticks().exact(),
        Some(folded.count_ticks().expect("count"))
    );
    for kind in GateKind::ALL {
        assert_eq!(
            summary.instructions_of(kind).exact(),
            Some(count(&|i| usize::from(i.gate().kind() == kind))),
            "{kind:?}"
        );
    }
    assert_eq!(
        summary.measurements_of(GateKind::HERALDED_ERASE).exact(),
        Some(3)
    );
    assert_eq!(summary.repeat_iterations().exact(), Some(3 + 6));
    assert_eq!(summary.folded_items(), 10);
    assert_eq!(summary.max_repeat_depth(), 2);
}

#[test]
fn overflowing_counts_are_values_reported_per_count() {
    // Each count exceeds u64 only when multiplied through both blocks, and the walk visits the
    // folded items once instead of allocating or looping per repetition.
    let huge = circuit(
        "REPEAT 4611686018427387904 {\n    REPEAT 4611686018427387904 {\n        X 0\n    }\n}\nTICK\n",
    );
    let summary = summary(&huge);
    assert!(summary.instructions_of(GateKind::X).exact().is_none());
    assert!(summary.instructions_of(GateKind::X).get().is_err());
    assert_eq!(summary.instructions_of(GateKind::X).saturating(), u64::MAX);
    assert!(summary.instructions().exact().is_none());
    assert_eq!(summary.measurements().exact(), Some(0));
    assert_eq!(summary.ticks().exact(), Some(1));
    assert!(summary.repeat_iterations().exact().is_none());
}

#[test]
fn nesting_beyond_the_limit_is_refused_before_recursing_further() {
    let mut nested = circuit("TICK\n");
    for _ in 0..=RepeatNestingLimit::HARD_MAX {
        let mut outer = Circuit::new();
        outer.append_repeat_block(repeat_block_with_tag_bytes(
            RepeatCount::try_new(2).expect("repeat count"),
            nested,
            None,
        ));
        nested = outer;
    }
    let refused = FoldedCircuitSummary::of(
        &nested,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
    );
    assert_eq!(
        refused,
        Err(FoldedWalkError::RepeatNesting {
            depth: RepeatNestingLimit::HARD_MAX + 1,
            limit: RepeatNestingLimit::HARD_MAX,
        })
    );

    let shallow = RepeatNestingLimit::try_new(1).expect("limit");
    let two_deep = circuit("REPEAT 2 {\n    REPEAT 2 {\n        TICK\n    }\n}\n");
    let mut events = 0;
    let walked = visit_folded_circuit(&two_deep, shallow, &Cancellation::never(), |event| {
        events += 1;
        match event {
            FoldedCircuitEvent::Repeat { body_depth, .. } => assert!(body_depth <= 2),
            FoldedCircuitEvent::Instruction { .. } => panic!("instruction below the limit"),
        }
        Ok::<(), ()>(())
    });
    // The inner block is reported before the walk refuses to enter its body.
    assert_eq!(events, 2);
    assert_eq!(
        walked,
        Err(FoldedVisitError::Walk(FoldedWalkError::RepeatNesting {
            depth: 2,
            limit: 1
        }))
    );
}

#[test]
fn a_cancelled_signal_stops_the_walk() {
    let mut text = String::new();
    for _ in 0..4096 {
        text.push_str("TICK\n");
    }
    let cancellation = Cancellation::new();
    cancellation.cancel();
    let cancelled = FoldedCircuitSummary::of(
        &circuit(&text),
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &cancellation,
    );
    assert_matches!(cancelled, Err(FoldedWalkError::Cancelled(_)));
}
