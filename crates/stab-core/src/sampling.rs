use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, GateCategory};

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledSampler {
    qubit_count: usize,
    operations: Vec<SampleOperation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SampleFormat {
    ZeroOne,
    B8,
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

    fn sample_shot<R>(&self, rng: &mut R) -> Vec<bool>
    where
        R: Rng,
    {
        let mut frame = DeterministicFrame::new(self.qubit_count);
        let mut measurements = Vec::new();
        execute_operations(&self.operations, &mut frame, &mut measurements, rng);
        measurements
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SampleOperation {
    Toggle {
        qubit: usize,
    },
    Reset {
        qubit: usize,
    },
    Measure {
        qubit: usize,
        inverted: bool,
        reset: bool,
    },
    Pad {
        value: bool,
    },
    ProbabilisticToggle {
        qubit: usize,
        probability: f64,
    },
    Repeat {
        count: u64,
        body: Vec<SampleOperation>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeterministicFrame {
    z_values: Vec<bool>,
}

impl DeterministicFrame {
    fn new(qubit_count: usize) -> Self {
        Self {
            z_values: vec![false; qubit_count],
        }
    }

    fn toggle(&mut self, qubit: usize) {
        if let Some(value) = self.z_values.get_mut(qubit) {
            *value = !*value;
        }
    }

    fn reset(&mut self, qubit: usize) {
        if let Some(value) = self.z_values.get_mut(qubit) {
            *value = false;
        }
    }

    fn measure(&self, qubit: usize, inverted: bool) -> bool {
        self.z_values.get(qubit).copied().unwrap_or(false) ^ inverted
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
        SampleFormat::Hits => append_hits_sample(sample, output),
        SampleFormat::Dets => append_dets_sample(sample, output),
    }
    if format != SampleFormat::B8 {
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
    match instruction.gate().canonical_name() {
        "TICK" | "QUBIT_COORDS" | "SHIFT_COORDS" | "DETECTOR" | "OBSERVABLE_INCLUDE" => Ok(()),
        "I" | "Z" => Ok(()),
        "X" | "Y" => {
            for target in instruction.targets() {
                operations.push(SampleOperation::Toggle {
                    qubit: qubit_index(instruction, target)?,
                });
            }
            Ok(())
        }
        "R" => {
            for target in instruction.targets() {
                operations.push(SampleOperation::Reset {
                    qubit: qubit_index(instruction, target)?,
                });
            }
            Ok(())
        }
        "M" | "MR" => compile_z_measurement(instruction, operations),
        "MPAD" => compile_measurement_pads(instruction, operations),
        "X_ERROR" | "Y_ERROR" => compile_single_qubit_probabilistic_toggle(
            instruction,
            operations,
            single_probability_argument(instruction)?.get(),
        ),
        "Z_ERROR" | "I_ERROR" => Ok(()),
        "DEPOLARIZE1" => compile_single_qubit_probabilistic_toggle(
            instruction,
            operations,
            single_probability_argument(instruction)?.get() * (2.0 / 3.0),
        ),
        "PAULI_CHANNEL_1" => {
            let Some(probabilities) = instruction.probability_arguments()? else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            let [x_probability, y_probability, _z_probability] = probabilities.as_slice() else {
                return Err(unsupported_sampler_instruction(instruction));
            };
            compile_single_qubit_probabilistic_toggle(
                instruction,
                operations,
                x_probability.get() + y_probability.get(),
            )
        }
        _ if zero_probability_noise(instruction)? => Ok(()),
        _ => Err(unsupported_sampler_instruction(instruction)),
    }
}

fn compile_z_measurement(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
) -> CircuitResult<()> {
    let flip = deterministic_measurement_flip(instruction)?;
    let reset = instruction.gate().canonical_name() == "MR";
    for target in instruction.targets() {
        operations.push(SampleOperation::Measure {
            qubit: qubit_index(instruction, target)?,
            inverted: target.is_inverted_result_target() ^ flip,
            reset,
        });
    }
    Ok(())
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

fn compile_single_qubit_probabilistic_toggle(
    instruction: &CircuitInstruction,
    operations: &mut Vec<SampleOperation>,
    probability: f64,
) -> CircuitResult<()> {
    for target in instruction.targets() {
        operations.push(SampleOperation::ProbabilisticToggle {
            qubit: qubit_index(instruction, target)?,
            probability,
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
    frame: &mut DeterministicFrame,
    measurements: &mut Vec<bool>,
    rng: &mut impl Rng,
) {
    for operation in operations {
        match operation {
            SampleOperation::Toggle { qubit } => frame.toggle(*qubit),
            SampleOperation::Reset { qubit } => frame.reset(*qubit),
            SampleOperation::Measure {
                qubit,
                inverted,
                reset,
            } => {
                measurements.push(frame.measure(*qubit, *inverted));
                if *reset {
                    frame.reset(*qubit);
                }
            }
            SampleOperation::Pad { value } => measurements.push(*value),
            SampleOperation::ProbabilisticToggle { qubit, probability } => {
                if rng.random_bool(*probability) {
                    frame.toggle(*qubit);
                }
            }
            SampleOperation::Repeat { count, body } => {
                for _ in 0..*count {
                    execute_operations(body, frame, measurements, rng);
                }
            }
        }
    }
}

fn unsupported_sampler_instruction(instruction: &CircuitInstruction) -> CircuitError {
    CircuitError::invalid_sampler_compilation(format!(
        "deterministic M8 sampler subset does not support {}",
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
    fn writes_stim_text_sample_formats() {
        let circuit = Circuit::from_stim_str("X 2 3 5\nM 0 1 2 3 4 5\n").expect("parse circuit");
        let sampler = CompiledSampler::compile(&circuit).expect("compile sampler");

        assert_eq!(sampler.sample_bytes(1, SampleFormat::ZeroOne), b"001101\n");
        assert_eq!(sampler.sample_bytes(1, SampleFormat::B8), &[0x2c]);
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
    fn rejects_hadamard_until_tableau_sampling_lands() {
        let circuit = Circuit::from_stim_str("H 0\nM 0\n").expect("parse circuit");
        assert_eq!(
            CompiledSampler::compile(&circuit),
            Err(CircuitError::invalid_sampler_compilation(
                "deterministic M8 sampler subset does not support H"
            ))
        );
    }
}
