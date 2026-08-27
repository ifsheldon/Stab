#[cfg(test)]
use rand::Rng;
use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_algebra::PauliBasis;
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, GateCategory, MeasureRecordOffset, ModelDialect,
    Pauli, Probability, QubitId, Target,
    advanced::{
        ClassicalControl, ControlledPauliTargetPair, classify_controlled_pauli_target_pair,
    },
};

use self::execute::count_determined_operations;
#[cfg(test)]
use self::execute::{ExecutionBuffers, execute_operations};
use self::operation::{SampleOperation, SampleProgram};
use self::stabilizer_frame::{LocalTableauTransform, MeasurementRandomness, StabilizerFrame};
use crate::{CompilationDescriptor, CompilationOperation, CompilationRequestFingerprint};

mod api;
mod direct_z_measurement;
mod execute;
mod measurement_flip;
mod noise;
mod operation;
pub(crate) mod pauli_product;
mod reference;
mod small_frame;
mod stabilizer_frame;

pub use api::{
    PlanFingerprint, RandomPolicy, ReferenceSampleLoopPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
};

/// Sampling compiler descriptor consumed by facade capability aggregation.
pub const COMPILATION_DESCRIPTOR: CompilationDescriptor = CompilationDescriptor::new(
    CompilationOperation::Sampling,
    ModelDialect::StimCircuit,
    CompilationRequestFingerprint::SAMPLING_COMPILER_SCHEMA_VERSION,
    Some(CompilationRequestFingerprint::SCHEMA_VERSION),
    false,
);

impl SamplingPlan {
    /// Counts deterministic measurements after admitting and allocating bounded analysis storage.
    pub fn try_count_determined_measurements(
        &self,
        unknown_input: bool,
    ) -> Result<u64, SamplingExecutionError> {
        if self.inner.measurement_count == 0 {
            return Ok(0);
        }
        if let api::SamplingPlanKind::DirectZ(direct) = self.inner.kind {
            return Ok(direct.determined_measurement_count(unknown_input));
        }
        validate_general_frame_work_storage(
            self.inner.qubit_count,
            self.inner.measurement_count,
            false,
        )?;
        let mut rng = SmallRng::seed_from_u64(0);
        let mut frame = if unknown_input {
            StabilizerFrame::try_new_unknown(self.inner.qubit_count)
        } else {
            StabilizerFrame::try_new(self.inner.qubit_count)
        }
        .map_err(|error| SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        })?;
        let mut record = api::try_bool_buffer(
            self.inner.measurement_count,
            "determined measurement record",
        )?;
        count_determined_operations(&self.inner.operations, &mut frame, &mut record, &mut rng)
    }

    /// Computes the deterministic reference sample with bounded, fallible storage.
    pub fn try_reference_sample(&self) -> Result<Vec<bool>, SamplingExecutionError> {
        api::compute_reference_sample(&self.inner)
    }

    pub(crate) fn sweep_bit_count(&self) -> usize {
        self.inner.sweep_bit_count
    }

    pub(crate) fn estimated_reference_work_storage_bytes(&self) -> u128 {
        if self.inner.measurement_count == 0 {
            return 0;
        }
        if matches!(self.inner.kind, api::SamplingPlanKind::DirectZ(_)) {
            return 1;
        }
        general_frame_work_storage_bytes(
            self.inner.qubit_count,
            self.inner.measurement_count,
            self.inner.uses_reference_state_snapshot(),
        )
    }

    #[cfg(test)]
    fn sample_shot_in_mode_into<R>(
        &self,
        rng: &mut R,
        mode: ExecutionMode,
        sweep_record: &[bool],
        frame: &mut StabilizerFrame,
        record: &mut Vec<bool>,
        output: &mut Vec<bool>,
    ) -> Result<(), SamplingExecutionError>
    where
        R: Rng,
    {
        frame.reset_to_z_basis();
        record.clear();
        output.clear();
        let mut correlated_error_occurred = false;
        let mut buffers = ExecutionBuffers {
            frame,
            record,
            output,
            correlated_error_occurred: &mut correlated_error_occurred,
        };
        execute_operations(
            &self.inner.operations,
            &mut buffers,
            rng,
            mode,
            sweep_record,
        )
    }
}

fn validate_general_frame_work_storage(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes = general_frame_work_storage_bytes(
        qubit_count,
        measurement_count,
        include_reference_snapshot,
    );
    if estimated_bytes > u128::from(api::MAX_SAMPLING_SESSION_STORAGE_BYTES) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: api::MAX_SAMPLING_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

fn general_frame_work_storage_bytes(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
) -> u128 {
    let qubits = qubit_count as u128;
    let measurements = measurement_count as u128;
    let base = qubits
        .saturating_mul(qubits)
        .saturating_mul(4)
        .saturating_add(qubits.saturating_mul(256))
        .saturating_add(measurements);
    if include_reference_snapshot {
        base.saturating_add(stabilizer_frame::StabilizerStateSnapshot::storage_bytes(
            qubit_count,
        ))
    } else {
        base
    }
}

fn reference_state_snapshot_fits(
    qubit_count: usize,
    measurement_count: usize,
    limit_bytes: u64,
) -> bool {
    general_frame_work_storage_bytes(qubit_count, measurement_count, true)
        <= u128::from(limit_bytes)
}

/// Error from [`count_determined_measurements`], covering compilation and bounded execution.
#[derive(Debug, thiserror::Error)]
pub enum CountDeterminedMeasurementsError {
    #[error(transparent)]
    Compile(#[from] SamplingCompileError),
    #[error(transparent)]
    Execution(#[from] SamplingExecutionError),
}

/// Error from computing a circuit's deterministic all-zero-sweep reference sample.
#[derive(Debug, thiserror::Error)]
pub enum CircuitReferenceSampleError {
    #[error(transparent)]
    Compile(#[from] SamplingCompileError),
    #[error(transparent)]
    Execution(#[from] SamplingExecutionError),
}

/// Computes a circuit's deterministic reference sample with every sweep bit set to false.
pub fn circuit_reference_sample(
    circuit: &Circuit,
) -> Result<Vec<bool>, CircuitReferenceSampleError> {
    let plan = SamplingCompiler::new().compile(circuit)?;
    let mut sweep_record = api::try_bool_buffer(plan.sweep_bit_count(), "reference sweep record")?;
    sweep_record.resize(plan.sweep_bit_count(), false);
    let mut output = api::try_bool_buffer(
        plan.measurement_width().get(),
        "reference measurement output",
    )?;
    plan.reference_measurement_record_with_sweep_into(&sweep_record, &mut output)?;
    Ok(output)
}

pub fn count_determined_measurements(
    circuit: &Circuit,
    unknown_input: bool,
) -> Result<u64, CountDeterminedMeasurementsError> {
    Ok(SamplingCompiler::new()
        .compile(circuit)?
        .try_count_determined_measurements(unknown_input)?)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionMode {
    Sample,
    ReferenceSample,
}

impl ExecutionMode {
    fn measurement_randomness(self) -> MeasurementRandomness {
        match self {
            Self::Sample => MeasurementRandomness::Random,
            Self::ReferenceSample => MeasurementRandomness::DeterministicFalse,
        }
    }

    fn includes_noise(self) -> bool {
        matches!(self, Self::Sample)
    }
}

fn sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompileState {
    measurement_count: u64,
    sweep_bit_count: u64,
    expanded_operations: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompiledCounts {
    measurements: usize,
    sweep_bits: usize,
    expanded_operations: u128,
}

impl CompileState {
    fn new() -> Self {
        Self {
            measurement_count: 0,
            sweep_bit_count: 0,
            expanded_operations: 0,
        }
    }

    fn add_measurements(&mut self, count: usize) -> Result<(), SamplingCompileError> {
        let count = u64::try_from(count).map_err(|_| {
            SamplingCompileError::invalid_circuit(
                "measurement record count cannot fit in u64 during sampler compilation",
            )
        })?;
        self.measurement_count = self.measurement_count.checked_add(count).ok_or_else(|| {
            SamplingCompileError::invalid_circuit(
                "measurement record count overflows during sampler compilation",
            )
        })?;
        Ok(())
    }

    fn add_repeated_measurements(
        &mut self,
        per_body: u64,
        repeat_count: u64,
    ) -> Result<(), SamplingCompileError> {
        let total = per_body.checked_mul(repeat_count).ok_or_else(|| {
            SamplingCompileError::invalid_circuit(
                "repeated measurement record count overflows during sampler compilation",
            )
        })?;
        self.measurement_count = self.measurement_count.checked_add(total).ok_or_else(|| {
            SamplingCompileError::invalid_circuit(
                "measurement record count overflows during sampler compilation",
            )
        })?;
        Ok(())
    }

    fn add_operations(&mut self, count: usize) {
        self.expanded_operations = self.expanded_operations.saturating_add(count as u128);
    }

    fn add_repeated_operations(&mut self, per_body: u128, repeat_count: u64) {
        self.expanded_operations = self
            .expanded_operations
            .saturating_add(per_body.saturating_mul(u128::from(repeat_count)));
    }

    fn validate_record_offset(
        self,
        instruction: &CircuitInstruction,
        offset: MeasureRecordOffset,
    ) -> Result<(), SamplingCompileError> {
        let required = u64::from(offset.get().unsigned_abs());
        if required == 0 {
            return Err(SamplingCompileError::invalid_circuit(format!(
                "measurement record target rec[{}] is not a valid lookback while compiling {} feedback",
                offset.stim_text(),
                instruction.gate().canonical_name()
            )));
        }
        if required <= self.measurement_count {
            return Ok(());
        }
        Err(SamplingCompileError::invalid_circuit(format!(
            "measurement record target rec[{}] is not available while compiling {} feedback",
            offset.get(),
            instruction.gate().canonical_name()
        )))
    }

    fn add_sweep_bit(&mut self, sweep_id: u32) -> Result<usize, SamplingCompileError> {
        let sweep_id = u64::from(sweep_id);
        self.sweep_bit_count =
            self.sweep_bit_count
                .max(sweep_id.checked_add(1).ok_or_else(|| {
                    SamplingCompileError::invalid_circuit("sweep bit count overflowed")
                })?);
        usize::try_from(sweep_id).map_err(|_| {
            SamplingCompileError::invalid_circuit(format!(
                "sweep bit id {sweep_id} cannot fit in this platform's usize"
            ))
        })
    }
}

fn compile_circuit(
    circuit: &Circuit,
    program: &mut SampleProgram,
) -> Result<CompiledCounts, SamplingCompileError> {
    let state = compile_circuit_with_state(circuit, program, CompileState::new())?;
    program.elide_leading_z_resets();
    let measurements = usize::try_from(state.measurement_count).map_err(|_| {
        SamplingCompileError::invalid_circuit(
            "measurement record count cannot fit in usize during sampler compilation",
        )
    })?;
    let sweep_bits = usize::try_from(state.sweep_bit_count).map_err(|_| {
        SamplingCompileError::invalid_circuit(
            "sweep bit count cannot fit in usize during sampler compilation",
        )
    })?;
    Ok(CompiledCounts {
        measurements,
        sweep_bits,
        expanded_operations: state.expanded_operations,
    })
}

fn compile_circuit_with_state(
    circuit: &Circuit,
    program: &mut SampleProgram,
    state: CompileState,
) -> Result<CompileState, SamplingCompileError> {
    let mut frames = arrayvec::ArrayVec::<
        CompileFrame<'_>,
        { stab_model::RepeatNestingLimit::HARD_MAX + 1 },
    >::new();
    frames.push(CompileFrame::root(circuit, state));

    loop {
        let item = frames.last_mut().and_then(|frame| frame.items.next());
        match item {
            Some(CircuitItem::Instruction(instruction)) => {
                let Some(frame) = frames.last_mut() else {
                    return Err(SamplingCompileError::invalid_circuit(
                        "sampler compilation lost its active circuit frame",
                    ));
                };
                let operations_before = program.entry_count();
                compile_instruction(instruction, program, &mut frame.state)?;
                frame
                    .state
                    .add_operations(program.entry_count().saturating_sub(operations_before));
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let Some(parent) = frames.last() else {
                    return Err(SamplingCompileError::invalid_circuit(
                        "sampler compilation lost its parent repeat frame",
                    ));
                };
                let marker = program.begin_repeat(repeat.repeat_count().get())?;
                let child = CompileFrame::repeated(
                    repeat.body(),
                    parent.state,
                    marker,
                    repeat.repeat_count().get(),
                );
                frames.try_push(child).map_err(|_| {
                    SamplingCompileError::invalid_circuit(format!(
                        "repeat nesting exceeds sampler limit {}",
                        stab_model::RepeatNestingLimit::HARD_MAX
                    ))
                })?;
            }
            None => {
                let Some(completed) = frames.pop() else {
                    return Err(SamplingCompileError::invalid_circuit(
                        "sampler compilation exhausted an empty traversal stack",
                    ));
                };
                let Some(repeat) = completed.repeat else {
                    return Ok(completed.state);
                };
                program.finish_repeat(repeat.marker)?;
                let Some(parent) = frames.last_mut() else {
                    return Err(SamplingCompileError::invalid_circuit(
                        "sampler compilation completed a repeat without a parent",
                    ));
                };
                let body_measurements = completed
                    .state
                    .measurement_count
                    .checked_sub(repeat.measurements_before_body)
                    .ok_or_else(|| {
                        SamplingCompileError::invalid_circuit(
                            "repeat-body measurement count moved backwards during sampler compilation",
                        )
                    })?;
                parent
                    .state
                    .add_repeated_measurements(body_measurements, repeat.count)?;
                let body_operations = completed
                    .state
                    .expanded_operations
                    .saturating_sub(repeat.operations_before_body);
                parent
                    .state
                    .add_repeated_operations(body_operations, repeat.count);
                parent.state.sweep_bit_count = parent
                    .state
                    .sweep_bit_count
                    .max(completed.state.sweep_bit_count);
            }
        }
    }
}

struct CompileFrame<'a> {
    items: std::slice::Iter<'a, CircuitItem>,
    state: CompileState,
    repeat: Option<RepeatCompile>,
}

impl<'a> CompileFrame<'a> {
    fn root(circuit: &'a Circuit, state: CompileState) -> Self {
        Self {
            items: circuit.items().iter(),
            state,
            repeat: None,
        }
    }

    fn repeated(circuit: &'a Circuit, state: CompileState, marker: usize, count: u64) -> Self {
        Self {
            items: circuit.items().iter(),
            state,
            repeat: Some(RepeatCompile {
                marker,
                count,
                measurements_before_body: state.measurement_count,
                operations_before_body: state.expanded_operations,
            }),
        }
    }
}

#[derive(Clone, Copy)]
struct RepeatCompile {
    marker: usize,
    count: u64,
    measurements_before_body: u64,
    operations_before_body: u128,
}

fn compile_instruction(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let gate = instruction.gate();
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => Ok(()),
        "R" | "RX" | "RY" => compile_reset(instruction, operations),
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => {
            compile_measurement(instruction, operations, state)
        }
        "MXX" | "MYY" | "MZZ" => compile_pair_measurement(instruction, operations, state),
        "MPP" => compile_pauli_product_measurement(instruction, operations, state),
        "MPAD" => compile_measurement_pads(instruction, operations, state),
        "SPP" | "SPP_DAG" => compile_decomposed_instruction(instruction, operations, state),
        "CX" | "XCZ" => {
            compile_controlled_or_feedback(instruction, operations, state, PauliBasis::X)
        }
        "CY" | "YCZ" => {
            compile_controlled_or_feedback(instruction, operations, state, PauliBasis::Y)
        }
        "CZ" => compile_controlled_or_feedback(instruction, operations, state, PauliBasis::Z),
        _ if stab_analysis::single_qubit_clifford_for_gate(gate).is_ok() => {
            compile_single_qubit_clifford(instruction, operations)
        }
        _ if stab_analysis::gate_has_tableau(gate) => {
            compile_unitary_tableau(instruction, operations)
        }
        "X_ERROR" => compile_single_qubit_pauli_channel(
            instruction,
            operations,
            [single_probability_argument(instruction)?.get(), 0.0, 0.0],
        ),
        "Y_ERROR" => compile_single_qubit_pauli_channel(
            instruction,
            operations,
            [0.0, single_probability_argument(instruction)?.get(), 0.0],
        ),
        "Z_ERROR" => compile_single_qubit_pauli_channel(
            instruction,
            operations,
            [0.0, 0.0, single_probability_argument(instruction)?.get()],
        ),
        "I_ERROR" => Ok(()),
        "DEPOLARIZE1" => {
            let probability = single_probability_argument(instruction)?.get() / 3.0;
            compile_single_qubit_pauli_channel(
                instruction,
                operations,
                [probability, probability, probability],
            )
        }
        "DEPOLARIZE2" => {
            let probability = single_probability_argument(instruction)?.get();
            compile_two_qubit_pauli_channel(instruction, operations, [probability / 15.0; 15])
        }
        "II_ERROR" => Ok(()),
        "PAULI_CHANNEL_1" => {
            let Some(probabilities) = instruction.probability_arguments()? else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            let [x_probability, y_probability, _z_probability] = probabilities.as_slice() else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            compile_single_qubit_pauli_channel(
                instruction,
                operations,
                [
                    x_probability.get(),
                    y_probability.get(),
                    _z_probability.get(),
                ],
            )
        }
        "PAULI_CHANNEL_2" => {
            let Some(probabilities) = instruction.probability_arguments()? else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            if probabilities.len() != 15 {
                return Err(unsupported_sampler_instruction(instruction));
            }
            let mut channel_probabilities = [0.0; 15];
            for (channel_probability, probability) in
                channel_probabilities.iter_mut().zip(probabilities.iter())
            {
                *channel_probability = probability.get();
            }
            compile_two_qubit_pauli_channel(instruction, operations, channel_probabilities)
        }
        "E" => compile_correlated_error(instruction, operations, false),
        "ELSE_CORRELATED_ERROR" => compile_correlated_error(instruction, operations, true),
        "HERALDED_ERASE" => {
            let probability = single_probability_argument(instruction)?.get() / 4.0;
            compile_heralded_pauli_channel(
                instruction,
                operations,
                state,
                [probability, probability, probability, probability],
            )
        }
        "HERALDED_PAULI_CHANNEL_1" => {
            let Some(probabilities) = instruction.probability_arguments()? else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            let [i_probability, x_probability, y_probability, z_probability] =
                probabilities.as_slice()
            else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            compile_heralded_pauli_channel(
                instruction,
                operations,
                state,
                [
                    i_probability.get(),
                    x_probability.get(),
                    y_probability.get(),
                    z_probability.get(),
                ],
            )
        }
        _ if zero_probability_noise(instruction)? => Ok(()),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn compile_decomposed_instruction(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let decomposed =
        stab_analysis::advanced::decomposed_single_instruction(instruction).map_err(|error| {
            SamplingCompileError::invalid_circuit(format!(
                "{} cannot be executed via decomposition: {error}",
                instruction.gate().canonical_name()
            ))
        })?;
    *state = compile_circuit_with_state(&decomposed, operations, *state)?;
    Ok(())
}

fn compile_reset(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
) -> Result<(), SamplingCompileError> {
    let basis = measurement_basis(instruction)?;
    for target in instruction.targets() {
        operations.push(SampleOperation::Reset {
            qubit: qubit_index(instruction, target)?,
            basis,
        })?;
    }
    Ok(())
}

fn compile_measurement(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let basis = measurement_basis(instruction)?;
    let flip_probability = measurement_flip_probability(instruction)?;
    let reset = matches!(instruction.gate().canonical_name(), "MR" | "MRX" | "MRY");
    for target in instruction.targets() {
        operations.push(SampleOperation::Measure {
            qubit: qubit_index(instruction, target)?,
            basis,
            inverted: target.is_inverted_result_target(),
            flip_probability,
            reset,
        })?;
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_pair_measurement(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let basis = pair_measurement_basis(instruction)?;
    let flip_probability = measurement_flip_probability(instruction)?;
    let groups = instruction.target_groups();
    for target_pair in &groups {
        let [left, right] = *target_pair else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::MeasureProduct {
            terms: vec![
                (qubit_index(instruction, left)?, basis),
                (qubit_index(instruction, right)?, basis),
            ],
            inverted: left.is_inverted_result_target() ^ right.is_inverted_result_target(),
            flip_probability,
        })?;
    }
    state.add_measurements(groups.len())?;
    Ok(())
}

fn compile_pauli_product_measurement(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let flip_probability = measurement_flip_probability(instruction)?;
    let groups = instruction.target_groups();
    for target_group in &groups {
        let mut raw_terms = Vec::new();
        for target in *target_group {
            if target.is_combiner() {
                continue;
            }
            let Some(pauli) = target.pauli_type() else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            raw_terms.push((
                qubit_index(instruction, target)?,
                pauli_basis(pauli),
                target.is_inverted_result_target(),
            ));
        }
        let (terms, inverted) = pauli_product::normalize_terms(raw_terms, false)?;
        operations.push(SampleOperation::MeasureProduct {
            terms,
            inverted,
            flip_probability,
        })?;
    }
    state.add_measurements(groups.len())?;
    Ok(())
}

fn measurement_basis(instruction: &CircuitInstruction) -> Result<PauliBasis, SamplingCompileError> {
    match instruction.gate().canonical_name() {
        "MX" | "MRX" | "RX" => Ok(PauliBasis::X),
        "MY" | "MRY" | "RY" => Ok(PauliBasis::Y),
        "M" | "MR" | "R" => Ok(PauliBasis::Z),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn pair_measurement_basis(
    instruction: &CircuitInstruction,
) -> Result<PauliBasis, SamplingCompileError> {
    match instruction.gate().canonical_name() {
        "MXX" => Ok(PauliBasis::X),
        "MYY" => Ok(PauliBasis::Y),
        "MZZ" => Ok(PauliBasis::Z),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}

fn compile_measurement_pads(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
) -> Result<(), SamplingCompileError> {
    let flip_probability = measurement_flip_probability(instruction)?;
    for target in instruction.targets() {
        let Some(qubit) = target.qubit_id() else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::Pad {
            value: qubit.get() == 1,
            flip_probability,
        })?;
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_controlled_or_feedback(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
    feedback_basis: PauliBasis,
) -> Result<(), SamplingCompileError> {
    for target_group in instruction.target_groups() {
        match classify_controlled_pauli_target_pair(instruction.gate(), target_group) {
            ControlledPauliTargetPair::Quantum { .. } => {
                compile_unitary_tableau_group(instruction, operations, target_group)?;
            }
            ControlledPauliTargetPair::Classical { control, target } => {
                let basis = if instruction.gate().canonical_name() == "CZ" {
                    PauliBasis::Z
                } else {
                    feedback_basis
                };
                operations.push(SampleOperation::ClassicallyControlledPauli {
                    control: admit_classical_control(instruction, state, control)?,
                    qubit: qubit_id_index(target)?,
                    basis,
                })?;
            }
            ControlledPauliTargetPair::ClassicalNoop { first, second } => {
                register_noop_control(state, first)?;
                register_noop_control(state, second)?;
            }
            ControlledPauliTargetPair::Unsupported => {
                return Err(unsupported_sampler_instruction(instruction));
            }
        }
    }
    Ok(())
}

fn admit_classical_control(
    instruction: &CircuitInstruction,
    state: &mut CompileState,
    control: ClassicalControl,
) -> Result<ClassicalControl, SamplingCompileError> {
    match control {
        ClassicalControl::Record(offset) => {
            state.validate_record_offset(instruction, offset)?;
        }
        ClassicalControl::Sweep(sweep_id) => {
            state.add_sweep_bit(sweep_id)?;
        }
    }
    Ok(control)
}

fn register_noop_control(
    state: &mut CompileState,
    control: ClassicalControl,
) -> Result<(), SamplingCompileError> {
    if let ClassicalControl::Sweep(sweep_id) = control {
        state.add_sweep_bit(sweep_id)?;
    }
    Ok(())
}

fn compile_single_qubit_clifford(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
) -> Result<(), SamplingCompileError> {
    if instruction.gate().canonical_name() == "H" {
        for target in instruction.targets() {
            operations.push(SampleOperation::ApplyHadamard {
                qubit: qubit_index(instruction, target)?,
            })?;
        }
        return Ok(());
    }

    let clifford = stab_analysis::single_qubit_clifford_for_gate(instruction.gate())
        .map_err(|error| SamplingCompileError::invalid_circuit(error.to_string()))?;
    let transform = LocalTableauTransform::from_tableau(&clifford.tableau())?;
    for target in instruction.targets() {
        operations.push(SampleOperation::ApplyTableau {
            targets: vec![qubit_index(instruction, target)?],
            transform: transform.clone(),
        })?;
    }
    Ok(())
}

fn compile_unitary_tableau(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
) -> Result<(), SamplingCompileError> {
    for target_group in instruction.target_groups() {
        compile_unitary_tableau_group(instruction, operations, target_group)?;
    }
    Ok(())
}

fn compile_unitary_tableau_group(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    target_group: &[Target],
) -> Result<(), SamplingCompileError> {
    let targets = target_group
        .iter()
        .map(|target| qubit_index(instruction, target))
        .collect::<Result<Vec<_>, SamplingCompileError>>()?;
    if instruction.gate().canonical_name() == "CX"
        && let [control, target] = targets.as_slice()
    {
        operations.push(SampleOperation::ApplyControlledX {
            control: *control,
            target: *target,
        })?;
        return Ok(());
    }

    let tableau = stab_analysis::gate_tableau(instruction.gate())?;
    let transform = LocalTableauTransform::from_tableau(&tableau)?;
    if targets.len() != transform.target_count() {
        return Err(unsupported_sampler_instruction(instruction));
    }
    operations.push(SampleOperation::ApplyTableau { targets, transform })?;
    Ok(())
}

fn compile_single_qubit_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    probabilities: [f64; 3],
) -> Result<(), SamplingCompileError> {
    let total_probability = probabilities.iter().sum();
    for target in instruction.targets() {
        operations.push(SampleOperation::SingleQubitPauliChannel {
            qubit: qubit_index(instruction, target)?,
            probabilities,
            total_probability,
        })?;
    }
    Ok(())
}

fn compile_two_qubit_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    probabilities: [f64; 15],
) -> Result<(), SamplingCompileError> {
    let total_probability = probabilities.iter().sum();
    for target_pair in instruction.target_groups() {
        let [left, right] = target_pair else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::TwoQubitPauliChannel {
            left: qubit_index(instruction, left)?,
            right: qubit_index(instruction, right)?,
            probabilities,
            total_probability,
        })?;
    }
    Ok(())
}

fn compile_heralded_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    state: &mut CompileState,
    probabilities: [f64; 4],
) -> Result<(), SamplingCompileError> {
    for target in instruction.targets() {
        operations.push(SampleOperation::HeraldedPauliChannel {
            qubit: qubit_index(instruction, target)?,
            probabilities,
        })?;
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_correlated_error(
    instruction: &CircuitInstruction,
    operations: &mut SampleProgram,
    else_branch: bool,
) -> Result<(), SamplingCompileError> {
    let probability = single_probability_argument(instruction)?.get();
    let mut terms = Vec::new();
    for target in instruction.targets() {
        // Pinned Stim consults only the Pauli X/Z bits here, so combiner
        // targets and inversion bits are ignored decoration
        // (frame_simulator.inl:767-775).
        if target.is_combiner() {
            continue;
        }
        let Some(pauli) = target.pauli_type() else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        terms.push((qubit_index(instruction, target)?, pauli_basis(pauli)));
    }
    operations.push(SampleOperation::CorrelatedError {
        else_branch,
        probability,
        terms,
    })?;
    Ok(())
}

fn single_probability_argument(
    instruction: &CircuitInstruction,
) -> Result<Probability, SamplingCompileError> {
    let Some(probabilities) = instruction.probability_arguments()? else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    match probabilities.as_slice() {
        [probability] => Ok(*probability),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn measurement_flip_probability(
    instruction: &CircuitInstruction,
) -> Result<f64, SamplingCompileError> {
    match instruction.probability_argument()? {
        None => Ok(0.0),
        Some(probability) => Ok(probability.get()),
    }
}

fn zero_probability_noise(instruction: &CircuitInstruction) -> Result<bool, SamplingCompileError> {
    if !matches!(
        instruction.gate().category(),
        GateCategory::Noise | GateCategory::HeraldedNoise
    ) {
        return Ok(false);
    }
    let Some(probabilities) = instruction.probability_arguments()? else {
        return Ok(false);
    };
    Ok(probabilities
        .iter()
        .all(|probability| probability.get() == 0.0))
}

fn qubit_index(
    instruction: &CircuitInstruction,
    target: &Target,
) -> Result<usize, SamplingCompileError> {
    let Some(qubit) = target.qubit_id() else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    qubit_id_index(qubit)
}

fn qubit_id_index(qubit: QubitId) -> Result<usize, SamplingCompileError> {
    usize::try_from(qubit.get()).map_err(|_| {
        SamplingCompileError::invalid_circuit(format!(
            "qubit target {} cannot fit in this platform's usize",
            qubit.get()
        ))
    })
}

fn unsupported_sampler_instruction(instruction: &CircuitInstruction) -> SamplingCompileError {
    SamplingCompileError::invalid_circuit(format!(
        "sampling execution does not support the {} target shape",
        instruction.gate().canonical_name()
    ))
}

#[cfg(test)]
mod tests;
