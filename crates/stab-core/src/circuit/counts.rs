use super::{Circuit, CircuitInstruction, CircuitItem};
use crate::Target;

impl Circuit {
    pub fn count_qubits(&self) -> usize {
        self.items
            .iter()
            .map(CircuitItem::count_qubits)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn count_simulated_qubits(&self) -> usize {
        self.items
            .iter()
            .map(CircuitItem::count_simulated_qubits)
            .max()
            .unwrap_or(0)
    }
}

impl CircuitItem {
    fn count_qubits(&self) -> usize {
        match self {
            Self::Instruction(instruction) => instruction.count_qubits(),
            Self::RepeatBlock(repeat) => repeat.body().count_qubits(),
        }
    }

    fn count_simulated_qubits(&self) -> usize {
        match self {
            Self::Instruction(instruction) => instruction.count_simulated_qubits(),
            Self::RepeatBlock(repeat) => repeat.body().count_simulated_qubits(),
        }
    }
}

impl CircuitInstruction {
    fn count_qubits(&self) -> usize {
        self.targets
            .iter()
            .filter_map(Target::qubit_id)
            .map(|qubit| qubit.get() as usize + 1)
            .max()
            .unwrap_or(0)
    }

    fn count_simulated_qubits(&self) -> usize {
        if self.gate.targets_are_measurement_pads() {
            0
        } else {
            self.count_qubits()
        }
    }
}
