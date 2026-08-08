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
}

impl CircuitItem {
    fn count_qubits(&self) -> usize {
        match self {
            Self::Instruction(instruction) => instruction.count_qubits(),
            Self::RepeatBlock(repeat) => repeat.body().count_qubits(),
        }
    }
}

impl CircuitInstruction {
    pub(crate) fn measurement_result_count(&self) -> usize {
        if self.gate.produces_measurements() {
            self.target_group_count()
        } else {
            0
        }
    }

    fn count_qubits(&self) -> usize {
        // Pad targets are metadata-only values, not qubits; see Gate::targets_are_pad_values.
        if self.gate.targets_are_pad_values() {
            return 0;
        }
        self.targets
            .iter()
            .filter_map(Target::qubit_id)
            .map(|qubit| qubit.get() as usize + 1)
            .max()
            .unwrap_or(0)
    }
}
