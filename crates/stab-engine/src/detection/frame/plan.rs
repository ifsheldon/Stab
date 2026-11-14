use rand::Rng;
use stab_model::{Cancellation, Circuit};

use super::program::{CircuitProgram, ProgramOptions};
use super::window::PlaneWindow;
use super::word::FrameWord;
use super::{BitPlaneDetectionFrame, FrameExecutionMode, FrameLanes};
use crate::LaneWidth;
use crate::detection::error::{
    DetectionError, DetectionResourceLimitError as ResourceLimitError, DetectionResult,
};
use crate::detection::{ConversionPlan, DetectionConversionLimits};
use crate::sampling::ReferenceBits;

/// Shots in one frame batch of the widest lane word, [`FrameLanes`]: the default for circuit
/// sampling and detection sampling, and the only width of measurement-to-detection conversion and
/// sweep correction.
pub(crate) const FRAME_BATCH_SHOTS: usize = FrameLanes::LANE_COUNT;

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
        let memory_budget = limits.memory_budget();
        let conversion_admission =
            ConversionPlan::admission_from_circuit_with_limits(circuit, limits)?;
        let conversion_bytes = conversion_admission.compiled_storage_bytes()?;
        let execution_admission = CircuitProgram::admit(
            circuit,
            ProgramOptions::DETECTION,
            conversion_bytes,
            memory_budget.bytes(),
        )?;
        admit_combined_compiled_storage(
            conversion_bytes,
            execution_admission.retained_bytes(),
            memory_budget.bytes(),
        )?;
        let conversion =
            ConversionPlan::materialize_circuit_from_admission(circuit, conversion_admission)?;
        let executable = CircuitProgram::materialize(circuit, &execution_admission)?;
        let result = Self {
            executable,
            conversion,
        };
        admit_combined_compiled_storage(result.compiled_bytes()?, 0, memory_budget.bytes())?;
        Ok(result)
    }

    pub(in crate::detection) fn measurement_count(&self) -> usize {
        self.conversion.measurement_count
    }

    pub(in crate::detection) fn memory_budget(&self) -> stab_model::MemoryBudget {
        self.conversion.limits.memory_budget()
    }

    pub(in crate::detection) fn cancellation(&self) -> &Cancellation {
        self.conversion.limits.cancellation()
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

    pub(in crate::detection) const fn executable(&self) -> &CircuitProgram {
        &self.executable
    }

    pub(in crate::detection) const fn conversion(&self) -> &ConversionPlan {
        &self.conversion
    }

    pub(in crate::detection) fn state<W: FrameWord>(&self) -> DetectionResult<FrameBatchState<W>> {
        FrameBatchState::for_detection(
            self.qubit_count(),
            self.measurement_count(),
            self.detector_count(),
            self.observable_count(),
        )
    }

    pub(in crate::detection) fn state_storage_bytes(&self, lane_width: LaneWidth) -> u128 {
        frame_state_storage_bytes(
            lane_width,
            self.qubit_count(),
            self.measurement_count(),
            self.detector_count(),
            self.observable_count(),
        )
    }

    /// Samples one batch of `shot_count` active lanes and converts it to detection planes.
    pub(in crate::detection) fn sample_batch<W: FrameWord>(
        &self,
        state: &mut FrameBatchState<W>,
        rng: &mut impl Rng,
        shot_count: usize,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_lanes: shot_count,
        };
        state.run_program(&self.executable, rng, mode, cancellation, None)?;
        state.convert_batch(&self.conversion, cancellation)
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

    pub(in crate::detection) fn state(&self) -> DetectionResult<FrameBatchState<FrameLanes>> {
        FrameBatchState::for_detection(
            self.executable.frame_qubit_count(),
            self.measurement_count,
            self.detector_count,
            self.observable_count,
        )
    }

    pub(in crate::detection) fn state_storage_bytes(&self) -> u128 {
        frame_state_storage_bytes(
            LaneWidth::Lanes256,
            self.executable.frame_qubit_count(),
            self.measurement_count,
            self.detector_count,
            self.observable_count,
        )
    }

    pub(in crate::detection) fn correct_batch<'a>(
        &self,
        conversion: &ConversionPlan,
        sweep_planes: &[FrameLanes],
        shot_count: usize,
        state: &'a mut FrameBatchState<FrameLanes>,
        rng: &mut impl Rng,
        cancellation: &Cancellation,
    ) -> DetectionResult<(&'a [FrameLanes], &'a [FrameLanes])> {
        if sweep_planes.len() != self.sweep_bit_count {
            return Err(DetectionError::invalid_result_format(format!(
                "sweep plane count {} does not match the compiled width {}",
                sweep_planes.len(),
                self.sweep_bit_count
            )));
        }
        if shot_count > FRAME_BATCH_SHOTS {
            return Err(DetectionError::invalid_result_format(format!(
                "sweep correction batches contain at most {FRAME_BATCH_SHOTS} shots, got {shot_count}"
            )));
        }
        let mode = FrameExecutionMode::SweepCorrection {
            sweep_planes,
            active_lanes: shot_count,
        };
        state.run_program(&self.executable, rng, mode, cancellation, None)?;
        if conversion.measurement_count != self.measurement_count
            || conversion.detector_count != self.detector_count
            || conversion.observable_count != self.observable_count
        {
            return Err(DetectionError::invalid_result_format(
                "sweep correction conversion dimensions disagree with its admitted plan",
            ));
        }
        state.convert_batch(conversion, cancellation)?;
        Ok((&state.detector_planes, &state.observable_planes))
    }
}

/// What a frame batch delivers to its consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameRecords {
    /// The measurement planes themselves, as circuit sampling delivers them.
    Measurements,
    /// Detector and observable planes converted from the measurement planes.
    Detection,
}

/// Reusable storage for one frame batch: the Pauli frame and what the batch records.
///
/// Circuit sampling records only the frame's measurement planes; detection sampling and sweep
/// correction also convert them into detector and observable planes. Every plane is one lane
/// word `W` per result bit, so a batch holds `W::LANE_COUNT` shots.
#[derive(Debug)]
pub(crate) struct FrameBatchState<W: FrameWord> {
    frame: BitPlaneDetectionFrame<W>,
    records: FrameRecords,
    measurement_count: usize,
    detector_planes: Vec<W>,
    observable_planes: Vec<W>,
}

/// Conservative reusable storage of a frame batch state with the given widths at `lane_width`:
/// two frame planes per qubit, one plane per measurement and detector, and two per observable
/// (the frame's Pauli-observable accumulator and the converted output).
pub(crate) fn frame_state_storage_bytes(
    lane_width: LaneWidth,
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
        .saturating_mul(lane_width.plane_bytes() as u128)
}

impl<W: FrameWord> FrameBatchState<W> {
    /// State that records only the measurement planes of `program`.
    pub(crate) fn for_measurements(program: &CircuitProgram) -> DetectionResult<Self> {
        Self::for_measurement_window(program, program.counts().measurements)
    }

    /// State that records `program`'s measurement planes and retains at most `capacity` of them
    /// at once, for batches that stream through a [`PlaneWindow`].
    pub(crate) fn for_measurement_window(
        program: &CircuitProgram,
        capacity: usize,
    ) -> DetectionResult<Self> {
        let mut state = Self::for_detection(program.frame_qubit_count(), capacity, 0, 0)?;
        state.measurement_count = program.counts().measurements;
        state.records = FrameRecords::Measurements;
        Ok(state)
    }

    /// State for a streaming detection batch of `program`: it retains at most `capacity`
    /// measurement planes and the frame's Pauli-observable accumulator, and converts nothing
    /// itself.
    pub(in crate::detection) fn for_detection_window(
        program: &CircuitProgram,
        capacity: usize,
        observable_count: usize,
    ) -> DetectionResult<Self> {
        let mut state =
            Self::for_detection(program.frame_qubit_count(), capacity, 0, observable_count)?;
        state.measurement_count = program.counts().measurements;
        state.records = FrameRecords::Measurements;
        Ok(state)
    }

    /// The frame's Pauli-observable accumulator after a batch.
    pub(in crate::detection) fn frame_observables(&self) -> &[W] {
        &self.frame.observables
    }

    pub(in crate::detection) fn for_detection(
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
            records: FrameRecords::Detection,
            measurement_count,
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

    /// Samples one full batch of `W::LANE_COUNT` shots of `program`'s measurements.
    pub(crate) fn sample_measurements(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_lanes: W::LANE_COUNT,
        };
        self.run_program(program, rng, mode, cancellation, None)
    }

    /// Samples one full batch of `W::LANE_COUNT` shots of `program`'s measurements, handing the
    /// measurement planes to `window` instead of keeping them all.
    pub(crate) fn sample_measurement_window(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
        cancellation: &Cancellation,
        window: &mut PlaneWindow<'_, W>,
    ) -> DetectionResult<()> {
        let mode = FrameExecutionMode::Sample {
            active_lanes: W::LANE_COUNT,
        };
        self.run_program(program, rng, mode, cancellation, Some(window))
    }

    fn run_program(
        &mut self,
        program: &CircuitProgram,
        rng: &mut impl Rng,
        mode: FrameExecutionMode<'_, W>,
        cancellation: &Cancellation,
        window: Option<&mut PlaneWindow<'_, W>>,
    ) -> DetectionResult<()> {
        self.frame.reset(rng, mode);
        match window {
            None => self
                .frame
                .execute_program(program, rng, mode, cancellation)?,
            Some(window) => {
                self.frame
                    .execute_program_windowed(program, rng, mode, cancellation, window)?
            }
        }
        if self.frame.recorded_measurements() != self.measurement_count {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "Pauli frame produced {} measurement planes but {} were compiled",
                self.frame.recorded_measurements(),
                self.measurement_count
            )));
        }
        Ok(())
    }

    pub(crate) fn measurement_planes(&self) -> &[W] {
        &self.frame.measurements
    }

    /// Flips every lane of each measurement plane whose reference bit is set.
    pub(crate) fn xor_reference(&mut self, reference: &ReferenceBits) -> DetectionResult<()> {
        if reference.len() != self.frame.measurements.len() {
            return Err(DetectionError::invalid_sampler_compilation(format!(
                "reference sample has {} bits but the frame recorded {} measurements",
                reference.len(),
                self.frame.measurements.len()
            )));
        }
        let all_lanes = W::active_mask(W::LANE_COUNT);
        for (planes, word) in self
            .frame
            .measurements
            .chunks_mut(u64::BITS as usize)
            .zip(reference.words())
        {
            let mut bits = *word;
            while bits != 0 {
                if let Some(plane) = planes.get_mut(bits.trailing_zeros() as usize) {
                    *plane ^= all_lanes;
                }
                bits &= bits - 1;
            }
        }
        Ok(())
    }

    pub(in crate::detection) fn detection_planes(&self) -> (&[W], &[W]) {
        (&self.detector_planes, &self.observable_planes)
    }

    /// Moves the lanes after the first `delivered` lanes of every recorded output plane down to
    /// lane zero, so the undelivered rest of a batch can be delivered as a prefix later.
    pub(crate) fn discard_delivered_lanes(&mut self, delivered: usize) {
        let planes: &mut [W] = match self.records {
            FrameRecords::Measurements => &mut self.frame.measurements,
            FrameRecords::Detection => &mut self.detector_planes,
        };
        for plane in planes {
            *plane = plane.shift_lanes_down(delivered);
        }
        if self.records == FrameRecords::Detection {
            for plane in &mut self.observable_planes {
                *plane = plane.shift_lanes_down(delivered);
            }
        }
    }

    fn convert_batch(
        &mut self,
        plan: &ConversionPlan,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
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
            cancellation,
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

/// One session's frame batch state at the lane width the session was created with.
///
/// Each variant is a separate monomorphization of the frame, so a session dispatches on its
/// width once per run and every batch runs on a fixed-size lane word.
#[derive(Debug)]
pub(crate) enum LaneFrameBatch {
    Lanes64(FrameBatchState<u64>),
    Lanes256(FrameBatchState<FrameLanes>),
}

impl LaneFrameBatch {
    /// State at `lane_width` that records only the measurement planes of `program`.
    pub(crate) fn for_measurements(
        program: &CircuitProgram,
        lane_width: LaneWidth,
    ) -> DetectionResult<Self> {
        Ok(match lane_width {
            LaneWidth::Lanes64 => Self::Lanes64(FrameBatchState::for_measurements(program)?),
            LaneWidth::Lanes256 => Self::Lanes256(FrameBatchState::for_measurements(program)?),
        })
    }

    /// Detection state at `lane_width` for `plan`.
    pub(in crate::detection) fn for_detection(
        plan: &DirectDetectorFramePlan,
        lane_width: LaneWidth,
    ) -> DetectionResult<Self> {
        Ok(match lane_width {
            LaneWidth::Lanes64 => Self::Lanes64(plan.state()?),
            LaneWidth::Lanes256 => Self::Lanes256(plan.state()?),
        })
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
