use crate::{Circuit, CircuitItem, RepeatBlock, circuit::CircuitAssembler};

/// Returns a compact copy of `circuit` with every instruction and repeat-block tag removed.
///
/// Item order, instruction boundaries, arguments, targets, repeat counts, and repeat nesting are
/// preserved. Removing tags does not fuse adjacent instructions that previously had distinct tags.
pub fn circuit_without_tags(circuit: &Circuit) -> Circuit {
    CircuitAssembler::from_unfused_items(
        circuit
            .items()
            .iter()
            .map(|item| match item {
                CircuitItem::Instruction(instruction) => {
                    CircuitItem::Instruction(instruction.without_tag())
                }
                CircuitItem::RepeatBlock(repeat) => CircuitItem::RepeatBlock(RepeatBlock::new(
                    repeat.repeat_count(),
                    circuit_without_tags(repeat.body()),
                    None,
                )),
            })
            .collect(),
    )
    .finish()
}
