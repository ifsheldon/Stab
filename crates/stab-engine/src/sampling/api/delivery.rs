//! The two ways a sampling session hands frame batches to its sink.
//!
//! A materializing session keeps every measurement plane of a batch and delivers lane prefixes
//! through the shared [`PendingLanes`] loop, so undelivered lanes survive for the next run. A
//! streaming run hands the planes over in bounded windows and keeps shot-major records of only
//! the lanes it delivers.

use rand::rngs::SmallRng;
use stab_model::Cancellation;
use stab_records::{MeasurementBatchView, MeasurementSink};

use super::{SamplingExecutionError, ShotCount};
use crate::detection::DetectionError;
use crate::detection::frame::{
    CircuitProgram, DeliveryFailure, DeliveryOutcome, FrameBatchSource, FrameBatchState, FrameWord,
    PendingLanes, PlaneWindow, SampledRecords, ShotRecordTransposer, WindowShape,
};
use crate::sampling::ReferenceBits;

pub(super) enum SamplingDeliveryError<SinkError> {
    Engine(SamplingExecutionError),
    Sink(SinkError),
}

/// Runs the shared delivery loop over one session frame at a fixed lane word.
pub(super) fn deliver_lanes<W, Sink>(
    pending: &mut PendingLanes,
    shots: ShotCount,
    mut source: SamplingBatchSource<'_, W, Sink>,
) -> Result<DeliveryOutcome, DeliveryFailure<SamplingDeliveryError<Sink::Error>>>
where
    W: FrameWord,
    Sink: MeasurementSink,
{
    pending.deliver(shots.get(), &mut source)
}

fn frame_error(error: DetectionError) -> SamplingExecutionError {
    match error {
        DetectionError::Cancelled(cancelled) => SamplingExecutionError::Cancelled(cancelled),
        error => SamplingExecutionError::InternalInvariant {
            message: error.to_string(),
        },
    }
}

/// One sampling run's view of its session frame for the shared frame-batch delivery loop.
pub(super) struct SamplingBatchSource<'run, W: FrameWord, Sink> {
    program: &'run CircuitProgram,
    /// The frame state, or `None` for a plan without measurements.
    frame: Option<&'run mut FrameBatchState<W>>,
    reference: Option<&'run ReferenceBits>,
    rng: &'run mut SmallRng,
    cancellation: &'run Cancellation,
    sink: &'run mut Sink,
}

impl<'run, W: FrameWord, Sink> SamplingBatchSource<'run, W, Sink> {
    pub(super) const fn new(
        program: &'run CircuitProgram,
        frame: Option<&'run mut FrameBatchState<W>>,
        reference: Option<&'run ReferenceBits>,
        rng: &'run mut SmallRng,
        cancellation: &'run Cancellation,
        sink: &'run mut Sink,
    ) -> Self {
        Self {
            program,
            frame,
            reference,
            rng,
            cancellation,
            sink,
        }
    }
}

impl<W, Sink> FrameBatchSource for SamplingBatchSource<'_, W, Sink>
where
    W: FrameWord,
    Sink: MeasurementSink,
{
    type Error = SamplingDeliveryError<Sink::Error>;

    const BATCH_SHOTS: usize = W::LANE_COUNT;

    fn is_cancelled(&self) -> bool {
        self.cancellation.is_cancelled()
    }

    fn refill(&mut self) -> Result<(), Self::Error> {
        let Some(state) = self.frame.as_deref_mut() else {
            return Ok(());
        };
        state
            .sample_measurements(self.program, self.rng, self.cancellation)
            .map_err(|error| SamplingDeliveryError::Engine(frame_error(error)))?;
        if let Some(reference) = self.reference {
            state.xor_reference(reference).map_err(|error| {
                SamplingDeliveryError::Engine(SamplingExecutionError::InternalInvariant {
                    message: error.to_string(),
                })
            })?;
        }
        Ok(())
    }

    fn deliver(&mut self, lanes: usize) -> Result<(), Self::Error> {
        let planes = self
            .frame
            .as_deref()
            .map_or(&[][..], FrameBatchState::measurement_planes);
        let view = W::plane_view(planes, lanes).map_err(|source| {
            SamplingDeliveryError::Engine(SamplingExecutionError::InternalInvariant {
                message: source.to_string(),
            })
        })?;
        self.sink
            .write_batch(MeasurementBatchView::from_bit_planes(view))
            .map_err(SamplingDeliveryError::Sink)
    }

    fn discard(&mut self, lanes: usize) {
        if let Some(state) = self.frame.as_deref_mut() {
            state.discard_delivered_lanes(lanes);
        }
    }
}

/// The frame of a streaming run: windowed planes and the delivered lanes' records.
#[derive(Debug)]
pub(super) struct StreamingFrame<W: FrameWord> {
    pub(super) state: FrameBatchState<W>,
    pub(super) shape: WindowShape,
    pub(super) records: ShotRecordTransposer,
}

/// One streaming run's frame, sink, and random stream.
pub(super) struct StreamingDelivery<'run, W: FrameWord, Sink> {
    pub(super) program: &'run CircuitProgram,
    pub(super) frame: &'run mut StreamingFrame<W>,
    pub(super) reference: Option<&'run ReferenceBits>,
    pub(super) rng: &'run mut SmallRng,
    pub(super) cancellation: &'run Cancellation,
    pub(super) sink: &'run mut Sink,
}

impl<W, Sink> StreamingDelivery<'_, W, Sink>
where
    W: FrameWord,
    Sink: MeasurementSink,
{
    /// Delivers `shots` shots in fresh batches, each keeping only the lanes it delivers.
    ///
    /// Every batch samples all `W::LANE_COUNT` lanes, so the random stream and the delivered
    /// records equal those of a materializing session's first run of `shots` shots.
    pub(super) fn deliver(
        self,
        shots: ShotCount,
    ) -> Result<DeliveryOutcome, DeliveryFailure<SamplingDeliveryError<Sink::Error>>> {
        let flips = self.reference.map_or(&[][..], ReferenceBits::words);
        let mut remaining = shots.get();
        let mut committed = 0_u64;
        while remaining > 0 {
            if self.cancellation.is_cancelled() {
                break;
            }
            let lanes = usize::try_from(remaining)
                .map_or(W::LANE_COUNT, |remaining| remaining.min(W::LANE_COUNT));
            let failure = |error| DeliveryFailure {
                error,
                committed,
                attempted: lanes as u64,
            };
            let frame = &mut *self.frame;
            frame
                .records
                .begin_batch(lanes)
                .map_err(|error| failure(SamplingDeliveryError::Engine(frame_error(error))))?;
            let mut consumer = SampledRecords {
                records: &mut frame.records,
                reference: flips,
            };
            let mut window = PlaneWindow::new(frame.shape, &mut consumer);
            frame
                .state
                .sample_measurement_window(self.program, self.rng, self.cancellation, &mut window)
                .map_err(|error| failure(SamplingDeliveryError::Engine(frame_error(error))))?;
            let view = frame
                .records
                .view()
                .map_err(|error| failure(SamplingDeliveryError::Engine(frame_error(error))))?;
            self.sink
                .write_batch(MeasurementBatchView::new(view))
                .map_err(|error| failure(SamplingDeliveryError::Sink(error)))?;
            committed += lanes as u64;
            remaining -= lanes as u64;
        }
        Ok(DeliveryOutcome {
            committed,
            completed: remaining == 0,
        })
    }
}
