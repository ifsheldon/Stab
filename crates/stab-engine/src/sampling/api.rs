use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, OnceLock};

use rand::rngs::SmallRng;
use sha2::{Digest as _, Sha256};
use stab_model::{Cancellation, Circuit, MemoryBudget, MemoryBudgetExceeded};
use stab_records::{MeasurementSink, MeasurementWidth};
use thiserror::Error;

use super::reference_sample::ReferenceSample;
use super::{compile_program, sampler_rng};
use crate::detection::frame::{
    CircuitProgram, DeliveryFailure, FrameBatchState, FrameLanes, LaneFrameBatch, PendingLanes,
    ShotRecordTransposer, WindowShape,
};
use crate::work_poll::WorkPoll;
use crate::{
    CompilationRequestFingerprint, LaneWidth, RunError, RunProgress, RunStatus, RunSummary,
    SinkFailurePhase,
};

mod compile_error;
mod delivery;
mod session_helpers;

use delivery::{
    SamplingBatchSource, SamplingDeliveryError, StreamingDelivery, StreamingFrame, deliver_lanes,
};

pub use compile_error::SamplingCompileError;
#[cfg(test)]
pub(super) use session_helpers::compute_stabilizer_frame_reference_sample;
pub(super) use session_helpers::{compute_reference_sample, validate_reference_width};
use session_helpers::{validate_session_storage, validate_streaming_storage};

/// Executable discriminator of the bit-plane Pauli frame, the only sampling executor.
const PAULI_FRAME_EXECUTABLE_DISCRIMINATOR: u8 = 4;
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
///
/// The default draws the seed from entropy, as Stim does without `--seed`.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum RandomPolicy {
    #[default]
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

/// The state the qubits start in when counting determined measurements.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum InitialState {
    /// Every qubit starts in `|0>`, as in an ordinary run.
    #[default]
    Zero,
    /// Every qubit starts maximally entangled with a private reference qubit, so only the
    /// circuit's own measurements and resets can determine a measurement.
    Unknown,
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

/// Options of one sampling session or streaming run; see [`SamplingPlan::session`].
///
/// The default draws its seed from entropy, uses the reference sample, samples 256 lanes per
/// batch, and admits its storage against the plan's memory budget.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SamplingSessionOptions {
    /// How the session's random stream is initialized.
    pub random: RandomPolicy,
    /// Whether output is relative to the reference sample.
    pub reference_sample_mode: ReferenceSampleMode,
    /// Shots each frame batch samples together. The seeded stream is a function of the plan,
    /// the seed, and this width.
    pub lane_width: LaneWidth,
    /// Budget for the session's frame and reference-sample storage; `None` admits it against
    /// the plan's [`SamplingCompiler::memory_budget`].
    pub memory_budget: Option<MemoryBudget>,
}

/// Reference-sample execution policy for compact repeat blocks.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ReferenceSampleLoopPolicy {
    /// Reuse a measured repeat body when its quantum state and control-independent output recur.
    #[default]
    Fold,
    /// Execute every represented repeat iteration while computing the reference sample.
    Iterate,
}

impl ReferenceSampleLoopPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fold => "fold",
            Self::Iterate => "iterate",
        }
    }

    const fn discriminator(self) -> u8 {
        match self {
            Self::Fold => 1,
            Self::Iterate => 2,
        }
    }
}

/// Options of an immutable sampling plan, and the compiler that builds it.
///
/// The memory budget bounds the compiled program, every session's reusable storage unless the
/// session's options override it, and the reference-simulator scratch, each admitted before it
/// is allocated.
///
/// The cancellation signal is polled while the plan's reference sample is computed and by every
/// session of the plan, which also returns it from [`SamplingSession::cancellation`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SamplingCompiler {
    /// How the reference sample executes compact repeat blocks.
    pub reference_sample_loop_policy: ReferenceSampleLoopPolicy,
    /// Budget for the compiled program and, by default, for each session's storage.
    pub memory_budget: MemoryBudget,
    /// Signal polled by the plan and its sessions.
    pub cancellation: Cancellation,
}

impl SamplingCompiler {
    pub fn compile(&self, circuit: &Circuit) -> Result<SamplingPlan, SamplingCompileError> {
        self.cancellation.check()?;
        let program = compile_program(circuit, self.memory_budget)?;
        let counts = program.counts();
        // Admission already walked every instruction, so the simulated qubit count comes from
        // that walk instead of a second pass over the circuit.
        let qubit_count = counts.simulated_qubits;
        Ok(SamplingPlan {
            inner: Arc::new(SamplingPlanInner {
                qubit_count,
                measurement_count: counts.measurements,
                sweep_bit_count: counts.sweep_bits,
                program,
                memory_budget: self.memory_budget,
                cancellation: self.cancellation.clone(),
                reference_sample_loop_policy: self.reference_sample_loop_policy,
                reference_sample: OnceLock::new(),
                backend: SAMPLING_BACKEND,
            }),
        })
    }

    /// Returns the backend-neutral identity of compiling `circuit` with this compiler.
    ///
    /// It hashes the circuit's [`stab_model::ModelFingerprint`], so its cost grows with the
    /// circuit and nothing computes it unless a caller asks.
    pub fn request_fingerprint(&self, circuit: &Circuit) -> CompilationRequestFingerprint {
        CompilationRequestFingerprint::for_sampling(circuit)
    }

    /// Returns the identity of the plan that [`Self::compile`] returns for `circuit`.
    ///
    /// A plan does not retain its source circuit, so its identity is computed on demand from
    /// the request it was compiled from: this compiler's options, its fixed backend selection,
    /// and `circuit`. The value is the same whether it is computed before or after compiling,
    /// and it identifies a plan only when compilation succeeds. It hashes the circuit's model
    /// fingerprint, so nothing pays for it unless a caller asks.
    pub fn plan_fingerprint(&self, circuit: &Circuit) -> PlanFingerprint {
        PlanFingerprint::for_sampling(
            self.request_fingerprint(circuit),
            SAMPLING_BACKEND,
            PAULI_FRAME_EXECUTABLE_DISCRIMINATOR,
            self.reference_sample_loop_policy,
            SamplingPlan::EXECUTABLE_CONTRACT_SCHEMA_VERSION,
        )
    }
}

/// The one backend every sampling plan selects.
const SAMPLING_BACKEND: SamplingBackend = SamplingBackend::Scalar;

/// The plan-cached reference sample a session XORs into its output, if it uses one.
fn session_reference(
    plan: &SamplingPlan,
    uses_reference_sample: bool,
) -> Result<Option<&ReferenceSample>, SamplingExecutionError> {
    if !uses_reference_sample {
        return Ok(None);
    }
    plan.inner.reference_sample.get().map(Some).ok_or_else(|| {
        SamplingExecutionError::InternalInvariant {
            message: "sampling session lost its plan-cached reference sample".to_owned(),
        }
    })
}

/// Backend-bearing identity of one immutable compiled plan.
///
/// [`SamplingCompiler::plan_fingerprint`] computes it on demand from the compiler and the
/// source circuit, because a compiled plan does not retain its source.
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
        reference_sample_loop_policy: ReferenceSampleLoopPolicy,
        executable_contract_schema_version: u16,
    ) -> Self {
        let executable_contract_digest = executable_contract_digest(
            backend,
            executable_discriminator,
            reference_sample_loop_policy,
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
    reference_sample_loop_policy: ReferenceSampleLoopPolicy,
    schema_version: u16,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(EXECUTABLE_CONTRACT_DOMAIN);
    hasher.update(schema_version.to_be_bytes());
    hasher.update([
        backend.discriminator(),
        executable_discriminator,
        reference_sample_loop_policy.discriminator(),
    ]);
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
            && self.inner.program == other.inner.program
            && self.inner.reference_sample_loop_policy == other.inner.reference_sample_loop_policy
    }
}

impl fmt::Debug for SamplingPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SamplingPlan")
            .field("backend", &self.backend())
            .field("qubit_count", &self.qubit_count())
            .field("measurement_width", &self.measurement_width())
            .field(
                "reference_sample_loop_policy",
                &self.reference_sample_loop_policy(),
            )
            .finish_non_exhaustive()
    }
}

impl SamplingPlan {
    pub const EXECUTABLE_CONTRACT_SCHEMA_VERSION: u16 = 7;

    pub fn backend(&self) -> SamplingBackend {
        self.inner.backend
    }

    pub fn reference_sample_loop_policy(&self) -> ReferenceSampleLoopPolicy {
        self.inner.reference_sample_loop_policy
    }

    pub fn measurement_width(&self) -> MeasurementWidth {
        MeasurementWidth::new(self.inner.measurement_count)
    }

    pub fn qubit_count(&self) -> usize {
        self.inner.qubit_count
    }

    /// Conservative heap bytes retained by this compiled plan's lowered program.
    pub fn retained_bytes(&self) -> u64 {
        self.inner.program.retained_bytes()
    }

    /// Creates a reusable session.
    ///
    /// Session storage is admitted at the options' lane width against the options' memory
    /// budget, or the plan's when the options leave it unset.
    pub fn session(
        &self,
        options: &SamplingSessionOptions,
    ) -> Result<SamplingSession, SamplingExecutionError> {
        SamplingSession::new(self.clone(), options, None)
    }

    /// Prepares one run of exactly `shots` shots that streams wide records.
    ///
    /// The run's output equals the first [`SamplingSession::run`] of `shots` shots on a
    /// [`Self::session`] with the same options. Instead of materializing one plane per
    /// measurement for every lane of a batch, it hands the planes over in bounded windows and
    /// keeps shot-major records of only the lanes it delivers, so its storage is admitted for
    /// `min(shots, lane_width)` records rather than `lane_width` planes per measurement. Because
    /// it keeps no undelivered lanes, it runs once.
    pub fn streaming_run(
        &self,
        options: &SamplingSessionOptions,
        shots: ShotCount,
    ) -> Result<StreamingSamplingRun, SamplingExecutionError> {
        let session = SamplingSession::new(self.clone(), options, Some(shots))?;
        Ok(StreamingSamplingRun { session, shots })
    }
}

/// One prepared streaming sampling run; see [`SamplingPlan::streaming_run`].
#[derive(Debug)]
pub struct StreamingSamplingRun {
    session: SamplingSession,
    shots: ShotCount,
}

impl StreamingSamplingRun {
    /// The signal the run polls between and inside batches, as for
    /// [`SamplingSession::cancellation`].
    pub fn cancellation(&self) -> Cancellation {
        self.session.cancellation()
    }

    /// Shots each frame batch of the run samples together.
    pub const fn lane_width(&self) -> LaneWidth {
        self.session.lane_width()
    }

    /// Shots the run delivers.
    pub const fn shots(&self) -> ShotCount {
        self.shots
    }

    /// Executes the run with the sink lifecycle of [`SamplingSession::run`].
    ///
    /// Cooperative cancellation, observed before or inside a batch, ends the run with
    /// [`RunStatus::Cancelled`], the complete batches delivered, and the sink finalized.
    /// The run consumed itself, so nothing can resume it; a fresh run restarts the seeded stream.
    pub fn run<Sink>(
        mut self,
        sink: &mut Sink,
    ) -> Result<RunSummary, RunError<SamplingExecutionError, Sink::Error>>
    where
        Sink: MeasurementSink,
    {
        self.session.run(self.shots, sink)
    }
}

#[derive(Clone, Debug)]
pub(super) struct SamplingPlanInner {
    pub(super) qubit_count: usize,
    pub(super) measurement_count: usize,
    pub(super) sweep_bit_count: usize,
    pub(super) program: CircuitProgram,
    pub(super) memory_budget: MemoryBudget,
    pub(super) cancellation: Cancellation,
    pub(super) reference_sample_loop_policy: ReferenceSampleLoopPolicy,
    /// The all-zero-sweep reference sample, packed one bit per measurement and computed at most
    /// once per successful plan.
    reference_sample: OnceLock<ReferenceSample>,
    backend: SamplingBackend,
}

impl SamplingPlanInner {
    /// Returns the plan's reference sample, computing it on first use.
    ///
    /// Failures are returned to the caller and not cached, so a later call retries.
    pub(super) fn cached_reference_sample(
        &self,
    ) -> Result<&ReferenceSample, SamplingExecutionError> {
        self.cached_reference_sample_with_budget(self.memory_budget)
    }

    pub(super) fn cached_reference_sample_with_budget(
        &self,
        memory_budget: MemoryBudget,
    ) -> Result<&ReferenceSample, SamplingExecutionError> {
        if let Some(sample) = self.reference_sample.get() {
            return Ok(sample);
        }
        let sample = compute_reference_sample(self, memory_budget)?;
        Ok(self.reference_sample.get_or_init(|| sample))
    }

    pub(super) fn has_reference_state_snapshot_candidate(&self) -> bool {
        self.reference_sample_loop_policy == ReferenceSampleLoopPolicy::Fold
            && super::reference_run::has_reference_fold_candidate(&self.program, self.qubit_count)
    }

    #[cfg(test)]
    pub(super) fn uses_reference_state_snapshot(&self) -> bool {
        self.uses_reference_state_snapshot_with_budget(self.memory_budget)
    }

    pub(super) fn uses_reference_state_snapshot_with_budget(
        &self,
        memory_budget: MemoryBudget,
    ) -> bool {
        self.has_reference_state_snapshot_candidate()
            && super::reference_state_snapshot_fits(
                self.qubit_count,
                self.measurement_count,
                memory_budget,
            )
    }
}

#[derive(Debug)]
enum SessionFrame {
    /// A plan without measurements delivers zero-width batches of this width.
    Empty(LaneWidth),
    Pauli(LaneFrameBatch),
    Streaming64(StreamingFrame<u64>),
    Streaming256(StreamingFrame<FrameLanes>),
}

impl SessionFrame {
    /// Reserves one streaming run's storage, zeroing its records in chunks that poll
    /// `cancellation`.
    fn streaming(
        program: &CircuitProgram,
        lane_width: LaneWidth,
        shots: ShotCount,
        cancellation: &Cancellation,
    ) -> Result<Self, SamplingExecutionError> {
        let measurements = program.counts().measurements;
        let lanes = usize::try_from(shots.get()).map_or(lane_width.shots_per_batch(), |shots| {
            shots.min(lane_width.shots_per_batch())
        });
        let shape = WindowShape::for_program(program, lane_width.plane_bytes(), 0);
        let allocation = |error: crate::detection::DetectionError| match error {
            crate::detection::DetectionError::Cancelled(cancelled) => {
                SamplingExecutionError::Cancelled(cancelled)
            }
            error => SamplingExecutionError::SessionStorageAllocation {
                message: error.to_string(),
            },
        };
        let records =
            ShotRecordTransposer::try_new(lanes, measurements, &mut WorkPoll::new(cancellation))
                .map_err(allocation)?;
        Ok(match lane_width {
            LaneWidth::Lanes64 => Self::Streaming64(StreamingFrame {
                state: FrameBatchState::for_measurement_window(program, shape.capacity())
                    .map_err(allocation)?,
                shape,
                records,
            }),
            LaneWidth::Lanes256 => Self::Streaming256(StreamingFrame {
                state: FrameBatchState::for_measurement_window(program, shape.capacity())
                    .map_err(allocation)?,
                shape,
                records,
            }),
        })
    }
}

/// Signal used by sessions whose plan has no cancellable signal and that never handed one out.
static NEVER_CANCELLED: Cancellation = Cancellation::never();

/// The cancellation a session polls: its plan's signal when that one is cancellable, otherwise
/// the session-local signal created by its first `cancellation()` call, if any.
pub(crate) fn session_cancellation(slot: &OnceLock<Cancellation>) -> &Cancellation {
    slot.get().unwrap_or(&NEVER_CANCELLED)
}

/// A session's cancellation slot, holding `plan_signal` when it is cancellable.
pub(crate) fn session_cancellation_slot(plan_signal: &Cancellation) -> OnceLock<Cancellation> {
    if plan_signal.is_cancellable() {
        OnceLock::from(plan_signal.clone())
    } else {
        OnceLock::new()
    }
}

/// Engine-side sampling execution failure.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SamplingExecutionError {
    #[error("sampling session is poisoned")]
    SessionPoisoned,

    #[error("sampling session shot counter overflowed")]
    ShotCounterOverflow,

    /// The memory budget refused the session's bounded storage before any of it was reserved.
    #[error(
        "sampling session needs an estimated {} bytes of bounded storage, exceeding the {}-byte memory budget; raise the memory budget to admit it",
        exceeded.required(),
        exceeded.budget().bytes()
    )]
    SessionStorageLimit { exceeded: MemoryBudgetExceeded },

    #[error("sampling session could not allocate bounded storage: {message}")]
    SessionStorageAllocation { message: String },

    #[error("count_determined_measurements unhandled measurement type {gate}")]
    UnsupportedDeterminedMeasurementGate { gate: &'static str },

    #[error("sweep record expected {expected} bits, got {actual}")]
    InvalidSweepRecordWidth { expected: usize, actual: usize },

    #[error("sampling execution violated an internal batch invariant: {message}")]
    InternalInvariant { message: String },

    /// Cancellation was observed while creating a session: while computing the reference
    /// sample, which creating a session that uses it does first and the plan does not cache, or
    /// while zeroing a streaming run's records; a later call starts over.
    #[error(transparent)]
    Cancelled(#[from] stab_model::Cancelled),
}

impl SamplingExecutionError {
    /// The memory-budget refusal this error reports, if it is one.
    pub fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::SessionStorageLimit { exceeded } => Some(exceeded),
            _ => None,
        }
    }
}

/// Mutable reusable execution state for one immutable sampling plan.
pub struct SamplingSession {
    plan: SamplingPlan,
    rng: SmallRng,
    frame: SessionFrame,
    pending: PendingLanes,
    uses_reference_sample: bool,
    cancellation: OnceLock<Cancellation>,
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
                &session_cancellation(&self.cancellation).is_cancelled(),
            )
            .field("total_committed_shots", &self.total_committed_shots)
            .field("poisoned", &self.poisoned)
            .finish_non_exhaustive()
    }
}

impl SamplingSession {
    /// A materializing session, or the session of one streaming run of `streaming_shots` shots.
    fn new(
        plan: SamplingPlan,
        options: &SamplingSessionOptions,
        streaming_shots: Option<ShotCount>,
    ) -> Result<Self, SamplingExecutionError> {
        let SamplingSessionOptions {
            random: random_policy,
            reference_sample_mode: reference_mode,
            lane_width,
            memory_budget,
        } = *options;
        let memory_budget = memory_budget.unwrap_or(plan.inner.memory_budget);
        match streaming_shots {
            None => {
                validate_session_storage(&plan.inner, reference_mode, lane_width, memory_budget)?
            }
            Some(shots) => {
                validate_streaming_storage(
                    &plan.inner,
                    reference_mode,
                    lane_width,
                    shots,
                    memory_budget,
                )?;
            }
        }
        let rng = match random_policy {
            RandomPolicy::Entropy => sampler_rng(None),
            RandomPolicy::Seeded(seed) => sampler_rng(Some(seed.get())),
        };
        let uses_reference_sample = reference_mode == ReferenceSampleMode::UseReferenceSample;
        if uses_reference_sample {
            plan.inner
                .cached_reference_sample_with_budget(memory_budget)?;
        }
        let frame = if plan.inner.measurement_count == 0 {
            SessionFrame::Empty(lane_width)
        } else if let Some(shots) = streaming_shots {
            SessionFrame::streaming(
                &plan.inner.program,
                lane_width,
                shots,
                &plan.inner.cancellation,
            )?
        } else {
            SessionFrame::Pauli(
                LaneFrameBatch::for_measurements(&plan.inner.program, lane_width).map_err(
                    |error| SamplingExecutionError::SessionStorageAllocation {
                        message: error.to_string(),
                    },
                )?,
            )
        };
        let cancellation = session_cancellation_slot(&plan.inner.cancellation);
        Ok(Self {
            plan,
            rng,
            frame,
            pending: PendingLanes::default(),
            uses_reference_sample,
            cancellation,
            total_committed_shots: 0,
            poisoned: false,
            not_sync: PhantomData,
        })
    }

    /// The signal this session polls between and inside batches.
    ///
    /// It is the plan's signal when [`SamplingCompiler::cancellation`] is a cancellable one,
    /// and otherwise a session-local signal created by the first call.
    pub fn cancellation(&self) -> Cancellation {
        self.cancellation.get_or_init(Cancellation::new).clone()
    }

    /// Whether a sink failure or an internal invariant violation stopped an earlier run partway
    /// through a batch, after which every run fails with [`SamplingExecutionError::SessionPoisoned`].
    /// Cancellation never poisons a session.
    pub const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Shots this session's frame batches sample together.
    pub const fn lane_width(&self) -> LaneWidth {
        match &self.frame {
            SessionFrame::Empty(lane_width) => *lane_width,
            SessionFrame::Pauli(LaneFrameBatch::Lanes64(_)) | SessionFrame::Streaming64(_) => {
                LaneWidth::Lanes64
            }
            SessionFrame::Pauli(LaneFrameBatch::Lanes256(_)) | SessionFrame::Streaming256(_) => {
                LaneWidth::Lanes256
            }
        }
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
    /// Cancellation returns [`RunStatus::Cancelled`] with the shots delivered so far and
    /// the sink finalized, and leaves the session resumable: wherever it is observed, before or
    /// inside a batch, the sink holds only complete batches, and a batch it interrupts is
    /// discarded with the random stream rewound to where that batch began. A later run on the
    /// same session (after [`Cancellation::reset`]) therefore continues the seeded stream exactly
    /// where the delivered shots end, as if the cancelled run had asked for only those shots.
    ///
    /// A zero-shot request neither calls nor finalizes the sink.
    pub fn run<Sink>(
        &mut self,
        shots: ShotCount,
        sink: &mut Sink,
    ) -> Result<RunSummary, RunError<SamplingExecutionError, Sink::Error>>
    where
        Sink: MeasurementSink,
    {
        if self.poisoned {
            return Err(RunError::Engine {
                source: SamplingExecutionError::SessionPoisoned,
                progress: RunProgress::new(0, 0),
            });
        }
        if self
            .total_committed_shots
            .checked_add(shots.get())
            .is_none()
        {
            return Err(RunError::Engine {
                source: SamplingExecutionError::ShotCounterOverflow,
                progress: RunProgress::new(0, 0),
            });
        }
        if shots.get() == 0 {
            return Ok(self.summary(RunStatus::Completed, shots, 0));
        }

        let reference = match session_reference(&self.plan, self.uses_reference_sample) {
            Ok(reference) => reference,
            Err(source) => {
                self.poisoned = true;
                return Err(RunError::Engine {
                    source,
                    progress: RunProgress::new(0, 0),
                });
            }
        };
        let program = &self.plan.inner.program;
        let rng = &mut self.rng;
        let cancellation = session_cancellation(&self.cancellation);
        let pending = &mut self.pending;
        let delivered = match &mut self.frame {
            SessionFrame::Empty(LaneWidth::Lanes64) => deliver_lanes::<u64, _>(
                pending,
                shots,
                SamplingBatchSource::new(program, None, reference, rng, cancellation, sink),
            ),
            SessionFrame::Empty(LaneWidth::Lanes256) => deliver_lanes::<FrameLanes, _>(
                pending,
                shots,
                SamplingBatchSource::new(program, None, reference, rng, cancellation, sink),
            ),
            SessionFrame::Pauli(LaneFrameBatch::Lanes64(state)) => deliver_lanes(
                pending,
                shots,
                SamplingBatchSource::new(program, Some(state), reference, rng, cancellation, sink),
            ),
            SessionFrame::Pauli(LaneFrameBatch::Lanes256(state)) => deliver_lanes(
                pending,
                shots,
                SamplingBatchSource::new(program, Some(state), reference, rng, cancellation, sink),
            ),
            SessionFrame::Streaming64(frame) => StreamingDelivery {
                program,
                frame,
                reference,
                rng,
                cancellation,
                sink,
            }
            .deliver(shots),
            SessionFrame::Streaming256(frame) => StreamingDelivery {
                program,
                frame,
                reference,
                rng,
                cancellation,
                sink,
            }
            .deliver(shots),
        };
        let outcome = match delivered {
            Ok(outcome) => outcome,
            Err(DeliveryFailure {
                error,
                committed,
                attempted,
            }) => {
                self.poisoned = true;
                self.total_committed_shots += committed;
                let progress = RunProgress::new(committed, attempted);
                return Err(match error {
                    SamplingDeliveryError::Engine(source) => RunError::Engine { source, progress },
                    SamplingDeliveryError::Sink(source) => RunError::Sink {
                        phase: SinkFailurePhase::WriteBatch,
                        source,
                        progress,
                    },
                });
            }
        };
        let committed = outcome.committed;
        self.total_committed_shots += committed;

        if let Err(source) = sink.finish() {
            self.poisoned = true;
            return Err(RunError::Sink {
                phase: SinkFailurePhase::Finish,
                source,
                progress: RunProgress::new(committed, 0),
            });
        }
        let status = if outcome.completed {
            RunStatus::Completed
        } else {
            RunStatus::Cancelled
        };
        Ok(self.summary(status, shots, committed))
    }

    const fn summary(
        &self,
        status: RunStatus,
        requested_shots: ShotCount,
        committed_shots: u64,
    ) -> RunSummary {
        RunSummary::new(
            status,
            requested_shots,
            committed_shots,
            self.total_committed_shots,
        )
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "sampling API unit tests use compact fixture setup"
)]
mod tests;
