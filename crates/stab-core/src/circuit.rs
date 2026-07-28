pub use stab_model::{
    Circuit, CircuitFlattenedInstructionIter, CircuitFlattenedInstructionRevIter,
    CircuitInstruction, CircuitItem, RepeatBlock,
};

use crate::{CircuitResult, Gate, RepeatCount, Target};

#[derive(Debug)]
pub(crate) struct CircuitAssembler(stab_model::advanced::CircuitBuilder);

impl CircuitAssembler {
    pub(crate) fn new() -> Self {
        Self(stab_model::advanced::CircuitBuilder::new())
    }

    pub(crate) fn from_unfused_items(items: Vec<CircuitItem>) -> Self {
        Self(stab_model::advanced::CircuitBuilder::from_unfused_items(
            items,
        ))
    }

    pub(crate) fn try_reserve_exact(&mut self, additional: usize) -> CircuitResult<()> {
        self.0.try_reserve_exact(additional).map_err(Into::into)
    }

    pub(crate) fn try_append_instruction(
        &mut self,
        instruction: CircuitInstruction,
    ) -> CircuitResult<()> {
        self.0
            .try_append_instruction(instruction)
            .map_err(Into::into)
    }

    pub(crate) fn try_append_repeat_block(&mut self, repeat: RepeatBlock) -> CircuitResult<()> {
        self.0.try_append_repeat_block(repeat).map_err(Into::into)
    }

    pub(crate) fn finish(self) -> Circuit {
        self.0.finish()
    }
}

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

pub(crate) fn circuit_instruction_without_tag(
    instruction: &CircuitInstruction,
) -> CircuitInstruction {
    stab_model::advanced::circuit_instruction_without_tag(instruction)
}

pub(crate) fn circuit_simulated_qubit_count(circuit: &Circuit) -> usize {
    stab_model::advanced::circuit_simulated_qubit_count(circuit)
}

pub(crate) fn circuit_instruction_measurement_result_count(
    instruction: &CircuitInstruction,
) -> usize {
    stab_model::advanced::circuit_instruction_measurement_result_count(instruction)
}
