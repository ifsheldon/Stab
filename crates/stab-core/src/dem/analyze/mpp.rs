use std::collections::BTreeMap;

use crate::{CircuitError, CircuitInstruction, CircuitResult, Pauli, QubitId, Target};

use super::Analyzer;
use super::effects::AnalyzerBasis;

impl Analyzer {
    pub(super) fn apply_spp(&mut self, instruction: &CircuitInstruction) -> CircuitResult<()> {
        for group in instruction.target_groups() {
            let terms = reduced_pauli_product_terms(instruction.gate().canonical_name(), group)?;
            let qubits = terms
                .iter()
                .map(|(qubit, _basis)| *qubit)
                .collect::<Vec<_>>();
            self.expand_pending_single_qubit_channels_touching(&qubits)?;
            for pending in &mut self.pending_errors {
                pending.apply_spp_product(&terms);
            }
            self.observable_sensitivity.apply_spp_product(&terms);
        }
        Ok(())
    }

    pub(super) fn record_pauli_product_measurements(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<()> {
        for group in instruction.target_groups() {
            let terms = reduced_pauli_product_terms(instruction.gate().canonical_name(), group)?;
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

pub(super) fn reduced_pauli_product_terms(
    gate_name: &str,
    group: &[Target],
) -> CircuitResult<Vec<(QubitId, AnalyzerBasis)>> {
    let mut phase = 0_u8;
    let mut terms = BTreeMap::<QubitId, Pauli>::new();
    let mut order = Vec::<QubitId>::new();

    for target in group {
        match target {
            Target::Pauli {
                pauli,
                id,
                inverted,
            } => {
                if *inverted {
                    phase = (phase + 2) % 4;
                }
                if !terms.contains_key(id) && !order.contains(id) {
                    order.push(*id);
                }
                let current = terms.remove(id);
                let (phase_delta, product) = multiply_pauli(current, *pauli);
                phase = (phase + phase_delta) % 4;
                if let Some(product) = product {
                    terms.insert(*id, product);
                }
            }
            Target::Combiner => {}
            _ => {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "{gate_name} expected Pauli product targets, got {target}"
                )));
            }
        }
    }

    if !phase.is_multiple_of(2) {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "{gate_name} Pauli product is anti-Hermitian"
        )));
    }

    Ok(order
        .into_iter()
        .filter_map(|qubit| {
            terms
                .remove(&qubit)
                .map(|pauli| (qubit, AnalyzerBasis::from_pauli(pauli)))
        })
        .collect())
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

fn multiply_pauli(current: Option<Pauli>, next: Pauli) -> (u8, Option<Pauli>) {
    let Some(current) = current else {
        return (0, Some(next));
    };
    match (current, next) {
        (Pauli::X, Pauli::X) | (Pauli::Y, Pauli::Y) | (Pauli::Z, Pauli::Z) => (0, None),
        (Pauli::X, Pauli::Y) => (1, Some(Pauli::Z)),
        (Pauli::Y, Pauli::Z) => (1, Some(Pauli::X)),
        (Pauli::Z, Pauli::X) => (1, Some(Pauli::Y)),
        (Pauli::Y, Pauli::X) => (3, Some(Pauli::Z)),
        (Pauli::Z, Pauli::Y) => (3, Some(Pauli::X)),
        (Pauli::X, Pauli::Z) => (3, Some(Pauli::Y)),
    }
}
