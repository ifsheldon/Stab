use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, CompiledSampler,
    DemTarget, Flow, FlowMeasurementIndex, Gate, MeasureRecordOffset, PauliBasis, PauliSign,
    PauliString, QubitId, RepeatBlock, Target, sparse_rev_frame_tracker::SparseReverseFrameTracker,
};

const SAMPLED_FLOW_SAMPLE_WORD_WIDTH: usize = 256;

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

/// Probabilistically checks signed stabilizer flows by sampling augmented noiseless circuits.
///
/// This is the scoped Rust counterpart to Stim's `sample_if_circuit_has_stabilizer_flows`.
/// Unlike [`check_if_circuit_has_unsigned_stabilizer_flows`], signs are meaningful and each queried
/// flow is checked by appending an ancilla witness measurement to a noiseless copy of the circuit.
/// Each false flow has a 50 percent chance of surviving an individual sample, so callers should use
/// enough samples for their desired confidence. The effective sample count is rounded up to 256 to
/// match Stim's `MAX_BITWORD_WIDTH` confidence behavior on the public Python path.
pub fn sample_if_circuit_has_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    sample_count: usize,
    seed: Option<u64>,
) -> CircuitResult<Vec<bool>> {
    let noiseless = circuit.without_noise()?;
    let measurement_count = usize::try_from(noiseless.count_measurements()?).map_err(|_| {
        CircuitError::invalid_detector_error_model(
            "circuit measurement count does not fit usize during sampled flow checking",
        )
    })?;
    let sample_count = rounded_sampled_flow_count(sample_count)?;
    flows
        .iter()
        .enumerate()
        .map(|(flow_index, flow)| {
            sample_if_noiseless_circuit_has_stabilizer_flow(
                &noiseless,
                flow,
                measurement_count,
                sample_count,
                sampled_flow_seed(seed, flow_index),
            )
        })
        .collect()
}

fn rounded_sampled_flow_count(sample_count: usize) -> CircuitResult<usize> {
    let remainder = sample_count % SAMPLED_FLOW_SAMPLE_WORD_WIDTH;
    if remainder == 0 {
        return Ok(sample_count);
    }
    sample_count
        .checked_add(SAMPLED_FLOW_SAMPLE_WORD_WIDTH - remainder)
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "sample count overflows while rounding sampled flow checks to Stim word width",
            )
        })
}

pub(crate) fn check_unsigned_flow_with_sparse_tracker(
    circuit: &Circuit,
    flow: &Flow,
) -> CircuitResult<bool> {
    Ok(diagnose_unsigned_flow_with_sparse_tracker(circuit, flow)?.has_flow)
}

pub(crate) fn diagnose_unsigned_flow_with_sparse_tracker(
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

fn sample_if_noiseless_circuit_has_stabilizer_flow(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
    sample_count: usize,
    seed: Option<u64>,
) -> CircuitResult<bool> {
    let augmented = augmented_flow_test_circuit(circuit, flow, measurement_count)?;
    let sampler = CompiledSampler::compile(&augmented)?;
    let witness_index = measurement_count;
    let mut passed = true;
    sampler.for_each_sample_with_seed_and_reference_mode(
        sample_count,
        seed,
        false,
        |record| -> CircuitResult<()> {
            let witness = record.get(witness_index).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "sampled flow witness measurement was missing from augmented circuit",
                )
            })?;
            if *witness {
                passed = false;
            }
            Ok(())
        },
    )?;
    Ok(passed)
}

fn augmented_flow_test_circuit(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
) -> CircuitResult<Circuit> {
    let qubit_count = circuit
        .count_qubits()
        .max(flow.input().len())
        .max(flow.output().len());
    let ancilla = qubit_id_from_index(qubit_count, "sampled flow ancilla qubit")?;
    let mut augmented = Circuit::new();

    for qubit in 0..qubit_count {
        append_one_target_instruction(
            &mut augmented,
            "X_ERROR",
            vec![0.5],
            Target::qubit(
                qubit_id_from_index(qubit, "sampled flow X_ERROR qubit")?,
                false,
            ),
            None,
        )?;
    }
    for qubit in 0..qubit_count {
        append_one_target_instruction(
            &mut augmented,
            "Z_ERROR",
            vec![0.5],
            Target::qubit(
                qubit_id_from_index(qubit, "sampled flow Z_ERROR qubit")?,
                false,
            ),
            None,
        )?;
    }

    append_pauli_controlled_not(&mut augmented, flow.input(), ancilla, None)?;
    let observables = flow.observables().collect::<Vec<_>>();
    append_flow_test_block_for_circuit(&mut augmented, circuit, ancilla, &observables)?;
    for measurement in flow.measurements() {
        let record = sampled_flow_measurement_target(measurement, measurement_count)?;
        append_two_target_instruction(
            &mut augmented,
            "CX",
            record,
            Target::qubit(ancilla, false),
            None,
        )?;
    }
    append_pauli_controlled_not(&mut augmented, flow.output(), ancilla, None)?;
    append_one_target_instruction(
        &mut augmented,
        "M",
        Vec::new(),
        Target::qubit(ancilla, false),
        None,
    )?;

    Ok(augmented)
}

fn append_flow_test_block_for_circuit(
    output: &mut Circuit,
    circuit: &Circuit,
    ancilla: QubitId,
    observables: &[u32],
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction)
                if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE"
                    && observable_is_selected(instruction, observables)? =>
            {
                append_selected_observable_feedback(output, instruction, ancilla)?;
            }
            CircuitItem::Instruction(instruction) => output.append_instruction(instruction.clone()),
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = Circuit::new();
                append_flow_test_block_for_circuit(&mut body, repeat.body(), ancilla, observables)?;
                output.append_repeat_block(RepeatBlock::new(
                    repeat.repeat_count(),
                    body,
                    repeat.tag().map(str::to_owned),
                ));
            }
        }
    }
    Ok(())
}

fn append_selected_observable_feedback(
    output: &mut Circuit,
    instruction: &CircuitInstruction,
    ancilla: QubitId,
) -> CircuitResult<()> {
    for target in instruction.targets() {
        if target.is_inverted_result_target() {
            append_one_target_instruction(
                output,
                "X",
                Vec::new(),
                Target::qubit(ancilla, false),
                instruction.tag().map(str::to_owned),
            )?;
        }
        if target.is_measurement_record_target() {
            append_two_target_instruction(
                output,
                "CX",
                target.clone(),
                Target::qubit(ancilla, false),
                instruction.tag().map(str::to_owned),
            )?;
        } else if target.is_x_target() {
            append_pauli_observable_feedback(output, "XCX", target, ancilla, instruction)?;
        } else if target.is_y_target() {
            append_pauli_observable_feedback(output, "YCX", target, ancilla, instruction)?;
        } else if target.is_z_target() {
            append_pauli_observable_feedback(output, "CX", target, ancilla, instruction)?;
        } else {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "sampled flow checking does not support OBSERVABLE_INCLUDE target {target}"
            )));
        }
    }
    Ok(())
}

fn append_pauli_observable_feedback(
    output: &mut Circuit,
    gate_name: &'static str,
    target: &Target,
    ancilla: QubitId,
    source: &CircuitInstruction,
) -> CircuitResult<()> {
    let qubit = target.qubit_id().ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!(
            "sampled flow checking expected Pauli observable target {target} to contain a qubit"
        ))
    })?;
    append_two_target_instruction(
        output,
        gate_name,
        Target::qubit(qubit, false),
        Target::qubit(ancilla, false),
        source.tag().map(str::to_owned),
    )
}

fn append_pauli_controlled_not(
    circuit: &mut Circuit,
    pauli: &PauliString,
    ancilla: QubitId,
    tag: Option<String>,
) -> CircuitResult<()> {
    for (index, basis) in pauli.active_terms() {
        let gate_name = match basis {
            PauliBasis::X => "XCX",
            PauliBasis::Y => "YCX",
            PauliBasis::Z => "ZCX",
            PauliBasis::I => continue,
        };
        append_two_target_instruction(
            circuit,
            gate_name,
            Target::qubit(
                qubit_id_from_index(index, "sampled flow Pauli control qubit")?,
                false,
            ),
            Target::qubit(ancilla, false),
            tag.clone(),
        )?;
    }
    if pauli.sign() == PauliSign::Minus {
        append_one_target_instruction(
            circuit,
            "X",
            Vec::new(),
            Target::qubit(ancilla, false),
            tag,
        )?;
    }
    Ok(())
}

fn observable_is_selected(
    instruction: &CircuitInstruction,
    selected_observables: &[u32],
) -> CircuitResult<bool> {
    let observable = instruction.args().first().ok_or_else(|| {
        CircuitError::invalid_detector_error_model(
            "OBSERVABLE_INCLUDE missing observable index during sampled flow checking",
        )
    })?;
    let observable = checked_observable_arg_to_u32(*observable)?;
    Ok(selected_observables.contains(&observable))
}

fn checked_observable_arg_to_u32(observable: f64) -> CircuitResult<u32> {
    if !observable.is_finite()
        || observable < 0.0
        || observable > f64::from(u32::MAX)
        || observable.fract() != 0.0
    {
        return Err(CircuitError::invalid_detector_error_model(
            "OBSERVABLE_INCLUDE has invalid observable index during sampled flow checking",
        ));
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "observable was validated as a non-negative integer within u32 range"
    )]
    let observable = observable as u32;
    Ok(observable)
}

fn sampled_flow_measurement_target(
    measurement: i32,
    measurement_count: usize,
) -> CircuitResult<Target> {
    if flow_record_index(measurement, measurement_count).is_none() {
        return Err(CircuitError::invalid_detector_error_model(format!(
            "flow measurement record {measurement} is outside sampled flow circuit with {measurement_count} measurements"
        )));
    }
    let offset = if measurement >= 0 {
        let measurement_count = i64::try_from(measurement_count).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "measurement count does not fit i64 during sampled flow checking",
            )
        })?;
        i64::from(measurement)
            .checked_sub(measurement_count)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "measurement record offset underflowed during sampled flow checking",
                )
            })?
    } else {
        i64::from(measurement)
    };
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        i32::try_from(offset).map_err(|_| {
            CircuitError::invalid_detector_error_model(format!(
                "measurement record offset {offset} does not fit i32 during sampled flow checking"
            ))
        })?,
    )?))
}

fn append_one_target_instruction(
    circuit: &mut Circuit,
    gate_name: &'static str,
    args: Vec<f64>,
    target: Target,
    tag: Option<String>,
) -> CircuitResult<()> {
    circuit.append_instruction(CircuitInstruction::new(
        Gate::from_name(gate_name)?,
        args,
        vec![target],
        tag,
    )?);
    Ok(())
}

fn append_two_target_instruction(
    circuit: &mut Circuit,
    gate_name: &'static str,
    first: Target,
    second: Target,
    tag: Option<String>,
) -> CircuitResult<()> {
    circuit.append_instruction(CircuitInstruction::new(
        Gate::from_name(gate_name)?,
        Vec::new(),
        vec![first, second],
        tag,
    )?);
    Ok(())
}

fn qubit_id_from_index(index: usize, context: &'static str) -> CircuitResult<QubitId> {
    let index = u32::try_from(index).map_err(|_| {
        CircuitError::invalid_detector_error_model(format!("{context} index {index} exceeds u32"))
    })?;
    QubitId::new(index)
}

fn sampled_flow_seed(seed: Option<u64>, flow_index: usize) -> Option<u64> {
    seed.map(|seed| seed.wrapping_add(flow_index as u64))
}

#[cfg(test)]
mod tests {
    use super::{SAMPLED_FLOW_SAMPLE_WORD_WIDTH, rounded_sampled_flow_count};

    #[test]
    fn sampled_flow_counts_round_to_stim_word_width() {
        assert!(matches!(rounded_sampled_flow_count(0), Ok(0)));
        assert!(matches!(
            rounded_sampled_flow_count(1),
            Ok(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        ));
        assert!(matches!(
            rounded_sampled_flow_count(SAMPLED_FLOW_SAMPLE_WORD_WIDTH),
            Ok(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        ));
        assert!(matches!(
            rounded_sampled_flow_count(SAMPLED_FLOW_SAMPLE_WORD_WIDTH + 1),
            Ok(count) if count == SAMPLED_FLOW_SAMPLE_WORD_WIDTH * 2
        ));
        assert!(
            rounded_sampled_flow_count(usize::MAX).is_err(),
            "overflow should stay fail-closed"
        );
    }
}
