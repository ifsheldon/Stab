use crate::{CircuitError, CircuitInstruction, CircuitResult, circuit_tableau::gate_tableau};

use super::{Analyzer, AnalyzerPauli, NoiseEffect};

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
            self.expand_pending_single_qubit_channels_through_two_qubit_clifford(left, right)?;
            for pending in &mut self.pending_errors {
                pending.apply_two_qubit_tableau(gate_name, left, right, &tableau)?;
            }
            self.observable_sensitivity
                .apply_two_qubit_tableau(gate_name, left, right, &tableau)?;
        }
        Ok(())
    }

    fn expand_pending_single_qubit_channels_through_two_qubit_clifford(
        &mut self,
        left: crate::QubitId,
        right: crate::QubitId,
    ) -> CircuitResult<()> {
        let pending_channels = std::mem::take(&mut self.pending_pauli_channels);
        for pending in pending_channels {
            if pending.qubit != left && pending.qubit != right {
                self.pending_pauli_channels.push(pending);
                continue;
            }

            let mut group_id = None;
            for (probability, pauli) in [
                (pending.x_probability, AnalyzerPauli::X),
                (pending.y_probability, AnalyzerPauli::Y),
                (pending.z_probability, AnalyzerPauli::Z),
            ] {
                if probability.get() == 0.0 {
                    continue;
                }
                let group = match group_id {
                    Some(group) => group,
                    None => {
                        let group = self.allocate_disjoint_group_id()?;
                        group_id = Some(group);
                        group
                    }
                };
                self.push_pending_error(
                    probability,
                    vec![NoiseEffect {
                        qubit: pending.qubit,
                        pauli,
                    }],
                    Vec::new(),
                    Some(group),
                    pending.tag.clone(),
                );
            }
        }
        Ok(())
    }
}
