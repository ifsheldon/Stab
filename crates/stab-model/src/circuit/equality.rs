use super::{Circuit, CircuitItem};
use crate::{AbsoluteTolerance, approximate::arguments_approx_equal};

pub(super) fn circuits_equal(left: &Circuit, right: &Circuit) -> bool {
    compare_circuits(left, right, |left, right| left == right)
}

pub(super) fn circuits_approx_equal(
    left: &Circuit,
    right: &Circuit,
    tolerance: AbsoluteTolerance,
) -> bool {
    compare_circuits(left, right, |left, right| {
        left.gate == right.gate
            && left.targets == right.targets
            && left.tag == right.tag
            && arguments_approx_equal(&left.args, &right.args, tolerance)
    })
}

fn compare_circuits(
    left: &Circuit,
    right: &Circuit,
    instructions_equal: impl Fn(&super::CircuitInstruction, &super::CircuitInstruction) -> bool,
) -> bool {
    let mut pending = vec![(left.items.as_slice(), right.items.as_slice())];
    while let Some((left_items, right_items)) = pending.pop() {
        if left_items.len() != right_items.len() {
            return false;
        }
        for (left_item, right_item) in left_items.iter().zip(right_items) {
            match (left_item, right_item) {
                (CircuitItem::Instruction(left), CircuitItem::Instruction(right))
                    if instructions_equal(left, right) => {}
                (CircuitItem::RepeatBlock(left), CircuitItem::RepeatBlock(right))
                    if left.repeat_count == right.repeat_count && left.tag == right.tag =>
                {
                    pending.push((left.body.items.as_slice(), right.body.items.as_slice()));
                }
                _ => return false,
            }
        }
    }
    true
}
