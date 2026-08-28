use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use rand::SeedableRng as _;
use rand::rngs::SmallRng;

use stab_model::Circuit;
use stab_records::{
    BitPlane64BatchView, DetectionBatchView, DetectionSink, DetectorWidth, MeasurementBatchView,
    MeasurementWidth, ObservableWidth, PackedShotBatch,
};

use super::error::DetectionError;
use super::frame::{DetectorFrameState, DirectDetectorFramePlan};
use super::{
    DetectionConversionLimits, DetectionRecordBuffer, PreparedMeasurementToDetection, try_false_vec,
};
use crate::{RandomPolicy, ReferenceSampleMode, SamplingCancellation, ShotCount, SinkFailurePhase};

mod contracts;
mod delivery;
mod execution;

pub use contracts::{
    DetectionCompileError, DetectionExecutionError, DetectionRunError, DetectionRunProgress,
    DetectionRunStatus, DetectionRunSummary,
};
pub use delivery::MeasurementToDetectionTransaction;
use execution::{
    copy_record, detection_rng, invariant_error, run_direct, storage_error,
    validate_conversion_session_storage, validate_direct_session_storage,
};

const MAX_BATCH_SHOTS: usize = 64;
const MAX_DETECTION_SESSION_STORAGE_BYTES: u64 = 256 * 1024 * 1024;

/// Builder for immutable measurement-to-detection conversion plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub const fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub const fn reference_sample_mode(mut self, mode: ReferenceSampleMode) -> Self {
        self.reference_mode = mode;
        self
    }

    pub fn compile(
        self,
        circuit: &Circuit,
    ) -> Result<MeasurementToDetectionPlan, DetectionCompileError> {
        let converter = PreparedMeasurementToDetection::compile_with_limits(
            circuit,
            self.reference_mode,
            self.limits,
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

struct DetectionBatchBuffers {
    detectors: PackedShotBatch,
    observables: PackedShotBatch,
}

impl DetectionBatchBuffers {
    fn new(
        detector_width: DetectorWidth,
        observable_width: ObservableWidth,
    ) -> Result<Self, DetectionExecutionError> {
        Ok(Self {
            detectors: PackedShotBatch::zeros(MAX_BATCH_SHOTS, detector_width.get())
                .map_err(storage_error)?,
            observables: PackedShotBatch::zeros(MAX_BATCH_SHOTS, observable_width.get())
                .map_err(storage_error)?,
        })
    }

    fn view(&self, shot_count: usize) -> Result<DetectionBatchView<'_>, DetectionExecutionError> {
        let detectors = self
            .detectors
            .view_prefix(shot_count)
            .map_err(invariant_error)?;
        let observables = self
            .observables
            .view_prefix(shot_count)
            .map_err(invariant_error)?;
        DetectionBatchView::try_new(detectors, observables).map_err(invariant_error)
    }

    fn copy_range_from(
        &mut self,
        source: &Self,
        source_start: usize,
        shot_count: usize,
    ) -> Result<(), DetectionExecutionError> {
        copy_packed_range(
            &mut self.detectors,
            &source.detectors,
            source_start,
            shot_count,
        )?;
        copy_packed_range(
            &mut self.observables,
            &source.observables,
            source_start,
            shot_count,
        )
    }

    fn copy_from_planes(
        &mut self,
        detector_planes: &[u64],
        observable_planes: &[u64],
    ) -> Result<(), DetectionExecutionError> {
        copy_planes_into_packed(&mut self.detectors, detector_planes)?;
        copy_planes_into_packed(&mut self.observables, observable_planes)
    }
}

fn copy_planes_into_packed(
    target: &mut PackedShotBatch,
    planes: &[u64],
) -> Result<(), DetectionExecutionError> {
    if target.shot_count() != MAX_BATCH_SHOTS || target.bits_per_shot() != planes.len() {
        return Err(DetectionExecutionError::InternalInvariant {
            message: "detection output planes disagree with packed batch dimensions".to_owned(),
        });
    }
    let source = BitPlane64BatchView::try_from_words(planes, MAX_BATCH_SHOTS, planes.len())
        .map_err(invariant_error)?;
    target.copy_from_bit_planes(source).map_err(invariant_error)
}

fn copy_packed_range(
    target: &mut PackedShotBatch,
    source: &PackedShotBatch,
    source_start: usize,
    shot_count: usize,
) -> Result<(), DetectionExecutionError> {
    if target.bits_per_shot() != source.bits_per_shot()
        || shot_count > target.shot_count()
        || source_start
            .checked_add(shot_count)
            .is_none_or(|end| end > source.shot_count())
    {
        return Err(DetectionExecutionError::InternalInvariant {
            message: "pending detection batch dimensions disagree".to_owned(),
        });
    }
    for target_shot in 0..shot_count {
        let source_shot = source_start.checked_add(target_shot).ok_or_else(|| {
            DetectionExecutionError::InternalInvariant {
                message: "pending detection shot index overflowed".to_owned(),
            }
        })?;
        for bit_index in 0..source.bits_per_shot() {
            let bit = source.get(source_shot, bit_index).ok_or_else(|| {
                DetectionExecutionError::InternalInvariant {
                    message: "pending detection source escaped its dimensions".to_owned(),
                }
            })?;
            target
                .set(target_shot, bit_index, bit)
                .map_err(invariant_error)?;
        }
    }
    Ok(())
}

/// Mutable reusable state for measurement-to-detection conversion.
pub struct MeasurementToDetectionSession {
    plan: MeasurementToDetectionPlan,
    reference_sample: Vec<bool>,
    sweep_correction: Option<DetectorFrameState>,
    correction_rng: SmallRng,
    measurement_record: Vec<bool>,
    sweep_record: Vec<bool>,
    detection_record: DetectionRecordBuffer,
    batch: DetectionBatchBuffers,
    cancellation: OnceLock<SamplingCancellation>,
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
                &self
                    .cancellation
                    .get()
                    .is_some_and(SamplingCancellation::is_cancelled),
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
        Ok(Self {
            reference_sample: converter
                .try_reusable_reference_sample()
                .map_err(DetectionExecutionError::Conversion)?,
            sweep_correction: converter
                .try_reusable_sweep_correction()
                .map_err(DetectionExecutionError::Conversion)?,
            correction_rng: SmallRng::seed_from_u64(0),
            measurement_record: try_false_vec(
                converter.measurement_count(),
                "measurement-to-detection input record",
            )
            .map_err(DetectionExecutionError::Conversion)?,
            sweep_record: try_false_vec(
                converter.sweep_bit_count(),
                "measurement-to-detection sweep record",
            )
            .map_err(DetectionExecutionError::Conversion)?,
            detection_record: converter
                .try_reusable_detection_record()
                .map_err(DetectionExecutionError::Conversion)?,
            batch: DetectionBatchBuffers::new(plan.detector_width(), plan.observable_width())?,
            plan,
            cancellation: OnceLock::new(),
            total_committed_shots: 0,
            poisoned: false,
            transaction_active: false,
            not_sync: PhantomData,
        })
    }

    pub fn cancellation(&self) -> SamplingCancellation {
        self.cancellation
            .get_or_init(SamplingCancellation::default)
            .clone()
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
        if self
            .cancellation
            .get()
            .is_some_and(SamplingCancellation::is_cancelled)
        {
            return Ok(self.summary(DetectionRunStatus::Cancelled, shots, 0));
        }
        let shot_count = usize::try_from(shots.get()).map_err(|_| DetectionRunError::Engine {
            source: DetectionExecutionError::InternalInvariant {
                message: "bounded conversion batch did not fit usize".to_owned(),
            },
            progress: DetectionRunProgress::new(committed_shots, shots.get()),
        })?;
        if let Err(source) = self.fill_batch(measurements, sweeps, shot_count) {
            self.poisoned = true;
            return Err(DetectionRunError::Engine {
                source,
                progress: DetectionRunProgress::new(committed_shots, shots.get()),
            });
        }
        let batch = match self.batch.view(shot_count) {
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
        for shot_index in 0..shot_count {
            copy_record(
                measurements,
                shot_index,
                &mut self.measurement_record,
                "measurement",
            )?;
            if let Some(sweeps) = sweeps {
                copy_record(sweeps, shot_index, &mut self.sweep_record, "sweep")?;
            } else {
                self.sweep_record.fill(false);
            }
            converter
                .convert_record_with_sweep_into(
                    &self.measurement_record,
                    &self.sweep_record,
                    &mut self.reference_sample,
                    &mut self.detection_record,
                    self.sweep_correction.as_mut(),
                    &mut self.correction_rng,
                )
                .map_err(DetectionExecutionError::Conversion)?;
            self.batch
                .detectors
                .copy_shot_from_bools(shot_index, &self.detection_record.detectors)
                .map_err(invariant_error)?;
            self.batch
                .observables
                .copy_shot_from_bools(shot_index, &self.detection_record.observables)
                .map_err(invariant_error)?;
        }
        Ok(())
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

/// Builder for immutable circuit detection-sampling plans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    pub const fn limits(mut self, limits: DetectionConversionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn compile(
        self,
        circuit: &Circuit,
    ) -> Result<DetectionSamplingPlan, DetectionCompileError> {
        let direct = DirectDetectorFramePlan::compile(circuit, self.limits)?;
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
    frame: DetectorFrameState,
    batch: DetectionBatchBuffers,
    delivery: DetectionBatchBuffers,
    pending_start: usize,
    pending_count: usize,
    cancellation: OnceLock<SamplingCancellation>,
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
            batch: DetectionBatchBuffers::new(plan.detector_width(), plan.observable_width())?,
            delivery: DetectionBatchBuffers::new(plan.detector_width(), plan.observable_width())?,
            pending_start: 0,
            pending_count: 0,
            cancellation: OnceLock::new(),
        };
        Ok(Self {
            plan,
            state,
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    pub fn cancellation(&self) -> SamplingCancellation {
        self.state
            .cancellation
            .get_or_init(SamplingCancellation::default)
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
