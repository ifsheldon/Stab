use std::collections::BTreeMap;

use crate::{CircuitError, CircuitInstruction, CircuitResult, QubitId, Target};

use super::TrackerBasis;

pub(super) fn pauli_product_terms_reversed(
    instruction: &CircuitInstruction,
) -> CircuitResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction
        .target_groups()
        .into_iter()
        .rev()
        .map(|group| normalize_pauli_product_terms(instruction.gate().canonical_name(), group))
        .collect()
}

pub(super) fn pauli_product_measurement_terms_reversed(
    instruction: &CircuitInstruction,
) -> CircuitResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    pauli_product_terms_reversed(instruction)
}

fn normalize_pauli_product_terms(
    gate_name: &str,
    group: &[Target],
) -> CircuitResult<Vec<(QubitId, TrackerBasis)>> {
    let mut terms = BTreeMap::new();
    let mut phase = 0u8;
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
        multiply_tracker_term(
            &mut terms,
            qubit,
            TrackerBasis::from_pauli(pauli),
            &mut phase,
        );
    }
    match phase {
        0 | 2 => Ok(terms.into_iter().collect()),
        _ => Err(CircuitError::invalid_detector_error_model(format!(
            "{gate_name} Pauli product is anti-Hermitian"
        ))),
    }
}

fn multiply_tracker_term(
    terms: &mut BTreeMap<QubitId, TrackerBasis>,
    qubit: QubitId,
    incoming: TrackerBasis,
    phase: &mut u8,
) {
    let Some(existing) = terms.remove(&qubit) else {
        terms.insert(qubit, incoming);
        return;
    };
    let (product, phase_delta) = multiply_tracker_bases(existing, incoming);
    *phase = (*phase + phase_delta) % 4;
    if let Some(product) = product {
        terms.insert(qubit, product);
    }
}

fn multiply_tracker_bases(left: TrackerBasis, right: TrackerBasis) -> (Option<TrackerBasis>, u8) {
    match (left, right) {
        (TrackerBasis::X, TrackerBasis::X)
        | (TrackerBasis::Y, TrackerBasis::Y)
        | (TrackerBasis::Z, TrackerBasis::Z) => (None, 0),
        (TrackerBasis::X, TrackerBasis::Y) => (Some(TrackerBasis::Z), 1),
        (TrackerBasis::Y, TrackerBasis::Z) => (Some(TrackerBasis::X), 1),
        (TrackerBasis::Z, TrackerBasis::X) => (Some(TrackerBasis::Y), 1),
        (TrackerBasis::Y, TrackerBasis::X) => (Some(TrackerBasis::Z), 3),
        (TrackerBasis::Z, TrackerBasis::Y) => (Some(TrackerBasis::X), 3),
        (TrackerBasis::X, TrackerBasis::Z) => (Some(TrackerBasis::Y), 3),
    }
}
