use super::{DemItem, DetectorErrorModel};

struct CloneFrame<'a> {
    source: &'a DetectorErrorModel,
    index: usize,
    output: DetectorErrorModel,
    owning_repeat: Option<&'a super::DemRepeatBlock>,
}

pub(super) fn clone_model(model: &DetectorErrorModel) -> DetectorErrorModel {
    let mut stack = vec![CloneFrame {
        source: model,
        index: 0,
        output: DetectorErrorModel::new(),
        owning_repeat: None,
    }];
    loop {
        let Some(frame) = stack.last_mut() else {
            unreachable!("the root DEM clone frame remains until completion");
        };
        if let Some(item) = frame.source.items.get(frame.index) {
            frame.index += 1;
            match item {
                DemItem::Instruction(instruction) => {
                    frame.output.push_instruction(instruction.clone());
                }
                DemItem::RepeatBlock(repeat) => stack.push(CloneFrame {
                    source: &repeat.body,
                    index: 0,
                    output: DetectorErrorModel::new(),
                    owning_repeat: Some(repeat),
                }),
            }
            continue;
        }

        let Some(completed) = stack.pop() else {
            unreachable!("a completed DEM clone frame is available");
        };
        let Some(repeat) = completed.owning_repeat else {
            return completed.output;
        };
        let cloned_repeat = super::DemRepeatBlock {
            repeat_count: repeat.repeat_count,
            body: completed.output,
            tag: repeat.tag.clone(),
        };
        let Some(parent) = stack.last_mut() else {
            unreachable!("a repeated DEM clone body has a parent frame");
        };
        parent.output.push_repeat_block(cloned_repeat);
    }
}

pub(super) fn drop_items(items: &mut Vec<DemItem>) {
    drop_items_bounded(items, 0);
}

const SHALLOW_DROP_DEPTH: usize = 32;

fn drop_items_bounded(items: &mut Vec<DemItem>, depth: usize) {
    if items.is_empty() {
        return;
    }
    if depth == SHALLOW_DROP_DEPTH {
        drop_items_iterative(items);
        return;
    }
    // Native vector destruction is substantially cheaper for ordinary shallow models. Empty each
    // nested body first, then retain the iterative path before recursion can exhaust the stack.
    for item in items.iter_mut() {
        if let DemItem::RepeatBlock(repeat) = item {
            drop_items_bounded(&mut repeat.body.items, depth + 1);
        }
    }
    items.clear();
}

fn drop_items_iterative(items: &mut Vec<DemItem>) {
    let mut pending = std::mem::take(items);
    while let Some(mut item) = pending.pop() {
        // Borrow the variant because moving fields out of a type containing a
        // DetectorErrorModel is forbidden once the model implements Drop.
        if let DemItem::RepeatBlock(repeat) = &mut item {
            append_items(&mut pending, &mut repeat.body);
        }
    }
}

fn append_items(pending: &mut Vec<DemItem>, model: &mut DetectorErrorModel) {
    pending.append(&mut model.items);
}
