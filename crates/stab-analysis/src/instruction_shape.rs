//! Narrows a circuit instruction's typed views to the shape its gate's rules name.
//!
//! Analyses dispatch on [`stab_model::GateKind`], and the gate table fixes each kind's argument
//! and target rule, so every narrowing here succeeds for every instruction a dispatch arm
//! receives. A mismatch would mean an analysis and the gate table disagree about a gate, which
//! is reported once, here, as an internal invariant error instead of being re-checked per target
//! at every call site.

use stab_model::{
    CircuitInstruction, ControlledPairs, GateArguments, GateTargets, ObservableId,
    ObservableTargets, PadValues, PauliProducts, PauliTerms, Probability, QubitId, QubitIds,
    QubitPairs, QubitTargetPairs, QubitTargets, RecordTargets,
};

use crate::{AnalysisError, AnalysisOperation, AnalysisResult};

fn target_rule_mismatch(
    operation: AnalysisOperation,
    instruction: &CircuitInstruction,
    expected: &'static str,
) -> AnalysisError {
    AnalysisError::internal(
        operation,
        format!(
            "{} has the {:?} target rule, but its analysis expects {expected}",
            instruction.gate().canonical_name(),
            instruction.gate().target_rule()
        ),
    )
}

fn argument_rule_mismatch(
    operation: AnalysisOperation,
    instruction: &CircuitInstruction,
    expected: &'static str,
) -> AnalysisError {
    AnalysisError::internal(
        operation,
        format!(
            "{} has the {:?} argument rule, but its analysis expects {expected}",
            instruction.gate().canonical_name(),
            instruction.gate().argument_rule()
        ),
    )
}

/// The single probability of a `ProbabilityList(1)` gate.
pub(crate) fn probability(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<Probability> {
    match instruction.gate_arguments() {
        GateArguments::Probability(probability) => Ok(probability),
        _ => Err(argument_rule_mismatch(
            operation,
            instruction,
            "one probability",
        )),
    }
}

/// The `N` disjoint probabilities of a `ProbabilityList(N)` gate.
pub(crate) fn probabilities<const N: usize>(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<[Probability; N]> {
    match instruction.gate_arguments() {
        GateArguments::Probabilities(probabilities) => probabilities.to_array::<N>(),
        _ => None,
    }
    .ok_or_else(|| argument_rule_mismatch(operation, instruction, "a fixed probability list"))
}

/// The observable id of an `OBSERVABLE_INCLUDE`.
pub(crate) fn observable(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<ObservableId> {
    match instruction.gate_arguments() {
        GateArguments::Observable(observable) => Ok(observable),
        _ => Err(argument_rule_mismatch(
            operation,
            instruction,
            "an observable id",
        )),
    }
}

/// The plain qubit targets of an `AnySingleQubit` gate.
pub(crate) fn qubits(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<QubitIds<'_>> {
    match instruction.gate_targets() {
        GateTargets::Qubits(qubits) => Ok(qubits),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "plain qubit targets",
        )),
    }
}

/// The qubit targets of a single-qubit measurement gate, with their inversion flags.
pub(crate) fn measured_qubits(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<QubitTargets<'_>> {
    match instruction.gate_targets() {
        GateTargets::MeasuredQubits(qubits) => Ok(qubits),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "measured qubit targets",
        )),
    }
}

/// The plain qubit pairs of a `PlainPairs` gate.
pub(crate) fn qubit_pairs(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<QubitPairs<'_>> {
    match instruction.gate_targets() {
        GateTargets::QubitPairs(pairs) => Ok(pairs),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "plain qubit pairs",
        )),
    }
}

/// The measured qubit pairs of an `MXX`, `MYY`, or `MZZ`.
pub(crate) fn measured_qubit_pairs(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<QubitTargetPairs<'_>> {
    match instruction.gate_targets() {
        GateTargets::MeasuredQubitPairs(pairs) => Ok(pairs),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "measured qubit pairs",
        )),
    }
}

/// The classified target pairs of a classically controllable controlled-Pauli gate.
pub(crate) fn controlled_pairs(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<ControlledPairs<'_>> {
    match instruction.gate_targets() {
        GateTargets::ControlledPairs(pairs) => Ok(pairs),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "controlled-Pauli target pairs",
        )),
    }
}

/// The measurement records of a `DETECTOR`.
pub(crate) fn records(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<RecordTargets<'_>> {
    match instruction.gate_targets() {
        GateTargets::Records(records) => Ok(records),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "measurement-record targets",
        )),
    }
}

/// The record and Pauli targets of an `OBSERVABLE_INCLUDE`.
pub(crate) fn observable_targets(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<ObservableTargets<'_>> {
    match instruction.gate_targets() {
        GateTargets::ObservableTargets(targets) => Ok(targets),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "observable targets",
        )),
    }
}

/// The Pauli products of an `MPP`, `SPP`, or `SPP_DAG`.
pub(crate) fn pauli_products(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<PauliProducts<'_>> {
    match instruction.gate_targets() {
        GateTargets::PauliProducts(products) => Ok(products),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "Pauli products",
        )),
    }
}

/// The Pauli targets of an `E` or `ELSE_CORRELATED_ERROR`.
pub(crate) fn pauli_terms(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<PauliTerms<'_>> {
    match instruction.gate_targets() {
        GateTargets::PauliTerms(terms) => Ok(terms),
        _ => Err(target_rule_mismatch(operation, instruction, "Pauli terms")),
    }
}

/// The qubits of a single-qubit gate, whether its targets are plain qubits (unitaries, resets,
/// and noise) or measured qubits that may carry an inversion (measurements).
pub(crate) fn single_qubits(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<SingleQubits<'_>> {
    match instruction.gate_targets() {
        GateTargets::Qubits(qubits) => Ok(SingleQubits::Plain(qubits)),
        GateTargets::MeasuredQubits(qubits) => Ok(SingleQubits::Measured(qubits)),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "single-qubit targets",
        )),
    }
}

/// The qubit ids of a single-qubit gate's targets, inversion flags dropped.
#[derive(Clone, Debug)]
pub(crate) enum SingleQubits<'a> {
    Plain(QubitIds<'a>),
    Measured(QubitTargets<'a>),
}

impl SingleQubits<'_> {
    /// Returns the qubit at `index` among those not yet iterated.
    pub(crate) fn get(&self, index: usize) -> Option<QubitId> {
        match self {
            Self::Plain(qubits) => qubits.get(index),
            Self::Measured(qubits) => qubits.get(index).map(|target| target.id),
        }
    }
}

impl Iterator for SingleQubits<'_> {
    type Item = QubitId;

    fn next(&mut self) -> Option<QubitId> {
        match self {
            Self::Plain(qubits) => qubits.next(),
            Self::Measured(qubits) => qubits.next().map(|target| target.id),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Plain(qubits) => qubits.size_hint(),
            Self::Measured(qubits) => qubits.size_hint(),
        }
    }
}

impl DoubleEndedIterator for SingleQubits<'_> {
    fn next_back(&mut self) -> Option<QubitId> {
        match self {
            Self::Plain(qubits) => qubits.next_back(),
            Self::Measured(qubits) => qubits.next_back().map(|target| target.id),
        }
    }
}

impl ExactSizeIterator for SingleQubits<'_> {}

/// The pad values of an `MPAD`.
pub(crate) fn pad_values(
    instruction: &CircuitInstruction,
    operation: AnalysisOperation,
) -> AnalysisResult<PadValues<'_>> {
    match instruction.gate_targets() {
        GateTargets::MeasurementPads(pads) => Ok(pads),
        _ => Err(target_rule_mismatch(
            operation,
            instruction,
            "measurement pad values",
        )),
    }
}
