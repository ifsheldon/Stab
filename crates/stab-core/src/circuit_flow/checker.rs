use crate::{
    Circuit, CircuitError, CircuitResult, DemTarget, Flow, FlowMeasurementIndex, PauliBasis,
    PauliSign, PauliString, QubitId, sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

/// Checks unsigned stabilizer flows against the supported unitary and sparse-tracker subsets.
pub fn check_if_circuit_has_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    let all_flows_are_unitary = flows
        .iter()
        .all(|flow| flow.measurements().next().is_none() && flow.observables().next().is_none());
    let tableau = all_flows_are_unitary
        .then(|| circuit.to_tableau(false, false, false).ok())
        .flatten();
    flows
        .iter()
        .map(|flow| {
            if flow.measurements().next().is_none()
                && flow.observables().next().is_none()
                && let Some(tableau) = &tableau
            {
                return tableau
                    .apply(flow.input())
                    .is_ok_and(|actual| paulis_match_unsigned(&actual, flow.output()));
            }
            check_unsigned_flow_with_sparse_tracker(circuit, flow).unwrap_or(false)
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
    let all_flows_are_unitary = flows
        .iter()
        .all(|flow| flow.measurements().next().is_none() && flow.observables().next().is_none());
    let tableau = all_flows_are_unitary
        .then(|| circuit.to_tableau(false, false, false).ok())
        .flatten();
    flows
        .iter()
        .map(|flow| {
            if flow.measurements().next().is_none()
                && flow.observables().next().is_none()
                && let Some(tableau) = &tableau
            {
                return match tableau.apply(flow.input()) {
                    Ok(actual) if paulis_match_unsigned(&actual, flow.output()) => {
                        UnsignedStabilizerFlowCheck::passed()
                    }
                    Ok(actual) => UnsignedStabilizerFlowCheck::failed(
                        UnsignedStabilizerFlowFailure::OutputMismatch {
                            expected_output: unsigned_pauli(flow.output()),
                            actual_output: unsigned_pauli(&actual),
                        },
                    ),
                    Err(error) => UnsignedStabilizerFlowCheck::unsupported(error.to_string()),
                };
            }
            diagnose_unsigned_flow_with_sparse_tracker(circuit, flow)
                .unwrap_or_else(|error| UnsignedStabilizerFlowCheck::unsupported(error.to_string()))
        })
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

pub(crate) fn check_unsigned_flow_with_sparse_tracker(
    circuit: &Circuit,
    flow: &Flow,
) -> CircuitResult<bool> {
    Ok(diagnose_unsigned_flow_with_sparse_tracker(circuit, flow)?.has_flow)
}

fn diagnose_unsigned_flow_with_sparse_tracker(
    circuit: &Circuit,
    flow: &Flow,
) -> CircuitResult<UnsignedStabilizerFlowCheck> {
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "circuit measurement count does not fit usize during flow checking",
        )
    })?;
    let detector_count = circuit.count_detectors()?;
    let tracked_target = DemTarget::numeric(0);
    let qubit_count = circuit
        .count_qubits()
        .max(flow.input().len())
        .max(flow.output().len());
    let mut tracker =
        SparseReverseFrameTracker::new(qubit_count, measurement_count, detector_count, true);

    seed_flow_pauli_output(&mut tracker, flow.output(), tracked_target)?;
    for measurement in flow.measurements() {
        let Some(record_index) = flow_record_index(measurement, measurement_count) else {
            return Ok(UnsignedStabilizerFlowCheck::failed(
                UnsignedStabilizerFlowFailure::MeasurementRecordOutOfRange {
                    record: FlowMeasurementIndex::new(measurement),
                    measurement_count,
                },
            ));
        };
        tracker.toggle_record_target_absolute(record_index, tracked_target)?;
    }
    tracker.undo_circuit(circuit)?;

    let mut bases = vec![PauliBasis::I; qubit_count];
    xor_region(
        &mut bases,
        tracker.region_for_target(tracked_target)?.value(),
    );
    for observable in flow.observables() {
        let observable_target = DemTarget::logical_observable(u64::from(observable))?;
        xor_region(
            &mut bases,
            tracker.region_for_target(observable_target)?.value(),
        );
    }
    let actual = PauliString::from_bases(PauliSign::Plus, bases);
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

fn flow_record_index(index: i32, measurement_count: usize) -> Option<usize> {
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
