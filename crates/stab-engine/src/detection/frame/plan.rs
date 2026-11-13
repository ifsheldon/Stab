use rand::Rng;
use stab_bits::{BIT_BLOCK_WORDS, BitBlock};
use stab_model::Circuit;

use super::program::{CircuitProgram, ProgramOptions};
use super::word::FrameWord;
use super::{BitPlaneDetectionFrame, FrameExecutionMode};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::detection::{ConversionPlan, DetectionConversionLimits};

pub(crate) const PAULI_FRAME_BATCH_SHOTS: usize = BIT_BLOCK_WORDS * u64::BITS as usize;

#[derive(Clone, Debug)]
pub(in crate::detection) struct DirectDetectorFramePlan {
    executable: CircuitProgram,
    conversion: ConversionPlan,
}

impl DirectDetectorFramePlan {
    pub(in crate::detection) fn compile(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<Self> {
        let conversion_admission =
            ConversionPlan::admission_from_circuit_with_limits(circuit, limits)?;
        let conversion_bytes = conversion_admission.compiled_storage_bytes()?;
        let execution_admission = CircuitProgram::admit(
            circuit,
            ProgramOptions::DETECTION,
            conversion_bytes,
            limits.max_compiled_bytes(),
        )?;
        admit_combined_compiled_storage(
            conversion_bytes,
            execution_admission.retained_bytes(),
            limits.max_compiled_bytes(),
        )?;
        let conversion =
            ConversionPlan::materialize_circuit_from_admission(circuit, conversion_admission)?;
        let executable = CircuitProgram::materialize(circuit, &execution_admission)?;
        let result = Self {
            executable,
            conversion,
        };
        admit_combined_compiled_storage(result.compiled_bytes()?, 0, limits.max_compiled_bytes())?;
        Ok(result)
    }

    pub(in crate::detection) fn measurement_count(&self) -> usize {
        self.conversion.measurement_count
    }

    pub(in crate::detection) fn qubit_count(&self) -> usize {
        self.executable.frame_qubit_count()
    }

    pub(in crate::detection) fn detector_count(&self) -> usize {
        self.conversion.detector_count
    }

    pub(in crate::detection) fn observable_count(&self) -> usize {
        self.conversion.observable_count
    }

    pub(in crate::detection) fn compiled_bytes(&self) -> DetectionResult<u64> {
        self.conversion
            .compiled_storage_bytes()?
            .checked_add(self.executable.retained_bytes())
            .ok_or_else(storage_overflow)
    }

    pub(in crate::detection) fn state(&self) -> DetectionResult<DetectorFrameState> {
        DetectorFrameState::new(
            self.qubit_count(),
            self.measurement_count(),
            self.detector_count(),
            self.observable_count(),
        )
    }

    pub(in crate::detection) fn state_storage_bytes(&self) -> u128 {
        DetectorFrameState::storage_bytes(
            self.qubit_count(),
            self.measurement_count(),
            self.detector_count(),
            self.observable_count(),
        )
    }

    pub(in crate::detection) fn sample_batch(
        &self,
        state: &mut DetectorFrameState,
        rng: &mut impl Rng,
        shot_count: usize,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_lanes: shot_count,
        };
        state.frame.reset(rng, mode);
        state.frame.execute_program(&self.executable, rng, mode)?;
        state.convert_batch(&self.conversion)?;
        Ok(())
    }

    pub(in crate::detection) fn output_planes<'a>(
        &self,
        state: &'a DetectorFrameState,
    ) -> (&'a [u64], &'a [u64]) {
        (&state.detector_planes, &state.observable_planes)
    }
}

/// Reusable bit-plane Pauli-frame storage for sampling one [`CircuitProgram`].
#[derive(Debug)]
pub(crate) struct PauliFrameSamplingState {
    frame: BitPlaneDetectionFrame<BitBlock>,
    measurement_count: usize,
}

impl PauliFrameSamplingState {
    pub(crate) fn try_new(program: &CircuitProgram) -> DetectionResult<Self> {
        let measurement_count = program.counts().measurements;
        Ok(Self {
            frame: BitPlaneDetectionFrame::try_reusable(
                program.frame_qubit_count(),
                measurement_count,
                0,
            )?,
            measurement_count,
        })
    }

    pub(crate) fn storage_bytes(program: &CircuitProgram) -> u128 {
        (program.frame_qubit_count() as u128)
            .saturating_mul(2)
            .saturating_add(program.counts().measurements as u128)
            .saturating_mul(size_of::<BitBlock>() as u128)
    }

    /// Samples one full batch of [`PAULI_FRAME_BATCH_SHOTS`] shots.
    pub(crate) fn sample_batch(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_lanes: PAULI_FRAME_BATCH_SHOTS,
        };
        self.frame.reset(rng, mode);
        self.frame.execute_program(program, rng, mode)?;
        if self.frame.measurements.len() != self.measurement_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "Pauli-frame sampler produced {} measurement planes but {} were compiled",
                self.frame.measurements.len(),
                self.measurement_count
            )));
        }
        Ok(())
    }

    pub(crate) const fn measurement_count(&self) -> usize {
        self.measurement_count
    }

    pub(crate) fn measurement_segment(
        &self,
        bit_index: usize,
        start: usize,
        count: usize,
    ) -> DetectionResult<u64> {
        self.frame
            .measurements
            .get(bit_index)
            .copied()
            .map(|plane| plane.extract_u64(start, count))
            .ok_or_else(|| {
                DetectionError::invalid_sampler_compilation(
                    "Pauli-frame measurement plane escaped its compiled width",
                )
            })
    }
}

#[derive(Clone, Debug)]
pub(in crate::detection) struct SweepCorrectionPlan {
    executable: CircuitProgram,
    measurement_count: usize,
    sweep_bit_count: usize,
    detector_count: usize,
    observable_count: usize,
}

impl SweepCorrectionPlan {
    /// Wraps a detection program whose counts match the conversion plan it corrects.
    pub(in crate::detection) fn new(
        executable: CircuitProgram,
        measurement_count: usize,
        sweep_bit_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> Self {
        Self {
            executable,
            measurement_count,
            sweep_bit_count,
            detector_count,
            observable_count,
        }
    }

    pub(in crate::detection) fn state(&self) -> DetectionResult<DetectorFrameState> {
        DetectorFrameState::new(
            self.executable.frame_qubit_count(),
            self.measurement_count,
            self.detector_count,
            self.observable_count,
        )
    }

    pub(in crate::detection) fn state_storage_bytes(&self) -> u128 {
        DetectorFrameState::storage_bytes(
            self.executable.frame_qubit_count(),
            self.measurement_count,
            self.detector_count,
            self.observable_count,
        )
    }

    pub(in crate::detection) fn correct_batch<'a>(
        &self,
        conversion: &ConversionPlan,
        sweep_planes: &[u64],
        shot_count: usize,
        state: &'a mut DetectorFrameState,
        rng: &mut impl Rng,
    ) -> DetectionResult<(&'a [u64], &'a [u64])> {
        if sweep_planes.len() != self.sweep_bit_count {
            return Err(DetectionError::invalid_result_format(format!(
                "sweep plane count {} does not match the compiled width {}",
                sweep_planes.len(),
                self.sweep_bit_count
            )));
        }
        if shot_count > u64::BITS as usize {
            return Err(DetectionError::invalid_result_format(format!(
                "sweep correction batches contain at most {} shots, got {shot_count}",
                u64::BITS
            )));
        }
        let mode = FrameExecutionMode::SweepCorrection {
            sweep_planes,
            active_lanes: shot_count,
        };
        state.frame.reset(rng, mode);
        state.frame.execute_program(&self.executable, rng, mode)?;
        if conversion.measurement_count != self.measurement_count
            || conversion.detector_count != self.detector_count
            || conversion.observable_count != self.observable_count
        {
            return Err(DetectionError::invalid_result_format(
                "sweep correction conversion dimensions disagree with its admitted plan",
            ));
        }
        state.convert_batch(conversion)?;
        Ok((&state.detector_planes, &state.observable_planes))
    }
}

#[derive(Debug)]
pub(in crate::detection) struct DetectorFrameState {
    pub(super) frame: BitPlaneDetectionFrame<u64>,
    detector_planes: Vec<u64>,
    observable_planes: Vec<u64>,
}

impl DetectorFrameState {
    fn storage_bytes(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> u128 {
        (qubit_count as u128)
            .saturating_mul(2)
            .saturating_add(measurement_count as u128)
            .saturating_add(detector_count as u128)
            .saturating_add((observable_count as u128).saturating_mul(2))
            .saturating_mul(size_of::<u64>() as u128)
    }

    fn new(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            frame: BitPlaneDetectionFrame::try_reusable(
                qubit_count,
                measurement_count,
                observable_count,
            )?,
            detector_planes: super::try_zero_words(
                detector_count,
                "detection frame detector planes",
            )?,
            observable_planes: super::try_zero_words(
                observable_count,
                "detection frame observable planes",
            )?,
        })
    }

    fn convert_batch(&mut self, plan: &ConversionPlan) -> DetectionResult<()> {
        if self.frame.measurements.len() != plan.measurement_count
            || self.frame.observables.len() != plan.observable_count
        {
            return Err(DetectionError::invalid_result_format(
                "detector frame dimensions disagree with its compiled conversion plan",
            ));
        }
        plan.convert_word_planes_into(
            &self.frame.measurements,
            &mut self.detector_planes,
            &mut self.observable_planes,
        )?;
        for (plane, frame_observable) in self
            .observable_planes
            .iter_mut()
            .zip(&self.frame.observables)
        {
            *plane ^= *frame_observable;
        }
        Ok(())
    }
}

pub(in crate::detection) fn admit_combined_compiled_storage(
    conversion_bytes: u64,
    execution_bytes: u64,
    limit_bytes: u64,
) -> DetectionResult<u64> {
    let combined = conversion_bytes
        .checked_add(execution_bytes)
        .ok_or_else(storage_overflow)?;
    if combined > limit_bytes {
        return Err(ResourceLimitError::detection_compiled_bytes(combined, limit_bytes).into());
    }
    Ok(combined)
}

fn storage_overflow() -> DetectionError {
    DetectionError::invalid_sampler_compilation(
        "direct detector-frame retained byte count overflowed",
    )
}

#[cfg(test)]
mod sparse_noise_tests;
