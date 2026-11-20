//! The DEM's side of the walks shared with the circuit in [`crate::model_tree`].

use super::{DemInstruction, DemItem, DemRepeatBlock, DetectorErrorModel, tag::DemTag};
use crate::model_tree::{ModelTree, Node, trees_equal};
use crate::{AbsoluteTolerance, ModelDialect, approximate::arguments_approx_equal};

impl ModelTree for DetectorErrorModel {
    const DIALECT: ModelDialect = ModelDialect::DetectorErrorModel;
    const BYTE_PARSER: &'static str = "DEM byte parser";

    type Item = DemItem;
    type Instruction = DemInstruction;
    type Repeat = DemRepeatBlock;
    type Tag = Option<DemTag>;

    fn node(item: &DemItem) -> Node<&DemInstruction, &DemRepeatBlock> {
        match item {
            DemItem::Instruction(instruction) => Node::Instruction(instruction),
            DemItem::RepeatBlock(repeat) => Node::Repeat(repeat),
        }
    }

    fn node_mut(item: &mut DemItem) -> Node<&mut DemInstruction, &mut DemRepeatBlock> {
        match item {
            DemItem::Instruction(instruction) => Node::Instruction(instruction),
            DemItem::RepeatBlock(repeat) => Node::Repeat(repeat),
        }
    }

    fn body(repeat: &DemRepeatBlock) -> &[DemItem] {
        &repeat.body.items
    }

    fn body_mut(repeat: &mut DemRepeatBlock) -> &mut Vec<DemItem> {
        &mut repeat.body.items
    }

    fn tag(item: &DemItem) -> &Option<DemTag> {
        match item {
            DemItem::Instruction(instruction) => &instruction.tag,
            DemItem::RepeatBlock(repeat) => &repeat.tag,
        }
    }

    fn tag_mut(item: &mut DemItem) -> &mut Option<DemTag> {
        match item {
            DemItem::Instruction(instruction) => &mut instruction.tag,
            DemItem::RepeatBlock(repeat) => &mut repeat.tag,
        }
    }

    fn instruction_counts(instruction: &DemInstruction) -> (usize, usize) {
        (instruction.targets().len(), instruction.args().len())
    }

    fn repeat_headers_equal(left: &DemRepeatBlock, right: &DemRepeatBlock) -> bool {
        left.repeat_count == right.repeat_count && left.tag == right.tag
    }

    fn instruction_item(instruction: DemInstruction) -> DemItem {
        DemItem::Instruction(instruction)
    }

    fn repeat_item_with_body(repeat: &DemRepeatBlock, body: Vec<DemItem>) -> DemItem {
        DemItem::RepeatBlock(DemRepeatBlock {
            repeat_count: repeat.repeat_count,
            body: DetectorErrorModel { items: body },
            tag: repeat.tag.clone(),
        })
    }
}

pub(super) fn models_equal(left: &DetectorErrorModel, right: &DetectorErrorModel) -> bool {
    trees_equal::<DetectorErrorModel>(&left.items, &right.items, |left, right| left == right)
}

pub(super) fn models_approx_equal(
    left: &DetectorErrorModel,
    right: &DetectorErrorModel,
    tolerance: AbsoluteTolerance,
) -> bool {
    trees_equal::<DetectorErrorModel>(&left.items, &right.items, |left, right| {
        left.same_except_args(right) && arguments_approx_equal(left.args(), right.args(), tolerance)
    })
}
