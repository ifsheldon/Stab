pub use stab_model::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, RepeatBlock,
};

use crate::{CircuitResult, Gate, RepeatCount, Target};

pub(crate) fn circuit_instruction_with_tag_bytes(
    gate: Gate,
    args: Vec<f64>,
    targets: Vec<Target>,
    tag: Option<&[u8]>,
) -> CircuitResult<CircuitInstruction> {
    stab_model::advanced::circuit_instruction_with_tag_bytes(gate, args, targets, tag)
        .map_err(Into::into)
}

pub(crate) fn repeat_block_with_tag_bytes(
    repeat_count: RepeatCount,
    body: Circuit,
    tag: Option<&[u8]>,
) -> RepeatBlock {
    stab_model::advanced::repeat_block_with_tag_bytes(repeat_count, body, tag)
}
