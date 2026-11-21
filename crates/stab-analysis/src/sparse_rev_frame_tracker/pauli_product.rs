use std::collections::BTreeMap;

use stab_model::{CircuitInstruction, Gate, PauliProduct, PauliTarget, QubitId};

use crate::{AnalysisError, AnalysisOperation, AnalysisResult, instruction_shape};

use super::TrackerBasis;

/// The normalized products of an `MPP`, `SPP`, or `SPP_DAG`, last first.
///
/// Every product is normalized before any is undone, so an anti-Hermitian product is refused
/// before the tracker changes.
pub(super) fn pauli_product_terms_reversed(
    instruction: &CircuitInstruction,
) -> AnalysisResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction_shape::pauli_products(instruction, AnalysisOperation::ReverseFrameTracking)?
        .rev()
        .map(|product| normalize_pauli_product_terms(instruction.gate(), product))
        .collect()
}

fn normalize_pauli_product_terms(
    gate: Gate,
    product: PauliProduct<'_>,
) -> AnalysisResult<Vec<(QubitId, TrackerBasis)>> {
    let mut terms = BTreeMap::new();
    let mut phase = 0u8;
    for PauliTarget { pauli, id, .. } in product {
        multiply_tracker_term(&mut terms, id, TrackerBasis::from_pauli(pauli), &mut phase);
    }
    match phase {
        0 | 2 => Ok(terms.into_iter().collect()),
        _ => Err(AnalysisError::AntiHermitianPauliProduct {
            operation: AnalysisOperation::ReverseFrameTracking,
            gate: gate.kind(),
            phase: None,
        }),
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
