use super::{Circuit, CircuitInstruction, CircuitItem};
use crate::Target;

impl Circuit {
    pub fn count_qubits(&self) -> usize {
        self.count_qubits_with(|_| true)
    }

    pub(crate) fn simulated_qubit_count(&self) -> usize {
        self.count_qubits_with(|instruction| !instruction.gate.targets_are_pad_values())
    }

    fn count_qubits_with(&self, include: impl Fn(&CircuitInstruction) -> bool) -> usize {
        let mut count = 0;
        let mut pending = Vec::new();
        let mut items = self.items.iter();

        loop {
            match items.next() {
                Some(CircuitItem::Instruction(instruction)) => {
                    if include(instruction) {
                        count = count.max(instruction.count_qubits());
                    }
                }
                Some(CircuitItem::RepeatBlock(repeat)) => {
                    pending.push(items);
                    items = repeat.body().items().iter();
                }
                None => {
                    let Some(parent) = pending.pop() else {
                        return count;
                    };
                    items = parent;
                }
            }
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
        self.targets
            .iter()
            .filter_map(Target::qubit_id)
            .map(|qubit| qubit.get() as usize + 1)
            .max()
            .unwrap_or(0)
    }
}
