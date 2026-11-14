use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use stab_model::{Cancellation, Circuit};
use stab_records::{
    BitPlaneBatchView, DetectionBatchView, DetectionSink, DetectorWidth, MeasurementBatchView,
    MeasurementWidth, ObservableWidth,
};

use super::error::DetectionError;
use super::frame::{
    DirectDetectorFramePlan, FRAME_BATCH_SHOTS, FrameBatchState, FrameLanes, PendingLanes,
};
use super::{DetectionConversionLimits, PreparedMeasurementToDetection};
use crate::sampling::{session_cancellation, session_cancellation_slot};
use crate::{RandomPolicy, ReferenceSampleMode, ShotCount, SinkFailurePhase};

mod contracts;
mod delivery;
mod execution;

pub use contracts::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary,
};
pub use delivery::MeasurementToDetectionTransaction;
use execution::{
    detection_rng, invariant_error, run_direct, validate_conversion_session_storage,
    validate_direct_session_storage,
};

use crate::MAX_BATCH_SHOTS;

/// Builder for immutable measurement-to-detection conversion plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementToDetectionCompiler {
    limits: DetectionConversionLimits,
    reference_mode: ReferenceSampleMode,
}

impl MeasurementToDetectionCompiler {
    pub fn new() -> Self {
        Self {
            limits: DetectionConversionLimits::default(),
            reference_mode: ReferenceSampleMode::UseReferenceSample,
        }
    }

    #[must_use]
    pub fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub const fn reference_sample_mode(mut self, mode: ReferenceSampleMode) -> Self {
        self.reference_mode = mode;
        self
    }

    pub fn compile(
        &self,
        circuit: &Circuit,
    ) -> Result<MeasurementToDetectionPlan, DetectionCompileError> {
        let converter = PreparedMeasurementToDetection::compile_with_limits(
            circuit,
            self.reference_mode,
            self.limits.clone(),
        )?;
        Ok(MeasurementToDetectionPlan::from_converter(converter))
    }
}

impl Default for MeasurementToDetectionCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable, shareable measurement-to-detection conversion plan.
#[derive(Clone)]
pub struct MeasurementToDetectionPlan {
    inner: Arc<MeasurementToDetectionPlanInner>,
}

#[derive(Clone)]
struct MeasurementToDetectionPlanInner {
    converter: PreparedMeasurementToDetection,
}

impl fmt::Debug for MeasurementToDetectionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionPlan")
            .field("measurement_width", &self.measurement_width())
            .field("sweep_width", &self.sweep_width())
            .field("detector_width", &self.detector_width())
            .field("observable_width", &self.observable_width())
            .finish_non_exhaustive()
    }
}

impl MeasurementToDetectionPlan {
    fn from_converter(converter: PreparedMeasurementToDetection) -> Self {
        Self {
            inner: Arc::new(MeasurementToDetectionPlanInner { converter }),
        }
    }

    pub fn measurement_width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.inner.converter.measurement_count())
    }

    pub fn sweep_width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.inner.converter.sweep_bit_count())
    }

    pub fn detector_width(&self) -> DetectorWidth {
        DetectorWidth::new(self.inner.converter.detector_count())
    }

    pub fn observable_width(&self) -> ObservableWidth {
        ObservableWidth::new(self.inner.converter.observable_count())
    }

    pub fn session(&self) -> Result<MeasurementToDetectionSession, DetectionExecutionError> {
        MeasurementToDetectionSession::new(self.clone())
    }
}

/// Borrows the first `shot_count` lanes of detector and observable frame planes as one batch.
fn detection_plane_view<'a>(
    detector_planes: &'a [FrameLanes],
    observable_planes: &'a [FrameLanes],
    shot_count: usize,
) -> Result<DetectionBatchView<'a>, DetectionExecutionError> {
    DetectionBatchView::try_new(
        BitPlaneBatchView::try_from_blocks(detector_planes, shot_count).map_err(invariant_error)?,
        BitPlaneBatchView::try_from_blocks(observable_planes, shot_count)
            .map_err(invariant_error)?,
    )
    .map_err(invariant_error)
}

/// Mutable reusable state for measurement-to-detection conversion.
pub struct MeasurementToDetectionSession {
    plan: MeasurementToDetectionPlan,
    reference_sample: Vec<bool>,
    sweep_correction: Option<FrameBatchState>,
    correction_rng: SmallRng,
    measurement_planes: Vec<FrameLanes>,
    sweep_planes: Vec<FrameLanes>,
    detector_planes: Vec<FrameLanes>,
    observable_planes: Vec<FrameLanes>,
    input_column: Vec<u64>,
    cancellation: OnceLock<Cancellation>,
    total_committed_shots: u64,
    poisoned: bool,
    transaction_active: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl fmt::Debug for MeasurementToDetectionSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MeasurementToDetectionSession")
            .field("plan", &self.plan)
            .field(
                "cancelled",
                &session_cancellation(&self.cancellation).is_cancelled(),
            )
            .field("total_committed_shots", &self.total_committed_shots)
            .field("poisoned", &self.poisoned)
            .field("transaction_active", &self.transaction_active)
            .finish_non_exhaustive()
    }
}

impl MeasurementToDetectionSession {
    fn new(plan: MeasurementToDetectionPlan) -> Result<Self, DetectionExecutionError> {
        validate_conversion_session_storage(&plan)?;
        let converter = &plan.inner.converter;
        let mut reference_sample = converter
            .try_reusable_reference_sample()
            .map_err(DetectionExecutionError::Conversion)?;
        converter
            .reference_sample
            .fill(converter.measurement_count(), &mut reference_sample)
            .map_err(DetectionExecutionError::Conversion)?;
        Ok(Self {
            reference_sample,
            sweep_correction: converter
                .try_reusable_sweep_correction()
                .map_err(DetectionExecutionError::Conversion)?,
            correction_rng: SmallRng::seed_from_u64(0),
            measurement_planes: try_zero_word_planes(
                converter.measurement_count(),
                "measurement-to-detection input planes",
            )?,
            sweep_planes: try_zero_word_planes(
                converter.sweep_bit_count(),
                "measurement-to-detection sweep planes",
            )?,
            detector_planes: try_zero_word_planes(
                converter.detector_count(),
                "measurement-to-detection detector planes",
            )?,
            observable_planes: try_zero_word_planes(
                converter.observable_count(),
                "measurement-to-detection observable planes",
            )?,
            input_column: try_zero_word_planes(
                converter
                    .measurement_count()
                    .max(converter.sweep_bit_count()),
                "measurement-to-detection input transpose column",
            )?,
            cancellation: session_cancellation_slot(converter.plan.limits.cancellation()),
            plan,
            total_committed_shots: 0,
            poisoned: false,
            transaction_active: false,
            not_sync: PhantomData,
        })
    }

    /// The signal this session polls before and while converting a batch.
    ///
    /// It is the signal of the compiler's [`DetectionConversionLimits`] when that one is
    /// cancellable, and otherwise a session-local signal created by the first call. Conversion is
    /// deterministic, so a batch cancelled before or during conversion is rejected whole and the
    /// session stays resumable.
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.get_or_init(Cancellation::new).clone()
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        ShotCount::new(self.total_committed_shots)
    }

    /// Starts one incremental conversion and sink lifecycle.
    ///
    /// The returned transaction binds this session to exactly one sink and preserves already
    /// committed output when a later input record is malformed. Once a batch commits, the
    /// transaction must be finalized; an untouched transaction may be dropped without poisoning
    /// the session.
    pub fn start_transaction<'session, 'sink, Sink>(
        &'session mut self,
        sink: &'sink mut Sink,
    ) -> Result<MeasurementToDetectionTransaction<'session, 'sink, Sink>, DetectionExecutionError>
    where
        Sink: DetectionSink,
    {
        if self.poisoned {
            return Err(DetectionExecutionError::SessionPoisoned);
        }
        if self.transaction_active {
            return Err(DetectionExecutionError::TransactionActive);
        }
        self.transaction_active = true;
        Ok(MeasurementToDetectionTransaction::new(self, sink))
    }

    fn write_batch_with_progress<Sink>(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
        sink: &mut Sink,
        committed_shots: u64,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        let shots = self
            .validate_request(measurements, sweeps)
            .map_err(|source| DetectionRunError::Engine {
                source,
                progress: DetectionRunProgress::new(committed_shots, 0),
            })?;
        if shots.get() == 0 {
            return Ok(self.summary(DetectionRunStatus::Completed, shots, 0));
        }
        if session_cancellation(&self.cancellation).is_cancelled() {
            return Ok(self.summary(DetectionRunStatus::Cancelled, shots, 0));
        }
        let shot_count = usize::try_from(shots.get()).map_err(|_| DetectionRunError::Engine {
            source: DetectionExecutionError::InternalInvariant {
                message: "bounded conversion batch did not fit usize".to_owned(),
            },
            progress: DetectionRunProgress::new(committed_shots, shots.get()),
        })?;
        if let Err(source) = self.fill_batch(measurements, sweeps, shot_count) {
            if matches!(source, DetectionExecutionError::Cancelled(_)) {
                return Ok(self.summary(DetectionRunStatus::Cancelled, shots, 0));
            }
            self.poisoned = true;
            return Err(DetectionRunError::Engine {
                source,
                progress: DetectionRunProgress::new(committed_shots, shots.get()),
            });
        }
        let batch = match detection_plane_view(
            &self.detector_planes,
            &self.observable_planes,
            shot_count,
        ) {
            Ok(batch) => batch,
            Err(source) => {
                self.poisoned = true;
                return Err(DetectionRunError::Engine {
                    source,
                    progress: DetectionRunProgress::new(committed_shots, shots.get()),
                });
            }
        };
        if let Err(source) = sink.write_batch(batch) {
            self.poisoned = true;
            return Err(DetectionRunError::Sink {
                phase: SinkFailurePhase::WriteBatch,
                source,
                progress: DetectionRunProgress::new(committed_shots, shots.get()),
            });
        }
        self.total_committed_shots += shots.get();
        Ok(self.summary(DetectionRunStatus::Completed, shots, shots.get()))
    }

    /// Converts one bounded batch and owns the supplied sink's complete non-empty lifecycle.
    /// A zero-shot request leaves the sink untouched.
    pub fn run<Sink>(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
        sink: &mut Sink,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        let requested = ShotCount::new(u64::try_from(measurements.shot_count()).map_err(|_| {
            DetectionRunError::Engine {
                source: DetectionExecutionError::ShotCounterOverflow,
                progress: DetectionRunProgress::new(0, 0),
            }
        })?);
        let mut delivery =
            self.start_transaction(sink)
                .map_err(|source| DetectionRunError::Engine {
                    source,
                    progress: DetectionRunProgress::new(0, 0),
                })?;
        let summary = delivery.write_batch_with_sweep(measurements, sweeps)?;
        if requested.get() == 0 {
            return Ok(summary);
        }
        delivery.finish()?;
        Ok(summary)
    }

    fn finish_sink<Sink>(
        &mut self,
        sink: &mut Sink,
        committed_shots: u64,
    ) -> Result<(), DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        if self.poisoned {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::SessionPoisoned,
                progress: DetectionRunProgress::new(committed_shots, 0),
            });
        }
        if let Err(source) = sink.finish() {
            self.poisoned = true;
            return Err(DetectionRunError::Sink {
                phase: SinkFailurePhase::Finish,
                source,
                progress: DetectionRunProgress::new(committed_shots, 0),
            });
        }
        Ok(())
    }

    fn validate_request(
        &self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
    ) -> Result<ShotCount, DetectionExecutionError> {
        if self.poisoned {
            return Err(DetectionExecutionError::SessionPoisoned);
        }
        if measurements.width() != self.plan.measurement_width() {
            return Err(DetectionExecutionError::Conversion(
                DetectionError::invalid_result_format(format!(
                    "measurement batch expected {} bits per shot, got {}",
                    self.plan.measurement_width().get(),
                    measurements.width().get()
                )),
            ));
        }
        if measurements.shot_count() > MAX_BATCH_SHOTS {
            return Err(DetectionExecutionError::Conversion(
                DetectionError::invalid_result_format(format!(
                    "measurement-to-detection batches contain at most {MAX_BATCH_SHOTS} shots, got {}",
                    measurements.shot_count()
                )),
            ));
        }
        if let Some(sweeps) = sweeps {
            if sweeps.width() != self.plan.sweep_width() {
                return Err(DetectionExecutionError::Conversion(
                    DetectionError::invalid_result_format(format!(
                        "sweep batch expected {} bits per shot, got {}",
                        self.plan.sweep_width().get(),
                        sweeps.width().get()
                    )),
                ));
            }
            if sweeps.shot_count() != measurements.shot_count() {
                return Err(DetectionExecutionError::Conversion(
                    DetectionError::invalid_result_format(format!(
                        "measurement batch has {} shots but sweep batch has {}",
                        measurements.shot_count(),
                        sweeps.shot_count()
                    )),
                ));
            }
        }
        let shots = u64::try_from(measurements.shot_count())
            .map(ShotCount::new)
            .map_err(|_| DetectionExecutionError::ShotCounterOverflow)?;
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(DetectionExecutionError::ShotCounterOverflow);
        }
        Ok(shots)
    }

    fn fill_batch(
        &mut self,
        measurements: MeasurementBatchView<'_>,
        sweeps: Option<MeasurementBatchView<'_>>,
        shot_count: usize,
    ) -> Result<(), DetectionExecutionError> {
        let converter = &self.plan.inner.converter;
        copy_batch_into_frame_planes(
            measurements,
            &mut self.measurement_planes,
            &mut self.input_column,
        )?;
        if let Some(sweeps) = sweeps {
            copy_batch_into_frame_planes(sweeps, &mut self.sweep_planes, &mut self.input_column)?;
        } else {
            self.sweep_planes.fill(FrameLanes::ZERO);
        }
        converter
            .convert_word_planes_with_sweep_into(
                &mut self.measurement_planes,
                &self.sweep_planes,
                shot_count,
                &self.reference_sample,
                &mut self.detector_planes,
                &mut self.observable_planes,
                self.sweep_correction.as_mut(),
                &mut self.correction_rng,
                session_cancellation(&self.cancellation),
            )
            .map_err(DetectionExecutionError::from)
    }

    const fn summary(
        &self,
        status: DetectionRunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
    ) -> DetectionRunSummary {
        DetectionRunSummary {
            status,
            requested_shots,
            committed_shots: ShotCount::new(committed_shots),
            total_committed_shots: ShotCount::new(self.total_committed_shots),
        }
    }
}

fn try_zero_word_planes<Plane: Copy + Default>(
    width: usize,
    context: &'static str,
) -> Result<Vec<Plane>, DetectionExecutionError> {
    let mut planes = Vec::new();
    planes.try_reserve_exact(width).map_err(|error| {
        DetectionExecutionError::SessionStorageAllocation {
            message: format!("could not reserve {width} planes for {context}: {error}"),
        }
    })?;
    planes.resize(width, Plane::default());
    Ok(planes)
}

/// Replaces every frame plane with the records of `batch`, at most [`FRAME_BATCH_SHOTS`] of
/// them, using `column` as scratch for one 64-shot word per result bit.
fn copy_batch_into_frame_planes(
    batch: MeasurementBatchView<'_>,
    target: &mut [FrameLanes],
    column: &mut [u64],
) -> Result<(), DetectionExecutionError> {
    let width = target.len();
    let column =
        column
            .get_mut(..width)
            .ok_or_else(|| DetectionExecutionError::InternalInvariant {
                message: "measurement-to-detection transpose column is narrower than its input"
                    .to_owned(),
            })?;
    if batch.width().get() != width || batch.shot_count() > FRAME_BATCH_SHOTS {
        return Err(DetectionExecutionError::InternalInvariant {
            message: "measurement-to-detection input exceeds its frame planes".to_owned(),
        });
    }
    target.fill(FrameLanes::ZERO);
    for (word_index, row_start) in (0..batch.shot_count()).step_by(64).enumerate() {
        let row_count = (batch.shot_count() - row_start).min(64);
        let copied = if let Some(records) = batch.shot_major_records() {
            records.copy_rows_into_bit_plane_words(row_start, row_count, column)
        } else if let Some(planes) = batch.bit_planes() {
            planes.copy_word_column_into(word_index, column)
        } else {
            return Err(DetectionExecutionError::InternalInvariant {
                message: "measurement batch has no recognized storage representation".to_owned(),
            });
        };
        copied.map_err(invariant_error)?;
        for (plane, word) in target.iter_mut().zip(column.iter()) {
            let mut words = plane.words();
            if let Some(slot) = words.get_mut(word_index) {
                *slot = *word;
            }
            *plane = FrameLanes::from_words(words);
        }
    }
    Ok(())
}

/// Builder for immutable circuit detection-sampling plans.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionSamplingCompiler {
    limits: DetectionConversionLimits,
}

impl DetectionSamplingCompiler {
    pub fn new() -> Self {
        Self {
            limits: DetectionConversionLimits::default(),
        }
    }

    #[must_use]
    pub fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn compile(
        &self,
        circuit: &Circuit,
    ) -> Result<DetectionSamplingPlan, DetectionCompileError> {
        self.limits
            .cancellation()
            .check()
            .map_err(DetectionError::from)?;
        let direct = DirectDetectorFramePlan::compile(circuit, self.limits.clone())?;
        Ok(DetectionSamplingPlan {
            inner: Arc::new(DetectionSamplingPlanInner { direct }),
        })
    }
}

impl Default for DetectionSamplingCompiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable, shareable circuit detection-sampling plan.
#[derive(Clone)]
pub struct DetectionSamplingPlan {
    inner: Arc<DetectionSamplingPlanInner>,
}

struct DetectionSamplingPlanInner {
    direct: DirectDetectorFramePlan,
}

impl fmt::Debug for DetectionSamplingPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DetectionSamplingPlan")
            .field("measurement_width", &self.measurement_width())
            .field("detector_width", &self.detector_width())
            .field("observable_width", &self.observable_width())
            .finish_non_exhaustive()
    }
}

impl DetectionSamplingPlan {
    pub fn measurement_width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.inner.direct.measurement_count())
    }

    pub fn detector_width(&self) -> DetectorWidth {
        DetectorWidth::new(self.inner.direct.detector_count())
    }

    pub fn observable_width(&self) -> ObservableWidth {
        ObservableWidth::new(self.inner.direct.observable_count())
    }

    pub fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<DetectionSamplingSession, DetectionExecutionError> {
        DetectionSamplingSession::new(self.clone(), random_policy)
    }
}

struct DirectDetectionState {
    rng: SmallRng,
    frame: FrameBatchState,
    pending: PendingLanes,
    cancellation: OnceLock<Cancellation>,
}

/// Mutable reusable state for circuit detection sampling.
pub struct DetectionSamplingSession {
    plan: DetectionSamplingPlan,
    state: DirectDetectionState,
    total_committed_shots: u64,
    poisoned: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl fmt::Debug for DetectionSamplingSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DetectionSamplingSession")
            .field("plan", &self.plan)
            .field("total_committed_shots", &self.total_committed_shots)
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}

impl DetectionSamplingSession {
    fn new(
        plan: DetectionSamplingPlan,
        random_policy: RandomPolicy,
    ) -> Result<Self, DetectionExecutionError> {
        validate_direct_session_storage(&plan.inner.direct)?;
        let state = DirectDetectionState {
            rng: detection_rng(random_policy),
            frame: plan
                .inner
                .direct
                .state()
                .map_err(DetectionExecutionError::Conversion)?,
            pending: PendingLanes::default(),
            cancellation: session_cancellation_slot(plan.inner.direct.cancellation()),
        };
        Ok(Self {
            plan,
            state,
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    /// The signal this session polls between and inside batches.
    ///
    /// It is the signal of the compiler's [`DetectionConversionLimits`] when that one is
    /// cancellable, and otherwise a session-local signal created by the first call.
    /// Cancellation observed before a batch starts returns [`DetectionRunStatus::Cancelled`] and
    /// leaves the session resumable; cancellation observed while a batch is sampled returns
    /// [`DetectionExecutionError::Cancelled`], finalizes nothing, and poisons the session because
    /// the random stream has advanced into the discarded batch.
    pub fn cancellation(&self) -> Cancellation {
        self.state
            .cancellation
            .get_or_init(Cancellation::new)
            .clone()
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        ShotCount::new(self.total_committed_shots)
    }

    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<DetectionRunSummary, DetectionRunError<Sink::Error>>
    where
        Sink: DetectionSink,
    {
        if self.is_poisoned() {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::SessionPoisoned,
                progress: DetectionRunProgress::new(0, 0),
            });
        }
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(DetectionRunError::Engine {
                source: DetectionExecutionError::ShotCounterOverflow,
                progress: DetectionRunProgress::new(0, 0),
            });
        }
        if shots.get() == 0 {
            return Ok(self.summary(DetectionRunStatus::Completed, shots, 0));
        }

        let result = run_direct(&self.plan.inner.direct, &mut self.state, shots, sink);

        match result {
            Ok(summary) => {
                self.total_committed_shots += summary.committed_shots().get();
                Ok(DetectionRunSummary {
                    status: summary.status(),
                    requested_shots: shots,
                    committed_shots: summary.committed_shots(),
                    total_committed_shots: ShotCount::new(self.total_committed_shots),
                })
            }
            Err(error) => {
                self.total_committed_shots += error.progress().committed_shots().get();
                if matches!(
                    error,
                    DetectionRunError::Engine {
                        source: DetectionExecutionError::ShotCounterOverflow,
                        ..
                    }
                ) {
                    return Err(error);
                }
                self.poisoned = true;
                Err(error)
            }
        }
    }

    const fn summary(
        &self,
        status: DetectionRunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
    ) -> DetectionRunSummary {
        DetectionRunSummary {
            status,
            requested_shots,
            committed_shots: ShotCount::new(committed_shots),
            total_committed_shots: ShotCount::new(self.total_committed_shots),
        }
    }
}

#[cfg(test)]
mod tests;
