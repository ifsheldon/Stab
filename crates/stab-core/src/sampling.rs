use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, GateCategory,
    PauliBasis, PauliSign, PauliString, SingleQubitClifford,
};

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
        let mut frame = LocalFrame::new(self.qubit_count);
        let mut measurements = Vec::new();
        execute_operations(&self.operations, &mut frame, &mut measurements, rng);
        measurements
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SampleOperation {
    ApplyClifford {
        qubit: usize,
        transform: LocalCliffordTransform,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignedBasis {
    negative: bool,
    basis: PauliBasis,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalCliffordTransform {
    x: SignedBasis,
    y: SignedBasis,
    z: SignedBasis,
}

impl LocalCliffordTransform {
    fn from_clifford(clifford: SingleQubitClifford) -> CircuitResult<Self> {
        let tableau = clifford.tableau();
        Ok(Self {
            x: transform_signed_basis(&tableau, PauliBasis::X)?,
            y: transform_signed_basis(&tableau, PauliBasis::Y)?,
            z: transform_signed_basis(&tableau, PauliBasis::Z)?,
        })
    }

    fn output_for(self, basis: PauliBasis) -> SignedBasis {
        match basis {
            PauliBasis::X => self.x,
            PauliBasis::Y => self.y,
            PauliBasis::Z => self.z,
            PauliBasis::I => SignedBasis {
                negative: false,
                basis: PauliBasis::I,
            },
        }
    }
}

fn transform_signed_basis(
    tableau: &crate::Tableau,
    basis: PauliBasis,
) -> CircuitResult<SignedBasis> {
    let input = PauliString::from_bases(PauliSign::Plus, [basis]);
    let output = tableau
        .apply(&input)
        .map_err(|error| CircuitError::invalid_sampler_compilation(error.to_string()))?;
    let Some(output_basis) = output.get(0) else {
        return Err(CircuitError::invalid_sampler_compilation(
            "single-qubit Clifford output is missing its Pauli basis",
        ));
    };
    Ok(SignedBasis {
        negative: output.sign().is_negative(),
        basis: output_basis,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalQubitState {
    negative: bool,
    basis: PauliBasis,
}

impl LocalQubitState {
    fn plus_z() -> Self {
        Self {
            negative: false,
            basis: PauliBasis::Z,
        }
    }

    fn apply_clifford(&mut self, transform: LocalCliffordTransform) {
        let output = transform.output_for(self.basis);
        self.negative ^= output.negative;
        self.basis = output.basis;
    }

    fn apply_pauli(&mut self, pauli: PauliBasis) {
        if pauli != self.basis && self.basis != PauliBasis::I {
            self.negative = !self.negative;
        }
    }

    fn reset(&mut self, basis: PauliBasis) {
        self.basis = basis;
        self.negative = false;
    }

    fn measure(&mut self, basis: PauliBasis, inverted: bool, rng: &mut impl Rng) -> bool {
        let raw_result = match self.basis {
            state_basis if state_basis == basis => self.negative,
            PauliBasis::I | PauliBasis::X | PauliBasis::Y | PauliBasis::Z => {
                let sampled = rng.random_bool(0.5);
                self.basis = basis;
                self.negative = sampled;
                sampled
            }
        };
        raw_result ^ inverted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalFrame {
    states: Vec<LocalQubitState>,
}

impl LocalFrame {
    fn new(qubit_count: usize) -> Self {
        Self {
            states: vec![LocalQubitState::plus_z(); qubit_count],
        }
    }

    fn apply_clifford(&mut self, qubit: usize, transform: LocalCliffordTransform) {
        if let Some(state) = self.states.get_mut(qubit) {
            state.apply_clifford(transform);
        }
    }

    fn apply_pauli(&mut self, qubit: usize, basis: PauliBasis) {
        if let Some(state) = self.states.get_mut(qubit) {
            state.apply_pauli(basis);
        }
    }

    fn reset(&mut self, qubit: usize, basis: PauliBasis) {
        if let Some(state) = self.states.get_mut(qubit) {
            state.reset(basis);
        }
    }

    fn measure(
        &mut self,
        qubit: usize,
        basis: PauliBasis,
        inverted: bool,
        rng: &mut impl Rng,
    ) -> bool {
        self.states
            .get_mut(qubit)
            .map(|state| state.measure(basis, inverted, rng))
            .unwrap_or(inverted)
    }
}

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

fn compile_circuit(circuit: &Circuit, operations: &mut Vec<SampleOperation>) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                compile_instruction(instruction, operations)?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = Vec::new();
                compile_circuit(repeat.body(), &mut body)?;
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
) -> CircuitResult<()> {
    let gate = instruction.gate();
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => Ok(()),
        "R" | "RX" | "RY" => compile_reset(instruction, operations),
        "M" | "MX" | "MY" | "MR" | "MRX" | "MRY" => compile_measurement(instruction, operations),
        "MPAD" => compile_measurement_pads(instruction, operations),
        _ if SingleQubitClifford::from_gate(gate).is_ok() => {
            compile_single_qubit_clifford(instruction, operations)
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

fn compile_measurement_pads(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
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
    Ok(())
}

fn compile_single_qubit_clifford(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
) -> CircuitResult<()> {
    let clifford = SingleQubitClifford::from_gate(instruction.gate())
        .map_err(|error| CircuitError::invalid_sampler_compilation(error.to_string()))?;
    let transform = LocalCliffordTransform::from_clifford(clifford)?;
    for target in instruction.targets() {
        operations.push(SampleOperation::ApplyClifford {
            qubit: qubit_index(instruction, target)?,
            transform,
        });
    }
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
    frame: &mut LocalFrame,
    measurements: &mut Vec<bool>,
    rng: &mut impl Rng,
) {
    for operation in operations {
        match operation {
            SampleOperation::ApplyClifford { qubit, transform } => {
                frame.apply_clifford(*qubit, *transform);
            }
            SampleOperation::Reset { qubit, basis } => frame.reset(*qubit, *basis),
            SampleOperation::Measure {
                qubit,
                basis,
                inverted,
                reset,
            } => {
                measurements.push(frame.measure(*qubit, *basis, *inverted, rng));
                if *reset {
                    frame.reset(*qubit, *basis);
                }
            }
            SampleOperation::Pad { value } => measurements.push(*value),
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
            SampleOperation::Repeat { count, body } => {
                for _ in 0..*count {
                    execute_operations(body, frame, measurements, rng);
                }
            }
        }
    }
}

fn apply_single_qubit_pauli_channel(
    frame: &mut LocalFrame,
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
    frame: &mut LocalFrame,
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
        "local M8 sampler subset does not support {}",
        instruction.gate().canonical_name()
    ))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "sampling unit tests use direct fixture parsing assertions for compact diagnostics"
)]
mod tests {
    use super::*;

    fn samples(input: &str, shots: usize) -> Vec<Vec<bool>> {
        let circuit = Circuit::from_stim_str(input).expect("parse circuit");
        CompiledSampler::compile(&circuit)
            .expect("compile sampler")
            .sample_zero_one(shots)
    }

    #[test]
    fn samples_m8_basic_measurements_as_zeroes() {
        assert_eq!(
            samples(
                include_str!("../../../oracle/fixtures/inputs/sample_basic.stim"),
                2
            ),
            vec![vec![false, false], vec![false, false]]
        );
    }

    #[test]
    fn samples_x_and_inverted_measurements_like_command_sample() {
        assert_eq!(samples("X 0\nM 0\n", 1), vec![vec![true]]);
        assert_eq!(samples("M !0\n", 1), vec![vec![true]]);
    }

    #[test]
    fn samples_reset_and_measure_reset_deterministically() {
        assert_eq!(samples("X 0\nR 0\nM 0\n", 1), vec![vec![false]]);
        assert_eq!(samples("X 0\nMR 0\nMR 0\n", 1), vec![vec![true, false]]);
    }

    #[test]
    fn samples_repeat_blocks_without_flattening_during_compilation() {
        assert_eq!(
            samples("REPEAT 2 {\n    X 0\n    M 0\n}\n", 1),
            vec![vec![true, false]]
        );
    }

    #[test]
    fn samples_single_qubit_clifford_measurements() {
        assert_eq!(samples("H 0\nS 0\nS 0\nH 0\nM 0\n", 3), vec![vec![true]; 3]);

        let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
        let first = sampler.sample_zero_one_with_seed(1000, Some(5));
        let second = sampler.sample_zero_one_with_seed(1000, Some(5));
        assert_eq!(first, second);

        let hits = first.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (400..=600).contains(&hits),
            "expected roughly 500 H-basis measurement hits, got {hits}"
        );
    }

    #[test]
    fn samples_x_and_y_basis_measurements_deterministically() {
        assert_eq!(samples("H 0\nMX 0\n", 1), vec![vec![false]]);
        assert_eq!(samples("X 0\nH 0\nMX 0\n", 1), vec![vec![true]]);
        assert_eq!(samples("H 0\nS 0\nMY 0\n", 1), vec![vec![false]]);
        assert_eq!(samples("H 0\nZ 0\nS 0\nMY 0\n", 1), vec![vec![true]]);
    }

    #[test]
    fn random_basis_measurement_collapses_to_the_measured_basis() {
        let circuit = Circuit::from_stim_str("MX 0\nMX 0\nMY 1\nMY 1\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
            assert_eq!(shot.first(), shot.get(1));
            assert_eq!(shot.get(2), shot.get(3));
        }
    }

    #[test]
    fn reset_and_measure_reset_use_their_measurement_basis() {
        assert_eq!(
            samples("RX 0\nMX 0\nRY 1\nMY 1\n", 1),
            vec![vec![false, false]]
        );

        let circuit = Circuit::from_stim_str("MRX 0\nMX 0\nMRY 1\nMY 1\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");
        for shot in sampler.sample_zero_one_with_seed(100, Some(5)) {
            assert_eq!(
                shot.get(1),
                Some(&false),
                "MRX should reset to +X after reporting"
            );
            assert_eq!(
                shot.get(3),
                Some(&false),
                "MRY should reset to +Y after reporting"
            );
        }
    }

    #[test]
    fn z_error_flips_x_basis_measurements_after_hadamards() {
        let circuit =
            Circuit::from_stim_str("H 0\nZ_ERROR(0.25) 0\nH 0\nM 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
        let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (175..=325).contains(&hits),
            "expected roughly 250 Z-error X-basis hits, got {hits}"
        );
    }

    #[test]
    fn writes_stim_text_sample_formats() {
        let circuit = Circuit::from_stim_str("X 2 3 5\nM 0 1 2 3 4 5\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(sampler.sample_bytes(1, SampleFormat::ZeroOne), b"001101\n");
        assert_eq!(sampler.sample_bytes(1, SampleFormat::B8), &[0x2c]);
        assert_eq!(
            sampler.sample_bytes(1, SampleFormat::R8),
            &[0x02, 0x00, 0x01, 0x00]
        );
        assert_eq!(sampler.sample_bytes(1, SampleFormat::Hits), b"2,3,5\n");
        assert_eq!(
            sampler.sample_bytes(1, SampleFormat::Dets),
            b"shot M2 M3 M5\n"
        );
        assert_eq!(
            sampler.sample_bytes(2, SampleFormat::Hits),
            b"2,3,5\n2,3,5\n"
        );
    }

    #[test]
    fn writes_r8_samples_with_long_false_runs() {
        let circuit =
            Circuit::from_stim_str("X 1\nM 0 0 0 0 0 0 0 0 0 1\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(sampler.sample_bytes(1, SampleFormat::R8), &[0x09, 0x00]);

        let long_zero_circuit =
            Circuit::from_stim_str(&format!("MPAD {}\n", "0 ".repeat(260))).expect("parse circuit");
        let long_zero_sampler =
            CompiledSampler::compile(&long_zero_circuit).expect("compile sampler");
        assert_eq!(
            long_zero_sampler.sample_bytes(1, SampleFormat::R8),
            &[0xff, 0x05]
        );
    }

    #[test]
    fn writes_ptb64_samples_in_measurement_major_shot_groups() {
        let circuit = Circuit::from_stim_str("X 1\nM 0 1\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(
            sampler
                .sample_ptb64_bytes_with_seed(64, Some(5))
                .expect("sample ptb64"),
            [
                0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff
            ]
        );
    }

    #[test]
    fn rejects_ptb64_shot_counts_that_are_not_multiple_of_64() {
        let circuit = Circuit::from_stim_str("M 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(
            sampler.sample_ptb64_bytes_with_seed(63, Some(5)),
            Err(CircuitError::invalid_sampler_compilation(
                "shots must be a multiple of 64 to use ptb64 format"
            ))
        );
    }

    #[test]
    fn writes_b8_samples_with_per_shot_padding() {
        let circuit =
            Circuit::from_stim_str("X 0 8\nM 0 1 2 3 4 5 6 7 8\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(
            sampler.sample_bytes(2, SampleFormat::B8),
            &[0x01, 0x01, 0x01, 0x01]
        );
    }

    #[test]
    fn seeded_x_error_sampling_is_reproducible_and_statistical() {
        let circuit = Circuit::from_stim_str("X_ERROR(0.25) 0\nM 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        let first = sampler.sample_zero_one_with_seed(1000, Some(5));
        let second = sampler.sample_zero_one_with_seed(1000, Some(5));
        assert_eq!(first, second);

        let hits = first.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (175..=325).contains(&hits),
            "expected roughly 250 noisy hits, got {hits}"
        );
    }

    #[test]
    fn z_and_identity_errors_do_not_flip_z_basis_measurements() {
        assert_eq!(
            samples("Z_ERROR(0.9) 0\nI_ERROR(0.8) 0\nM 0\n", 20),
            vec![vec![false]; 20]
        );
    }

    #[test]
    fn depolarize1_flips_z_basis_measurements_with_x_or_y_probability() {
        let circuit = Circuit::from_stim_str("DEPOLARIZE1(0.3) 0\nM 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
        let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (125..=275).contains(&hits),
            "expected roughly 200 depolarize1 Z-basis hits, got {hits}"
        );
    }

    #[test]
    fn pauli_channel1_flips_z_basis_measurements_for_x_or_y_cases() {
        let circuit = Circuit::from_stim_str("PAULI_CHANNEL_1(0.1, 0.2, 0.3) 0\nM 0\n")
            .expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
        let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (215..=385).contains(&hits),
            "expected roughly 300 pauli-channel1 Z-basis hits, got {hits}"
        );
    }

    #[test]
    fn depolarize2_flips_z_basis_measurements_for_two_qubit_x_or_y_cases() {
        let circuit = Circuit::from_stim_str("DEPOLARIZE2(0.3) 0 1\nM 0\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        let samples = sampler.sample_zero_one_with_seed(1000, Some(5));
        let hits = samples.iter().filter(|shot| shot == &&vec![true]).count();
        assert!(
            (95..=225).contains(&hits),
            "expected roughly 160 depolarize2 Z-basis hits, got {hits}"
        );
    }

    #[test]
    fn pauli_channel2_uses_stim_probability_order_for_z_basis_toggles() {
        let circuit = Circuit::from_stim_str(
            "PAULI_CHANNEL_2(0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0) 0 1\nM 0 1\n",
        )
        .expect("parse circuit");

        assert_eq!(
            CompiledSampler::compile(&circuit)
                .expect("compile sampler")
                .sample_zero_one_with_seed(5, Some(5)),
            vec![vec![true, false]; 5]
        );
    }

    #[test]
    fn rejects_entangling_gates_until_tableau_sampling_lands() {
        let circuit = Circuit::from_stim_str("CX 0 1\nM 0 1\n").expect("parse circuit");
        assert_eq!(
            CompiledSampler::compile(&circuit),
            Err(CircuitError::invalid_sampler_compilation(
                "local M8 sampler subset does not support CX"
            ))
        );
    }
}
