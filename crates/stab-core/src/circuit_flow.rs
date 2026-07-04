use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemTarget, Flow,
    GateCategory, PauliBasis, PauliSign, PauliString, QubitId, Target,
    sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

/// Returns unsigned stabilizer-flow generators for the supported tableau and measurement subset.
///
/// The current implementation supports unitary tableau circuits and the PFM5 single-instruction
/// measurement/reset/MPAD subset. Richer measured-circuit composition, pair measurements,
/// Pauli-product measurements, feedback, and noisy-flow semantics remain fail-closed.
pub fn circuit_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    if circuit_needs_measurement_rich_generators(circuit) {
        return simple_measurement_rich_flow_generators(circuit)?
            .ok_or_else(|| unsupported_flow_generator_error(circuit));
    }
    unitary_flow_generators(circuit)
}

fn unitary_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    let tableau = circuit.to_tableau(false, false, false)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in (0..tableau.len()).rev() {
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
    }
    Ok(flows)
}

fn circuit_needs_measurement_rich_generators(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => {
            instruction.gate().produces_measurements()
                || matches!(
                    instruction.gate().category(),
                    GateCategory::Collapsing | GateCategory::PairMeasurement
                )
        }
        CircuitItem::RepeatBlock(repeat) => {
            circuit_needs_measurement_rich_generators(repeat.body())
        }
    })
}

fn simple_measurement_rich_flow_generators(circuit: &Circuit) -> CircuitResult<Option<Vec<Flow>>> {
    let [CircuitItem::Instruction(instruction)] = circuit.items() else {
        return Ok(None);
    };
    Ok(match instruction.gate().canonical_name() {
        "M" => simple_measurement_flows(instruction, PauliBasis::Z)?,
        "MX" => simple_measurement_flows(instruction, PauliBasis::X)?,
        "MY" => simple_measurement_flows(instruction, PauliBasis::Y)?,
        "R" => simple_reset_flows(instruction, PauliBasis::Z),
        "RX" => simple_reset_flows(instruction, PauliBasis::X),
        "RY" => simple_reset_flows(instruction, PauliBasis::Y),
        "MR" => simple_measure_reset_flows(instruction, PauliBasis::Z)?,
        "MRX" => simple_measure_reset_flows(instruction, PauliBasis::X)?,
        "MRY" => simple_measure_reset_flows(instruction, PauliBasis::Y)?,
        "MPAD" => Some(measurement_pad_flows(instruction)?),
        _ => None,
    })
}

fn simple_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let mut flows = Vec::new();
    let mut last_records_by_qubit = vec![None; qubit_count];
    for (record_index, target) in instruction.targets().iter().enumerate() {
        let Some(qubit) = plain_target_index(target) else {
            return Ok(None);
        };
        let Some(slot) = last_records_by_qubit.get_mut(qubit) else {
            return Ok(None);
        };
        if let Some(previous_record) = *slot {
            flows.push(record_equality_flow(previous_record, record_index)?);
        }
        *slot = Some(record_index);
    }

    for (qubit, record_index) in last_records_by_qubit.into_iter().enumerate() {
        if let Some(record_index) = record_index {
            flows.push(output_measurement_flow(
                qubit_count,
                qubit,
                basis,
                record_index,
            )?);
            flows.push(input_measurement_flow(
                qubit_count,
                qubit,
                basis,
                record_index,
            )?);
        }
    }
    Ok(Some(flows))
}

fn simple_reset_flows(instruction: &CircuitInstruction, basis: PauliBasis) -> Option<Vec<Flow>> {
    let qubit_count = instruction_qubit_count(instruction);
    let qubits = unique_plain_target_indices(instruction)?;
    Some(
        qubits
            .into_iter()
            .map(|qubit| reset_flow(qubit_count, qubit, basis))
            .collect(),
    )
}

fn simple_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let qubits = match unique_plain_target_indices(instruction) {
        Some(qubits) => qubits,
        None => return Ok(None),
    };
    let mut flows = Vec::with_capacity(instruction.targets().len() * 2);
    for &qubit in &qubits {
        flows.push(reset_flow(qubit_count, qubit, basis));
    }
    for (record_index, qubit) in qubits.into_iter().enumerate() {
        flows.push(input_measurement_flow(
            qubit_count,
            qubit,
            basis,
            record_index,
        )?);
    }
    Ok(Some(flows))
}

fn measurement_pad_flows(instruction: &CircuitInstruction) -> CircuitResult<Vec<Flow>> {
    let mut positive_records = Vec::new();
    let mut negative_records = Vec::new();
    for (record_index, target) in instruction.targets().iter().enumerate() {
        match target.qubit_id().map(|id| id.get()) {
            Some(0) => positive_records.push(record_index),
            Some(1) => negative_records.push(record_index),
            _ => {
                return Err(CircuitError::invalid_tableau_conversion(format!(
                    "MPAD flow generator has invalid pad target {target}"
                )));
            }
        }
    }
    let mut flows = Vec::with_capacity(positive_records.len() + negative_records.len());
    for record in positive_records {
        flows.push(positive_record_flow(record)?);
    }
    for record in negative_records {
        flows.push(negative_record_flow(record)?);
    }
    Ok(flows)
}

fn instruction_qubit_count(instruction: &CircuitInstruction) -> usize {
    instruction
        .targets()
        .iter()
        .filter_map(Target::qubit_id)
        .map(|qubit| qubit.get() as usize + 1)
        .max()
        .unwrap_or(0)
}

fn plain_target_index(target: &Target) -> Option<usize> {
    if target.is_inverted_result_target() {
        return None;
    }
    target.qubit_id().map(|qubit| qubit.get() as usize)
}

fn unique_plain_target_indices(instruction: &CircuitInstruction) -> Option<Vec<usize>> {
    let mut qubits = Vec::with_capacity(instruction.targets().len());
    for target in instruction.targets() {
        let qubit = plain_target_index(target)?;
        if qubits.contains(&qubit) {
            return None;
        }
        qubits.push(qubit);
    }
    Some(qubits)
}

fn record_equality_flow(left_record: usize, right_record: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::identity(0),
        [
            record_index_i32(left_record)?,
            record_index_i32(right_record)?,
        ],
        [],
    ))
}

fn output_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        single_pauli(qubit_count, qubit, basis),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn input_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        single_pauli(qubit_count, qubit, basis),
        PauliString::identity(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn reset_flow(qubit_count: usize, qubit: usize, basis: PauliBasis) -> Flow {
    Flow::new(
        PauliString::identity(0),
        single_pauli(qubit_count, qubit, basis),
        [],
        [],
    )
}

fn positive_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::identity(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn negative_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::from_bases(PauliSign::Minus, []),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn record_index_i32(record_index: usize) -> CircuitResult<i32> {
    i32::try_from(record_index).map_err(|_| {
        CircuitError::invalid_tableau_conversion(format!(
            "flow measurement record index {record_index} does not fit i32"
        ))
    })
}

fn unsupported_flow_generator_error(circuit: &Circuit) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "circuit_flow_generators only supports unitary tableau circuits and single-instruction M/MX/MY, R/RX/RY, MR/MRX/MRY, and MPAD circuits; got {} top-level item(s)",
        circuit.items().len()
    ))
}

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

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn stabilizer_to_circuit_error(error: crate::StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}
