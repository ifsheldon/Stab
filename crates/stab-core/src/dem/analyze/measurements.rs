use crate::{CircuitError, CircuitInstruction, CircuitResult, QubitId};

use super::Analyzer;
use super::effects::{AnalyzerBasis, AnalyzerPauli, NoiseEffect, PendingError};
use super::instructions::measurement_basis;

impl Analyzer {
    pub(super) fn record_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        let gate_name = instruction.gate().canonical_name();
        let basis = measurement_basis(gate_name).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("unknown measurement basis")
        })?;
        let reset_after_measurement = measurement_resets_qubit(gate_name);
        for group in instruction.target_groups() {
            let Some(target) = group.first() else {
                continue;
            };
            let Some(qubit) = target.qubit_id() else {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                )));
            };
            let measurement_index = self.measurement_count;
            self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
            self.project_pending_errors_through_measurement(
                qubit,
                basis,
                measurement_index,
                reset_after_measurement,
            );
            if let Some(probability) = instruction.probability_argument()?
                && probability.get() > 0.0
            {
                self.completed_errors.push(PendingError {
                    probability,
                    effects: Vec::new(),
                    measurements: vec![measurement_index],
                    observables: Vec::new(),
                    disjoint_group: None,
                    tag: instruction.tag().map(str::to_owned),
                });
            }
            self.project_pending_pauli_channels_through_measurement(
                qubit,
                basis,
                measurement_index,
                reset_after_measurement,
            )?;
        }
        Ok(())
    }

    fn project_pending_errors_through_measurement(
        &mut self,
        qubit: QubitId,
        basis: AnalyzerBasis,
        measurement_index: usize,
        reset_after_measurement: bool,
    ) {
        let mut still_pending = Vec::new();
        for mut pending in self.pending_errors.drain(..) {
            if pending.touches_qubit(qubit) {
                pending.project_through_measurement(
                    qubit,
                    basis,
                    measurement_index,
                    reset_after_measurement,
                );
                if pending.effects.is_empty() {
                    self.completed_errors.push(pending);
                } else {
                    still_pending.push(pending);
                }
            } else {
                still_pending.push(pending);
            }
        }
        self.pending_errors = still_pending;
    }

    fn project_pending_pauli_channels_through_measurement(
        &mut self,
        qubit: QubitId,
        basis: AnalyzerBasis,
        measurement_index: usize,
        reset_after_measurement: bool,
    ) -> CircuitResult<()> {
        let pending_channels = std::mem::take(&mut self.pending_pauli_channels);
        for pending in pending_channels {
            if pending.qubit == qubit {
                let probability = pending.flip_probability(basis)?;
                if probability.get() > 0.0 {
                    let measurements = vec![measurement_index];
                    if reset_after_measurement {
                        self.completed_errors.push(PendingError {
                            probability,
                            effects: Vec::new(),
                            measurements,
                            observables: Vec::new(),
                            disjoint_group: None,
                            tag: pending.tag,
                        });
                    } else {
                        self.push_pending_error(
                            probability,
                            vec![NoiseEffect {
                                qubit,
                                pauli: measurement_flip_representative_pauli(basis),
                            }],
                            measurements,
                            None,
                            pending.tag,
                        );
                    }
                }
            } else {
                self.pending_pauli_channels.push(pending);
            }
        }
        Ok(())
    }
}

fn measurement_resets_qubit(gate_name: &str) -> bool {
    matches!(gate_name, "MR" | "MRX" | "MRY")
}

fn measurement_flip_representative_pauli(basis: AnalyzerBasis) -> AnalyzerPauli {
    match basis {
        AnalyzerBasis::X => AnalyzerPauli::Z,
        AnalyzerBasis::Y | AnalyzerBasis::Z => AnalyzerPauli::X,
    }
}
