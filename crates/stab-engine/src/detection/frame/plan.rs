use rand::Rng;
use stab_model::Circuit;

use super::program::{FrameProgram, FrameProgramAdmission};
use super::{FrameExecutionMode, ScalarDetectionFrame};
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
            FrameProgram::admit(circuit, conversion_bytes, limits.max_compiled_bytes())?;
        admit_combined_compiled_storage(
            conversion_bytes,
            execution_admission.retained_bytes(),
            limits.max_compiled_bytes(),
        )?;
        let conversion =
            ConversionPlan::materialize_circuit_from_admission(circuit, conversion_admission)?;
        let executable = FrameProgram::materialize(circuit, execution_admission)?;
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

    pub(in crate::detection) fn sample<'a>(
        &self,
        state: &'a mut DetectorFrameState,
        rng: &mut impl Rng,
    ) -> DetectionResult<(&'a [bool], &'a [bool])> {
        state.frame.reset(rng, FrameExecutionMode::Sample);
        state
            .frame
            .execute_program(&self.executable, rng, FrameExecutionMode::Sample)?;
        state.convert(&self.conversion)?;
        Ok((&state.record.detectors, &state.record.observables))
    }
}

#[derive(Clone, Debug)]
pub(in crate::detection) struct SweepCorrectionPlan {
    executable: FrameProgram,
    measurement_count: usize,
    detector_count: usize,
    observable_count: usize,
}

impl SweepCorrectionPlan {
    pub(in crate::detection) fn admit(
        circuit: &Circuit,
        retained_base_bytes: u64,
        max_combined_bytes: u64,
    ) -> DetectionResult<FrameProgramAdmission> {
        FrameProgram::admit(circuit, retained_base_bytes, max_combined_bytes)
    }

    pub(in crate::detection) fn materialize(
        circuit: &Circuit,
        admission: FrameProgramAdmission,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            executable: FrameProgram::materialize(circuit, admission)?,
            measurement_count,
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
            .saturating_add((self.measurement_count as u128).saturating_mul(2))
            .saturating_add(self.detector_count as u128)
            .saturating_add((self.observable_count as u128).saturating_mul(2))
    }

    pub(in crate::detection) const fn retained_bytes(&self) -> u64 {
        self.executable.retained_bytes()
    }

    pub(in crate::detection) fn correct<'a>(
        &self,
        conversion: &ConversionPlan,
        sweep_record: &'a [bool],
        state: &'a mut DetectorFrameState,
        rng: &mut impl Rng,
    ) -> DetectionResult<(&'a [bool], &'a [bool])> {
        let mode = FrameExecutionMode::SweepCorrection(sweep_record);
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
        state.convert(conversion)?;
        Ok((&state.record.detectors, &state.record.observables))
    }
}

#[derive(Debug)]
pub(in crate::detection) struct DetectorFrameState {
    pub(super) frame: ScalarDetectionFrame,
    zero_reference: Vec<bool>,
    record: crate::detection::DetectionRecordBuffer,
}

impl DetectorFrameState {
    fn new(
        qubit_count: usize,
        measurement_count: usize,
        detector_count: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        Ok(Self {
            frame: ScalarDetectionFrame::try_reusable(
                qubit_count,
                measurement_count,
                observable_count,
            )?,
            zero_reference: crate::detection::try_false_vec(
                measurement_count,
                "detection frame zero reference",
            )?,
            record: crate::detection::DetectionRecordBuffer {
                detectors: crate::detection::try_false_vec(
                    detector_count,
                    "detection frame detector output",
                )?,
                observables: crate::detection::try_false_vec(
                    observable_count,
                    "detection frame observable output",
                )?,
            },
        })
    }

    fn convert(&mut self, plan: &ConversionPlan) -> DetectionResult<()> {
        if self.frame.measurements.len() != plan.measurement_count
            || self.frame.observables.len() != plan.observable_count
            || self.zero_reference.len() != plan.measurement_count
        {
            return Err(DetectionError::invalid_result_format(
                "detector frame dimensions disagree with its compiled conversion plan",
            ));
        }
        plan.convert_record_into(
            &self.frame.measurements,
            &self.zero_reference,
            &mut self.record,
        )?;
        super::super::xor_bits(
            &mut self.record.observables,
            &self.frame.observables,
            "Pauli-observable",
        )?;
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
