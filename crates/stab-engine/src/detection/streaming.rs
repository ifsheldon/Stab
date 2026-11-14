//! Detection batches that stream their measurement and detector planes through bounded windows.
//!
//! The frame hands each window of measurement planes to a [`DetectionWindow`], which resumes the
//! conversion program over it, writes the detectors it can complete into a detector window, and
//! transposes every full detector window into shot-major records of the delivered lanes. Only
//! the planes a later detector, observable, or classical control can still read stay alive.

use rand::Rng;
use stab_model::Cancellation;

use super::conversion_plan::{ConversionPlan, ConversionRunner, ConversionStop};
use super::error::{DetectionError, DetectionResult};
use super::frame::{
    DirectDetectorFramePlan, FrameBatchState, FrameWord, PlaneConsumer, PlaneWindow,
    ShotRecordTransposer, WindowShape, frame_state_storage_bytes, window_planes,
};
use super::try_vec_with_capacity;
use crate::LaneWidth;

/// Reusable storage of one streaming detection run at one lane word.
#[derive(Debug)]
pub(super) struct StreamingDetectionFrame<W: FrameWord> {
    state: FrameBatchState<W>,
    shape: WindowShape,
    detector_window: Vec<W>,
    observable_planes: Vec<W>,
    detectors: ShotRecordTransposer,
    observables: ShotRecordTransposer,
    /// Most measurements back from its own position that a detector or observable reads.
    lookback: usize,
}

/// The planes and records of one streaming detection run's storage, before it is allocated.
#[derive(Clone, Copy, Debug)]
pub(super) struct StreamingDetectionShape {
    window: WindowShape,
    detector_window: usize,
    lanes: usize,
    lookback: usize,
}

impl StreamingDetectionShape {
    pub(super) fn new(plan: &DirectDetectorFramePlan, lane_width: LaneWidth, lanes: usize) -> Self {
        let lookback = plan.conversion().max_record_lookback();
        Self {
            window: WindowShape::for_program(plan.executable(), lane_width.plane_bytes(), lookback),
            detector_window: window_planes(lane_width.plane_bytes()).min(plan.detector_count()),
            lanes,
            lookback,
        }
    }

    /// Storage of the frame with its windows, and of the delivered lanes' records.
    pub(super) fn storage_bytes(
        self,
        plan: &DirectDetectorFramePlan,
        lane_width: LaneWidth,
    ) -> u128 {
        frame_state_storage_bytes(
            lane_width,
            plan.qubit_count(),
            self.window.capacity(),
            self.detector_window,
            plan.observable_count(),
        )
        .saturating_add(ShotRecordTransposer::storage_bytes(
            self.lanes,
            plan.detector_count(),
        ))
        .saturating_add(ShotRecordTransposer::storage_bytes(
            self.lanes,
            plan.observable_count(),
        ))
    }
}

impl<W: FrameWord> StreamingDetectionFrame<W> {
    pub(super) fn try_new(
        plan: &DirectDetectorFramePlan,
        shape: StreamingDetectionShape,
    ) -> DetectionResult<Self> {
        let mut detector_window =
            try_vec_with_capacity(shape.detector_window, "streaming detector window")?;
        detector_window.resize(shape.detector_window, W::default());
        let mut observable_planes =
            try_vec_with_capacity(plan.observable_count(), "streaming observable planes")?;
        observable_planes.resize(plan.observable_count(), W::default());
        Ok(Self {
            state: FrameBatchState::for_detection_window(
                plan.executable(),
                shape.window.capacity(),
                plan.observable_count(),
            )?,
            shape: shape.window,
            detector_window,
            observable_planes,
            detectors: ShotRecordTransposer::try_new(shape.lanes, plan.detector_count())?,
            observables: ShotRecordTransposer::try_new(shape.lanes, plan.observable_count())?,
            lookback: shape.lookback,
        })
    }

    /// Samples one batch of `W::LANE_COUNT` lanes and keeps records of its first `lanes`.
    pub(super) fn sample_batch(
        &mut self,
        plan: &DirectDetectorFramePlan,
        lanes: usize,
        rng: &mut impl Rng,
        cancellation: &Cancellation,
    ) -> DetectionResult<()> {
        self.detectors.begin_batch(lanes)?;
        self.observables.begin_batch(lanes)?;
        self.observable_planes.fill(W::default());
        let mut consumer = DetectionWindow {
            conversion: plan.conversion(),
            runner: ConversionRunner::default(),
            lookback: self.lookback,
            detector_window: &mut self.detector_window,
            detector_base: 0,
            detectors: &mut self.detectors,
            observables: &mut self.observable_planes,
            cancellation,
        };
        let mut window = PlaneWindow::new(self.shape, &mut consumer);
        self.state
            .sample_measurement_window(plan.executable(), rng, cancellation, &mut window)?;
        for (plane, frame_observable) in self
            .observable_planes
            .iter_mut()
            .zip(self.state.frame_observables())
        {
            *plane ^= *frame_observable;
        }
        let observable_count = self.observable_planes.len();
        self.observables
            .transpose_until(0, &self.observable_planes, observable_count, &[])
    }

    pub(super) const fn detectors(&self) -> &ShotRecordTransposer {
        &self.detectors
    }

    pub(super) const fn observables(&self) -> &ShotRecordTransposer {
        &self.observables
    }
}

/// The consumer of a streaming detection batch's measurement planes.
struct DetectionWindow<'a, W> {
    conversion: &'a ConversionPlan,
    runner: ConversionRunner,
    /// Most measurements back from its own position that a detector or observable reads.
    lookback: usize,
    detector_window: &'a mut [W],
    /// Absolute detector index of `detector_window[0]`.
    detector_base: usize,
    detectors: &'a mut ShotRecordTransposer,
    observables: &'a mut [W],
    cancellation: &'a Cancellation,
}

impl<W: FrameWord> PlaneConsumer<W> for DetectionWindow<'_, W> {
    fn consume(&mut self, base: usize, planes: &[W], last: bool) -> DetectionResult<usize> {
        let available = base + planes.len();
        loop {
            let stop = self.conversion.convert_window(
                &mut self.runner,
                planes,
                base,
                self.detector_window,
                self.detector_base,
                self.observables,
                self.cancellation,
            )?;
            match stop {
                ConversionStop::DetectorWindowFull => {
                    let end = self.detector_base + self.detector_window.len();
                    self.detectors.transpose_until(
                        self.detector_base,
                        self.detector_window,
                        end,
                        &[],
                    )?;
                    self.detector_base = end;
                }
                ConversionStop::NeedsMeasurements if last => {
                    return Err(DetectionError::invalid_sampler_compilation(
                        "streaming conversion needed measurements past the end of its batch",
                    ));
                }
                ConversionStop::NeedsMeasurements | ConversionStop::Finished => break,
            }
        }
        if last {
            self.conversion.check_finished(&self.runner)?;
            let end = self.conversion.detector_count;
            self.detectors
                .transpose_until(self.detector_base, self.detector_window, end, &[])?;
            return Ok(available);
        }
        Ok(available.saturating_sub(self.lookback))
    }
}

#[cfg(test)]
mod tests;
