use super::{DemItem, DetectorErrorModel};
use crate::{AbsoluteTolerance, approximate::arguments_approx_equal};

pub(super) fn models_equal(left: &DetectorErrorModel, right: &DetectorErrorModel) -> bool {
    compare_models(left, right, |left, right| left == right)
}

pub(super) fn models_approx_equal(
    left: &DetectorErrorModel,
    right: &DetectorErrorModel,
    tolerance: AbsoluteTolerance,
) -> bool {
    compare_models(left, right, |left, right| {
        left.kind == right.kind
            && left.targets == right.targets
            && left.tag == right.tag
            && arguments_approx_equal(&left.args, &right.args, tolerance)
    })
}

fn compare_models(
    left: &DetectorErrorModel,
    right: &DetectorErrorModel,
    instructions_equal: impl Fn(&super::DemInstruction, &super::DemInstruction) -> bool,
) -> bool {
    let mut pending = vec![(left.items.as_slice(), right.items.as_slice())];
    while let Some((left_items, right_items)) = pending.pop() {
        if left_items.len() != right_items.len() {
            return false;
        }
        for (left_item, right_item) in left_items.iter().zip(right_items) {
            match (left_item, right_item) {
                (DemItem::Instruction(left), DemItem::Instruction(right))
                    if instructions_equal(left, right) => {}
                (DemItem::RepeatBlock(left), DemItem::RepeatBlock(right))
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
