#![allow(
    clippy::expect_used,
    reason = "hostile-nesting regressions use direct fixture assertions"
)]

use stab_model::{Circuit, RepeatBlock, RepeatCount};

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
            assert_eq!(format!("{body:?}"), "Circuit { top_level_items: 1, .. }");
            drop(cloned);
            drop(body);
        })
        .expect("spawn small-stack circuit worker")
        .join()
        .expect("deep circuit clone, comparison, and drop remain stack safe");
}
