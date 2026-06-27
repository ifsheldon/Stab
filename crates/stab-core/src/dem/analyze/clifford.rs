use crate::{
    CircuitError, CircuitInstruction, CircuitResult, QubitId, circuit_tableau::gate_tableau,
};

use super::Analyzer;

impl Analyzer {
    pub(super) fn apply_two_qubit_clifford(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        let tableau = gate_tableau(gate_name).map_err(|error| {
            CircuitError::invalid_detector_error_model(format!(
                "failed to load {gate_name} tableau during error analysis: {error}"
            ))
        })?;
        for group in instruction.target_groups() {
            let [left, right] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired qubit targets"
                )));
            };
            let left = left.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {left} is not a qubit"
                ))
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} target {right} is not a qubit"
                ))
            })?;
            self.reject_pending_single_qubit_channels_through_two_qubit_clifford(
                gate_name, left, right,
            )?;
            for pending in &mut self.pending_errors {
                pending.apply_two_qubit_tableau(gate_name, left, right, &tableau)?;
            }
            self.observable_sensitivity
                .apply_two_qubit_tableau(gate_name, left, right, &tableau)?;
        }
        Ok(())
    }

    fn reject_pending_single_qubit_channels_through_two_qubit_clifford(
        &self,
        gate_name: &str,
        left: QubitId,
        right: QubitId,
    ) -> CircuitResult<()> {
        if self
            .pending_pauli_channels
            .iter()
            .any(|pending| pending.qubit == left || pending.qubit == right)
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "analyze_errors does not yet support propagating pending single-qubit Pauli channels through {gate_name}"
            )));
        }
        Ok(())
    }
}
