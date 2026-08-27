use stab_model::{Circuit, CircuitInstruction, CircuitItem, Target};

use crate::circuit_flow::transitions::{ReverseFlowTransition, reverse_flow_transition};

pub(super) fn circuit_has_instructions(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(_) => true,
        CircuitItem::RepeatBlock(repeat) => circuit_has_instructions(repeat.body()),
    })
}

pub(super) fn circuit_is_ignored_only(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            matches!(
                reverse_flow_transition(instruction),
                ReverseFlowTransition::Ignored
            )
        }
        CircuitItem::RepeatBlock(repeat) => circuit_is_ignored_only(repeat.body()),
    })
}

pub(super) fn circuit_requires_reverse_flow_solver(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => {
            instruction_requires_reverse_flow_solver(instruction)
        }
        CircuitItem::RepeatBlock(repeat) => circuit_requires_reverse_flow_solver(repeat.body()),
    })
}

pub(super) fn instruction_requires_reverse_flow_solver(instruction: &CircuitInstruction) -> bool {
    let transition = reverse_flow_transition(instruction);
    transition.is_measurement_rich()
        || (matches!(transition, ReverseFlowTransition::ControlledPauli(_))
            && instruction
                .targets()
                .iter()
                .any(Target::is_classical_bit_target))
}
