use crate::{CircuitError, CircuitInstruction, CircuitResult, QubitId, Target};

use super::Analyzer;
use super::effects::AnalyzerBasis;

impl Analyzer {
    pub(super) fn record_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups() {
            let terms = pauli_product_terms(instruction.gate().canonical_name(), group)?;
            let qubits = terms
                .iter()
                .map(|(qubit, _basis)| *qubit)
                .collect::<Vec<_>>();
            self.expand_pending_single_qubit_channels_touching(&qubits)?;
            let measurement_index = self.measurement_count;
            self.measurement_count = self.measurement_count.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("measurement count overflowed")
            })?;
            for pending in &mut self.pending_errors {
                if pending.flips_product_measurement(&terms) {
                    pending.measurements.push(measurement_index);
                }
            }
            if let Some(probability) = instruction.probability_argument()?
                && probability.get() > 0.0
            {
                self.completed_errors.push(super::PendingError {
                    probability,
                    effects: Vec::new(),
                    measurements: vec![measurement_index],
                    observables: Vec::new(),
                    disjoint_group: None,
                    tag: instruction.tag().map(str::to_owned),
                });
            }
        }
        Ok(())
    }
}

pub(super) fn pauli_product_terms(
    gate_name: &str,
    group: &[Target],
) -> CircuitResult<Vec<(QubitId, AnalyzerBasis)>> {
    let mut terms = Vec::new();
    for target in group {
        if target.is_combiner() {
            continue;
        }
        let pauli = target.pauli_type().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{gate_name} target {target} is not a Pauli target"
            ))
        })?;
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "{gate_name} target {target} does not identify a qubit"
            ))
        })?;
        terms.push((qubit, AnalyzerBasis::from_pauli(pauli)));
    }
    Ok(terms)
}
