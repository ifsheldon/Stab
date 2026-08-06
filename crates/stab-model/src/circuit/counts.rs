use super::{Circuit, CircuitInstruction, CircuitItem};
use crate::{GateTargetRule, Target};

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
        match self.gate.canonical_name() {
            "M"
            | "MX"
            | "MY"
            | "MR"
            | "MRX"
            | "MRY"
            | "MPAD"
            | "HERALDED_ERASE"
            | "HERALDED_PAULI_CHANNEL_1" => self.targets.len(),
            "MXX" | "MYY" | "MZZ" | "MPP" => self.target_groups().len(),
            _ => 0,
        }
    }

    fn count_qubits(&self) -> usize {
        // Stim excludes MPAD pad values from qubit counting (circuit_instruction.cc:64-69):
        // pads reserve measurement records, and their 0/1 targets are values, not qubits.
        if self.gate.target_rule() == GateTargetRule::MeasurementPads {
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
