//! Non-recursive printing order of a detector error model.

use super::{DemInstruction, DemItem, DemRepeatBlock, DetectorErrorModel};

/// One step of a canonical-text walk, with the indentation of the line it prints.
pub(super) enum DemTextEvent<'a> {
    Instruction(&'a DemInstruction, usize),
    /// A `repeat` header, printed before its body.
    RepeatOpen(&'a DemRepeatBlock, usize),
    /// The closing brace of a `repeat` block, printed after its body.
    RepeatClose(usize),
}

/// Visits `model` in printing order without recursion.
///
/// The explicit stack holds one frame per enclosing repeat block, so a model nested far deeper
/// than the parser admits prints without exhausting the thread's stack.
pub(super) fn walk_dem_text<'a, E>(
    model: &'a DetectorErrorModel,
    mut visit: impl FnMut(DemTextEvent<'a>) -> Result<(), E>,
) -> Result<(), E> {
    let mut enclosing: Vec<(std::slice::Iter<'a, DemItem>, usize)> = Vec::new();
    let mut items = model.items.iter();
    let mut indent = 0_usize;
    loop {
        match items.next() {
            Some(DemItem::Instruction(instruction)) => {
                visit(DemTextEvent::Instruction(instruction, indent))?;
            }
            Some(DemItem::RepeatBlock(repeat)) => {
                visit(DemTextEvent::RepeatOpen(repeat, indent))?;
                let parent = std::mem::replace(&mut items, repeat.body.items.iter());
                enclosing.push((parent, indent));
                indent = indent.saturating_add(4);
            }
            None => {
                let Some((parent, parent_indent)) = enclosing.pop() else {
                    return Ok(());
                };
                visit(DemTextEvent::RepeatClose(parent_indent))?;
                items = parent;
                indent = parent_indent;
            }
        }
    }
}
