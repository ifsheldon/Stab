use std::collections::{BTreeMap, BTreeSet};

use stab_algebra::{Flow, PauliBasis, PauliSign, PauliString};
use stab_analysis::advanced::flow_record_index;
use stab_model::advanced::{CancellationCountdown, circuit_declaration_admitted_bytes};
use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, GateCategory, GateKind,
    MeasureRecordOffset, MemoryBudget, QubitId, Target, TargetKind,
};
use stab_records::{MeasurementBatchView, MeasurementSink};
use thiserror::Error;

use crate::{
    LaneWidth, RandomPolicy, ReferenceSampleMode, RunError, SamplingCompiler,
    SamplingExecutionError, SamplingRunStatus, Seed, ShotCount,
};

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

    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl SampledFlowError {
    fn invalid_flow(message: impl Into<String>) -> Self {
        Self::InvalidFlow {
            message: message.into(),
        }
    }
}

type SampledFlowResult<T> = Result<T, SampledFlowError>;

/// Resource and cancellation policy of [`sample_if_circuit_has_stabilizer_flows_with_options`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SampledFlowOptions {
    /// Budget that admits, for each queried flow, its augmented noiseless witness circuit, the
    /// sampling plan compiled from it, and the simulator and session storage held alongside the
    /// compiled plan. The flows are checked one after another, so each check is admitted on its
    /// own.
    pub memory_budget: MemoryBudget,
    /// Signal polled before each flow and while building, compiling, and sampling each witness
    /// circuit.
    pub cancellation: Cancellation,
}

/// Probabilistically checks signed stabilizer flows by sampling augmented noiseless circuits.
///
/// This is the scoped Rust counterpart to Stim's `sample_if_circuit_has_stabilizer_flows`.
/// Unlike [`stab_analysis::check_if_circuit_has_unsigned_stabilizer_flows`], signs are meaningful
/// and each queried flow is checked by appending an ancilla witness measurement to a noiseless copy
/// of the circuit. Each false flow has a 50 percent chance of surviving an individual sample, so
/// callers should use enough samples for their desired confidence. The effective sample count is
/// rounded up to 256 to match Stim's `MAX_BITWORD_WIDTH` confidence behavior on the public Python
/// path.
///
/// Admits its storage against [`MemoryBudget::DEFAULT`] and is not cancellable; see
/// [`sample_if_circuit_has_stabilizer_flows_with_options`].
pub fn sample_if_circuit_has_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
    sample_count: ShotCount,
    random_policy: RandomPolicy,
) -> SampledFlowResult<Vec<bool>> {
    sample_if_circuit_has_stabilizer_flows_with_options(
        circuit,
        flows,
        sample_count,
        random_policy,
        SampledFlowOptions::default(),
    )
}

/// [`sample_if_circuit_has_stabilizer_flows`] under a caller's memory budget and cancellation.
///
/// A check that the budget cannot admit fails with the typed admission error of the stage that
/// refused it: [`crate::SamplingCompileError::CompiledBytesLimit`] for the augmented witness
/// circuit or its compiled plan, or [`SamplingExecutionError::SessionStorageLimit`] for the
/// witness simulator or the sampling session. Observed cancellation fails with
/// [`SampledFlowError::Cancelled`] rather than returning a verdict from partial samples.
pub fn sample_if_circuit_has_stabilizer_flows_with_options(
    circuit: &Circuit,
    flows: &[Flow],
    sample_count: ShotCount,
    random_policy: RandomPolicy,
    options: SampledFlowOptions,
) -> SampledFlowResult<Vec<bool>> {
    options.cancellation.check()?;
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        SampledFlowError::invalid_flow(
            "circuit measurement count does not fit usize during sampled flow checking",
        )
    })?;
    let sample_count = rounded_sampled_flow_count(sample_count)?;
    flows
        .iter()
        .enumerate()
        .map(|(flow_index, flow)| {
            options.cancellation.check()?;
            sample_if_noiseless_circuit_has_stabilizer_flow(
                circuit,
                flow,
                measurement_count,
                sample_count,
                sampled_flow_random_policy(random_policy, flow_index),
                &options,
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
    options: &SampledFlowOptions,
) -> SampledFlowResult<bool> {
    let (augmented, simulator_qubits, augmented_measurements) =
        augmented_flow_test_circuit(circuit, flow, measurement_count, options)?;
    let augmented_bytes = augmented.admitted_bytes();
    let compile_budget = MemoryBudget::from_bytes(
        options
            .memory_budget
            .bytes()
            .saturating_sub(augmented_bytes),
    );
    let plan = SamplingCompiler::new()
        .memory_budget(compile_budget)
        .cancellation(options.cancellation.clone())
        .compile(&augmented)
        .map_err(|error| match error {
            crate::SamplingCompileError::CompiledBytesLimit { required, .. } => {
                crate::SamplingCompileError::CompiledBytesLimit {
                    required: augmented_bytes.saturating_add(required),
                    budget: options.memory_budget.bytes(),
                }
                .into()
            }
            other => SampledFlowError::from(other),
        })?;
    drop(augmented);

    let plan_bytes = plan.retained_bytes();
    let session_budget =
        MemoryBudget::from_bytes(options.memory_budget.bytes().saturating_sub(plan_bytes));
    let map_session_refusal = |error: SamplingExecutionError| -> SampledFlowError {
        match error {
            SamplingExecutionError::SessionStorageLimit {
                estimated_bytes, ..
            } => SamplingExecutionError::SessionStorageLimit {
                estimated_bytes: u128::from(plan_bytes).saturating_add(estimated_bytes),
                limit_bytes: options.memory_budget.bytes(),
            }
            .into(),
            other => other.into(),
        }
    };
    crate::sampling::validate_simulator_work_storage(
        simulator_qubits,
        augmented_measurements,
        true,
        session_budget,
    )
    .map_err(map_session_refusal)?;
    let witness_index = measurement_count;
    let mut session = plan
        .session_with_budget(
            random_policy,
            ReferenceSampleMode::UseReferenceSample,
            LaneWidth::default(),
            session_budget,
        )
        .map_err(map_session_refusal)?;
    let mut sink = SampledFlowWitnessSink {
        witness_index,
        passed: true,
    };
    let summary = session
        .run(sample_count, &mut sink)
        .map_err(sampled_flow_run_error)?;
    if summary.status() == SamplingRunStatus::Cancelled {
        // A cancelled run saw only some samples, so its verdict could pass a false flow.
        options.cancellation.check()?;
        return Err(SampledFlowError::invalid_flow(
            "sampled flow session reported cancellation that its signal does not show",
        ));
    }
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

struct AugmentedAdmission<'a> {
    charged_bytes: u64,
    budget: MemoryBudget,
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl<'a> AugmentedAdmission<'a> {
    const fn new(budget: MemoryBudget, cancellation: &'a Cancellation) -> Self {
        Self {
            charged_bytes: 0,
            budget,
            cancellation,
            countdown: CancellationCountdown::new(),
        }
    }

    fn admit_declaration(
        &mut self,
        target_count: usize,
        argument_count: usize,
        tag_bytes: usize,
    ) -> SampledFlowResult<()> {
        self.countdown.tick(self.cancellation)?;
        let added = circuit_declaration_admitted_bytes(target_count, argument_count, tag_bytes);
        let required = self.charged_bytes.saturating_add(added);
        if self.budget.admits(u128::from(required)) {
            self.charged_bytes = required;
            return Ok(());
        }
        Err(crate::SamplingCompileError::CompiledBytesLimit {
            required,
            budget: self.budget.bytes(),
        }
        .into())
    }
}

fn append_admitted_instruction(
    circuit: &mut Circuit,
    admission: &mut AugmentedAdmission<'_>,
    instruction: CircuitInstruction,
) -> SampledFlowResult<()> {
    admission.admit_declaration(
        instruction.targets().len(),
        instruction.args().len(),
        instruction.tag_bytes().map_or(0, <[u8]>::len),
    )?;
    circuit.append_instruction(instruction);
    Ok(())
}

fn augmented_flow_test_circuit(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
    options: &SampledFlowOptions,
) -> SampledFlowResult<(Circuit, usize, usize)> {
    let observables = flow.observables().collect::<BTreeSet<_>>();
    let qubits = DenseQubitMap::for_flow(circuit, flow, &observables)?;
    let qubit_count = qubits.len();
    let simulator_qubits = qubit_count.checked_add(1).ok_or_else(|| {
        SampledFlowError::invalid_flow("sampled flow ancilla qubit count overflowed")
    })?;
    let augmented_measurements = measurement_count.checked_add(1).ok_or_else(|| {
        SampledFlowError::invalid_flow(
            "sampled flow witness measurement count overflowed during admission",
        )
    })?;
    crate::sampling::validate_simulator_work_storage(
        simulator_qubits,
        augmented_measurements,
        true,
        options.memory_budget,
    )?;
    let ancilla = qubit_id_from_index(qubit_count, "sampled flow ancilla qubit")?;
    let mut augmented = Circuit::new();
    let mut admission = AugmentedAdmission::new(options.memory_budget, &options.cancellation);

    for qubit in 0..qubit_count {
        append_one_target_instruction(
            &mut augmented,
            &mut admission,
            GateKind::X_ERROR,
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
            &mut admission,
            GateKind::Z_ERROR,
            vec![0.5],
            Target::qubit(
                qubit_id_from_index(qubit, "sampled flow Z_ERROR qubit")?,
                false,
            ),
            None,
        )?;
    }

    append_pauli_controlled_not(
        &mut augmented,
        &mut admission,
        flow.input(),
        ancilla,
        &qubits,
        None,
    )?;
    append_flow_test_block_for_circuit(
        &mut augmented,
        &mut admission,
        circuit,
        ancilla,
        &observables,
        &qubits,
    )?;
    for measurement in flow.measurements() {
        let record = sampled_flow_measurement_target(measurement, measurement_count)?;
        append_two_target_instruction(
            &mut augmented,
            &mut admission,
            GateKind::CX,
            record,
            Target::qubit(ancilla, false),
            None,
        )?;
    }
    append_pauli_controlled_not(
        &mut augmented,
        &mut admission,
        flow.output(),
        ancilla,
        &qubits,
        None,
    )?;
    append_one_target_instruction(
        &mut augmented,
        &mut admission,
        GateKind::M,
        Vec::new(),
        Target::qubit(ancilla, false),
        None,
    )?;

    Ok((augmented, simulator_qubits, augmented_measurements))
}

struct DenseQubitMap {
    dense_ids: BTreeMap<u32, QubitId>,
}

impl DenseQubitMap {
    fn for_flow(
        circuit: &Circuit,
        flow: &Flow,
        observables: &BTreeSet<u32>,
    ) -> SampledFlowResult<Self> {
        let mut originals = BTreeSet::new();
        collect_flow_qubits(flow.input(), &mut originals)?;
        collect_flow_qubits(flow.output(), &mut originals)?;
        collect_circuit_qubits(circuit, observables, &mut originals)?;
        let dense_ids = originals
            .into_iter()
            .enumerate()
            .map(|(dense, original)| {
                Ok((
                    original,
                    qubit_id_from_index(dense, "sampled flow dense qubit")?,
                ))
            })
            .collect::<SampledFlowResult<_>>()?;
        Ok(Self { dense_ids })
    }

    fn len(&self) -> usize {
        self.dense_ids.len()
    }

    fn get(&self, original: QubitId) -> SampledFlowResult<QubitId> {
        self.dense_ids.get(&original.get()).copied().ok_or_else(|| {
            SampledFlowError::invalid_flow(format!(
                "sampled flow qubit {} has no dense mapping",
                original.get()
            ))
        })
    }

    fn get_index(&self, original: usize) -> SampledFlowResult<QubitId> {
        let original = u32::try_from(original).map_err(|_| {
            SampledFlowError::invalid_flow(format!(
                "sampled flow Pauli qubit index {original} exceeds u32"
            ))
        })?;
        self.dense_ids.get(&original).copied().ok_or_else(|| {
            SampledFlowError::invalid_flow(format!(
                "sampled flow Pauli qubit {original} has no dense mapping"
            ))
        })
    }
}

fn collect_flow_qubits(pauli: &PauliString, qubits: &mut BTreeSet<u32>) -> SampledFlowResult<()> {
    for (index, _) in pauli.active_terms() {
        qubits.insert(u32::try_from(index).map_err(|_| {
            SampledFlowError::invalid_flow(format!(
                "sampled flow Pauli qubit index {index} exceeds u32"
            ))
        })?);
    }
    Ok(())
}

fn collect_circuit_qubits(
    circuit: &Circuit,
    observables: &BTreeSet<u32>,
    qubits: &mut BTreeSet<u32>,
) -> SampledFlowResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                let gate = instruction.gate();
                let selected_observable = gate.kind() == GateKind::OBSERVABLE_INCLUDE
                    && observable_is_selected(instruction, observables)?;
                if gate.category() == GateCategory::Annotation && !selected_observable {
                    continue;
                }
                if gate.targets_are_pad_values() || gate.produces_heralded_results() {
                    continue;
                }
                if !gate.produces_measurements() && gate.is_noisy() {
                    continue;
                }
                qubits.extend(
                    instruction
                        .targets()
                        .iter()
                        .filter_map(Target::qubit_id)
                        .map(QubitId::get),
                );
            }
            CircuitItem::RepeatBlock(repeat) => {
                collect_circuit_qubits(repeat.body(), observables, qubits)?;
            }
        }
    }
    Ok(())
}

fn append_flow_test_block_for_circuit(
    output: &mut Circuit,
    admission: &mut AugmentedAdmission<'_>,
    circuit: &Circuit,
    ancilla: QubitId,
    observables: &BTreeSet<u32>,
    qubits: &DenseQubitMap,
) -> SampledFlowResult<()> {
    for item in circuit.items() {
        admission.countdown.tick(admission.cancellation)?;
        match item {
            CircuitItem::Instruction(instruction)
                if instruction.gate().kind() == GateKind::OBSERVABLE_INCLUDE
                    && observable_is_selected(instruction, observables)? =>
            {
                append_selected_observable_feedback(
                    output,
                    admission,
                    instruction,
                    ancilla,
                    qubits,
                )?;
            }
            CircuitItem::Instruction(instruction)
                if instruction.gate().category() == GateCategory::Annotation => {}
            CircuitItem::Instruction(instruction) => {
                let gate = instruction.gate();
                if gate.produces_heralded_results() {
                    let target_count = instruction.targets().len();
                    let tag = instruction.tag_bytes();
                    admission.admit_declaration(target_count, 0, tag.map_or(0, <[u8]>::len))?;
                    let mut pad_targets = Vec::new();
                    pad_targets
                        .try_reserve_exact(target_count)
                        .map_err(|error| {
                            SampledFlowError::invalid_flow(format!(
                                "unable to reserve {target_count} heralded pad targets: {error}"
                            ))
                        })?;
                    pad_targets.resize(target_count, Target::qubit(QubitId::new(0)?, false));
                    output.append_instruction(
                        stab_model::advanced::circuit_instruction_with_tag_bytes(
                            GateKind::MPAD.gate(),
                            Vec::new(),
                            pad_targets,
                            tag,
                        )?,
                    );
                } else if gate.produces_measurements() {
                    let remapped = remap_instruction_with_args(instruction, Vec::new(), qubits)?;
                    append_admitted_instruction(output, admission, remapped)?;
                } else if !gate.is_noisy() {
                    let remapped = remap_instruction_with_args(
                        instruction,
                        instruction.args().to_vec(),
                        qubits,
                    )?;
                    append_admitted_instruction(output, admission, remapped)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = Circuit::new();
                append_flow_test_block_for_circuit(
                    &mut body,
                    admission,
                    repeat.body(),
                    ancilla,
                    observables,
                    qubits,
                )?;
                if !body.is_empty() {
                    let tag = repeat.tag_bytes();
                    admission.admit_declaration(0, 0, tag.map_or(0, <[u8]>::len))?;
                    output.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                        repeat.repeat_count(),
                        body,
                        tag,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn remap_instruction_with_args(
    instruction: &CircuitInstruction,
    args: Vec<f64>,
    qubits: &DenseQubitMap,
) -> SampledFlowResult<CircuitInstruction> {
    let targets = if instruction.gate().targets_are_pad_values() {
        instruction.targets().to_vec()
    } else {
        instruction
            .targets()
            .iter()
            .map(|target| match target.kind() {
                TargetKind::Qubit { id, inverted } => Ok(Target::qubit(qubits.get(id)?, inverted)),
                TargetKind::Pauli {
                    pauli,
                    id,
                    inverted,
                } => Ok(Target::pauli(pauli, qubits.get(id)?, inverted)),
                TargetKind::MeasurementRecord { .. }
                | TargetKind::SweepBit { .. }
                | TargetKind::Combiner => Ok(*target),
            })
            .collect::<SampledFlowResult<Vec<_>>>()?
    };
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        args,
        targets,
        instruction.tag_bytes(),
    )?)
}

fn append_selected_observable_feedback(
    output: &mut Circuit,
    admission: &mut AugmentedAdmission<'_>,
    instruction: &CircuitInstruction,
    ancilla: QubitId,
    qubits: &DenseQubitMap,
) -> SampledFlowResult<()> {
    for target in instruction.targets() {
        if target.is_inverted_result_target() {
            append_one_target_instruction(
                output,
                admission,
                GateKind::X,
                Vec::new(),
                Target::qubit(ancilla, false),
                instruction.tag_bytes(),
            )?;
        }
        if target.is_measurement_record_target() {
            append_two_target_instruction(
                output,
                admission,
                GateKind::CX,
                *target,
                Target::qubit(ancilla, false),
                instruction.tag_bytes(),
            )?;
        } else if target.is_x_target() {
            append_pauli_observable_feedback(
                output,
                admission,
                GateKind::XCX,
                target,
                ancilla,
                instruction,
                qubits,
            )?;
        } else if target.is_y_target() {
            append_pauli_observable_feedback(
                output,
                admission,
                GateKind::YCX,
                target,
                ancilla,
                instruction,
                qubits,
            )?;
        } else if target.is_z_target() {
            append_pauli_observable_feedback(
                output,
                admission,
                GateKind::CX,
                target,
                ancilla,
                instruction,
                qubits,
            )?;
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
    admission: &mut AugmentedAdmission<'_>,
    gate: GateKind,
    target: &Target,
    ancilla: QubitId,
    source: &CircuitInstruction,
    qubits: &DenseQubitMap,
) -> SampledFlowResult<()> {
    let qubit = target.qubit_id().ok_or_else(|| {
        SampledFlowError::invalid_flow(format!(
            "sampled flow checking expected Pauli observable target {target} to contain a qubit"
        ))
    })?;
    append_two_target_instruction(
        output,
        admission,
        gate,
        Target::qubit(qubits.get(qubit)?, false),
        Target::qubit(ancilla, false),
        source.tag_bytes(),
    )
}

fn append_pauli_controlled_not(
    circuit: &mut Circuit,
    admission: &mut AugmentedAdmission<'_>,
    pauli: &PauliString,
    ancilla: QubitId,
    qubits: &DenseQubitMap,
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
    for (index, basis) in pauli.active_terms() {
        let gate = match basis {
            PauliBasis::X => GateKind::XCX,
            PauliBasis::Y => GateKind::YCX,
            PauliBasis::Z => GateKind::CX,
            PauliBasis::I => continue,
        };
        append_two_target_instruction(
            circuit,
            admission,
            gate,
            Target::qubit(qubits.get_index(index)?, false),
            Target::qubit(ancilla, false),
            tag,
        )?;
    }
    if pauli.sign() == PauliSign::Minus {
        append_one_target_instruction(
            circuit,
            admission,
            GateKind::X,
            Vec::new(),
            Target::qubit(ancilla, false),
            tag,
        )?;
    }
    Ok(())
}

fn observable_is_selected(
    instruction: &CircuitInstruction,
    selected_observables: &BTreeSet<u32>,
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
    admission: &mut AugmentedAdmission<'_>,
    gate: GateKind,
    args: Vec<f64>,
    target: Target,
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
    let instruction = stab_model::advanced::circuit_instruction_with_tag_bytes(
        gate.gate(),
        args,
        vec![target],
        tag,
    )?;
    append_admitted_instruction(circuit, admission, instruction)
}

fn append_two_target_instruction(
    circuit: &mut Circuit,
    admission: &mut AugmentedAdmission<'_>,
    gate: GateKind,
    first: Target,
    second: Target,
    tag: Option<&[u8]>,
) -> SampledFlowResult<()> {
    let instruction = stab_model::advanced::circuit_instruction_with_tag_bytes(
        gate.gate(),
        Vec::new(),
        vec![first, second],
        tag,
    )?;
    append_admitted_instruction(circuit, admission, instruction)
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
    use std::assert_matches;

    use super::{SAMPLED_FLOW_SAMPLE_WORD_WIDTH, ShotCount, rounded_sampled_flow_count};

    #[test]
    fn sampled_flow_counts_round_to_stim_word_width() {
        assert_matches!(
            rounded_sampled_flow_count(ShotCount::new(0)),
            Ok(count) if count == ShotCount::new(0)
        );
        assert_matches!(
            rounded_sampled_flow_count(ShotCount::new(1)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        );
        assert_matches!(
            rounded_sampled_flow_count(ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH)
        );
        assert_matches!(
            rounded_sampled_flow_count(ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH + 1)),
            Ok(count) if count == ShotCount::new(SAMPLED_FLOW_SAMPLE_WORD_WIDTH * 2)
        );
        assert!(
            rounded_sampled_flow_count(ShotCount::new(u64::MAX)).is_err(),
            "overflow should stay fail-closed"
        );
    }
}
