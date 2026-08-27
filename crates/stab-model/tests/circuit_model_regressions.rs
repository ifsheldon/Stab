#![allow(
    clippy::expect_used,
    reason = "hostile-nesting regressions use direct fixture assertions"
)]

use stab_model::{Circuit, ModelError, RepeatBlock, RepeatCount, ValidationError};

/// WS6 item 7: deeply nested API-built circuits must clone, compare, and drop
/// without exhausting the stack; parse-path nesting is capped, so the
/// programmatic path is the hostile one. The old derived Drop/Clone/PartialEq
/// recursed per nesting level and aborted the process.
#[test]
fn deeply_programmatic_circuit_clones_compares_and_drops_on_a_small_stack() {
    std::thread::Builder::new()
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut body = Circuit::new();
            for _ in 0..4_096 {
                let mut outer = Circuit::new();
                outer.append_repeat_block(RepeatBlock::new(
                    RepeatCount::try_new(1).expect("unit repeat count"),
                    body,
                    None,
                ));
                body = outer;
            }
            let cloned = body.clone();
            assert_eq!(cloned, body);
            drop(cloned);
            drop(body);
        })
        .expect("spawn small-stack circuit worker")
        .join()
        .expect("deep circuit clone, comparison, and drop remain stack safe");
}

#[test]
fn circuit_counts_accept_u64_maximum_and_reject_the_first_overflow() {
    let maximum_repeat = RepeatCount::try_new(u64::MAX).expect("maximum repeat count");
    let mut exact_maximum = Circuit::new();
    let exact_body =
        Circuit::from_stim_bytes(b"M 0\nDETECTOR rec[-1]\nTICK\n").expect("exact-count body");
    exact_maximum.append_repeat_block(RepeatBlock::new(maximum_repeat, exact_body, None));

    assert_eq!(
        exact_maximum.count_measurements().expect("maximum count"),
        u64::MAX
    );
    assert_eq!(
        exact_maximum.count_detectors().expect("maximum detectors"),
        u64::MAX
    );
    assert_eq!(
        exact_maximum.count_ticks().expect("maximum ticks"),
        u64::MAX
    );

    let mut overflowing = Circuit::new();
    let overflowing_body = Circuit::from_stim_bytes(b"M 0 1\n").expect("overflowing body");
    overflowing.append_repeat_block(RepeatBlock::new(maximum_repeat, overflowing_body, None));

    assert_eq!(
        overflowing.count_measurements(),
        Err(ModelError::Validation(
            ValidationError::CircuitCountOverflow
        ))
    );
}
