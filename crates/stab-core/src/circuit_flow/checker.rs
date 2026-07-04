use crate::{
    Circuit, CircuitError, CircuitResult, DemTarget, Flow, PauliBasis, PauliSign, PauliString,
    QubitId, sparse_rev_frame_tracker::SparseReverseFrameTracker,
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

fn check_unsigned_flow_with_sparse_tracker(circuit: &Circuit, flow: &Flow) -> CircuitResult<bool> {
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "circuit measurement count does not fit usize during flow checking",
        )
    })?;
    let detector_count = circuit.count_detectors()?;
    let tracked_target = DemTarget::relative_detector(detector_count)?;
    let qubit_count = circuit
        .count_qubits()
        .max(flow.input().len())
        .max(flow.output().len());
    let mut tracker =
        SparseReverseFrameTracker::new(qubit_count, measurement_count, detector_count, true);

    seed_flow_pauli_output(&mut tracker, flow.output(), tracked_target)?;
    for measurement in flow.measurements() {
        let Some(record_index) = flow_record_index(measurement, measurement_count) else {
            return Ok(false);
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
    Ok(paulis_match_unsigned(&actual, flow.input()))
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
