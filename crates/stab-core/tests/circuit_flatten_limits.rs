#![allow(
    clippy::expect_used,
    reason = "flattening policy tests use compact parse and exact-output assertions"
)]

use std::hint::black_box;

use stab_core::{
    Circuit, CircuitFlattenLimits, CircuitItem, RepeatBlock, RepeatCount, RepeatNestingLimit,
    ResourceKind, ResourceOperation, Target,
    analysis::{
        flattened_circuit, flattened_circuit_operations_with_limits, flattened_circuit_with_limits,
    },
};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse circuit")
}

macro_rules! limits {
    ($max_expanded_operations:expr) => {
        CircuitFlattenLimits::default().with_max_expanded_operations($max_expanded_operations)
    };
}

#[test]
fn policy_preserves_defaults_and_rejects_before_output_allocation() {
    assert_eq!(
        CircuitFlattenLimits::DEFAULT_MAX_EXPANDED_OPERATIONS,
        1_000_000
    );
    assert_eq!(
        CircuitFlattenLimits::default().max_expanded_operations(),
        CircuitFlattenLimits::DEFAULT_MAX_EXPANDED_OPERATIONS
    );
    assert_eq!(
        CircuitFlattenLimits::default().max_expanded_targets(),
        CircuitFlattenLimits::DEFAULT_MAX_EXPANDED_TARGETS
    );
    assert_eq!(
        CircuitFlattenLimits::default().max_expanded_arguments(),
        CircuitFlattenLimits::DEFAULT_MAX_EXPANDED_ARGUMENTS
    );
    assert_eq!(
        CircuitFlattenLimits::default().max_materialized_bytes(),
        CircuitFlattenLimits::DEFAULT_MAX_MATERIALIZED_BYTES
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

    let accepted = circuit(
        "
        REPEAT 3 {
            H 0
        }
    ",
    );
    let accepted = flattened_circuit_with_limits(&accepted, limits!(3))
        .expect("exact custom maximum should flatten");

    assert_eq!(accepted.to_stim_string(), "H 0 0 0\n");

    let rejected = circuit(
        "
        REPEAT 4 {
            H 0
        }
    ",
    );
    let rejected = flattened_circuit_with_limits(&rejected, limits!(3))
        .expect_err("first operation above the custom maximum should fail");

    assert_eq!(
        rejected.to_string(),
        "invalid flattened circuit operation count value 4 exceeds current materialized limit 3"
    );
    let resource = rejected
        .resource_limit_error()
        .expect("flattening rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::CircuitFlatten);
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);

    let excessive = circuit(
        "
        REPEAT 1000001 {
            H 0
        }
    ",
    );
    let existing_error = flattened_circuit(&excessive)
        .expect_err("existing default rejection")
        .to_string();
    let limited_error = flattened_circuit_with_limits(&excessive, CircuitFlattenLimits::default())
        .expect_err("limited default rejection")
        .to_string();

    assert_eq!(limited_error, existing_error);
    assert_eq!(
        limited_error,
        "invalid flattened circuit operation count value 1000001 exceeds current materialized limit 1000000"
    );

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
            let error = flattened_circuit_with_limits(input, limits!(3))
                .expect_err("expanded count must reject before output materialization");
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
fn retained_payload_dimensions_have_exact_boundaries() {
    let wide = circuit("REPEAT 3 {\nX_ERROR(0.125) 0 1\n}\n");
    let base = CircuitFlattenLimits::default()
        .with_max_expanded_operations(3)
        .with_max_expanded_targets(6)
        .with_max_expanded_arguments(3);
    flattened_circuit_with_limits(&wide, base)
        .expect("exact target and argument boundaries should materialize");

    for (limits, expected_resource, actual, limit) in [
        (
            base.with_max_expanded_targets(5),
            ResourceKind::TargetOccurrences,
            6,
            5,
        ),
        (
            base.with_max_expanded_arguments(2),
            ResourceKind::ArgumentValues,
            3,
            2,
        ),
    ] {
        let error = flattened_circuit_with_limits(&wide, limits)
            .expect_err("the first retained payload above its boundary must reject");
        let resource = error
            .resource_limit_error()
            .expect("payload rejection should remain typed");
        assert_eq!(resource.operation(), ResourceOperation::CircuitFlatten);
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(resource.actual(), actual);
        assert_eq!(resource.limit(), limit);
    }

    let one = circuit("H 0\n");
    let exact_bytes = (std::mem::size_of::<CircuitItem>() + std::mem::size_of::<Target>()) as u64;
    flattened_circuit_with_limits(
        &one,
        CircuitFlattenLimits::default().with_max_materialized_bytes(exact_bytes),
    )
    .expect("the exact conservative byte boundary should materialize");
    let error = flattened_circuit_with_limits(
        &one,
        CircuitFlattenLimits::default().with_max_materialized_bytes(exact_bytes - 1),
    )
    .expect_err("the first byte below the conservative requirement must reject");
    let resource = error
        .resource_limit_error()
        .expect("byte rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(resource.actual(), exact_bytes);
    assert_eq!(resource.limit(), exact_bytes - 1);
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

    let error = flattened_circuit_with_limits(&overflowing, limits!(u64::MAX))
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

        let error = flattened_circuit_with_limits(&overflowing, limits!(u64::MAX))
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
    let limits = limits!(u64::MAX);

    for result in [
        flattened_circuit_with_limits(&enormous, limits).map(|_| ()),
        flattened_circuit_operations_with_limits(&enormous, limits).map(|_| ()),
    ] {
        let error = result.expect_err("platform vector capacity must reject before allocation");
        let resource = error
            .resource_limit_error()
            .expect("platform materialization rejection should remain typed");
        assert_eq!(resource.operation(), ResourceOperation::CircuitFlatten);
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
        flattened_circuit_with_limits(&exact, limits!(1))
            .expect("the fixed recursive envelope is accepted")
            .to_stim_string(),
        "H 0\n"
    );
    assert_eq!(
        flattened_circuit_operations_with_limits(&exact, limits!(1))
            .expect("the operation-vector adapter uses the same admission")
            .len(),
        1
    );

    for result in [
        flattened_circuit_with_limits(&nested(RepeatNestingLimit::HARD_MAX + 1), limits!(1))
            .map(|_| ()),
        flattened_circuit_operations_with_limits(
            &nested(RepeatNestingLimit::HARD_MAX + 1),
            limits!(1),
        )
        .map(|_| ()),
    ] {
        let error = result.expect_err("the first unsafe programmatic nesting level must reject");
        let resource = error
            .resource_limit_error()
            .expect("flatten nesting rejection should expose typed context");
        assert_eq!(resource.operation(), ResourceOperation::CircuitFlatten);
        assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), (RepeatNestingLimit::HARD_MAX + 1) as u64);
        assert_eq!(resource.limit(), RepeatNestingLimit::HARD_MAX as u64);
    }
}
