use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use self::stabilizer_frame::{LocalTableauTransform, StabilizerFrame};
use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, GateCategory,
    MeasureRecordOffset, Pauli, PauliBasis, SingleQubitClifford,
};

mod stabilizer_frame;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSampler {
    qubit_count: usize,
    operations: Vec<SampleOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleFormat {
    ZeroOne,
    B8,
    R8,
    Hits,
    Dets,
}

impl CompiledSampler {
    pub fn compile(circuit: &Circuit) -> CircuitResult<Self> {
        let mut operations = Vec::new();
        compile_circuit(circuit, &mut operations)?;
        Ok(Self {
            qubit_count: circuit.count_qubits(),
            operations,
        })
    }

    pub fn sample_zero_one(&self, shots: usize) -> Vec<Vec<bool>> {
        self.sample_zero_one_with_seed(shots, None)
    }

    pub fn sample_zero_one_with_seed(&self, shots: usize, seed: Option<u64>) -> Vec<Vec<bool>> {
        let mut rng = sampler_rng(seed);
        (0..shots).map(|_| self.sample_shot(&mut rng)).collect()
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
        let mut rng = sampler_rng(seed);
        let mut output = Vec::new();
        for _ in 0..shots {
            append_sample(&self.sample_shot(&mut rng), format, &mut output);
        }
        output
    }

    pub fn sample_ptb64_bytes_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<Vec<u8>> {
        if !shots.is_multiple_of(64) {
            return Err(CircuitError::invalid_sampler_compilation(
                "shots must be a multiple of 64 to use ptb64 format",
            ));
        }
        let mut rng = sampler_rng(seed);
        let samples = (0..shots)
            .map(|_| self.sample_shot(&mut rng))
            .collect::<Vec<_>>();
        Ok(ptb64_samples(&samples))
    }

    fn sample_shot<R>(&self, rng: &mut R) -> Vec<bool>
    where
        R: Rng,
    {
        let mut frame = StabilizerFrame::new(self.qubit_count);
        let mut record = Vec::new();
        let mut output = Vec::new();
        execute_operations(&self.operations, &mut frame, &mut record, &mut output, rng);
        output
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SampleOperation {
    ApplyTableau {
        targets: Vec<usize>,
        transform: LocalTableauTransform,
    },
    Reset {
        qubit: usize,
        basis: PauliBasis,
    },
    Measure {
        qubit: usize,
        basis: PauliBasis,
        inverted: bool,
        reset: bool,
    },
    MeasureProduct {
        terms: Vec<(usize, PauliBasis)>,
        inverted: bool,
    },
    Pad {
        value: bool,
    },
    SingleQubitPauliChannel {
        qubit: usize,
        probabilities: [f64; 3],
    },
    TwoQubitPauliChannel {
        left: usize,
        right: usize,
        probabilities: [f64; 15],
    },
    HeraldedPauliChannel {
        qubit: usize,
        probabilities: [f64; 4],
    },
    FeedbackPauli {
        offset: MeasureRecordOffset,
        qubit: usize,
        basis: PauliBasis,
    },
    Repeat {
        count: u64,
        body: Vec<SampleOperation>,
    },
}

const SINGLE_QUBIT_PAULI_CHANNEL_BASES: [PauliBasis; 3] =
    [PauliBasis::X, PauliBasis::Y, PauliBasis::Z];

const TWO_QUBIT_PAULI_CHANNEL_BASES: [(Option<PauliBasis>, Option<PauliBasis>); 15] = [
    (None, Some(PauliBasis::X)),
    (None, Some(PauliBasis::Y)),
    (None, Some(PauliBasis::Z)),
    (Some(PauliBasis::X), None),
    (Some(PauliBasis::X), Some(PauliBasis::X)),
    (Some(PauliBasis::X), Some(PauliBasis::Y)),
    (Some(PauliBasis::X), Some(PauliBasis::Z)),
    (Some(PauliBasis::Y), None),
    (Some(PauliBasis::Y), Some(PauliBasis::X)),
    (Some(PauliBasis::Y), Some(PauliBasis::Y)),
    (Some(PauliBasis::Y), Some(PauliBasis::Z)),
    (Some(PauliBasis::Z), None),
    (Some(PauliBasis::Z), Some(PauliBasis::X)),
    (Some(PauliBasis::Z), Some(PauliBasis::Y)),
    (Some(PauliBasis::Z), Some(PauliBasis::Z)),
];

fn sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}

fn append_sample(sample: &[bool], format: SampleFormat, output: &mut Vec<u8>) {
    match format {
        SampleFormat::ZeroOne => {
            for bit in sample {
                output.push(if *bit { b'1' } else { b'0' });
            }
        }
        SampleFormat::B8 => append_b8_sample(sample, output),
        SampleFormat::R8 => append_r8_sample(sample, output),
        SampleFormat::Hits => append_hits_sample(sample, output),
        SampleFormat::Dets => append_dets_sample(sample, output),
    }
    if !matches!(format, SampleFormat::B8 | SampleFormat::R8) {
        output.push(b'\n');
    }
}

fn append_b8_sample(sample: &[bool], output: &mut Vec<u8>) {
    for byte_bits in sample.chunks(8) {
        let mut byte = 0u8;
        for (bit_index, bit) in byte_bits.iter().enumerate() {
            if *bit {
                byte |= 1u8 << bit_index;
            }
        }
        output.push(byte);
    }
}

fn ptb64_samples(samples: &[Vec<bool>]) -> Vec<u8> {
    let mut output = Vec::new();
    for shot_group in samples.chunks_exact(64) {
        let bits_per_shot = shot_group.first().map_or(0, Vec::len);
        for measurement_index in 0..bits_per_shot {
            let mut word = 0u64;
            for (shot_index, shot) in shot_group.iter().enumerate() {
                if shot.get(measurement_index).copied().unwrap_or(false) {
                    word |= 1u64 << shot_index;
                }
            }
            output.extend_from_slice(&word.to_le_bytes());
        }
    }
    output
}

fn append_r8_sample(sample: &[bool], output: &mut Vec<u8>) {
    let mut false_run = 0u8;
    for bit in sample.iter().copied().chain(std::iter::once(true)) {
        if bit {
            if false_run == u8::MAX {
                output.push(u8::MAX);
                false_run = 0;
            }
            output.push(false_run);
            false_run = 0;
        } else {
            if false_run == u8::MAX {
                output.push(u8::MAX);
                false_run = 0;
            }
            false_run = false_run.saturating_add(1);
        }
    }
}

fn append_hits_sample(sample: &[bool], output: &mut Vec<u8>) {
    let mut first = true;
    for (index, bit) in sample.iter().enumerate() {
        if !bit {
            continue;
        }
        if !first {
            output.push(b',');
        }
        first = false;
        output.extend_from_slice(index.to_string().as_bytes());
    }
}

fn append_dets_sample(sample: &[bool], output: &mut Vec<u8>) {
    output.extend_from_slice(b"shot");
    for (index, bit) in sample.iter().enumerate() {
        if *bit {
            output.extend_from_slice(b" M");
            output.extend_from_slice(index.to_string().as_bytes());
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CompileState {
    measurement_count: u64,
}

impl CompileState {
    fn new() -> Self {
        Self {
            measurement_count: 0,
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
}

fn compile_circuit(circuit: &Circuit, operations: &mut Vec<SampleOperation>) -> CircuitResult<()> {
    let mut state = CompileState::new();
    compile_circuit_with_state(circuit, operations, &mut state)
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
                operations.push(SampleOperation::Repeat {
                    count: repeat.repeat_count().get(),
                    body,
                });
            }
        }
    }
    Ok(())
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
    let flip = deterministic_measurement_flip(instruction)?;
    let reset = matches!(instruction.gate().canonical_name(), "MR" | "MRX" | "MRY");
    for target in instruction.targets() {
        operations.push(SampleOperation::Measure {
            qubit: qubit_index(instruction, target)?,
            basis,
            inverted: target.is_inverted_result_target() ^ flip,
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
    let flip = deterministic_measurement_flip(instruction)?;
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
            inverted: left.is_inverted_result_target() ^ right.is_inverted_result_target() ^ flip,
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
    let flip = deterministic_measurement_flip(instruction)?;
    let groups = instruction.target_groups();
    for target_group in &groups {
        let mut terms = Vec::new();
        let mut inverted = flip;
        for target in *target_group {
            if target.is_combiner() {
                continue;
            }
            let Some(pauli) = target.pauli_type() else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            terms.push((qubit_index(instruction, target)?, pauli_basis(pauli)));
            inverted ^= target.is_inverted_result_target();
        }
        operations.push(SampleOperation::MeasureProduct { terms, inverted });
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
    let flip = deterministic_measurement_flip(instruction)?;
    for target in instruction.targets() {
        let Some(qubit) = target.qubit_id() else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::Pad {
            value: (qubit.get() == 1) ^ flip,
        });
    }
    state.add_measurements(instruction.targets().len())?;
    Ok(())
}

fn compile_controlled_or_feedback(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &CompileState,
    feedback_basis: PauliBasis,
) -> CircuitResult<()> {
    for target_group in instruction.target_groups() {
        if target_group
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

fn compile_feedback_pauli_group(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    state: &CompileState,
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
    let tableau = crate::circuit_tableau::gate_tableau(instruction.gate().canonical_name())?;
    let transform = LocalTableauTransform::from_tableau(&tableau)?;
    let targets = target_group
        .iter()
        .map(|target| qubit_index(instruction, target))
        .collect::<CircuitResult<Vec<_>>>()?;
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
    for target in instruction.targets() {
        operations.push(SampleOperation::SingleQubitPauliChannel {
            qubit: qubit_index(instruction, target)?,
            probabilities,
        });
    }
    Ok(())
}

fn compile_two_qubit_pauli_channel(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    probabilities: [f64; 15],
) -> CircuitResult<()> {
    for target_pair in instruction.target_groups() {
        let [left, right] = target_pair else {
            return Err(unsupported_sampler_instruction(instruction));
        };
        operations.push(SampleOperation::TwoQubitPauliChannel {
            left: qubit_index(instruction, left)?,
            right: qubit_index(instruction, right)?,
            probabilities,
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

fn deterministic_measurement_flip(instruction: &CircuitInstruction) -> CircuitResult<bool> {
    match instruction.probability_argument()? {
        None => Ok(false),
        Some(probability) if probability.get() == 0.0 => Ok(false),
        Some(probability) if probability.get() == 1.0 => Ok(true),
        Some(_) => Err(unsupported_sampler_instruction(instruction)),
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

fn execute_operations(
    operations: &[SampleOperation],
    frame: &mut StabilizerFrame,
    record: &mut Vec<bool>,
    output: &mut Vec<bool>,
    rng: &mut impl Rng,
) {
    for operation in operations {
        match operation {
            SampleOperation::ApplyTableau { targets, transform } => {
                frame.apply_tableau(targets, transform);
            }
            SampleOperation::Reset { qubit, basis } => frame.reset(*qubit, *basis, rng),
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                reset,
            } => {
                let result = frame.measure(*qubit, *basis, *inverted, rng);
                record.push(result);
                output.push(result);
                if *reset {
                    frame.reset(*qubit, *basis, rng);
                }
            }
            SampleOperation::MeasureProduct { terms, inverted } => {
                let result = frame.measure_pauli_product(terms, *inverted, rng);
                record.push(result);
                output.push(result);
            }
            SampleOperation::Pad { value } => {
                record.push(*value);
                output.push(*value);
            }
            SampleOperation::SingleQubitPauliChannel {
                qubit,
                probabilities,
            } => {
                apply_single_qubit_pauli_channel(frame, *qubit, probabilities, rng);
            }
            SampleOperation::TwoQubitPauliChannel {
                left,
                right,
                probabilities,
            } => {
                apply_two_qubit_pauli_channel(frame, *left, *right, probabilities, rng);
            }
            SampleOperation::HeraldedPauliChannel {
                qubit,
                probabilities,
            } => {
                apply_heralded_pauli_channel(frame, *qubit, probabilities, record, rng);
            }
            SampleOperation::FeedbackPauli {
                offset,
                qubit,
                basis,
            } => {
                if measurement_record_bit(record, *offset) {
                    frame.apply_pauli(*qubit, *basis);
                }
            }
            SampleOperation::Repeat { count, body } => {
                for _ in 0..*count {
                    execute_operations(body, frame, record, output, rng);
                }
            }
        }
    }
}

fn measurement_record_bit(measurements: &[bool], offset: MeasureRecordOffset) -> bool {
    let Ok(len) = i64::try_from(measurements.len()) else {
        return false;
    };
    let index = len + i64::from(offset.get());
    let Ok(index) = usize::try_from(index) else {
        return false;
    };
    measurements.get(index).copied().unwrap_or(false)
}

fn apply_heralded_pauli_channel(
    frame: &mut StabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 4],
    measurements: &mut Vec<bool>,
    rng: &mut impl Rng,
) {
    let [i_probability, x_probability, y_probability, z_probability] = *probabilities;
    let mut sampled_probability = rng.random::<f64>();
    if sampled_probability < i_probability {
        measurements.push(true);
        return;
    }
    sampled_probability -= i_probability;
    for (basis, probability) in [
        (PauliBasis::X, x_probability),
        (PauliBasis::Y, y_probability),
        (PauliBasis::Z, z_probability),
    ] {
        if sampled_probability < probability {
            measurements.push(true);
            frame.apply_pauli(qubit, basis);
            return;
        }
        sampled_probability -= probability;
    }
    measurements.push(false);
}

fn apply_single_qubit_pauli_channel(
    frame: &mut StabilizerFrame,
    qubit: usize,
    probabilities: &[f64; 3],
    rng: &mut impl Rng,
) {
    let mut sampled_probability = rng.random::<f64>();
    for (basis, probability) in SINGLE_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            frame.apply_pauli(qubit, basis);
            return;
        }
        sampled_probability -= probability;
    }
}

fn apply_two_qubit_pauli_channel(
    frame: &mut StabilizerFrame,
    left: usize,
    right: usize,
    probabilities: &[f64; 15],
    rng: &mut impl Rng,
) {
    let mut sampled_probability = rng.random::<f64>();
    for ((left_basis, right_basis), probability) in TWO_QUBIT_PAULI_CHANNEL_BASES
        .into_iter()
        .zip(probabilities.iter().copied())
    {
        if sampled_probability < probability {
            if let Some(basis) = left_basis {
                frame.apply_pauli(left, basis);
            }
            if let Some(basis) = right_basis {
                frame.apply_pauli(right, basis);
            }
            return;
        }
        sampled_probability -= probability;
    }
}

fn unsupported_sampler_instruction(instruction: &CircuitInstruction) -> CircuitError {
    CircuitError::invalid_sampler_compilation(format!(
        "M8 sampler subset does not support {}",
        instruction.gate().canonical_name()
    ))
}

#[cfg(test)]
mod tests;
