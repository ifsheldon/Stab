use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString};
use stab_analysis::{advanced::flow_record_index, circuit_without_noise};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, Gate, MeasureRecordOffset, QubitId, Target,
};
use stab_records::{MeasurementBatchView, MeasurementSink};
use thiserror::Error;

use crate::{RandomPolicy, RunError, SamplingCompiler, SamplingExecutionError, Seed, ShotCount};

const SAMPLED_FLOW_SAMPLE_WORD_WIDTH: u64 = 256;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SampledFlowError {
    #[error(transparent)]
    Model(#[from] stab_model::ModelError),

    #[error(transparent)]
    Analysis(#[from] stab_analysis::AnalysisError),

    #[error(transparent)]
    SamplingCompile(#[from] crate::SamplingCompileError),

    #[error(transparent)]
    SamplingExecution(#[from] SamplingExecutionError),

    #[error("{message}")]
    InvalidFlow { message: String },
}

impl SampledFlowError {
    fn invalid_flow(message: impl Into<String>) -> Self {
        Self::InvalidFlow {
            message: message.into(),
        }
    }
}

type SampledFlowResult<T> = Result<T, SampledFlowError>;

/// Probabilistically checks signed stabilizer flows by sampling augmented noiseless circuits.
///
/// This is the scoped Rust counterpart to Stim's `sample_if_circuit_has_stabilizer_flows`.
/// Unlike [`stab_analysis::check_if_circuit_has_unsigned_stabilizer_flows`], signs are meaningful
/// and each queried flow is checked by appending an ancilla witness measurement to a noiseless copy
/// of the circuit. Each false flow has a 50 percent chance of surviving an individual sample, so
/// callers should use enough samples for their desired confidence. The effective sample count is
/// rounded up to 256 to match Stim's `MAX_BITWORD_WIDTH` confidence behavior on the public Python
/// path.
pub fn sample_if_circuit_has_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    sample_count: ShotCount,
    random_policy: RandomPolicy,
) -> SampledFlowResult<Vec<bool>> {
    let noiseless = circuit_without_noise(circuit)?;
    let measurement_count = usize::try_from(noiseless.count_measurements()?).map_err(|_| {
        SampledFlowError::invalid_flow(
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
                sampled_flow_random_policy(random_policy, flow_index),
            )
        })
        .collect()
}

fn rounded_sampled_flow_count(sample_count: ShotCount) -> SampledFlowResult<ShotCount> {
    let sample_count = sample_count.get();
    let remainder = sample_count % SAMPLED_FLOW_SAMPLE_WORD_WIDTH;
    if remainder == 0 {
        return Ok(ShotCount::new(sample_count));
    }
    sample_count
        .checked_add(SAMPLED_FLOW_SAMPLE_WORD_WIDTH - remainder)
        .map(ShotCount::new)
        .ok_or_else(|| {
            SampledFlowError::invalid_flow(
                "sample count overflows while rounding sampled flow checks to Stim word width",
            )
        })
}

fn sample_if_noiseless_circuit_has_stabilizer_flow(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
    sample_count: ShotCount,
    random_policy: RandomPolicy,
) -> SampledFlowResult<bool> {
    let augmented = augmented_flow_test_circuit(circuit, flow, measurement_count)?;
    let plan = SamplingCompiler::new().compile(&augmented)?;
    let witness_index = measurement_count;
    let mut session = plan.session(random_policy)?;
    let mut sink = SampledFlowWitnessSink {
        witness_index,
        passed: true,
    };
    session
        .run(sample_count, &mut sink)
        .map_err(sampled_flow_run_error)?;
    Ok(sink.passed)
}

struct SampledFlowWitnessSink {
    witness_index: usize,
    passed: bool,
}

impl MeasurementSink for SampledFlowWitnessSink {
    type Error = SamplingExecutionError;

    fn write_batch(&mut self, batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        for shot_index in 0..batch.shot_count() {
            let witness = batch.get(shot_index, self.witness_index).ok_or_else(|| {
                SamplingExecutionError::InternalInvariant {
                    message: "sampled flow witness measurement was missing from augmented circuit"
                        .to_owned(),
                }
            })?;
            self.passed &= !witness;
        }
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn sampled_flow_run_error(error: RunError<SamplingExecutionError>) -> SampledFlowError {
    match error {
        RunError::Engine { source, .. } | RunError::Sink { source, .. } => source.into(),
    }
}

fn augmented_flow_test_circuit(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
) -> SampledFlowResult<Circuit> {
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
) -> SampledFlowResult<()> {
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
                output.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    body,
                    repeat.tag_bytes(),
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
) -> SampledFlowResult<()> {
    for target in instruction.targets() {
        if target.is_inverted_result_target() {
            append_one_target_instruction(
                output,
                "X",
                Vec::new(),
                Target::qubit(ancilla, false),
                instruction.tag_bytes(),
            )?;
        }
        if target.is_measurement_record_target() {
            append_two_target_instruction(
                output,
                "CX",
                target.clone(),
                Target::qubit(ancilla, false),
                instruction.tag_bytes(),
            )?;
        } else if target.is_x_target() {
            append_pauli_observable_feedback(output, "XCX", target, ancilla, instruction)?;
        } else if target.is_y_target() {
            append_pauli_observable_feedback(output, "YCX", target, ancilla, instruction)?;
        } else if target.is_z_target() {
            append_pauli_observable_feedback(output, "CX", target, ancilla, instruction)?;
        } else {
            return Err(SampledFlowError::invalid_flow(format!(
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
) -> SampledFlowResult<()> {
    let qubit = target.qubit_id().ok_or_else(|| {
        SampledFlowError::invalid_flow(format!(
            "sampled flow checking expected Pauli observable target {target} to contain a qubit"
        ))
    })?;
    append_two_target_instruction(
        output,
        gate_name,
        Target::qubit(qubit, false),
        Target::qubit(ancilla, false),
        source.tag_bytes(),
    )
}

fn append_pauli_controlled_not(
    circuit: &mut Circuit,
    pauli: &PauliString,
    ancilla: QubitId,
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
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
            tag,
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
) -> SampledFlowResult<bool> {
    let observable = instruction.args().first().ok_or_else(|| {
        SampledFlowError::invalid_flow(
            "OBSERVABLE_INCLUDE missing observable index during sampled flow checking",
        )
    })?;
    let observable = checked_observable_arg_to_u32(*observable)?;
    Ok(selected_observables.contains(&observable))
}

fn checked_observable_arg_to_u32(observable: f64) -> SampledFlowResult<u32> {
    if !observable.is_finite()
        || observable < 0.0
        || observable > f64::from(u32::MAX)
        || observable.fract() != 0.0
    {
        return Err(SampledFlowError::invalid_flow(
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
) -> SampledFlowResult<Target> {
    if flow_record_index(measurement, measurement_count).is_none() {
        return Err(SampledFlowError::invalid_flow(format!(
            "flow measurement record {measurement} is outside sampled flow circuit with {measurement_count} measurements"
        )));
    }
    let offset = if measurement >= 0 {
        let measurement_count = i64::try_from(measurement_count).map_err(|_| {
            SampledFlowError::invalid_flow(
                "measurement count does not fit i64 during sampled flow checking",
            )
        })?;
        i64::from(measurement)
            .checked_sub(measurement_count)
            .ok_or_else(|| {
                SampledFlowError::invalid_flow(
                    "measurement record offset underflowed during sampled flow checking",
                )
            })?
    } else {
        i64::from(measurement)
    };
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        i32::try_from(offset).map_err(|_| {
            SampledFlowError::invalid_flow(format!(
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
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
    circuit.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
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
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
    circuit.append_instruction(stab_model::advanced::circuit_instruction_with_tag_bytes(
        Gate::from_name(gate_name)?,
        Vec::new(),
        vec![first, second],
        tag,
    )?);
    Ok(())
}

fn qubit_id_from_index(index: usize, context: &'static str) -> SampledFlowResult<QubitId> {
    let index = u32::try_from(index).map_err(|_| {
        SampledFlowError::invalid_flow(format!("{context} index {index} exceeds u32"))
    })?;
    Ok(QubitId::new(index)?)
}

fn sampled_flow_random_policy(random_policy: RandomPolicy, flow_index: usize) -> RandomPolicy {
    match random_policy {
        RandomPolicy::Entropy => RandomPolicy::Entropy,
        RandomPolicy::Seeded(seed) => {
            RandomPolicy::Seeded(Seed::new(seed.get().wrapping_add(flow_index as u64)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SAMPLED_FLOW_SAMPLE_WORD_WIDTH, ShotCount, rounded_sampled_flow_count};

    #[test]
    fn sampled_flow_counts_round_to_stim_word_width() {
        assert!(matches!(
            rounded_sampled_flow_count(ShotCount::new(0)),
            Ok(count) if count == ShotCount::new(0)
        ));
        assert!(matches!(
            rounded_sampled_flow_count(ShotCount::new(1)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        ));
        assert!(matches!(
            rounded_sampled_flow_count(ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        ));
        assert!(matches!(
            rounded_sampled_flow_count(ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH + 1)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH * 2)
        ));
        assert!(
            rounded_sampled_flow_count(ShotCount::new(u64::MAX)).is_err(),
            "overflow should stay fail-closed"
        );
    }
}
