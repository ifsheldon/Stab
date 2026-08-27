use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use sha2::{Digest as _, Sha256};
use stab_model::Circuit;
use stab_records::{
    BitPlane64Batch, BitPlane64BatchView, MeasurementBatchView, MeasurementSink, MeasurementWidth,
};
use thiserror::Error;

use super::direct_z_measurement::DirectZMeasurementPlan;
use super::execute::{ExecutionBuffers, execute_operations};
use super::operation::SampleOperation;
use super::small_frame::SmallStabilizerFrame;
use super::stabilizer_frame::StabilizerFrame;
use super::{ExecutionMode, compile_circuit, direct_z_measurement, sampler_rng, small_frame};
use crate::CompilationRequestFingerprint;

const MAX_BATCH_SHOTS: usize = 64;
pub(super) const MAX_SAMPLING_SESSION_STORAGE_BYTES: u64 = 256 * 1024 * 1024;
const PLAN_FINGERPRINT_DOMAIN: &[u8] = b"stab:plan-fingerprint\0";
const EXECUTABLE_CONTRACT_DOMAIN: &[u8] = b"stab:sampling-executable-contract\0";

/// Backend selected for an immutable sampling plan.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SamplingBackend {
    Scalar,
}

impl SamplingBackend {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "scalar",
        }
    }

    const fn discriminator(self) -> u8 {
        match self {
            Self::Scalar => 1,
        }
    }
}

/// Stable code classifying a sampling compilation failure.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SamplingCompileErrorCode {
    InvalidCircuit,
}

impl SamplingCompileErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidCircuit => "invalid-circuit",
        }
    }
}

/// Failure to compile an immutable sampling plan.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SamplingCompileError {
    #[error(transparent)]
    Model(#[from] stab_model::ModelError),

    #[error(transparent)]
    Analysis(#[from] stab_analysis::AnalysisError),

    #[error("cannot compile circuit sampler: {message}")]
    InvalidCircuit { message: String },
}

impl SamplingCompileError {
    pub(crate) fn invalid_circuit(message: impl Into<String>) -> Self {
        Self::InvalidCircuit {
            message: message.into(),
        }
    }

    pub const fn code(&self) -> SamplingCompileErrorCode {
        SamplingCompileErrorCode::InvalidCircuit
    }
}

/// Seed used to initialize one sampling session.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Seed(u64);

impl Seed {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Number of shots requested from a sampling session.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ShotCount(u64);

impl ShotCount {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl TryFrom<usize> for ShotCount {
    type Error = SamplingExecutionError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        u64::try_from(value)
            .map(Self::new)
            .map_err(|_| SamplingExecutionError::ShotCounterOverflow)
    }
}

/// Policy used to initialize a session's random stream.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RandomPolicy {
    Entropy,
    Seeded(Seed),
}

impl RandomPolicy {
    /// Returns the deterministic seed, or `None` when entropy should initialize the stream.
    pub const fn seed(self) -> Option<Seed> {
        match self {
            Self::Entropy => None,
            Self::Seeded(seed) => Some(seed),
        }
    }
}

/// Whether sampling uses Stim's reference sample.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ReferenceSampleMode {
    /// Preserve ordinary reference-sample behavior.
    #[default]
    UseReferenceSample,
    /// Skip the reference sample and report frame changes relative to it.
    SkipReferenceSample,
}

/// Builder for immutable sampling plans.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SamplingCompiler;

impl SamplingCompiler {
    pub const fn new() -> Self {
        Self
    }

    pub fn compile(self, circuit: &Circuit) -> Result<SamplingPlan, SamplingCompileError> {
        let backend = SamplingBackend::Scalar;
        let request_fingerprint = CompilationRequestFingerprint::for_sampling(circuit);
        let mut operations = Vec::new();
        let counts = compile_circuit(circuit, &mut operations)?;
        let kind = select_plan_kind(circuit.count_qubits(), counts.measurements, &operations);
        let fingerprint = PlanFingerprint::for_sampling(
            request_fingerprint,
            backend,
            kind.executable_discriminator(),
            SamplingPlan::EXECUTABLE_CONTRACT_SCHEMA_VERSION,
        );
        Ok(SamplingPlan {
            inner: Arc::new(SamplingPlanInner {
                qubit_count: circuit.count_qubits(),
                measurement_count: counts.measurements,
                sweep_bit_count: counts.sweep_bits,
                operations,
                kind,
                backend,
                request_fingerprint,
                fingerprint,
            }),
        })
    }
}

fn select_plan_kind(
    qubit_count: usize,
    measurement_count: usize,
    operations: &[SampleOperation],
) -> SamplingPlanKind {
    if let Some(plan) = direct_z_measurement::compile(operations, measurement_count) {
        return SamplingPlanKind::DirectZ(plan);
    }
    if qubit_count <= SmallStabilizerFrame::MAX_QUBITS
        && small_frame::supports_operations(operations)
    {
        return SamplingPlanKind::SmallFrame;
    }
    SamplingPlanKind::GeneralFrame
}

/// Backend-bearing identity of one immutable compiled plan.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PlanFingerprint {
    schema_version: u16,
    request_fingerprint: CompilationRequestFingerprint,
    backend: SamplingBackend,
    executable_contract_schema_version: u16,
    executable_contract_digest: [u8; 32],
    digest: [u8; 32],
}

impl PlanFingerprint {
    pub const SCHEMA_VERSION: u16 = 1;
    pub const ALGORITHM: &'static str = "sha256";

    fn for_sampling(
        request_fingerprint: CompilationRequestFingerprint,
        backend: SamplingBackend,
        executable_discriminator: u8,
        executable_contract_schema_version: u16,
    ) -> Self {
        let executable_contract_digest = executable_contract_digest(
            backend,
            executable_discriminator,
            executable_contract_schema_version,
        );
        let mut hasher = Sha256::new();
        hasher.update(PLAN_FINGERPRINT_DOMAIN);
        hasher.update(Self::SCHEMA_VERSION.to_be_bytes());
        hasher.update(request_fingerprint.schema_version().to_be_bytes());
        hasher.update(request_fingerprint.digest());
        hasher.update([backend.discriminator()]);
        hasher.update(executable_contract_schema_version.to_be_bytes());
        hasher.update(executable_contract_digest);
        Self {
            schema_version: Self::SCHEMA_VERSION,
            request_fingerprint,
            backend,
            executable_contract_schema_version,
            executable_contract_digest,
            digest: hasher.finalize().into(),
        }
    }

    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub const fn request_fingerprint(self) -> CompilationRequestFingerprint {
        self.request_fingerprint
    }

    pub const fn backend(self) -> SamplingBackend {
        self.backend
    }

    pub const fn executable_contract_schema_version(self) -> u16 {
        self.executable_contract_schema_version
    }

    pub const fn executable_contract_digest(self) -> [u8; 32] {
        self.executable_contract_digest
    }

    pub fn executable_contract_digest_hex(self) -> String {
        hex::encode(self.executable_contract_digest)
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }

    pub fn digest_hex(self) -> String {
        hex::encode(self.digest)
    }
}

fn executable_contract_digest(
    backend: SamplingBackend,
    executable_discriminator: u8,
    schema_version: u16,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(EXECUTABLE_CONTRACT_DOMAIN);
    hasher.update(schema_version.to_be_bytes());
    hasher.update([backend.discriminator(), executable_discriminator]);
    hasher.finalize().into()
}

/// Immutable, shareable sampling plan.
#[derive(Clone)]
pub struct SamplingPlan {
    pub(super) inner: Arc<SamplingPlanInner>,
}

impl PartialEq for SamplingPlan {
    fn eq(&self, other: &Self) -> bool {
        self.inner.qubit_count == other.inner.qubit_count
            && self.inner.measurement_count == other.inner.measurement_count
            && self.inner.sweep_bit_count == other.inner.sweep_bit_count
            && self.inner.operations == other.inner.operations
    }
}

impl fmt::Debug for SamplingPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SamplingPlan")
            .field("backend", &self.backend())
            .field("qubit_count", &self.qubit_count())
            .field("measurement_width", &self.measurement_width())
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl SamplingPlan {
    pub const EXECUTABLE_CONTRACT_SCHEMA_VERSION: u16 = 2;

    pub fn backend(&self) -> SamplingBackend {
        self.inner.backend
    }

    pub fn request_fingerprint(&self) -> CompilationRequestFingerprint {
        self.inner.request_fingerprint
    }

    pub fn fingerprint(&self) -> PlanFingerprint {
        self.inner.fingerprint
    }

    pub fn measurement_width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.inner.measurement_count)
    }

    pub fn qubit_count(&self) -> usize {
        self.inner.qubit_count
    }

    #[inline]
    pub fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<SamplingSession, SamplingExecutionError> {
        self.session_with_reference_mode(random_policy, ReferenceSampleMode::UseReferenceSample)
    }

    #[inline]
    pub fn session_with_reference_mode(
        &self,
        random_policy: RandomPolicy,
        reference_mode: ReferenceSampleMode,
    ) -> Result<SamplingSession, SamplingExecutionError> {
        SamplingSession::new(self.clone(), random_policy, reference_mode)
    }

    pub(crate) fn estimated_session_storage_bytes(
        &self,
        reference_mode: ReferenceSampleMode,
    ) -> u128 {
        session_storage_bytes(&self.inner, reference_mode)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SamplingPlanInner {
    pub(super) qubit_count: usize,
    pub(super) measurement_count: usize,
    pub(super) sweep_bit_count: usize,
    pub(super) operations: Vec<SampleOperation>,
    pub(super) kind: SamplingPlanKind,
    backend: SamplingBackend,
    request_fingerprint: CompilationRequestFingerprint,
    fingerprint: PlanFingerprint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SamplingPlanKind {
    DirectZ(DirectZMeasurementPlan),
    SmallFrame,
    GeneralFrame,
}

impl SamplingPlanKind {
    const fn executable_discriminator(self) -> u8 {
        match self {
            Self::DirectZ(_) => 1,
            Self::SmallFrame => 2,
            Self::GeneralFrame => 3,
        }
    }
}

// The bounded 6 KiB small frame stays inline so session construction has no
// infallible heap allocation after its explicit storage admission check.
#[allow(
    clippy::large_enum_variant,
    reason = "the bounded inline frame avoids infallible heap allocation after admission"
)]
#[derive(Debug)]
enum SessionFrame {
    DirectZ,
    Small(SmallStabilizerFrame),
    General(StabilizerFrame),
}

#[derive(Debug)]
enum SessionBatch {
    DirectZ([u64; 1]),
    BitPlanes(BitPlane64Batch),
}

/// Cooperative cancellation state checked between completed bounded sampling batches.
///
/// One expensive shot may delay the next check, so this does not promise a wall-clock deadline.
#[derive(Clone, Debug, Default)]
pub struct SamplingCancellation {
    cancelled: Arc<AtomicBool>,
}

impl SamplingCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    #[inline(always)]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Engine-side sampling execution failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SamplingExecutionError {
    #[error("sampling session is poisoned")]
    SessionPoisoned,

    #[error("sampling session shot counter overflowed")]
    ShotCounterOverflow,

    #[error(
        "sampling session needs an estimated {estimated_bytes} bytes of bounded storage, exceeding the {limit_bytes}-byte safety limit"
    )]
    SessionStorageLimit {
        estimated_bytes: u128,
        limit_bytes: u64,
    },

    #[error("sampling session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error("count_determined_measurements unhandled measurement type {gate}")]
    UnsupportedDeterminedMeasurementGate { gate: &'static str },

    #[error("sweep record expected {expected} bits, got {actual}")]
    InvalidSweepRecordWidth { expected: usize, actual: usize },

    #[error("sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },
}

/// Sink operation that failed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SinkFailurePhase {
    WriteBatch,
    Finish,
}

impl SinkFailurePhase {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WriteBatch => "write-batch",
            Self::Finish => "finish",
        }
    }
}

/// Exact progress at an execution failure.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SamplingRunProgress {
    committed_shots: ShotCount,
    attempted_batch_shots: ShotCount,
}

impl SamplingRunProgress {
    pub(super) const fn new(committed_shots: u64, attempted_batch_shots: u64) -> Self {
        Self {
            committed_shots: ShotCount::new(committed_shots),
            attempted_batch_shots: ShotCount::new(attempted_batch_shots),
        }
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn attempted_batch_shots(self) -> ShotCount {
        self.attempted_batch_shots
    }
}

/// Non-lossy composition of engine and sink failures.
#[derive(Debug)]
pub enum RunError<SinkError> {
    Engine {
        source: SamplingExecutionError,
        progress: SamplingRunProgress,
    },
    Sink {
        phase: SinkFailurePhase,
        source: SinkError,
        progress: SamplingRunProgress,
    },
}

impl<SinkError> RunError<SinkError> {
    pub const fn progress(&self) -> SamplingRunProgress {
        match self {
            Self::Engine { progress, .. } | Self::Sink { progress, .. } => *progress,
        }
    }
}

impl<SinkError: fmt::Display> fmt::Display for RunError<SinkError> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Engine { source, progress } => write!(
                formatter,
                "{source} after {} committed shots",
                progress.committed_shots().get()
            ),
            Self::Sink {
                phase,
                source,
                progress,
            } => write!(
                formatter,
                "sampling sink {} failed after {} committed shots while attempting {} shots: {source}",
                phase.as_str(),
                progress.committed_shots().get(),
                progress.attempted_batch_shots().get()
            ),
        }
    }
}

impl<SinkError> std::error::Error for RunError<SinkError>
where
    SinkError: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine { source, .. } => Some(source),
            Self::Sink { source, .. } => Some(source),
        }
    }
}

/// Completion state of one sampling call.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SamplingRunStatus {
    Completed,
    Cancelled,
}

/// Summary of one completed or cooperatively cancelled call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SamplingRunSummary {
    status: SamplingRunStatus,
    requested_shots: ShotCount,
    committed_shots: ShotCount,
    total_committed_shots: ShotCount,
}

impl SamplingRunSummary {
    pub const fn status(self) -> SamplingRunStatus {
        self.status
    }

    pub const fn requested_shots(self) -> ShotCount {
        self.requested_shots
    }

    pub const fn committed_shots(self) -> ShotCount {
        self.committed_shots
    }

    pub const fn total_committed_shots(self) -> ShotCount {
        self.total_committed_shots
    }
}

/// Mutable reusable execution state for one immutable sampling plan.
pub struct SamplingSession {
    plan: SamplingPlan,
    rng: SmallRng,
    frame: SessionFrame,
    reference_sample: Option<Vec<bool>>,
    record: Vec<bool>,
    output: Vec<bool>,
    batch: SessionBatch,
    cancellation: OnceLock<Arc<AtomicBool>>,
    total_committed_shots: u64,
    poisoned: bool,
    not_sync: PhantomData<Cell<()>>,
}

impl fmt::Debug for SamplingSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SamplingSession")
            .field("plan", &self.plan)
            .field(
                "cancelled",
                &self
                    .cancellation
                    .get()
                    .is_some_and(|cancelled| cancelled.load(Ordering::Acquire)),
            )
            .field("total_committed_shots", &self.total_committed_shots)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl SamplingSession {
    fn new(
        plan: SamplingPlan,
        random_policy: RandomPolicy,
        reference_mode: ReferenceSampleMode,
    ) -> Result<Self, SamplingExecutionError> {
        validate_session_storage(&plan.inner, reference_mode)?;
        let rng = match random_policy {
            RandomPolicy::Entropy => sampler_rng(None),
            RandomPolicy::Seeded(seed) => sampler_rng(Some(seed.get())),
        };
        let reference_sample = match reference_mode {
            ReferenceSampleMode::UseReferenceSample => None,
            ReferenceSampleMode::SkipReferenceSample => {
                Some(compute_reference_sample(&plan.inner)?)
            }
        };
        let frame = match plan.inner.kind {
            SamplingPlanKind::DirectZ(_) => SessionFrame::DirectZ,
            SamplingPlanKind::SmallFrame => SessionFrame::Small(
                SmallStabilizerFrame::try_new(plan.inner.qubit_count).map_err(|error| {
                    SamplingExecutionError::SessionStorageAllocation {
                        message: error.to_string(),
                    }
                })?,
            ),
            SamplingPlanKind::GeneralFrame => {
                SessionFrame::General(StabilizerFrame::try_new(plan.inner.qubit_count).map_err(
                    |error| SamplingExecutionError::SessionStorageAllocation {
                        message: error.to_string(),
                    },
                )?)
            }
        };
        let batch = if matches!(plan.inner.kind, SamplingPlanKind::DirectZ(_)) {
            SessionBatch::DirectZ([0])
        } else {
            SessionBatch::BitPlanes(
                BitPlane64Batch::zeros(MAX_BATCH_SHOTS, plan.inner.measurement_count).map_err(
                    |source| SamplingExecutionError::SessionStorageAllocation {
                        message: source.to_string(),
                    },
                )?,
            )
        };
        let (record, output) = if matches!(plan.inner.kind, SamplingPlanKind::DirectZ(_)) {
            (Vec::new(), Vec::new())
        } else {
            (
                try_bool_buffer(plan.inner.measurement_count, "measurement record")?,
                try_bool_buffer(plan.inner.measurement_count, "measurement output")?,
            )
        };
        Ok(Self {
            record,
            output,
            plan,
            rng,
            frame,
            reference_sample,
            batch,
            cancellation: OnceLock::new(),
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    pub fn cancellation(&self) -> SamplingCancellation {
        SamplingCancellation {
            cancelled: Arc::clone(
                self.cancellation
                    .get_or_init(|| Arc::new(AtomicBool::new(false))),
            ),
        }
    }

    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub const fn total_committed_shots(&self) -> ShotCount {
        ShotCount::new(self.total_committed_shots)
    }

    /// Executes one logical run.
    ///
    /// A sink is owned by this call's output lifecycle. Sinks whose `finish` method is terminal,
    /// including the built-in codec sinks, must not be reused by a later call. To partition one
    /// seeded stream across calls, use a fresh sink for each call and concatenate or otherwise
    /// compose the completed outputs according to that sink's format.
    ///
    /// Successful and cooperatively cancelled nonempty requests finalize the sink. A sink-write or
    /// engine failure stops immediately without finalization because the sink may already contain
    /// an unknown partial prefix.
    ///
    /// A zero-shot request neither calls nor finalizes the sink.
    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<SamplingRunSummary, RunError<Sink::Error>>
    where
        Sink: MeasurementSink,
    {
        if self.poisoned {
            return Err(RunError::Engine {
                source: SamplingExecutionError::SessionPoisoned,
                progress: SamplingRunProgress::new(0, 0),
            });
        }
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(RunError::Engine {
                source: SamplingExecutionError::ShotCounterOverflow,
                progress: SamplingRunProgress::new(0, 0),
            });
        }
        if shots.get() == 0 {
            return Ok(self.summary(SamplingRunStatus::Completed, shots, 0));
        }

        let mut remaining = shots.get();
        let mut committed = 0_u64;
        while remaining > 0 {
            if self.is_cancelled() {
                break;
            }
            let batch_shots_u64 = remaining.min(MAX_BATCH_SHOTS as u64);
            let batch_shots = usize::try_from(batch_shots_u64).map_err(|_| RunError::Engine {
                source: SamplingExecutionError::InternalInvariant {
                    message: "bounded batch shot count did not fit usize".to_owned(),
                },
                progress: SamplingRunProgress::new(committed, batch_shots_u64),
            })?;
            if let Err(source) = self.fill_batch(batch_shots) {
                self.poisoned = true;
                return Err(RunError::Engine {
                    source,
                    progress: SamplingRunProgress::new(committed, batch_shots_u64),
                });
            }
            let bit_planes = match &self.batch {
                SessionBatch::DirectZ(words) => {
                    BitPlane64BatchView::try_from_words(words, batch_shots, 1)
                }
                SessionBatch::BitPlanes(batch) => batch.view_prefix(batch_shots),
            };
            let bit_planes = match bit_planes {
                Ok(bit_planes) => bit_planes,
                Err(source) => {
                    self.poisoned = true;
                    return Err(RunError::Engine {
                        source: SamplingExecutionError::InternalInvariant {
                            message: source.to_string(),
                        },
                        progress: SamplingRunProgress::new(committed, batch_shots_u64),
                    });
                }
            };
            let batch = MeasurementBatchView::from_bit_planes(bit_planes);
            if let Err(source) = sink.write_batch(batch) {
                self.poisoned = true;
                return Err(RunError::Sink {
                    phase: SinkFailurePhase::WriteBatch,
                    source,
                    progress: SamplingRunProgress::new(committed, batch_shots_u64),
                });
            }
            committed += batch_shots_u64;
            self.total_committed_shots += batch_shots_u64;
            remaining -= batch_shots_u64;
        }

        if let Err(source) = sink.finish() {
            self.poisoned = true;
            return Err(RunError::Sink {
                phase: SinkFailurePhase::Finish,
                source,
                progress: SamplingRunProgress::new(committed, 0),
            });
        }
        let status = if remaining == 0 {
            SamplingRunStatus::Completed
        } else {
            SamplingRunStatus::Cancelled
        };
        Ok(self.summary(status, shots, committed))
    }

    fn fill_batch(&mut self, shot_count: usize) -> Result<(), SamplingExecutionError> {
        if let SamplingPlanKind::DirectZ(plan) = self.plan.inner.kind {
            let reference_bit = self
                .reference_sample
                .as_deref()
                .and_then(|sample| sample.first())
                .copied()
                .unwrap_or(false);
            let mut plane = 0_u64;
            for shot_index in 0..shot_count {
                let bit = plan.sample(&mut self.rng) ^ reference_bit;
                if bit {
                    plane |= 1_u64 << shot_index;
                }
            }
            return match &mut self.batch {
                SessionBatch::DirectZ(words) => {
                    let word = words.first_mut().ok_or_else(|| {
                        SamplingExecutionError::InternalInvariant {
                            message: "direct batch storage omitted its required plane".to_owned(),
                        }
                    })?;
                    *word = plane;
                    Ok(())
                }
                SessionBatch::BitPlanes(_) => Err(SamplingExecutionError::InternalInvariant {
                    message: "direct sampling plan did not own direct batch storage".to_owned(),
                }),
            };
        }
        for shot_index in 0..shot_count {
            self.sample_shot();
            if self.output.len() != self.plan.inner.measurement_count {
                return Err(SamplingExecutionError::InternalInvariant {
                    message: format!(
                        "sampler produced {} bits for declared width {}",
                        self.output.len(),
                        self.plan.inner.measurement_count
                    ),
                });
            }
            match &mut self.batch {
                SessionBatch::DirectZ(_) => {
                    return Err(SamplingExecutionError::InternalInvariant {
                        message: "frame sampling plan owned direct batch storage".to_owned(),
                    });
                }
                SessionBatch::BitPlanes(batch) => {
                    batch
                        .copy_shot_from_bools(shot_index, &self.output)
                        .map_err(|source| SamplingExecutionError::InternalInvariant {
                            message: source.to_string(),
                        })?;
                }
            }
        }
        Ok(())
    }

    #[inline(always)]
    fn is_cancelled(&self) -> bool {
        self.cancellation
            .get()
            .is_some_and(|cancelled| cancelled.load(Ordering::Acquire))
    }

    fn sample_shot(&mut self) {
        let operations = &self.plan.inner.operations;
        let reference = self.reference_sample.as_deref();
        match (&self.plan.inner.kind, &mut self.frame) {
            (SamplingPlanKind::DirectZ(plan), SessionFrame::DirectZ) => {
                self.record.clear();
                self.output.clear();
                let mut bit = plan.sample(&mut self.rng);
                if let Some(reference_bit) = reference.and_then(|sample| sample.first()) {
                    bit ^= *reference_bit;
                }
                self.record.push(bit);
                self.output.push(bit);
            }
            (SamplingPlanKind::SmallFrame, SessionFrame::Small(frame)) => {
                small_frame::sample_shot_into(
                    operations,
                    frame,
                    &mut self.record,
                    &mut self.output,
                    reference,
                    &mut self.rng,
                );
            }
            (SamplingPlanKind::GeneralFrame, SessionFrame::General(frame)) => {
                sample_general_into(
                    operations,
                    frame,
                    &mut self.record,
                    &mut self.output,
                    reference,
                    &mut self.rng,
                );
            }
            _ => {
                self.output.clear();
            }
        }
    }

    const fn summary(
        &self,
        status: SamplingRunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
    ) -> SamplingRunSummary {
        SamplingRunSummary {
            status,
            requested_shots,
            committed_shots: ShotCount::new(committed_shots),
            total_committed_shots: ShotCount::new(self.total_committed_shots),
        }
    }
}

fn validate_session_storage(
    plan: &SamplingPlanInner,
    reference_mode: ReferenceSampleMode,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes = session_storage_bytes(plan, reference_mode);
    if estimated_bytes > u128::from(MAX_SAMPLING_SESSION_STORAGE_BYTES) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: MAX_SAMPLING_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

fn session_storage_bytes(plan: &SamplingPlanInner, reference_mode: ReferenceSampleMode) -> u128 {
    let measurements = plan.measurement_count as u128;
    let mut estimated_bytes = measurements.saturating_mul(size_of::<u64>() as u128);
    if !matches!(plan.kind, SamplingPlanKind::DirectZ(_)) {
        estimated_bytes = estimated_bytes.saturating_add(measurements.saturating_mul(2));
    }
    if reference_mode == ReferenceSampleMode::SkipReferenceSample {
        estimated_bytes = estimated_bytes.saturating_add(measurements);
    }
    if matches!(plan.kind, SamplingPlanKind::GeneralFrame)
        || (reference_mode == ReferenceSampleMode::SkipReferenceSample
            && !matches!(plan.kind, SamplingPlanKind::DirectZ(_)))
    {
        let qubits = plan.qubit_count as u128;
        estimated_bytes = estimated_bytes
            .saturating_add(qubits.saturating_mul(qubits).saturating_mul(4))
            .saturating_add(qubits.saturating_mul(256));
    }
    estimated_bytes
}

pub(super) fn try_bool_buffer(
    capacity: usize,
    label: &'static str,
) -> Result<Vec<bool>, SamplingExecutionError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(capacity).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: format!("{label} capacity {capacity}: {error}"),
        }
    })?;
    Ok(buffer)
}

pub(super) fn compute_reference_sample(
    plan: &SamplingPlanInner,
) -> Result<Vec<bool>, SamplingExecutionError> {
    if let SamplingPlanKind::DirectZ(direct) = plan.kind {
        let mut output = try_bool_buffer(1, "direct reference sample")?;
        output.push(direct.reference_bit());
        return Ok(output);
    }
    super::validate_general_frame_work_storage(plan.qubit_count, plan.measurement_count)?;
    let mut rng = SmallRng::seed_from_u64(0);
    let mut frame = StabilizerFrame::try_new(plan.qubit_count).map_err(|error| {
        SamplingExecutionError::SessionStorageAllocation {
            message: error.to_string(),
        }
    })?;
    let mut record = try_bool_buffer(plan.measurement_count, "reference measurement record")?;
    let mut output = try_bool_buffer(plan.measurement_count, "reference measurement output")?;
    frame.reset_to_z_basis();
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame: &mut frame,
        record: &mut record,
        output: &mut output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    execute_operations(
        &plan.operations,
        &mut buffers,
        &mut rng,
        ExecutionMode::ReferenceSample,
        &[],
    );
    Ok(output)
}

fn sample_general_into(
    operations: &[SampleOperation],
    frame: &mut StabilizerFrame,
    record: &mut Vec<bool>,
    output: &mut Vec<bool>,
    reference: Option<&[bool]>,
    rng: &mut impl rand::Rng,
) {
    frame.reset_to_z_basis();
    record.clear();
    output.clear();
    let mut correlated_error_occurred = false;
    let mut buffers = ExecutionBuffers {
        frame,
        record,
        output,
        correlated_error_occurred: &mut correlated_error_occurred,
    };
    execute_operations(operations, &mut buffers, rng, ExecutionMode::Sample, &[]);
    if let Some(reference) = reference {
        for (bit, reference_bit) in output.iter_mut().zip(reference) {
            *bit ^= *reference_bit;
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "sampling API unit tests use compact fixture setup"
)]
mod tests;
