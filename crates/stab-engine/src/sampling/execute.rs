//! Per-shot execution of the resolved circuit program on the generator-list stabilizer frame.
//!
//! Production sampling always runs on the bit-plane Pauli frame. This executor survives only as
//! the test suite's differential baseline: it walks the same program as every other executor
//! and consumes randomness per target in program order.

use rand::{Rng, RngExt as _};

use super::api::SamplingExecutionError;
use super::noise;
use super::reference_run::program_error;
use super::stabilizer_frame::{MeasurementRandomness, StabilizerFrame};
use crate::detection::frame::{
    CircuitProgram, ControlledPair, ProgramInstruction, ReferenceClifford,
};

/// Whether execution samples noise and random outcomes or reproduces the noiseless reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExecutionMode {
    Sample,
    /// Noiseless deterministic-false execution, the generator-list reference baseline of tests.
    #[cfg(test)]
    ReferenceSample,
}

impl ExecutionMode {
    fn measurement_randomness(self) -> MeasurementRandomness {
        match self {
            Self::Sample => MeasurementRandomness::Random,
            #[cfg(test)]
            Self::ReferenceSample => MeasurementRandomness::DeterministicFalse,
        }
    }

    fn includes_noise(self) -> bool {
        matches!(self, Self::Sample)
    }

    /// Samples a result flip. Stim's reference sample is strictly noiseless:
    /// `aliased_noiseless_circuit` drops result-flip probabilities before the reference run, so
    /// even a certain flip must not invert a reference bit.
    fn result_flip(self, probability: f64, rng: &mut impl Rng) -> bool {
        match self {
            Self::Sample => rng.random::<f64>() < probability,
            #[cfg(test)]
            Self::ReferenceSample => false,
        }
    }
}

pub(super) struct ExecutionBuffers<'a> {
    pub(super) frame: &'a mut StabilizerFrame,
    pub(super) record: &'a mut Vec<bool>,
    pub(super) output: &'a mut Vec<bool>,
    pub(super) correlated_error_occurred: &'a mut bool,
}

pub(super) fn execute_program(
    program: &CircuitProgram,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) -> Result<(), SamplingExecutionError> {
    let never = stab_model::Cancellation::never();
    let mut cursor = program.cursor(&never);
    while let Some(instruction) = cursor.next_instruction().map_err(program_error)? {
        execute_instruction(program, instruction, buffers, rng, mode, sweep_record)?;
    }
    Ok(())
}

fn apply_clifford(frame: &mut StabilizerFrame, clifford: &ReferenceClifford, targets: &[usize]) {
    match clifford {
        ReferenceClifford::Hadamard => {
            for &qubit in targets {
                frame.apply_hadamard(qubit);
            }
        }
        ReferenceClifford::ControlledX => {
            for pair in targets.chunks(2) {
                if let &[control, target] = pair {
                    frame.apply_controlled_x(control, target);
                }
            }
        }
        ReferenceClifford::Tableau { transform, .. } => {
            for group in targets.chunks(transform.target_count().max(1)) {
                frame.apply_tableau(group, transform);
            }
        }
    }
}

fn push_result(buffers: &mut ExecutionBuffers<'_>, result: bool) {
    buffers.record.push(result);
    buffers.output.push(result);
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive dispatch keeps every instruction's per-target stream order visible"
)]
fn execute_instruction(
    program: &CircuitProgram,
    instruction: &ProgramInstruction,
    buffers: &mut ExecutionBuffers<'_>,
    rng: &mut impl Rng,
    mode: ExecutionMode,
    sweep_record: &[bool],
) -> Result<(), SamplingExecutionError> {
    let randomness = mode.measurement_randomness();
    match instruction {
        ProgramInstruction::Clifford {
            reference, targets, ..
        } => apply_clifford(
            buffers.frame,
            program
                .reference_clifford(*reference)
                .map_err(program_error)?,
            targets,
        ),
        ProgramInstruction::Reset { basis, targets } => {
            for &qubit in targets {
                buffers.frame.reset(qubit, *basis, rng, randomness);
            }
        }
        ProgramInstruction::Measure {
            basis,
            reset,
            flip_probability,
            targets,
            inverted,
        } => {
            for (index, &qubit) in targets.iter().enumerate() {
                let flip = mode.result_flip(*flip_probability, rng);
                let inverted = inverted.get(index).copied().unwrap_or(false);
                let result = buffers
                    .frame
                    .measure(qubit, *basis, inverted ^ flip, rng, randomness);
                push_result(buffers, result);
                if *reset {
                    buffers.frame.reset(qubit, *basis, rng, randomness);
                }
            }
        }
        ProgramInstruction::MeasurePairs {
            basis,
            flip_probability,
            targets,
            inverted,
        } => {
            for (index, pair) in targets.chunks(2).enumerate() {
                let &[left, right] = pair else {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "pair measurement targets are not paired".to_owned(),
                    });
                };
                let flip = mode.result_flip(*flip_probability, rng);
                let inverted = inverted.get(2 * index).copied().unwrap_or(false)
                    ^ inverted.get(2 * index + 1).copied().unwrap_or(false);
                let result = buffers.frame.measure_pauli_product(
                    &[(left, *basis), (right, *basis)],
                    inverted ^ flip,
                    rng,
                    randomness,
                );
                push_result(buffers, result);
            }
        }
        ProgramInstruction::PauliProducts(measurement) => {
            for product in measurement.products() {
                let (terms, inverted) = product.map_err(program_error)?;
                let flip = mode.result_flip(measurement.flip_probability(), rng);
                let result =
                    buffers
                        .frame
                        .measure_pauli_product(terms, inverted ^ flip, rng, randomness);
                push_result(buffers, result);
            }
        }
        ProgramInstruction::MeasurePads {
            flip_probability,
            values,
        } => {
            for &value in values {
                let result = value ^ mode.result_flip(*flip_probability, rng);
                push_result(buffers, result);
            }
        }
        ProgramInstruction::SinglePauliNoise {
            probabilities,
            targets,
        } => {
            if mode.includes_noise() {
                let total = probabilities.iter().sum();
                for &qubit in targets {
                    noise::apply_single_qubit_pauli_channel(
                        buffers.frame,
                        qubit,
                        probabilities,
                        total,
                        rng,
                    );
                }
            }
        }
        ProgramInstruction::TwoQubitPauliNoise {
            probabilities,
            targets,
        } => {
            if mode.includes_noise() {
                let total = probabilities.iter().sum();
                for pair in targets.chunks(2) {
                    if let &[left, right] = pair {
                        noise::apply_two_qubit_pauli_channel(
                            buffers.frame,
                            left,
                            right,
                            probabilities,
                            total,
                            rng,
                        );
                    }
                }
            }
        }
        ProgramInstruction::CorrelatedError {
            else_branch,
            probability,
            terms,
        } => {
            if mode.includes_noise() {
                noise::apply_correlated_error(
                    buffers.frame,
                    terms,
                    *probability,
                    *else_branch,
                    buffers.correlated_error_occurred,
                    rng,
                );
            } else if !else_branch {
                *buffers.correlated_error_occurred = false;
            }
        }
        ProgramInstruction::HeraldedErase {
            probability,
            targets,
        } => {
            let quarter = probability / 4.0;
            for &qubit in targets {
                let herald = mode.includes_noise()
                    && noise::apply_heralded_pauli_channel(
                        buffers.frame,
                        qubit,
                        &[quarter; 4],
                        rng,
                    );
                push_result(buffers, herald);
            }
        }
        ProgramInstruction::HeraldedPauliChannel {
            probabilities,
            targets,
        } => {
            for &qubit in targets {
                let herald = mode.includes_noise()
                    && noise::apply_heralded_pauli_channel(
                        buffers.frame,
                        qubit,
                        probabilities,
                        rng,
                    );
                push_result(buffers, herald);
            }
        }
        ProgramInstruction::ControlledPauli {
            basis,
            reference,
            pairs,
            ..
        } => {
            let clifford = program
                .reference_clifford(*reference)
                .map_err(program_error)?;
            for pair in pairs {
                match *pair {
                    ControlledPair::Quantum { control, target } => {
                        apply_clifford(buffers.frame, clifford, &[control, target]);
                    }
                    ControlledPair::Record { offset, qubit } => {
                        if record_lookback(buffers.record, offset) {
                            buffers.frame.apply_pauli(qubit, *basis);
                        }
                    }
                    ControlledPair::Sweep { id, qubit } => {
                        let active = usize::try_from(id)
                            .ok()
                            .and_then(|index| sweep_record.get(index))
                            .copied()
                            .unwrap_or(false);
                        if active {
                            buffers.frame.apply_pauli(qubit, *basis);
                        }
                    }
                }
            }
        }
        ProgramInstruction::Observable { .. } => {}
    }
    Ok(())
}

/// The record bit `offset` steps back from the end of `record`, or false before its start.
fn record_lookback(record: &[bool], offset: stab_model::MeasureRecordOffset) -> bool {
    let index = i64::try_from(record.len())
        .ok()
        .and_then(|len| len.checked_add(i64::from(offset.get())))
        .and_then(|index| usize::try_from(index).ok());
    index
        .and_then(|index| record.get(index))
        .copied()
        .unwrap_or(false)
}
