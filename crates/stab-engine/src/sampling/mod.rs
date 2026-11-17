#[cfg(test)]
use rand::Rng;
use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_model::{Circuit, MemoryBudget, ModelDialect};

#[cfg(test)]
use self::execute::{ExecutionBuffers, ExecutionMode, execute_program};
#[cfg(test)]
use self::stabilizer_frame::StabilizerFrame;
use crate::detection::frame::CircuitProgram;
use crate::detection::{DetectionError, DetectionResourceKind};
use crate::{CompilationDescriptor, CompilationOperation, CompilationRequestFingerprint};

mod api;
#[cfg(test)]
mod execute;
mod local_tableau;
#[cfg(test)]
mod noise;
pub(crate) mod pauli_product;
mod reference;
mod reference_bits;
mod reference_fold;
mod reference_order;
mod reference_run;
mod reference_simulator;
#[cfg(test)]
mod stabilizer_frame;

pub use api::{
    PlanFingerprint, RandomPolicy, ReferenceSampleLoopPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler,
    SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase, StreamingSamplingRun,
};
pub(crate) use api::{session_cancellation, session_cancellation_slot};
pub(crate) use local_tableau::LocalTableauTransform;
pub(crate) use reference::{program_reference_sample, reference_work_storage_bytes};
pub(crate) use reference_bits::ReferenceBits;

/// Sampling compiler descriptor consumed by facade capability aggregation.
pub const COMPILATION_DESCRIPTOR: CompilationDescriptor = CompilationDescriptor::new(
    CompilationOperation::Sampling,
    ModelDialect::StimCircuit,
    CompilationRequestFingerprint::SAMPLING_COMPILER_SCHEMA_VERSION,
    Some(CompilationRequestFingerprint::SCHEMA_VERSION),
    true,
);

impl SamplingPlan {
    /// Counts deterministic measurements after admitting and allocating bounded analysis storage.
    pub fn try_count_determined_measurements(
        &self,
        unknown_input: bool,
    ) -> Result<u64, SamplingExecutionError> {
        if self.inner.measurement_count == 0 {
            return Ok(0);
        }
        validate_simulator_work_storage(
            self.inner.qubit_count,
            self.inner.measurement_count,
            false,
            self.inner.memory_budget,
        )?;
        reference_run::count_determined_measurements(
            &self.inner.program,
            self.inner.qubit_count,
            unknown_input,
            &self.inner.cancellation,
        )
    }

    /// Computes the deterministic reference sample with bounded, fallible storage.
    ///
    /// The sample is computed once per plan and reused by later calls and sessions.
    pub fn try_reference_sample(&self) -> Result<Vec<bool>, SamplingExecutionError> {
        self.inner
            .cached_reference_sample()?
            .to_bools("reference sample copy")
    }

    pub(crate) fn sweep_bit_count(&self) -> usize {
        self.inner.sweep_bit_count
    }

    /// Executes one shot on the generator-list stabilizer frame, the differential baseline of
    /// tests. Programs omit zero-probability Pauli channels, so this stream is a test-only one.
    #[cfg(test)]
    fn sample_shot_in_mode_into<R>(
        &self,
        rng: &mut R,
        mode: ExecutionMode,
        sweep_record: &[bool],
        frame: &mut StabilizerFrame,
        record: &mut Vec<bool>,
        output: &mut Vec<bool>,
    ) -> Result<(), SamplingExecutionError>
    where
        R: Rng,
    {
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
        execute_program(&self.inner.program, &mut buffers, rng, mode, sweep_record)
    }
}

/// Bytes of one generator in the generator-list frame that preceded the inverse-tableau
/// reference simulator, kept so that reference snapshot admission decisions do not change.
const REFERENCE_SNAPSHOT_GENERATOR_BYTES: u128 = 96;

/// Admission estimate for a reference repeat-fold snapshot.
///
/// This keeps the generator-list formula used before the inverse-tableau simulator so that
/// admission decisions do not change. It over-estimates the simulator's actual snapshot, which
/// holds `2n` rows of `2 * ceil(n / 64)` words and `2n` sign bytes, plus the transposed X halves
/// used to compare states (`2n` rows of `ceil(n / 64)` words), one gathered word and one phase
/// byte per qubit: at most `0.75 n^2 + 59 n` bytes against the admitted `n^2 + 96 n`.
fn reference_snapshot_admission_bytes(qubit_count: usize) -> u128 {
    (qubit_count as u128)
        .saturating_mul(REFERENCE_SNAPSHOT_GENERATOR_BYTES)
        .saturating_add(
            (qubit_count as u128)
                .saturating_mul(qubit_count as u128)
                .saturating_mul(size_of::<stab_algebra::PauliBasis>() as u128),
        )
}

pub(crate) fn validate_simulator_work_storage(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
    budget: MemoryBudget,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes =
        simulator_work_storage_bytes(qubit_count, measurement_count, include_reference_snapshot);
    if !budget.admits(estimated_bytes) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: budget.bytes(),
        });
    }
    Ok(())
}

/// Conservative scratch storage of one reference-simulator run, including its output record of
/// one bit per measurement.
fn simulator_work_storage_bytes(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
) -> u128 {
    let qubits = qubit_count as u128;
    let base = qubits
        .saturating_mul(qubits)
        .saturating_mul(4)
        .saturating_add(qubits.saturating_mul(256))
        .saturating_add(reference_bits::ReferenceBits::storage_bytes(
            measurement_count,
        ));
    if include_reference_snapshot {
        base.saturating_add(reference_snapshot_admission_bytes(qubit_count))
    } else {
        base
    }
}

fn reference_state_snapshot_fits(
    qubit_count: usize,
    measurement_count: usize,
    budget: MemoryBudget,
) -> bool {
    budget.admits(simulator_work_storage_bytes(
        qubit_count,
        measurement_count,
        true,
    ))
}

/// Converts a program-lowering failure into the sampling compiler's typed error.
fn sampling_compile_error(error: DetectionError) -> SamplingCompileError {
    match error {
        DetectionError::Model(error) => SamplingCompileError::Model(error),
        DetectionError::Analysis(error) => SamplingCompileError::Analysis(error),
        DetectionError::InvalidSamplerCompilation { message }
        | DetectionError::InvalidResultFormat { message } => {
            SamplingCompileError::InvalidCircuit { message }
        }
        DetectionError::ResourceLimit(limit) => match limit.kind() {
            DetectionResourceKind::RepeatNesting => SamplingCompileError::invalid_circuit(format!(
                "repeat nesting exceeds sampler limit {}",
                stab_model::RepeatNestingLimit::HARD_MAX
            )),
            DetectionResourceKind::CompiledBytes => SamplingCompileError::CompiledBytesLimit {
                required: limit.actual(),
                budget: limit.limit(),
            },
        },
        DetectionError::Cancelled(cancelled) => SamplingCompileError::Cancelled(cancelled),
    }
}

/// Error from [`count_determined_measurements`], covering compilation and bounded execution.
#[derive(Debug, thiserror::Error)]
pub enum CountDeterminedMeasurementsError {
    #[error(transparent)]
    Compile(#[from] SamplingCompileError),
    #[error(transparent)]
    Execution(#[from] SamplingExecutionError),
}

/// Error from computing a circuit's deterministic all-zero-sweep reference sample.
#[derive(Debug, thiserror::Error)]
pub enum CircuitReferenceSampleError {
    #[error(transparent)]
    Compile(#[from] SamplingCompileError),
    #[error(transparent)]
    Execution(#[from] SamplingExecutionError),
}

/// Computes a circuit's deterministic reference sample with every sweep bit set to false.
pub fn circuit_reference_sample(
    circuit: &Circuit,
) -> Result<Vec<bool>, CircuitReferenceSampleError> {
    let plan = SamplingCompiler::new().compile(circuit)?;
    let mut sweep_record = api::try_bool_buffer(plan.sweep_bit_count(), "reference sweep record")?;
    sweep_record.resize(plan.sweep_bit_count(), false);
    let mut output = api::try_bool_buffer(
        plan.measurement_width().get(),
        "reference measurement output",
    )?;
    plan.reference_measurement_record_with_sweep_into(&sweep_record, &mut output)?;
    Ok(output)
}

pub fn count_determined_measurements(
    circuit: &Circuit,
    unknown_input: bool,
) -> Result<u64, CountDeterminedMeasurementsError> {
    Ok(SamplingCompiler::new()
        .compile(circuit)?
        .try_count_determined_measurements(unknown_input)?)
}

fn sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}

/// Compiles the one program a sampling plan executes, refusing a program whose retained
/// storage exceeds `budget` before reserving any of it.
fn compile_program(
    circuit: &Circuit,
    budget: MemoryBudget,
) -> Result<CircuitProgram, SamplingCompileError> {
    use crate::detection::frame::ProgramOptions;
    let admission = CircuitProgram::admit(circuit, ProgramOptions::SAMPLING, 0, budget.bytes())
        .map_err(sampling_compile_error)?;
    let counts = admission.counts();
    if counts.measurements == 0 {
        // No shot observes any work, so the executor needs no program.
        return Ok(CircuitProgram::empty(counts));
    }
    CircuitProgram::materialize(circuit, &admission).map_err(sampling_compile_error)
}

#[cfg(test)]
mod tests;
