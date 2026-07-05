use crate::{CircuitError, CircuitInstruction, CircuitResult, QubitId, Target};

use super::{Analyzer, AnalyzerPauli, NoiseEffect, PendingError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControlledPauliAction {
    QuantumControlledPauli {
        left: QubitId,
        right: QubitId,
        left_basis: AnalyzerPauli,
        right_basis: AnalyzerPauli,
    },
    MeasurementFeedback {
        record_offset: i32,
        qubit: QubitId,
        pauli: AnalyzerPauli,
    },
    NoEffect,
}

impl Analyzer {
    pub(super) fn apply_controlled_pauli(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        for group in instruction.target_groups() {
            let [first, second] = group else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected paired targets during error analysis"
                )));
            };
            match controlled_pauli_action(gate_name, first, second)? {
                ControlledPauliAction::QuantumControlledPauli {
                    left,
                    right,
                    left_basis,
                    right_basis,
                } => {
                    self.apply_quantum_controlled_pauli(left, right, left_basis, right_basis)?;
                }
                ControlledPauliAction::MeasurementFeedback {
                    record_offset,
                    qubit,
                    pauli,
                } => self.apply_measurement_feedback(record_offset, qubit, pauli)?,
                ControlledPauliAction::NoEffect => {}
            }
        }
        Ok(())
    }

    fn apply_measurement_feedback(
        &mut self,
        record_offset: i32,
        qubit: QubitId,
        pauli: AnalyzerPauli,
    ) -> CircuitResult<()> {
        let measurement = self.measurement_index_from_offset(record_offset)?;
        let effect = NoiseEffect { qubit, pauli };
        let observables = self
            .observable_sensitivity
            .flipped_observables(std::slice::from_ref(&effect));

        let mut pending_errors = Vec::new();
        let mut completed_errors = Vec::new();
        for mut pending in self.pending_errors.drain(..) {
            if pending.flips_measurement_record(measurement) {
                apply_feedback_effect(&mut pending, effect.clone(), &observables);
            }
            if pending.effects.is_empty() {
                completed_errors.push(pending);
            } else {
                pending_errors.push(pending);
            }
        }
        for mut completed in self.completed_errors.drain(..) {
            if completed.flips_measurement_record(measurement) {
                apply_feedback_effect(&mut completed, effect.clone(), &observables);
            }
            if completed.effects.is_empty() {
                completed_errors.push(completed);
            } else {
                pending_errors.push(completed);
            }
        }
        self.pending_errors = pending_errors;
        self.completed_errors = completed_errors;
        Ok(())
    }
}

pub(super) fn controlled_pauli_action(
    gate_name: &str,
    first: &Target,
    second: &Target,
) -> CircuitResult<ControlledPauliAction> {
    let (left_basis, right_basis) = controlled_pauli_bases(gate_name)?;
    if let (Some(left), Some(right)) = (first.qubit_id(), second.qubit_id()) {
        return Ok(ControlledPauliAction::QuantumControlledPauli {
            left,
            right,
            left_basis,
            right_basis,
        });
    }
    match gate_name {
        "CX" => z_controlled_feedback_action(gate_name, first, second, AnalyzerPauli::X),
        "CY" => z_controlled_feedback_action(gate_name, first, second, AnalyzerPauli::Y),
        "CZ" => symmetric_z_controlled_action(gate_name, first, second),
        "XCZ" => reversed_z_controlled_action(gate_name, first, second, AnalyzerPauli::X),
        "YCZ" => reversed_z_controlled_action(gate_name, first, second, AnalyzerPauli::Y),
        _ => Ok(ControlledPauliAction::NoEffect),
    }
}

fn controlled_pauli_bases(gate_name: &str) -> CircuitResult<(AnalyzerPauli, AnalyzerPauli)> {
    match gate_name {
        "CX" => Ok((AnalyzerPauli::Z, AnalyzerPauli::X)),
        "CY" => Ok((AnalyzerPauli::Z, AnalyzerPauli::Y)),
        "CZ" => Ok((AnalyzerPauli::Z, AnalyzerPauli::Z)),
        "XCX" => Ok((AnalyzerPauli::X, AnalyzerPauli::X)),
        "XCY" => Ok((AnalyzerPauli::X, AnalyzerPauli::Y)),
        "XCZ" => Ok((AnalyzerPauli::X, AnalyzerPauli::Z)),
        "YCX" => Ok((AnalyzerPauli::Y, AnalyzerPauli::X)),
        "YCY" => Ok((AnalyzerPauli::Y, AnalyzerPauli::Y)),
        "YCZ" => Ok((AnalyzerPauli::Y, AnalyzerPauli::Z)),
        name => Err(CircuitError::invalid_detector_error_model(format!(
            "{name} is not a supported controlled Pauli analyzer gate"
        ))),
    }
}

fn z_controlled_feedback_action(
    gate_name: &str,
    control: &Target,
    target: &Target,
    pauli: AnalyzerPauli,
) -> CircuitResult<ControlledPauliAction> {
    let Some(qubit) = target.qubit_id() else {
        return Err(non_qubit_target_error(gate_name, "target", target));
    };
    if control.sweep_bit_id().is_some() {
        return Ok(ControlledPauliAction::NoEffect);
    }
    if let Some(record_offset) = control.measurement_record_offset() {
        return Ok(ControlledPauliAction::MeasurementFeedback {
            record_offset: record_offset.get(),
            qubit,
            pauli,
        });
    }
    Ok(ControlledPauliAction::NoEffect)
}

fn reversed_z_controlled_action(
    gate_name: &str,
    target: &Target,
    control: &Target,
    pauli: AnalyzerPauli,
) -> CircuitResult<ControlledPauliAction> {
    let qubit = require_qubit(gate_name, "target", target)?;
    if control.sweep_bit_id().is_some() {
        return Ok(ControlledPauliAction::NoEffect);
    }
    if let Some(record_offset) = control.measurement_record_offset() {
        return Ok(ControlledPauliAction::MeasurementFeedback {
            record_offset: record_offset.get(),
            qubit,
            pauli,
        });
    }
    Ok(ControlledPauliAction::NoEffect)
}

fn symmetric_z_controlled_action(
    gate_name: &str,
    first: &Target,
    second: &Target,
) -> CircuitResult<ControlledPauliAction> {
    match (
        first.measurement_record_offset(),
        second.measurement_record_offset(),
        first.sweep_bit_id(),
        second.sweep_bit_id(),
    ) {
        (Some(record_offset), None, None, None) => {
            let qubit = require_qubit(gate_name, "target", second)?;
            Ok(ControlledPauliAction::MeasurementFeedback {
                record_offset: record_offset.get(),
                qubit,
                pauli: AnalyzerPauli::Z,
            })
        }
        (None, Some(record_offset), None, None) => {
            let qubit = require_qubit(gate_name, "target", first)?;
            Ok(ControlledPauliAction::MeasurementFeedback {
                record_offset: record_offset.get(),
                qubit,
                pauli: AnalyzerPauli::Z,
            })
        }
        (None, None, Some(_), None) => {
            require_qubit(gate_name, "target", second)?;
            Ok(ControlledPauliAction::NoEffect)
        }
        (None, None, None, Some(_)) => {
            require_qubit(gate_name, "target", first)?;
            Ok(ControlledPauliAction::NoEffect)
        }
        (Some(_), Some(_), _, _) | (None, None, Some(_), Some(_)) => {
            Ok(ControlledPauliAction::NoEffect)
        }
        _ => Ok(ControlledPauliAction::NoEffect),
    }
}

fn apply_feedback_effect(pending: &mut PendingError, effect: NoiseEffect, observables: &[u64]) {
    pending.toggle_effect(effect);
    pending.toggle_observables(observables);
}

fn require_qubit(gate_name: &str, role: &str, target: &Target) -> CircuitResult<QubitId> {
    target
        .qubit_id()
        .ok_or_else(|| non_qubit_target_error(gate_name, role, target))
}

fn non_qubit_target_error(gate_name: &str, role: &str, target: &Target) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!(
        "{gate_name} {role} {target} is not a qubit"
    ))
}
