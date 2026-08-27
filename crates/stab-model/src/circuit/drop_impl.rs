use super::{Circuit, CircuitItem};

struct CloneFrame<'a> {
    source: &'a Circuit,
    index: usize,
    output: Circuit,
    owning_repeat: Option<&'a super::RepeatBlock>,
}

pub(super) fn clone_circuit(circuit: &Circuit) -> Circuit {
    let mut stack = vec![CloneFrame {
        source: circuit,
        index: 0,
        output: Circuit::new(),
        owning_repeat: None,
    }];
    loop {
        let Some(frame) = stack.last_mut() else {
            unreachable!("the root circuit clone frame remains until completion");
        };
        if let Some(item) = frame.source.items.get(frame.index) {
            frame.index += 1;
            match item {
                CircuitItem::Instruction(instruction) => {
                    frame
                        .output
                        .items
                        .push(CircuitItem::Instruction(instruction.clone()));
                }
                CircuitItem::RepeatBlock(repeat) => stack.push(CloneFrame {
                    source: &repeat.body,
                    index: 0,
                    output: Circuit::new(),
                    owning_repeat: Some(repeat),
                }),
            }
            continue;
        }

        let Some(completed) = stack.pop() else {
            unreachable!("a completed circuit clone frame is available");
        };
        let Some(repeat) = completed.owning_repeat else {
            return completed.output;
        };
        let cloned_repeat = super::RepeatBlock {
            repeat_count: repeat.repeat_count,
            body: completed.output,
            tag: repeat.tag.clone(),
        };
        let Some(parent) = stack.last_mut() else {
            unreachable!("a repeated circuit clone body has a parent frame");
        };
        parent
            .output
            .items
            .push(CircuitItem::RepeatBlock(cloned_repeat));
    }
}

pub(super) fn drop_items(items: &mut Vec<CircuitItem>) {
    drop_items_bounded(items, 0);
}

const SHALLOW_DROP_DEPTH: usize = 32;

fn drop_items_bounded(items: &mut Vec<CircuitItem>, depth: usize) {
    if items.is_empty() {
        return;
    }
    if depth == SHALLOW_DROP_DEPTH {
        drop_items_iterative(items);
        return;
    }
    // Native vector destruction is substantially cheaper for ordinary shallow
    // circuits. Empty each nested body first, then retain the iterative path
    // before recursion can exhaust the stack.
    for item in items.iter_mut() {
        if let CircuitItem::RepeatBlock(repeat) = item {
            drop_items_bounded(&mut repeat.body.items, depth + 1);
        }
    }
    items.clear();
}

fn drop_items_iterative(items: &mut Vec<CircuitItem>) {
    let mut pending = std::mem::take(items);
    while let Some(mut item) = pending.pop() {
        // Borrow the variant because moving fields out of a type containing a
        // Circuit is forbidden once the circuit implements Drop.
        if let CircuitItem::RepeatBlock(repeat) = &mut item {
            pending.append(&mut repeat.body.items);
        }
    }
}
