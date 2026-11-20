//! The circuit's side of the walks shared with the DEM in [`crate::model_tree`].

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};
use crate::model_tag::ItemTag;
use crate::model_tree::{ModelTree, Node, trees_equal};
use crate::{AbsoluteTolerance, ModelDialect, approximate::arguments_approx_equal};

impl ModelTree for Circuit {
    const DIALECT: ModelDialect = ModelDialect::StimCircuit;
    const BYTE_PARSER: &'static str = "Stim byte parser";

    type Item = CircuitItem;
    type Instruction = CircuitInstruction;
    type Repeat = RepeatBlock;
    type Tag = ItemTag;

    fn node(item: &CircuitItem) -> Node<&CircuitInstruction, &RepeatBlock> {
        match item {
            CircuitItem::Instruction(instruction) => Node::Instruction(instruction),
            CircuitItem::RepeatBlock(repeat) => Node::Repeat(repeat),
        }
    }

    fn node_mut(item: &mut CircuitItem) -> Node<&mut CircuitInstruction, &mut RepeatBlock> {
        match item {
            CircuitItem::Instruction(instruction) => Node::Instruction(instruction),
            CircuitItem::RepeatBlock(repeat) => Node::Repeat(repeat),
        }
    }

    fn body(repeat: &RepeatBlock) -> &[CircuitItem] {
        &repeat.body.items
    }

    fn body_mut(repeat: &mut RepeatBlock) -> &mut Vec<CircuitItem> {
        &mut repeat.body.items
    }

    fn tag(item: &CircuitItem) -> &ItemTag {
        match item {
            CircuitItem::Instruction(instruction) => &instruction.tag,
            CircuitItem::RepeatBlock(repeat) => &repeat.tag,
        }
    }

    fn tag_mut(item: &mut CircuitItem) -> &mut ItemTag {
        match item {
            CircuitItem::Instruction(instruction) => &mut instruction.tag,
            CircuitItem::RepeatBlock(repeat) => &mut repeat.tag,
        }
    }

    fn instruction_counts(instruction: &CircuitInstruction) -> (usize, usize) {
        (instruction.targets().len(), instruction.args().len())
    }

    fn repeat_headers_equal(left: &RepeatBlock, right: &RepeatBlock) -> bool {
        left.repeat_count == right.repeat_count && left.tag == right.tag
    }

    fn instruction_item(instruction: CircuitInstruction) -> CircuitItem {
        CircuitItem::Instruction(instruction)
    }

    fn repeat_item_with_body(repeat: &RepeatBlock, body: Vec<CircuitItem>) -> CircuitItem {
        CircuitItem::RepeatBlock(RepeatBlock {
            repeat_count: repeat.repeat_count,
            body: Circuit { items: body },
            tag: repeat.tag.clone(),
        })
    }
}

pub(super) fn circuits_equal(left: &Circuit, right: &Circuit) -> bool {
    trees_equal::<Circuit>(&left.items, &right.items, |left, right| left == right)
}

pub(super) fn circuits_approx_equal(
    left: &Circuit,
    right: &Circuit,
    tolerance: AbsoluteTolerance,
) -> bool {
    trees_equal::<Circuit>(&left.items, &right.items, |left, right| {
        left.gate == right.gate
            && left.targets == right.targets
            && left.tag == right.tag
            && arguments_approx_equal(&left.args, &right.args, tolerance)
    })
}
