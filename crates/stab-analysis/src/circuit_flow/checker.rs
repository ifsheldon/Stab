use stab_algebra::{
    Flow, FlowMeasurementIndex, PauliBasis, PauliSign, PauliString, StabilizerResult, Tableau,
};
use stab_model::{Circuit, CircuitItem, DemTarget, GateCategory, QubitId, Target};

use super::check_algorithm::{FlowCheckAlgorithm, FlowCheckCost};
use crate::{
    AnalysisError, AnalysisResult, circuit_to_tableau,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

/// Checks unsigned stabilizer flows against the supported unitary and sparse-tracker subsets.
///
/// Pauli-only flows of a measurement-free circuit are checked either against one dense Tableau or
/// with the sparse reverse tracker, whichever [`FlowCheckCost`] estimates to be cheaper; both
/// return the same result.
pub fn check_if_circuit_has_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    if flows.is_empty() {
        return Vec::new();
    }
    check_unsigned_flows_using(select_flow_check_algorithm(circuit, flows), circuit, flows)
}

/// Chooses the cheaper algorithm for a flow query, or the sparse tracker when only it applies.
fn select_flow_check_algorithm(circuit: &Circuit, flows: &[Flow]) -> FlowCheckAlgorithm {
    if dense_tableau_can_check(circuit, flows) {
        FlowCheckCost::measure(circuit, flows).cheaper_algorithm()
    } else {
        FlowCheckAlgorithm::SparseTracker
    }
}

/// Reports whether one dense Tableau can check every flow: the circuit is repeat-free and acts on
/// Pauli-only flows as a unitary, and every flow is a Pauli-only flow no wider than the circuit.
/// Repeat blocks stay on the sparse tracker, which folds them without raising a Tableau to a power.
fn dense_tableau_can_check(circuit: &Circuit, flows: &[Flow]) -> bool {
    let circuit_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    circuit
        .items()
        .iter()
        .all(|item| matches!(item, CircuitItem::Instruction(_)))
        && circuit_acts_unitarily_on_pauli_flows(circuit)
        && flows.iter().all(|flow| {
            is_pauli_only_flow(flow)
                && flow.input().len() <= circuit_qubits
                && flow.output().len() <= circuit_qubits
        })
}

/// Reports whether every instruction either is a unitary over qubit targets or cannot act on a
/// Pauli-only flow (annotations without record targets and noise without heralds), so the circuit
/// maps each Pauli-only flow input to exactly one output Pauli string.
fn circuit_acts_unitarily_on_pauli_flows(circuit: &Circuit) -> bool {
    circuit.items().iter().all(|item| match item {
        CircuitItem::Instruction(instruction) => {
            let gate = instruction.gate();
            !gate.produces_measurements()
                && !gate.is_reset()
                && !instruction
                    .targets()
                    .iter()
                    .any(Target::is_classical_bit_target)
                && (gate.is_unitary()
                    || matches!(
                        gate.category(),
                        GateCategory::Annotation | GateCategory::Noise
                    ))
        }
        CircuitItem::RepeatBlock(repeat) => circuit_acts_unitarily_on_pauli_flows(repeat.body()),
    })
}

fn is_pauli_only_flow(flow: &Flow) -> bool {
    flow.measurements().next().is_none() && flow.observables().next().is_none()
}

/// Checks flows with the given algorithm. The dense Tableau falls back to the sparse tracker when
/// the circuit cannot be converted, so either algorithm returns the same result for any query.
pub(crate) fn check_unsigned_flows_using(
    algorithm: FlowCheckAlgorithm,
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    if algorithm == FlowCheckAlgorithm::DenseTableau
        && let Some(tableau) = dense_flow_tableau(circuit, flows)
    {
        return flows
            .iter()
            .map(|flow| {
                dense_flow_image(&tableau, flow.input())
                    .is_ok_and(|actual| paulis_match_unsigned(&actual, flow.output()))
            })
            .collect();
    }
    check_unsigned_flows_with_sparse_tracker(circuit, flows)
        .unwrap_or_else(|_| vec![false; flows.len()])
}

/// Builds the dense Tableau when it can check every flow. Noise is skipped because unsigned flow
/// checking ignores it, exactly as the sparse tracker does.
fn dense_flow_tableau(circuit: &Circuit, flows: &[Flow]) -> Option<Tableau> {
    if !dense_tableau_can_check(circuit, flows) {
        return None;
    }
    circuit_to_tableau(circuit, true, false, false).ok()
}

/// Applies the Tableau to a flow input no wider than it, padding the input with identities.
fn dense_flow_image(tableau: &Tableau, input: &PauliString) -> StabilizerResult<PauliString> {
    if input.len() == tableau.len() {
        return tableau.apply(input);
    }
    let padded = PauliString::from_bases(
        PauliSign::Plus,
        (0..tableau.len()).map(|index| input.get(index).unwrap_or(PauliBasis::I)),
    )?;
    tableau.apply(&padded)
}

fn check_unsigned_flows_with_tableau(
    tableau: &Tableau,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    flows
        .iter()
        .map(|flow| match dense_flow_image(tableau, flow.input()) {
            Ok(actual) if paulis_match_unsigned(&actual, flow.output()) => {
                UnsignedStabilizerFlowCheck::passed()
            }
            Ok(actual) => {
                UnsignedStabilizerFlowCheck::failed(UnsignedStabilizerFlowFailure::OutputMismatch {
                    expected_output: unsigned_pauli(flow.output()),
                    actual_output: unsigned_pauli(&actual),
                })
            }
            Err(error) => UnsignedStabilizerFlowCheck::unsupported(error.to_string()),
        })
        .collect()
}

/// Diagnostic result for one unsigned stabilizer flow query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsignedStabilizerFlowCheck {
    has_flow: bool,
    failure: Option<UnsignedStabilizerFlowFailure>,
}

/// Reason an unsigned stabilizer flow query failed for the supported diagnostic subset.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnsignedStabilizerFlowFailure {
    /// A circuit of unitaries, annotations, and noise mapped the input of a Pauli-only flow to a
    /// different output Pauli string, whichever algorithm checked it.
    OutputMismatch {
        expected_output: PauliString,
        actual_output: PauliString,
    },
    /// A circuit with collapsing or classically controlled operations, or a flow with measurement
    /// or observable terms, pulled the requested output, measurement, and observable terms back to
    /// a different input Pauli string.
    InputMismatch {
        expected_input: PauliString,
        actual_input: PauliString,
    },
    /// A flow measurement term referenced a measurement record outside the circuit's measurement
    /// range.
    MeasurementRecordOutOfRange {
        record: FlowMeasurementIndex,
        measurement_count: usize,
    },
    /// A collapse operation anti-commuted with the tracked flow region.
    CollapseAnticommutation,
    /// The circuit or flow fell outside the supported unsigned checker subset.
    UnsupportedCircuit { reason: String },
}

/// Checks unsigned stabilizer flows and reports why unsupported or unsatisfied flows failed.
///
/// This is the diagnostic counterpart to
/// [`check_if_circuit_has_unsigned_stabilizer_flows`]. It preserves the same supported subset and
/// fail-closed semantics, but keeps the first local reason for a false row.
///
/// The mismatch kind depends only on the query, never on the algorithm that checked it. A
/// Pauli-only flow of a circuit whose instructions are unitaries, annotations, or noise reports
/// [`UnsignedStabilizerFlowFailure::OutputMismatch`] with the circuit's image of the flow input,
/// as wide as the widest of the circuit, the input, and the output. Every other Pauli mismatch
/// reports [`UnsignedStabilizerFlowFailure::InputMismatch`].
pub fn check_unsigned_stabilizer_flows_with_diagnostics(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    if flows.is_empty() {
        return Vec::new();
    }
    diagnose_unsigned_flows_using(select_flow_check_algorithm(circuit, flows), circuit, flows)
}

/// Diagnoses flows with the given algorithm; either algorithm returns the same diagnostics.
pub(crate) fn diagnose_unsigned_flows_using(
    algorithm: FlowCheckAlgorithm,
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    if algorithm == FlowCheckAlgorithm::DenseTableau
        && let Some(tableau) = dense_flow_tableau(circuit, flows)
    {
        return check_unsigned_flows_with_tableau(&tableau, flows);
    }
    let checks = match diagnose_unsigned_flows_with_sparse_tracker(circuit, flows) {
        Ok(checks) => checks,
        Err(error) => {
            return vec![UnsignedStabilizerFlowCheck::unsupported(error.to_string()); flows.len()];
        }
    };
    if circuit_acts_unitarily_on_pauli_flows(circuit) {
        report_unitary_mismatches_as_outputs(circuit, flows, checks)
    } else {
        checks
    }
}

/// Rewrites the input mismatches of Pauli-only flows of a unitary circuit as output mismatches, so
/// the sparse tracker reports what the dense Tableau reports. The circuit's images of the failing
/// inputs are found by tracking them backwards through the inverse of the circuit's unitary part,
/// so this costs one more sparse pass over the failing flows only.
fn report_unitary_mismatches_as_outputs(
    circuit: &Circuit,
    flows: &[Flow],
    mut checks: Vec<UnsignedStabilizerFlowCheck>,
) -> Vec<UnsignedStabilizerFlowCheck> {
    let failing = flows
        .iter()
        .zip(&checks)
        .enumerate()
        .filter(|(_, (flow, check))| {
            is_pauli_only_flow(flow)
                && matches!(
                    check.failure,
                    Some(UnsignedStabilizerFlowFailure::InputMismatch { .. })
                )
        })
        .map(|(index, (flow, _))| (index, flow))
        .collect::<Vec<_>>();
    if failing.is_empty() {
        return checks;
    }
    let inputs = failing
        .iter()
        .map(|(_, flow)| flow.input().clone())
        .collect::<Vec<_>>();
    let images = crate::circuit_inverse::unitary_part_inverse(circuit)
        .and_then(|inverse| pull_back_paulis_with_sparse_tracker(&inverse, &inputs));
    let circuit_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    for (position, (index, flow)) in failing.into_iter().enumerate() {
        let failure = match &images {
            Ok(images) => match images.get(position) {
                Some(image) => {
                    let width = circuit_qubits
                        .max(flow.input().len())
                        .max(flow.output().len());
                    let mut bases = vec![PauliBasis::I; width];
                    xor_region(&mut bases, image);
                    UnsignedStabilizerFlowFailure::OutputMismatch {
                        expected_output: unsigned_pauli(flow.output()),
                        actual_output: stab_algebra::advanced::pauli_from_bases_unchecked(
                            PauliSign::Plus,
                            bases,
                        ),
                    }
                }
                None => UnsignedStabilizerFlowFailure::UnsupportedCircuit {
                    reason: "sparse reverse tracking returned fewer images than flows".to_owned(),
                },
            },
            Err(error) => UnsignedStabilizerFlowFailure::UnsupportedCircuit {
                reason: error.to_string(),
            },
        };
        if let Some(check) = checks.get_mut(index) {
            *check = UnsignedStabilizerFlowCheck::failed(failure);
        }
    }
    checks
}

/// Tracks each Pauli string backwards through `circuit` and returns the Pauli strings it pulls
/// back to, unsigned and only as wide as their support.
fn pull_back_paulis_with_sparse_tracker(
    circuit: &Circuit,
    paulis: &[PauliString],
) -> AnalysisResult<Vec<PauliString>> {
    let qubit_count = paulis.iter().map(PauliString::len).fold(
        stab_model::advanced::circuit_simulated_qubit_count(circuit),
        usize::max,
    );
    let mut tracker = SparseReverseFrameTracker::new(qubit_count, 0, 0, false);
    let targets = (0..paulis.len())
        .map(|index| {
            u64::try_from(index).map(DemTarget::numeric).map_err(|_| {
                AnalysisError::invalid_detector_error_model("flow index does not fit u64")
            })
        })
        .collect::<AnalysisResult<Vec<_>>>()?;
    for (pauli, target) in paulis.iter().zip(&targets) {
        seed_flow_pauli_output(&mut tracker, pauli, *target)?;
    }
    tracker.undo_circuit(circuit)?;
    targets
        .into_iter()
        .map(|target| Ok(tracker.compact_region_for_target(target)?.value().clone()))
        .collect()
}

impl UnsignedStabilizerFlowCheck {
    /// Returns true when the flow was satisfied by the circuit under unsigned semantics.
    pub fn has_flow(&self) -> bool {
        self.has_flow
    }

    /// Returns the failure reason when the flow was not satisfied.
    pub fn failure(&self) -> Option<&UnsignedStabilizerFlowFailure> {
        self.failure.as_ref()
    }

    fn passed() -> Self {
        Self {
            has_flow: true,
            failure: None,
        }
    }

    fn failed(failure: UnsignedStabilizerFlowFailure) -> Self {
        Self {
            has_flow: false,
            failure: Some(failure),
        }
    }

    fn unsupported(reason: String) -> Self {
        Self::failed(UnsignedStabilizerFlowFailure::UnsupportedCircuit { reason })
    }
}

/// Returns true when the circuit has the given unsigned stabilizer flow.
pub fn circuit_has_unsigned_stabilizer_flow(circuit: &Circuit, flow: &Flow) -> bool {
    check_if_circuit_has_unsigned_stabilizer_flows(circuit, std::slice::from_ref(flow))
        .into_iter()
        .next()
        .unwrap_or(false)
}

/// Returns true when the circuit has every requested unsigned stabilizer flow.
///
/// This is the Rust unsigned counterpart of Stim's `has_all_flows` batch query for the supported
/// Stab flow-checker subset. Signs are ignored, matching
/// [`check_if_circuit_has_unsigned_stabilizer_flows`].
pub fn circuit_has_all_unsigned_stabilizer_flows(circuit: &Circuit, flows: &[Flow]) -> bool {
    check_if_circuit_has_unsigned_stabilizer_flows(circuit, flows)
        .into_iter()
        .all(|has_flow| has_flow)
}

pub(crate) fn check_unsigned_flows_with_sparse_tracker(
    circuit: &Circuit,
    flows: &[Flow],
) -> AnalysisResult<Vec<bool>> {
    Ok(diagnose_unsigned_flows_with_sparse_tracker(circuit, flows)?
        .into_iter()
        .map(|check| check.has_flow)
        .collect())
}

fn diagnose_unsigned_flows_with_sparse_tracker(
    circuit: &Circuit,
    flows: &[Flow],
) -> AnalysisResult<Vec<UnsignedStabilizerFlowCheck>> {
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        AnalysisError::invalid_detector_error_model(
            "circuit measurement count does not fit usize during flow checking",
        )
    })?;
    let detector_count = circuit.count_detectors()?;
    let flow_qubit_count = flows
        .iter()
        .flat_map(|flow| [flow.input().len(), flow.output().len()])
        .max()
        .unwrap_or(0);
    let qubit_count =
        stab_model::advanced::circuit_simulated_qubit_count(circuit).max(flow_qubit_count);
    let mut tracker =
        SparseReverseFrameTracker::new(qubit_count, measurement_count, detector_count, false);

    let mut preliminary = Vec::with_capacity(flows.len());
    for (flow_index, flow) in flows.iter().enumerate() {
        let tracked_target = DemTarget::numeric(u64::try_from(flow_index).map_err(|_| {
            AnalysisError::invalid_detector_error_model("flow index does not fit u64")
        })?);
        match flow_record_indices(flow, measurement_count) {
            Ok(record_indices) => {
                seed_flow_pauli_output(&mut tracker, flow.output(), tracked_target)?;
                for record_index in record_indices {
                    tracker.toggle_record_target_absolute(record_index, tracked_target)?;
                }
                for observable in flow.observables() {
                    tracker.toggle_observable_effect(observable, tracked_target);
                }
                preliminary.push(None);
            }
            Err(record) => preliminary.push(Some(
                UnsignedStabilizerFlowFailure::MeasurementRecordOutOfRange {
                    record: FlowMeasurementIndex::new(record),
                    measurement_count,
                },
            )),
        }
    }
    tracker.undo_circuit(circuit)?;

    flows
        .iter()
        .enumerate()
        .map(|(flow_index, flow)| {
            if let Some(failure) = preliminary.get(flow_index).cloned().flatten() {
                return Ok(UnsignedStabilizerFlowCheck::failed(failure));
            }
            let tracked_target = DemTarget::numeric(u64::try_from(flow_index).map_err(|_| {
                AnalysisError::invalid_detector_error_model("flow index does not fit u64")
            })?);
            if tracker.target_anticommuted(tracked_target) {
                return Ok(UnsignedStabilizerFlowCheck::failed(
                    UnsignedStabilizerFlowFailure::CollapseAnticommutation,
                ));
            }
            let mut bases = vec![PauliBasis::I; flow.input().len()];
            xor_region(
                &mut bases,
                tracker.compact_region_for_target(tracked_target)?.value(),
            );
            let actual = stab_algebra::advanced::pauli_from_bases_unchecked(PauliSign::Plus, bases);
            if paulis_match_unsigned(&actual, flow.input()) {
                Ok(UnsignedStabilizerFlowCheck::passed())
            } else {
                Ok(UnsignedStabilizerFlowCheck::failed(
                    UnsignedStabilizerFlowFailure::InputMismatch {
                        expected_input: unsigned_pauli(flow.input()),
                        actual_input: unsigned_pauli(&actual),
                    },
                ))
            }
        })
        .collect()
}

fn flow_record_indices(flow: &Flow, measurement_count: usize) -> Result<Vec<usize>, i32> {
    let mut result = Vec::new();
    for measurement in flow.measurements() {
        let Some(record_index) = flow_record_index(measurement, measurement_count) else {
            return Err(measurement);
        };
        result.push(record_index);
    }
    Ok(result)
}

fn seed_flow_pauli_output(
    tracker: &mut SparseReverseFrameTracker,
    output: &PauliString,
    target: DemTarget,
) -> AnalysisResult<()> {
    for (index, basis) in output.active_terms() {
        let qubit = u32::try_from(index)
            .ok()
            .and_then(|index| QubitId::new(index).ok())
            .ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(format!(
                    "flow output qubit index {index} is outside the supported target range"
                ))
            })?;
        tracker.toggle_pauli_target(qubit, basis, target)?;
    }
    Ok(())
}

pub fn flow_record_index(index: i32, measurement_count: usize) -> Option<usize> {
    if index >= 0 {
        return usize::try_from(index)
            .ok()
            .filter(|index| *index < measurement_count);
    }
    let measurement_count_i64 = i64::try_from(measurement_count).ok()?;
    let absolute = measurement_count_i64.checked_add(i64::from(index))?;
    usize::try_from(absolute)
        .ok()
        .filter(|index| *index < measurement_count)
}

fn xor_region(bases: &mut Vec<PauliBasis>, region: &PauliString) {
    if region.len() > bases.len() {
        bases.resize(region.len(), PauliBasis::I);
    }
    for (index, basis) in region.active_terms() {
        if let Some(existing) = bases.get_mut(index) {
            *existing = xor_basis(*existing, basis);
        }
    }
}

fn xor_basis(left: PauliBasis, right: PauliBasis) -> PauliBasis {
    PauliBasis::from_xz(left.x_bit() ^ right.x_bit(), left.z_bit() ^ right.z_bit())
}

fn paulis_match_unsigned(left: &PauliString, right: &PauliString) -> bool {
    (0..left.len().max(right.len())).all(|index| {
        left.get(index).unwrap_or(PauliBasis::I) == right.get(index).unwrap_or(PauliBasis::I)
    })
}

fn unsigned_pauli(pauli: &PauliString) -> PauliString {
    pauli.with_sign(PauliSign::Plus)
}
