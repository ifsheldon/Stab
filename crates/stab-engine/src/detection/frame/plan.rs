use rand::Rng;
use stab_model::Circuit;

use super::program::{FrameProgram, FrameProgramAdmission};
use super::{BitPlaneDetectionFrame, FrameExecutionMode, batch_active_mask};
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::detection::{ConversionPlan, DetectionConversionLimits};

#[derive(Clone, Debug)]
pub(in crate::detection) struct DirectDetectorFramePlan {
    executable: FrameProgram,
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
        let execution_admission =
            FrameProgram::admit(circuit, conversion_bytes, limits.max_compiled_bytes(), true)?;
        admit_combined_compiled_storage(
            conversion_bytes,
            execution_admission.retained_bytes(),
            limits.max_compiled_bytes(),
        )?;
        let conversion =
            ConversionPlan::materialize_circuit_from_admission(circuit, conversion_admission)?;
        let executable = FrameProgram::materialize(circuit, execution_admission, true)?;
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
        self.executable.qubit_count()
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

    pub(in crate::detection) fn sample_batch(
        &self,
        state: &mut DetectorFrameState,
        rng: &mut impl Rng,
        shot_count: usize,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_mask: batch_active_mask(shot_count),
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

#[derive(Clone, Debug)]
pub(crate) struct PauliFrameSamplingPlan {
    executable: FrameProgram,
    measurement_count: usize,
}

impl PauliFrameSamplingPlan {
    pub(crate) fn try_compile(
        circuit: &Circuit,
        measurement_count: usize,
        max_compiled_bytes: u64,
    ) -> DetectionResult<Self> {
        let admission = FrameProgram::admit(circuit, 0, max_compiled_bytes, false)?;
        let executable = FrameProgram::materialize(circuit, admission, false)?;
        Ok(Self {
            executable,
            measurement_count,
        })
    }

    pub(crate) fn state(&self) -> DetectionResult<PauliFrameSamplingState> {
        Ok(PauliFrameSamplingState {
            frame: BitPlaneDetectionFrame::try_reusable(
                self.executable.qubit_count(),
                self.measurement_count,
                0,
            )?,
        })
    }

    pub(crate) fn sample_batch(
        &self,
        state: &mut PauliFrameSamplingState,
        rng: &mut impl Rng,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_mask: u64::MAX,
        };
        state.frame.reset(rng, mode);
        state.frame.execute_program(&self.executable, rng, mode)?;
        if state.frame.measurements.len() != self.measurement_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "Pauli-frame sampler produced {} measurement planes but {} were compiled",
                state.frame.measurements.len(),
                self.measurement_count
            )));
        }
        Ok(())
    }

    pub(crate) fn measurement_planes<'a>(&self, state: &'a PauliFrameSamplingState) -> &'a [u64] {
        &state.frame.measurements
    }

    pub(crate) fn state_storage_bytes(&self) -> u128 {
        (self.executable.qubit_count() as u128)
            .saturating_mul(2)
            .saturating_add(self.measurement_count as u128)
            .saturating_mul(size_of::<u64>() as u128)
    }
}

#[derive(Debug)]
pub(crate) struct PauliFrameSamplingState {
    frame: BitPlaneDetectionFrame,
}

#[derive(Clone, Debug)]
pub(in crate::detection) struct SweepCorrectionPlan {
    executable: FrameProgram,
    measurement_count: usize,
    sweep_bit_count: usize,
    detector_count: usize,
    observable_count: usize,
}

impl SweepCorrectionPlan {
    pub(in crate::detection) fn admit(
        circuit: &Circuit,
        retained_base_bytes: u64,
        max_combined_bytes: u64,
    ) -> DetectionResult<FrameProgramAdmission> {
        FrameProgram::admit(circuit, retained_base_bytes, max_combined_bytes, true)
    }

    pub(in crate::detection) fn materialize(
        circuit: &Circuit,
        admission: FrameProgramAdmission,
        measurement_count: usize,
        sweep_bit_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            executable: FrameProgram::materialize(circuit, admission, true)?,
            measurement_count,
            sweep_bit_count,
            detector_count,
            observable_count,
        })
    }

    pub(in crate::detection) fn state(&self) -> DetectionResult<DetectorFrameState> {
        DetectorFrameState::new(
            self.executable.qubit_count(),
            self.measurement_count,
            self.detector_count,
            self.observable_count,
        )
    }

    pub(in crate::detection) fn state_storage_bytes(&self) -> u128 {
        (self.executable.qubit_count() as u128)
            .saturating_mul(2)
            .saturating_add(self.measurement_count as u128)
            .saturating_add(self.detector_count as u128)
            .saturating_add((self.observable_count as u128).saturating_mul(2))
            .saturating_mul(size_of::<u64>() as u128)
    }

    pub(in crate::detection) const fn retained_bytes(&self) -> u64 {
        self.executable.retained_bytes()
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
            active_mask: batch_active_mask(shot_count),
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
    pub(super) frame: BitPlaneDetectionFrame,
    detector_planes: Vec<u64>,
    observable_planes: Vec<u64>,
}

impl DetectorFrameState {
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
