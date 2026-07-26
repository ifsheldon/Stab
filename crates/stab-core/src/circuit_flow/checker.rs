use crate::{
    Circuit, CircuitError, CircuitItem, CircuitResult, DemTarget, Flow, FlowMeasurementIndex,
    PauliBasis, PauliSign, PauliString, QubitId, StabilizerResource,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

const MAX_BATCH_FLOW_TABLEAU_QUBITS: usize = StabilizerResource::TableauQubits.limit();

/// Checks unsigned stabilizer flows against the supported unitary and sparse-tracker subsets.
pub fn check_if_circuit_has_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    if flows.is_empty() {
        return Vec::new();
    }
    if should_use_batch_flow_tableau(circuit, flows)
        && let Ok(tableau) = circuit.to_tableau(false, false, false)
    {
        return flows
            .iter()
            .map(|flow| {
                tableau
                    .apply(flow.input())
                    .is_ok_and(|actual| paulis_match_unsigned(&actual, flow.output()))
            })
            .collect();
    }
    check_unsigned_flows_with_sparse_tracker(circuit, flows)
        .unwrap_or_else(|_| vec![false; flows.len()])
}

fn check_unsigned_flows_with_tableau(
    tableau: &crate::Tableau,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    flows
        .iter()
        .map(|flow| match tableau.apply(flow.input()) {
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

fn should_use_batch_flow_tableau(circuit: &Circuit, flows: &[Flow]) -> bool {
    let circuit_qubits = circuit.count_qubits();
    flows.iter().all(|flow| {
        flow.input().len() == circuit_qubits
            && flow.output().len() == circuit_qubits
            && flow.measurements().next().is_none()
            && flow.observables().next().is_none()
    }) && circuit_qubits <= MAX_BATCH_FLOW_TABLEAU_QUBITS
        && circuit
            .items()
            .iter()
            .all(|item| matches!(item, CircuitItem::Instruction(_)))
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
    /// A unitary circuit mapped the flow input to a different output Pauli string.
    OutputMismatch {
        expected_output: PauliString,
        actual_output: PauliString,
    },
    /// Sparse reverse tracking mapped the requested output, measurement, and observable terms to a
    /// different input Pauli string.
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
pub fn check_unsigned_stabilizer_flows_with_diagnostics(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<UnsignedStabilizerFlowCheck> {
    if flows.is_empty() {
        return Vec::new();
    }
    if should_use_batch_flow_tableau(circuit, flows)
        && let Ok(tableau) = circuit.to_tableau(false, false, false)
    {
        return check_unsigned_flows_with_tableau(&tableau, flows);
    }
    diagnose_unsigned_flows_with_sparse_tracker(circuit, flows).unwrap_or_else(|error| {
        vec![UnsignedStabilizerFlowCheck::unsupported(error.to_string()); flows.len()]
    })
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
) -> CircuitResult<Vec<bool>> {
    Ok(diagnose_unsigned_flows_with_sparse_tracker(circuit, flows)?
        .into_iter()
        .map(|check| check.has_flow)
        .collect())
}

fn diagnose_unsigned_flows_with_sparse_tracker(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Vec<UnsignedStabilizerFlowCheck>> {
    if flows.is_empty() {
        return Ok(Vec::new());
    }
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "circuit measurement count does not fit usize during flow checking",
        )
    })?;
    let detector_count = circuit.count_detectors()?;
    let flow_qubit_count = flows
        .iter()
        .flat_map(|flow| [flow.input().len(), flow.output().len()])
        .max()
        .unwrap_or(0);
    let qubit_count = circuit.count_qubits().max(flow_qubit_count);
    let mut tracker =
        SparseReverseFrameTracker::new(qubit_count, measurement_count, detector_count, false);

    let mut preliminary = Vec::with_capacity(flows.len());
    for (flow_index, flow) in flows.iter().enumerate() {
        let tracked_target = DemTarget::numeric(u64::try_from(flow_index).map_err(|_| {
            CircuitError::invalid_detector_error_model("flow index does not fit u64")
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
                CircuitError::invalid_detector_error_model("flow index does not fit u64")
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
            let actual = PauliString::from_bases_unchecked(PauliSign::Plus, bases);
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
) -> CircuitResult<()> {
    for (index, basis) in output.active_terms() {
        let qubit = u32::try_from(index)
            .ok()
            .and_then(|index| QubitId::new(index).ok())
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "flow output qubit index {index} is outside the supported target range"
                ))
            })?;
        tracker.toggle_pauli_target(qubit, basis, target)?;
    }
    Ok(())
}

pub(crate) fn flow_record_index(index: i32, measurement_count: usize) -> Option<usize> {
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
