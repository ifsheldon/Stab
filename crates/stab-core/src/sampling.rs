use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng as _};

use self::execute::{ExecutionBuffers, count_determined_operations, execute_operations};
use self::operation::SampleOperation;
use self::stabilizer_frame::{LocalTableauTransform, MeasurementRandomness, StabilizerFrame};
use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, GateCategory,
    MeasureRecordOffset, Pauli, PauliBasis, SampleFormat, SingleQubitClifford,
    result_formats::{MeasureRecordWriter, write_ptb64_records_checked},
};

mod direct_z_measurement;
mod execute;
mod measurement_flip;
mod noise;
mod operation;
pub(crate) mod pauli_product;
mod small_frame;
mod stabilizer_frame;
mod stream;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSampler {
    qubit_count: usize,
    measurement_count: usize,
    sweep_bit_count: usize,
    operations: Vec<SampleOperation>,
}

impl CompiledSampler {
    pub fn compile(circuit: &Circuit) -> CircuitResult<Self> {
        let mut operations = Vec::new();
        let counts = compile_circuit(circuit, &mut operations, SweepCompilation::Reject)?;
        Ok(Self {
            qubit_count: circuit.count_qubits(),
            measurement_count: counts.measurements,
            sweep_bit_count: counts.sweep_bits,
            operations,
        })
    }

    pub(crate) fn compile_allowing_sweep(circuit: &Circuit) -> CircuitResult<Self> {
        let mut operations = Vec::new();
        let counts = compile_circuit(circuit, &mut operations, SweepCompilation::Allow)?;
        Ok(Self {
            qubit_count: circuit.count_qubits(),
            measurement_count: counts.measurements,
            sweep_bit_count: counts.sweep_bits,
            operations,
        })
    }

    pub fn sample_zero_one(&self, shots: usize) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed(shots, None)
    }

    pub fn sample_zero_one_with_seed(&self, shots: usize, seed: Option<u64>) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed_and_reference_mode(shots, seed, false)
    }

    pub fn sample_zero_one_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<Vec<bool>> {
        let mut samples = Vec::with_capacity(shots);
        let result = self.for_each_sample_with_seed_and_reference_mode(
            shots,
            seed,
            skip_reference_sample,
            |sample| {
                samples.push(sample.to_vec());
                Ok::<(), std::convert::Infallible>(())
            },
        );
        match result {
            Ok(()) => {}
            Err(error) => match error {},
        }
        samples
    }

    pub fn sample_zero_one_bytes(&self, shots: usize) -> Vec<u8> {
        self.sample_bytes(shots, SampleFormat::ZeroOne)
    }

    pub fn sample_bytes(&self, shots: usize, format: SampleFormat) -> Vec<u8> {
        self.sample_bytes_with_seed(shots, format, None)
    }

    pub fn sample_bytes_with_seed(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
    ) -> Vec<u8> {
        self.sample_bytes_with_seed_and_reference_mode(shots, format, seed, false)
    }

    pub fn sample_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        format: SampleFormat,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> Vec<u8> {
        let mut rng = sampler_rng(seed);
        if !skip_reference_sample
            && format == SampleFormat::ZeroOne
            && let Some(bytes) = direct_z_measurement::sample_zero_one_bytes(
                &self.operations,
                self.measurement_count,
                shots,
                &mut rng,
            )
        {
            return bytes;
        }
        let reference_sample = skip_reference_sample.then(|| self.reference_sample());
        if let Some(bytes) = small_frame::sample_bytes(
            self.qubit_count,
            self.measurement_count,
            &self.operations,
            shots,
            format,
            reference_sample.as_deref(),
            &mut rng,
        ) {
            return bytes;
        }
        let mut writer = MeasureRecordWriter::with_capacity(
            format,
            estimated_sample_bytes_capacity(format, shots, self.measurement_count),
        );
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::with_capacity(self.measurement_count);
        let mut output = Vec::with_capacity(self.measurement_count);
        for _ in 0..shots {
            self.sample_shot_with_reference_into(
                &mut rng,
                reference_sample.as_deref(),
                &mut frame,
                &mut record,
                &mut output,
            );
            writer.write_bits(&output);
            writer.write_end();
        }
        writer.into_bytes()
    }

    pub fn sample_ptb64_bytes_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<Vec<u8>> {
        self.sample_ptb64_bytes_with_seed_and_reference_mode(shots, seed, false)
    }

    pub fn sample_ptb64_bytes_with_seed_and_reference_mode(
        &self,
        shots: usize,
        seed: Option<u64>,
        skip_reference_sample: bool,
    ) -> CircuitResult<Vec<u8>> {
        if !shots.is_multiple_of(64) {
            return Err(CircuitError::invalid_sampler_compilation(
                "shots must be a multiple of 64 to use ptb64 format",
            ));
        }
        let mut rng = sampler_rng(seed);
        let reference_sample = skip_reference_sample.then(|| self.reference_sample());
        let samples = (0..shots)
            .map(|_| self.sample_shot_with_reference(&mut rng, reference_sample.as_deref()))
            .collect::<Vec<_>>();
        write_ptb64_records_checked(&samples)
    }

    pub fn count_determined_measurements(&self, unknown_input: bool) -> u64 {
        let mut rng = SmallRng::seed_from_u64(0);
        let mut frame = if unknown_input {
            StabilizerFrame::new_unknown(self.qubit_count)
        } else {
            StabilizerFrame::new(self.qubit_count)
        };
        let mut record = Vec::new();
        count_determined_operations(&self.operations, &mut frame, &mut record, &mut rng)
    }

    fn sample_shot_with_reference<R>(&self, rng: &mut R, reference: Option<&[bool]>) -> Vec<bool>
    where
        R: Rng,
    {
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::with_capacity(self.measurement_count);
        let mut output = Vec::with_capacity(self.measurement_count);
        self.sample_shot_with_reference_into(rng, reference, &mut frame, &mut record, &mut output);
        output
    }

    fn sample_shot_with_reference_into<R>(
        &self,
        rng: &mut R,
        reference: Option<&[bool]>,
        frame: &mut StabilizerFrame,
        record: &mut Vec<bool>,
        output: &mut Vec<bool>,
    ) where
        R: Rng,
    {
        self.sample_shot_in_mode_into(rng, ExecutionMode::Sample, &[], frame, record, output);
        if let Some(reference) = reference {
            for (bit, reference_bit) in output.iter_mut().zip(reference) {
                *bit ^= *reference_bit;
            }
        }
    }

    pub fn reference_sample(&self) -> Vec<bool> {
        let mut rng = SmallRng::seed_from_u64(0);
        self.sample_shot_in_mode(&mut rng, ExecutionMode::ReferenceSample, &[])
    }

    pub(crate) fn sweep_bit_count(&self) -> usize {
        self.sweep_bit_count
    }

    pub(crate) fn reference_sample_with_sweep_into(
        &self,
        sweep_record: &[bool],
        output: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        if sweep_record.len() != self.sweep_bit_count {
            return Err(CircuitError::invalid_result_format(format!(
                "sweep record expected {} bits, got {}",
                self.sweep_bit_count,
                sweep_record.len()
            )));
        }
        let mut rng = SmallRng::seed_from_u64(0);
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::with_capacity(self.measurement_count);
        self.sample_shot_in_mode_into(
            &mut rng,
            ExecutionMode::ReferenceSample,
            sweep_record,
            &mut frame,
            &mut record,
            output,
        );
        Ok(())
    }

    fn sample_shot_in_mode<R>(
        &self,
        rng: &mut R,
        mode: ExecutionMode,
        sweep_record: &[bool],
    ) -> Vec<bool>
    where
        R: Rng,
    {
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::with_capacity(self.measurement_count);
        let mut output = Vec::with_capacity(self.measurement_count);
        self.sample_shot_in_mode_into(
            rng,
            mode,
            sweep_record,
            &mut frame,
            &mut record,
            &mut output,
        );
        output
    }

    fn sample_shot_in_mode_into<R>(
        &self,
        rng: &mut R,
        mode: ExecutionMode,
        sweep_record: &[bool],
        frame: &mut StabilizerFrame,
        record: &mut Vec<bool>,
        output: &mut Vec<bool>,
    ) where
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
        execute_operations(&self.operations, &mut buffers, rng, mode, sweep_record);
    }
}

pub fn count_determined_measurements(circuit: &Circuit, unknown_input: bool) -> CircuitResult<u64> {
    Ok(CompiledSampler::compile_allowing_sweep(circuit)?
        .count_determined_measurements(unknown_input))
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

fn estimated_sample_bytes_capacity(
    format: SampleFormat,
    shots: usize,
    bits_per_shot: usize,
) -> usize {
    let bytes_per_shot = match format {
        SampleFormat::ZeroOne => bits_per_shot.checked_add(1),
        SampleFormat::B8 => Some(bits_per_shot.div_ceil(8)),
        SampleFormat::R8 | SampleFormat::Hits | SampleFormat::Dets => None,
    };
    bytes_per_shot
        .and_then(|bytes_per_shot| shots.checked_mul(bytes_per_shot))
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompileState {
    measurement_count: u64,
    sweep_bit_count: u64,
    sweep_compilation: SweepCompilation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SweepCompilation {
    Reject,
    Allow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompiledCounts {
    measurements: usize,
    sweep_bits: usize,
}

impl CompileState {
    fn new(sweep_compilation: SweepCompilation) -> Self {
        Self {
            measurement_count: 0,
            sweep_bit_count: 0,
            sweep_compilation,
        }
    }

    fn add_measurements(&mut self, count: usize) -> CircuitResult<()> {
        let count = u64::try_from(count).map_err(|_| {
            CircuitError::invalid_sampler_compilation(
                "measurement record count cannot fit in u64 during sampler compilation",
            )
        })?;
        self.measurement_count = self.measurement_count.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "measurement record count overflows during sampler compilation",
            )
        })?;
        Ok(())
    }

    fn add_repeated_measurements(&mut self, per_body: u64, repeat_count: u64) -> CircuitResult<()> {
        let total = per_body.checked_mul(repeat_count).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "repeated measurement record count overflows during sampler compilation",
            )
        })?;
        self.measurement_count = self.measurement_count.checked_add(total).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "measurement record count overflows during sampler compilation",
            )
        })?;
        Ok(())
    }

    fn validate_record_offset(
        self,
        instruction: &CircuitInstruction,
        offset: MeasureRecordOffset,
    ) -> CircuitResult<()> {
        let required = u64::from(offset.get().unsigned_abs());
        if required <= self.measurement_count {
            return Ok(());
        }
        Err(CircuitError::invalid_sampler_compilation(format!(
            "measurement record target rec[{}] is not available while compiling {} feedback",
            offset.get(),
            instruction.gate().canonical_name()
        )))
    }

    fn add_sweep_bit(&mut self, sweep_id: u32) -> CircuitResult<usize> {
        let sweep_id = u64::from(sweep_id);
        self.sweep_bit_count =
            self.sweep_bit_count
                .max(sweep_id.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation("sweep bit count overflowed")
                })?);
        usize::try_from(sweep_id).map_err(|_| {
            CircuitError::invalid_sampler_compilation(format!(
                "sweep bit id {sweep_id} cannot fit in this platform's usize"
            ))
        })
    }
}

fn compile_circuit(
    circuit: &Circuit,
    operations: &mut Vec<SampleOperation>,
    sweep_compilation: SweepCompilation,
) -> CircuitResult<CompiledCounts> {
    let mut state = CompileState::new(sweep_compilation);
    compile_circuit_with_state(circuit, operations, &mut state)?;
    elide_leading_z_resets(operations);
    let measurements = usize::try_from(state.measurement_count).map_err(|_| {
        CircuitError::invalid_sampler_compilation(
            "measurement record count cannot fit in usize during sampler compilation",
        )
    })?;
    let sweep_bits = usize::try_from(state.sweep_bit_count).map_err(|_| {
        CircuitError::invalid_sampler_compilation(
            "sweep bit count cannot fit in usize during sampler compilation",
        )
    })?;
    Ok(CompiledCounts {
        measurements,
        sweep_bits,
    })
}

fn compile_circuit_with_state(
    circuit: &Circuit,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                compile_instruction(instruction, operations, state)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = Vec::new();
                let before_body = state.measurement_count;
                let mut body_state = *state;
                compile_circuit_with_state(repeat.body(), &mut body, &mut body_state)?;
                let body_measurements = body_state.measurement_count - before_body;
                state.add_repeated_measurements(body_measurements, repeat.repeat_count().get())?;
                state.sweep_bit_count = state.sweep_bit_count.max(body_state.sweep_bit_count);
                operations.push(SampleOperation::Repeat {
                    count: repeat.repeat_count().get(),
                    body,
                });
            }
        }
    }
    Ok(())
}

fn elide_leading_z_resets(operations: &mut Vec<SampleOperation>) {
    let leading_z_resets = operations
        .iter()
        .take_while(|operation| {
            matches!(
                operation,
                SampleOperation::Reset {
                    basis: PauliBasis::Z,
                    ..
                }
            )
        })
        .count();
    if leading_z_resets > 0 {
        operations.drain(..leading_z_resets);
    }
}

fn compile_instruction(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
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
        "CX" => compile_controlled_or_feedback(instruction, operations, state, PauliBasis::X),
        "CY" => compile_controlled_or_feedback(instruction, operations, state, PauliBasis::Y),
        "CZ" => compile_controlled_or_feedback(instruction, operations, state, PauliBasis::Z),
        _ if SingleQubitClifford::from_gate(gate).is_ok() => {
            compile_single_qubit_clifford(instruction, operations)
        }
        _ if crate::circuit_tableau::gate_tableau(gate.canonical_name()).is_ok() => {
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
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
    let decomposed =
        crate::circuit_simplify::decomposed_single_instruction(instruction).map_err(|error| {
            CircuitError::invalid_sampler_compilation(format!(
                "{} cannot be executed via decomposition: {error}",
                instruction.gate().canonical_name()
            ))
        })?;
    compile_circuit_with_state(&decomposed, operations, state)
}

fn compile_reset(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
) -> CircuitResult<()> {
    let basis = measurement_basis(instruction)?;
    for target in instruction.targets() {
        operations.push(SampleOperation::Reset {
            qubit: qubit_index(instruction, target)?,
            basis,
        });
    }
    Ok(())
}

fn compile_measurement(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
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
        });
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_pair_measurement(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
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
        });
    }
    state.add_measurements(groups.len())?;
    Ok(())
}

fn compile_pauli_product_measurement(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
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
        });
    }
    state.add_measurements(groups.len())?;
    Ok(())
}

fn measurement_basis(instruction: &CircuitInstruction) -> CircuitResult<PauliBasis> {
    match instruction.gate().canonical_name() {
        "MX" | "MRX" | "RX" => Ok(PauliBasis::X),
        "MY" | "MRY" | "RY" => Ok(PauliBasis::Y),
        "M" | "MR" | "R" => Ok(PauliBasis::Z),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn pair_measurement_basis(instruction: &CircuitInstruction) -> CircuitResult<PauliBasis> {
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
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
) -> CircuitResult<()> {
    let flip_probability = measurement_flip_probability(instruction)?;
    for target in instruction.targets() {
        let Some(qubit) = target.qubit_id() else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::Pad {
            value: qubit.get() == 1,
            flip_probability,
        });
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_controlled_or_feedback(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
    feedback_basis: PauliBasis,
) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        if target_group
            .iter()
            .any(|target| target.is_sweep_bit_target())
        {
            compile_sweep_pauli_group(
                instruction,
                operations,
                state,
                feedback_basis,
                target_group,
            )?;
        } else if target_group
            .first()
            .and_then(|target| target.measurement_record_offset())
            .is_some()
        {
            compile_feedback_pauli_group(
                instruction,
                operations,
                state,
                feedback_basis,
                target_group,
            )?;
        } else {
            compile_unitary_tableau_group(instruction, operations, target_group)?;
        }
    }
    Ok(())
}

fn compile_sweep_pauli_group(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
    basis: PauliBasis,
    target_group: &[crate::Target],
) -> CircuitResult<()> {
    if state.sweep_compilation == SweepCompilation::Reject {
        return Err(unsupported_sampler_instruction(instruction));
    }
    let [first, second] = target_group else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    let first_sweep = first
        .sweep_bit_id()
        .map(|sweep_id| state.add_sweep_bit(sweep_id))
        .transpose()?;
    let second_sweep = second
        .sweep_bit_id()
        .map(|sweep_id| state.add_sweep_bit(sweep_id))
        .transpose()?;

    match (
        instruction.gate().canonical_name(),
        first_sweep,
        second_sweep,
    ) {
        ("CX" | "CY", Some(sweep_id), None) if second.qubit_id().is_some() => {
            operations.push(SampleOperation::SweepPauli {
                sweep_id,
                qubit: qubit_index(instruction, second)?,
                basis,
            });
            Ok(())
        }
        ("CX" | "CY", None, Some(sweep_id)) if first.qubit_id().is_some() => {
            operations.push(SampleOperation::SweepPauli {
                sweep_id,
                qubit: qubit_index(instruction, first)?,
                basis,
            });
            Ok(())
        }
        ("CZ", Some(sweep_id), None) if second.qubit_id().is_some() => {
            operations.push(SampleOperation::SweepPauli {
                sweep_id,
                qubit: qubit_index(instruction, second)?,
                basis: PauliBasis::Z,
            });
            Ok(())
        }
        ("CZ", None, Some(sweep_id)) if first.qubit_id().is_some() => {
            operations.push(SampleOperation::SweepPauli {
                sweep_id,
                qubit: qubit_index(instruction, first)?,
                basis: PauliBasis::Z,
            });
            Ok(())
        }
        (_, Some(_), Some(_)) | (_, Some(_), None) | (_, None, Some(_)) => {
            Err(unsupported_sampler_instruction(instruction))
        }
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn compile_feedback_pauli_group(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
    basis: PauliBasis,
    target_group: &[crate::Target],
) -> CircuitResult<()> {
    let [record, target] = target_group else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    let Some(offset) = record.measurement_record_offset() else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    state.validate_record_offset(instruction, offset)?;
    operations.push(SampleOperation::FeedbackPauli {
        offset,
        qubit: qubit_index(instruction, target)?,
        basis,
    });
    Ok(())
}

fn compile_single_qubit_clifford(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
) -> CircuitResult<()> {
    if instruction.gate().canonical_name() == "H" {
        for target in instruction.targets() {
            operations.push(SampleOperation::ApplyHadamard {
                qubit: qubit_index(instruction, target)?,
            });
        }
        return Ok(());
    }

    let clifford = SingleQubitClifford::from_gate(instruction.gate())
        .map_err(|error| CircuitError::invalid_sampler_compilation(error.to_string()))?;
    let transform = LocalTableauTransform::from_tableau(&clifford.tableau())?;
    for target in instruction.targets() {
        operations.push(SampleOperation::ApplyTableau {
            targets: vec![qubit_index(instruction, target)?],
            transform: transform.clone(),
        });
    }
    Ok(())
}

fn compile_unitary_tableau(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        compile_unitary_tableau_group(instruction, operations, target_group)?;
    }
    Ok(())
}

fn compile_unitary_tableau_group(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    target_group: &[crate::Target],
) -> CircuitResult<()> {
    let targets = target_group
        .iter()
        .map(|target| qubit_index(instruction, target))
        .collect::<CircuitResult<Vec<_>>>()?;
    if instruction.gate().canonical_name() == "CX"
        && let [control, target] = targets.as_slice()
    {
        operations.push(SampleOperation::ApplyControlledX {
            control: *control,
            target: *target,
        });
        return Ok(());
    }

    let tableau = crate::circuit_tableau::gate_tableau(instruction.gate().canonical_name())?;
    let transform = LocalTableauTransform::from_tableau(&tableau)?;
    if targets.len() != transform.target_count() {
        return Err(unsupported_sampler_instruction(instruction));
    }
    operations.push(SampleOperation::ApplyTableau { targets, transform });
    Ok(())
}

fn compile_single_qubit_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    probabilities: [f64; 3],
) -> CircuitResult<()> {
    let total_probability = probabilities.iter().sum();
    for target in instruction.targets() {
        operations.push(SampleOperation::SingleQubitPauliChannel {
            qubit: qubit_index(instruction, target)?,
            probabilities,
            total_probability,
        });
    }
    Ok(())
}

fn compile_two_qubit_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    probabilities: [f64; 15],
) -> CircuitResult<()> {
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
        });
    }
    Ok(())
}

fn compile_heralded_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &mut CompileState,
    probabilities: [f64; 4],
) -> CircuitResult<()> {
    for target in instruction.targets() {
        operations.push(SampleOperation::HeraldedPauliChannel {
            qubit: qubit_index(instruction, target)?,
            probabilities,
        });
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_correlated_error(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    else_branch: bool,
) -> CircuitResult<()> {
    let probability = single_probability_argument(instruction)?.get();
    let mut terms = Vec::new();
    for target in instruction.targets() {
        if target.is_inverted_result_target() || target.is_combiner() {
            return Err(unsupported_sampler_instruction(instruction));
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
    });
    Ok(())
}

fn single_probability_argument(
    instruction: &CircuitInstruction,
) -> CircuitResult<crate::Probability> {
    let Some(probabilities) = instruction.probability_arguments()? else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    match probabilities.as_slice() {
        [probability] => Ok(*probability),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn measurement_flip_probability(instruction: &CircuitInstruction) -> CircuitResult<f64> {
    match instruction.probability_argument()? {
        None => Ok(0.0),
        Some(probability) => Ok(probability.get()),
    }
}

fn zero_probability_noise(instruction: &CircuitInstruction) -> CircuitResult<bool> {
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

fn qubit_index(instruction: &CircuitInstruction, target: &crate::Target) -> CircuitResult<usize> {
    let Some(qubit) = target.qubit_id() else {
        return Err(unsupported_sampler_instruction(instruction));
    };
    usize::try_from(qubit.get()).map_err(|_| {
        CircuitError::invalid_sampler_compilation(format!(
            "qubit target {} cannot fit in this platform's usize",
            qubit.get()
        ))
    })
}

fn unsupported_sampler_instruction(instruction: &CircuitInstruction) -> CircuitError {
    CircuitError::invalid_sampler_compilation(format!(
        "M8 sampler subset does not support {}",
        instruction.gate().canonical_name()
    ))
}

#[cfg(test)]
mod tests;
