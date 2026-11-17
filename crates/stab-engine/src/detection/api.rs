use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use stab_model::{Cancellation, Circuit, MemoryBudget};
use stab_records::{
    DetectionBatchView, DetectionSink, DetectorWidth, MeasurementBatchView, MeasurementWidth,
    ObservableWidth,
};

use super::conversion_window::WindowedConversion;
use super::error::DetectionError;
use super::frame::{
    DirectDetectorFramePlan, FRAME_BATCH_SHOTS, FrameBatchState, FrameLanes, FrameWord,
    LaneFrameBatch, PendingLanes,
};
use super::{DetectionConversionLimits, PreparedMeasurementToDetection};
use crate::sampling::{session_cancellation, session_cancellation_slot};
use crate::{LaneWidth, RandomPolicy, ReferenceSampleMode, ShotCount, SinkFailurePhase};

mod contracts;
mod delivery;
mod execution;
mod streaming;

pub use contracts::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary,
};
pub use delivery::MeasurementToDetectionTransaction;
use execution::{
    conversion_session_storage_bytes, detection_rng, invariant_error, run_direct,
    validate_conversion_session_storage, validate_direct_session_storage,
};
pub use streaming::StreamingDetectionRun;

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
        let retained_bytes = converter.compiled_bytes()?;
        Ok(MeasurementToDetectionPlan::from_converter(
            converter,
            retained_bytes,
        ))
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
    retained_bytes: u64,
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
    fn from_converter(converter: PreparedMeasurementToDetection, retained_bytes: u64) -> Self {
        Self {
            inner: Arc::new(MeasurementToDetectionPlanInner {
                converter,
                retained_bytes,
            }),
        }
    }

    /// Conservative bytes retained by this compiled conversion plan and any sweep-correction
    /// program it keeps.
    pub fn retained_bytes(&self) -> u64 {
        self.inner.retained_bytes
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
        self.session_with_budget(self.inner.converter.plan.limits.memory_budget())
    }

    /// Creates a conversion session, admitting its reusable storage against `memory_budget`
    /// instead of the plan's compile budget.
    pub fn session_with_budget(
        &self,
        memory_budget: MemoryBudget,
    ) -> Result<MeasurementToDetectionSession, DetectionExecutionError> {
        MeasurementToDetectionSession::new(self.clone(), memory_budget)
    }
}

/// Borrows the first `shot_count` lanes of detector and observable frame planes as one batch.
fn detection_plane_view<'a, W: FrameWord>(
    detector_planes: &'a [W],
    observable_planes: &'a [W],
    shot_count: usize,
) -> Result<DetectionBatchView<'a>, DetectionExecutionError> {
    DetectionBatchView::try_new(
        W::plane_view(detector_planes, shot_count).map_err(invariant_error)?,
        W::plane_view(observable_planes, shot_count).map_err(invariant_error)?,
    )
    .map_err(invariant_error)
}

/// Mutable reusable state for measurement-to-detection conversion.
pub struct MeasurementToDetectionSession {
    plan: MeasurementToDetectionPlan,
    reference_sample: Vec<bool>,
    storage: ConversionStorage,
    cancellation: OnceLock<Cancellation>,
    total_committed_shots: u64,
    poisoned: bool,
    transaction_active: bool,
    not_sync: PhantomData<Cell<()>>,
}

/// How a conversion session holds one batch while it converts it.
enum ConversionStorage {
    /// A plan without sweep correction converts through bounded windows of planes and keeps
    /// shot-major records of only the batch's detectors and observables.
    Windowed(WindowedConversion),
    /// Sweep correction replays the circuit's frame over every measurement plane, so a plan
    /// with it keeps one 256-lane plane per measurement, sweep bit, detector, and observable.
    Planes(ConversionPlanes),
}

/// The full-width planes of a conversion with sweep correction.
struct ConversionPlanes {
    sweep_correction: Option<FrameBatchState<FrameLanes>>,
    correction_rng: SmallRng,
    measurement_planes: Vec<FrameLanes>,
    sweep_planes: Vec<FrameLanes>,
    detector_planes: Vec<FrameLanes>,
    observable_planes: Vec<FrameLanes>,
    input_column: Vec<u64>,
}

impl ConversionPlanes {
    fn try_new(
        converter: &PreparedMeasurementToDetection,
    ) -> Result<Self, DetectionExecutionError> {
        Ok(Self {
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
        })
    }
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
    fn new(
        plan: MeasurementToDetectionPlan,
        memory_budget: MemoryBudget,
    ) -> Result<Self, DetectionExecutionError> {
        validate_conversion_session_storage(&plan, memory_budget)?;
        let converter = &plan.inner.converter;
        let mut reference_sample = converter
            .try_reusable_reference_sample()
            .map_err(DetectionExecutionError::Conversion)?;
        converter
            .reference_sample
            .fill(converter.measurement_count(), &mut reference_sample)
            .map_err(DetectionExecutionError::Conversion)?;
        let storage = if converter.has_sweep_correction() {
            ConversionStorage::Planes(ConversionPlanes::try_new(converter)?)
        } else {
            ConversionStorage::Windowed(
                WindowedConversion::try_new(&converter.plan)
                    .map_err(DetectionExecutionError::Conversion)?,
            )
        };
        Ok(Self {
            reference_sample,
            storage,
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

    /// Conservative bytes of reusable storage held by this conversion session.
    pub fn retained_bytes(&self) -> u64 {
        u64::try_from(conversion_session_storage_bytes(&self.plan)).unwrap_or(u64::MAX)
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
        let view = match &self.storage {
            ConversionStorage::Windowed(windowed) => windowed
                .batch_view()
                .map_err(DetectionExecutionError::Conversion),
            ConversionStorage::Planes(planes) => detection_plane_view(
                &planes.detector_planes,
                &planes.observable_planes,
                shot_count,
            ),
        };
        let batch = match view {
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
        let cancellation = session_cancellation(&self.cancellation);
        let planes = match &mut self.storage {
            ConversionStorage::Windowed(windowed) => {
                // Without sweep correction the sweep bits change nothing, as in the plane path.
                return windowed
                    .convert(
                        &converter.plan,
                        measurements,
                        shot_count,
                        &self.reference_sample,
                        cancellation,
                    )
                    .map_err(DetectionExecutionError::from);
            }
            ConversionStorage::Planes(planes) => planes,
        };
        copy_batch_into_frame_planes(
            measurements,
            &mut planes.measurement_planes,
            &mut planes.input_column,
        )?;
        if let Some(sweeps) = sweeps {
            copy_batch_into_frame_planes(
                sweeps,
                &mut planes.sweep_planes,
                &mut planes.input_column,
            )?;
        } else {
            planes.sweep_planes.fill(FrameLanes::ZERO);
        }
        converter
            .convert_word_planes_with_sweep_into(
                &mut planes.measurement_planes,
                &planes.sweep_planes,
                shot_count,
                &self.reference_sample,
                &mut planes.detector_planes,
                &mut planes.observable_planes,
                planes.sweep_correction.as_mut(),
                &mut planes.correction_rng,
                cancellation,
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
        let retained_bytes = direct.compiled_bytes()?;
        Ok(DetectionSamplingPlan {
            inner: Arc::new(DetectionSamplingPlanInner {
                direct,
                retained_bytes,
            }),
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
    retained_bytes: u64,
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
    /// Conservative bytes retained by this compiled detection-sampling plan.
    pub fn retained_bytes(&self) -> u64 {
        self.inner.retained_bytes
    }

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
        self.session_with_lane_width(random_policy, LaneWidth::default())
    }

    /// Creates a session whose frame batches sample `lane_width` shots at a time.
    ///
    /// The seeded stream is a function of this plan, the seed, and `lane_width`; the default
    /// width reproduces [`Self::session`]. Session storage is admitted at this width against the
    /// compiler's memory budget.
    pub fn session_with_lane_width(
        &self,
        random_policy: RandomPolicy,
        lane_width: LaneWidth,
    ) -> Result<DetectionSamplingSession, DetectionExecutionError> {
        self.session_with_budget(random_policy, lane_width, self.inner.direct.memory_budget())
    }

    /// Creates a session like [`Self::session_with_lane_width`], admitting its storage against
    /// `memory_budget` instead of the compiler's memory budget.
    pub fn session_with_budget(
        &self,
        random_policy: RandomPolicy,
        lane_width: LaneWidth,
        memory_budget: MemoryBudget,
    ) -> Result<DetectionSamplingSession, DetectionExecutionError> {
        DetectionSamplingSession::new(self.clone(), random_policy, lane_width, memory_budget)
    }
}

struct DirectDetectionState {
    rng: SmallRng,
    frame: LaneFrameBatch,
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
        lane_width: LaneWidth,
        memory_budget: MemoryBudget,
    ) -> Result<Self, DetectionExecutionError> {
        validate_direct_session_storage(&plan.inner.direct, lane_width, memory_budget)?;
        let state = DirectDetectionState {
            rng: detection_rng(random_policy),
            frame: LaneFrameBatch::for_detection(&plan.inner.direct, lane_width)
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
    /// Cancellation, observed before or inside a batch, returns [`DetectionRunStatus::Cancelled`]
    /// with the sink finalized and leaves the session resumable: a batch it interrupts is
    /// discarded with the random stream rewound, so a later run continues the seeded stream
    /// exactly where the delivered shots end.
    pub fn cancellation(&self) -> Cancellation {
        self.state
            .cancellation
            .get_or_init(Cancellation::new)
            .clone()
    }

    /// Whether a sink failure or an internal invariant violation stopped an earlier run partway
    /// through a batch; cancellation never poisons a detection-sampling session.
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Shots this session's frame batches sample together.
    pub const fn lane_width(&self) -> LaneWidth {
        match self.state.frame {
            LaneFrameBatch::Lanes64(_) => LaneWidth::Lanes64,
            LaneFrameBatch::Lanes256(_) => LaneWidth::Lanes256,
        }
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
