use std::collections::BTreeMap;

use stab_model::{CircuitInstruction, Gate, QubitId, Target};

use crate::{AnalysisError, AnalysisOperation, AnalysisResult};

use super::TrackerBasis;

/// The instruction's qubit targets, last first, without collecting them.
///
/// Measurement and reset targets are qubits by construction, so a non-qubit target is reported
/// when it is reached rather than before the first target is undone.
pub(super) fn qubits_reversed(
    instruction: &CircuitInstruction,
) -> impl Iterator<Item = AnalysisResult<QubitId>> {
    instruction.targets().iter().rev().map(|target| {
        target.qubit_id().ok_or_else(|| {
            AnalysisError::unsupported(
                AnalysisOperation::ReverseFrameTracking,
                instruction.gate().kind(),
                format!(
                    "{} target {target} is not a qubit",
                    instruction.gate().canonical_name()
                ),
            )
        })
    })
}

pub(super) fn pair_measurement_terms_reversed(
    instruction: &CircuitInstruction,
    basis: TrackerBasis,
) -> AnalysisResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction
        .target_groups()
        .into_iter()
        .rev()
        .map(|group| {
            let [left, right] = group else {
                return Err(AnalysisError::unsupported(
                    AnalysisOperation::ReverseFrameTracking,
                    instruction.gate().kind(),
                    format!(
                        "{} expected paired targets during sparse reverse tracking",
                        instruction.gate().canonical_name()
                    ),
                ));
            };
            let left = left.qubit_id().ok_or_else(|| {
                AnalysisError::unsupported(
                    AnalysisOperation::ReverseFrameTracking,
                    instruction.gate().kind(),
                    format!(
                        "{} target {left} is not a qubit",
                        instruction.gate().canonical_name()
                    ),
                )
            })?;
            let right = right.qubit_id().ok_or_else(|| {
                AnalysisError::unsupported(
                    AnalysisOperation::ReverseFrameTracking,
                    instruction.gate().kind(),
                    format!(
                        "{} target {right} is not a qubit",
                        instruction.gate().canonical_name()
                    ),
                )
            })?;
            Ok(vec![(left, basis), (right, basis)])
        })
        .collect()
}

pub(super) fn pauli_product_terms_reversed(
    instruction: &CircuitInstruction,
) -> AnalysisResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    instruction
        .target_groups()
        .into_iter()
        .rev()
        .map(|group| normalize_pauli_product_terms(instruction.gate(), group))
        .collect()
}

pub(super) fn pauli_product_measurement_terms_reversed(
    instruction: &CircuitInstruction,
) -> AnalysisResult<Vec<Vec<(QubitId, TrackerBasis)>>> {
    pauli_product_terms_reversed(instruction)
}

fn normalize_pauli_product_terms(
    gate: Gate,
    group: &[Target],
) -> AnalysisResult<Vec<(QubitId, TrackerBasis)>> {
    let gate_name = gate.canonical_name();
    let mut terms = BTreeMap::new();
    let mut phase = 0u8;
    for target in group {
        if target.is_combiner() {
            continue;
        }
        let pauli = target.pauli_type().ok_or_else(|| {
            AnalysisError::unsupported(
                AnalysisOperation::ReverseFrameTracking,
                gate.kind(),
                format!("{gate_name} target {target} is not a Pauli target"),
            )
        })?;
        let qubit = target.qubit_id().ok_or_else(|| {
            AnalysisError::unsupported(
                AnalysisOperation::ReverseFrameTracking,
                gate.kind(),
                format!("{gate_name} target {target} does not identify a qubit"),
            )
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
