#![allow(
    clippy::expect_used,
    reason = "flattening policy tests use compact parse and exact-output assertions"
)]

use std::hint::black_box;

use stab_analysis::{
    AnalysisOperation, CircuitFlattenLimits, ResourceKind, flattened_circuit,
    flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
};
use stab_model::{
    Circuit, CircuitItem, MemoryBudget, RepeatBlock, RepeatCount, RepeatNestingLimit, Target,
};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

fn budget(bytes: u64) -> CircuitFlattenLimits {
    CircuitFlattenLimits::default().with_memory_budget(MemoryBudget::from_bytes(bytes))
}

#[test]
fn default_budget_refuses_compact_expansion_beyond_the_budget() {
    assert_eq!(
        CircuitFlattenLimits::default().memory_budget(),
        MemoryBudget::DEFAULT
    );
    let default_input = circuit(
        "
        SHIFT_COORDS(5, 0)
        QUBIT_COORDS(1, 2, 3) 0
        REPEAT 2 {
            M 0
            DETECTOR(0, 0) rec[-1]
            SHIFT_COORDS(0, 1)
        }
    ",
    );
    assert_eq!(
        flattened_circuit_with_limits(&default_input, CircuitFlattenLimits::default())
            .expect("explicit default flatten"),
        flattened_circuit(&default_input).expect("existing default flatten")
    );

    let hostile = circuit("REPEAT 1000000000000 {\nH 0\n}\n");
    let error = flattened_circuit(&hostile).expect_err("a compact repeat beyond the budget");
    let resource = error.resource_limit_error().expect("typed budget refusal");
    assert_eq!(resource.code(), "resource-limit-exceeded");
    assert_eq!(resource.operation(), AnalysisOperation::CircuitFlatten);
    assert_eq!(resource.operation().as_str(), "circuit-flatten");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(resource.limit(), MemoryBudget::DEFAULT.bytes());
    assert!(error.to_string().contains("memory budget"), "{error}");
}

#[test]
fn materialized_bytes_admit_the_exact_budget_and_reject_the_first_excess_before_allocation() {
    let one = circuit("H 0\n");
    let exact_bytes = (std::mem::size_of::<CircuitItem>() + std::mem::size_of::<Target>()) as u64;
    flattened_circuit_with_limits(&one, budget(exact_bytes))
        .expect("the exact conservative byte boundary should materialize");
    let error = flattened_circuit_with_limits(&one, budget(exact_bytes - 1))
        .expect_err("the first byte below the conservative requirement must reject");
    let resource = error
        .resource_limit_error()
        .expect("byte rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(resource.actual(), exact_bytes);
    assert_eq!(resource.limit(), exact_bytes - 1);

    let small = circuit("REPEAT 4 {\nH 0\n}\n");
    let mut large_text = String::from("REPEAT 4 {\nH");
    for qubit in 0..4096 {
        large_text.push(' ');
        large_text.push_str(&qubit.to_string());
    }
    large_text.push_str("\n}\n");
    let large = circuit(&large_text);
    let rejection_allocations = |input: &Circuit| {
        allocation_counter::measure(|| {
            let error = flattened_circuit_with_limits(input, budget(exact_bytes))
                .expect_err("the budget must reject before output materialization");
            drop(black_box(error));
        })
    };
    let small_allocations = rejection_allocations(&small);
    let large_allocations = rejection_allocations(&large);
    assert!(
        large_allocations.count_total <= small_allocations.count_total + 2,
        "flatten rejection allocations scaled with target storage: small={small_allocations:?}, large={large_allocations:?}"
    );
    assert!(
        large_allocations.bytes_total <= small_allocations.bytes_total + 256,
        "flatten rejection bytes scaled with target storage: small={small_allocations:?}, large={large_allocations:?}"
    );
}

#[test]
fn operation_count_overflow_fails_before_materialization() {
    let overflowing = circuit(
        "
        REPEAT 1000000000000 {
            REPEAT 1000000000000 {
                H 0
            }
        }
    ",
    );

    let error = flattened_circuit_with_limits(&overflowing, budget(u64::MAX))
        .expect_err("operation count arithmetic should overflow before materialization");

    assert_eq!(
        error.to_string(),
        "invalid flattened circuit operation count value overflowed"
    );
}

#[test]
fn retained_payload_count_overflow_fails_before_materialization() {
    for (body, expected) in [
        (circuit("CX 0 1\n"), "target count overflowed"),
        (
            circuit("QUBIT_COORDS(1, 2) 0\n"),
            "argument count overflowed",
        ),
    ] {
        let mut overflowing = Circuit::new();
        overflowing.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(u64::MAX).expect("maximum repeat is representable"),
            body,
            None,
        ));

        let error = flattened_circuit_with_limits(&overflowing, budget(u64::MAX))
            .expect_err("retained payload arithmetic should overflow before materialization");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?}, got {error}"
        );
    }
}

#[test]
fn caller_raised_limit_cannot_exceed_platform_materialization_capacity() {
    let mut enormous = Circuit::new();
    enormous.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(u64::MAX).expect("maximum programmatic repeat is representable"),
        circuit("H 0\n"),
        None,
    ));
    let limits = budget(u64::MAX);

    for result in [
        flattened_circuit_with_limits(&enormous, limits.clone()).map(|_| ()),
        flattened_circuit_operations_with_limits(&enormous, limits).map(|_| ()),
    ] {
        let error = result.expect_err("platform vector capacity must reject before allocation");
        let resource = error
            .resource_limit_error()
            .expect("platform materialization rejection should remain typed");
        assert_eq!(resource.operation(), AnalysisOperation::CircuitFlatten);
        assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
        assert_eq!(resource.actual(), u64::MAX);
        assert!(resource.limit() < resource.actual());
    }
}

#[test]
fn programmatic_repeat_depth_is_admitted_before_recursive_flattening() {
    fn nested(depth: usize) -> Circuit {
        let mut body = circuit("H 0\n");
        for _ in 0..depth {
            let mut outer = Circuit::new();
            outer.append_repeat_block(RepeatBlock::new(
                RepeatCount::try_new(1).expect("one is a valid repeat count"),
                body,
                None,
            ));
            body = outer;
        }
        body
    }

    let exact = nested(RepeatNestingLimit::HARD_MAX);
    assert_eq!(
        flattened_circuit_with_limits(&exact, CircuitFlattenLimits::default())
            .expect("the fixed recursive envelope is accepted")
            .to_stim_string(),
        "H 0\n"
    );
    assert_eq!(
        flattened_circuit_operations_with_limits(&exact, CircuitFlattenLimits::default())
            .expect("the operation-vector adapter uses the same admission")
            .len(),
        1
    );

    for result in [
        flattened_circuit_with_limits(
            &nested(RepeatNestingLimit::HARD_MAX + 1),
            CircuitFlattenLimits::default(),
        )
        .map(|_| ()),
        flattened_circuit_operations_with_limits(
            &nested(RepeatNestingLimit::HARD_MAX + 1),
            CircuitFlattenLimits::default(),
        )
        .map(|_| ()),
    ] {
        let error = result.expect_err("the first unsafe programmatic nesting level must reject");
        let resource = error
            .resource_limit_error()
            .expect("flatten nesting rejection should expose typed context");
        assert_eq!(resource.operation(), AnalysisOperation::CircuitFlatten);
        assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), (RepeatNestingLimit::HARD_MAX + 1) as u64);
        assert_eq!(resource.limit(), RepeatNestingLimit::HARD_MAX as u64);
    }
}
