#[cfg(test)]
use rand::Rng;
use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_model::{Circuit, ModelDialect};

#[cfg(test)]
use self::execute::{ExecutionBuffers, ExecutionMode, execute_program};
#[cfg(test)]
use self::stabilizer_frame::StabilizerFrame;
use crate::detection::frame::CircuitProgram;
use crate::detection::{DetectionError, DetectionResourceKind};
use crate::{CompilationDescriptor, CompilationOperation, CompilationRequestFingerprint};

mod api;
mod execute;
mod noise;
pub(crate) mod pauli_product;
mod reference;
mod reference_run;
mod reference_simulator;
mod stabilizer_frame;

pub use api::{
    PlanFingerprint, RandomPolicy, ReferenceSampleLoopPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
};
pub(crate) use reference::{program_reference_sample, reference_work_storage_bytes};
pub(crate) use stabilizer_frame::LocalTableauTransform;

/// Sampling compiler descriptor consumed by facade capability aggregation.
pub const COMPILATION_DESCRIPTOR: CompilationDescriptor = CompilationDescriptor::new(
    CompilationOperation::Sampling,
    ModelDialect::StimCircuit,
    CompilationRequestFingerprint::SAMPLING_COMPILER_SCHEMA_VERSION,
    Some(CompilationRequestFingerprint::SCHEMA_VERSION),
    false,
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
        validate_general_frame_work_storage(
            self.inner.qubit_count,
            self.inner.measurement_count,
            false,
        )?;
        reference_run::count_determined_measurements(
            &self.inner.program,
            self.inner.qubit_count,
            unknown_input,
        )
    }

    /// Computes the deterministic reference sample with bounded, fallible storage.
    ///
    /// The sample is computed once per plan and reused by later calls and sessions.
    pub fn try_reference_sample(&self) -> Result<Vec<bool>, SamplingExecutionError> {
        let cached = self.inner.cached_reference_sample()?;
        let mut sample = api::try_bool_buffer(cached.len(), "reference sample copy")?;
        sample.extend_from_slice(cached);
        Ok(sample)
    }

    pub(crate) fn sweep_bit_count(&self) -> usize {
        self.inner.sweep_bit_count
    }

    /// Executes one shot on the general stabilizer frame, for differential tests.
    ///
    /// Programs selected for the bit-plane Pauli frame omit zero-probability Pauli channels, so
    /// on circuits containing them this stream differs from a general-frame plan's.
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

pub(crate) fn validate_general_frame_work_storage(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
) -> Result<(), SamplingExecutionError> {
    let estimated_bytes = general_frame_work_storage_bytes(
        qubit_count,
        measurement_count,
        include_reference_snapshot,
    );
    if estimated_bytes > u128::from(api::MAX_SAMPLING_SESSION_STORAGE_BYTES) {
        return Err(SamplingExecutionError::SessionStorageLimit {
            estimated_bytes,
            limit_bytes: api::MAX_SAMPLING_SESSION_STORAGE_BYTES,
        });
    }
    Ok(())
}

fn general_frame_work_storage_bytes(
    qubit_count: usize,
    measurement_count: usize,
    include_reference_snapshot: bool,
) -> u128 {
    let qubits = qubit_count as u128;
    let measurements = measurement_count as u128;
    let base = qubits
        .saturating_mul(qubits)
        .saturating_mul(4)
        .saturating_add(qubits.saturating_mul(256))
        .saturating_add(measurements);
    if include_reference_snapshot {
        base.saturating_add(stabilizer_frame::reference_snapshot_admission_bytes(
            qubit_count,
        ))
    } else {
        base
    }
}

fn reference_state_snapshot_fits(
    qubit_count: usize,
    measurement_count: usize,
    limit_bytes: u64,
) -> bool {
    general_frame_work_storage_bytes(qubit_count, measurement_count, true)
        <= u128::from(limit_bytes)
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
        DetectionError::ResourceLimit(limit)
            if limit.kind() == DetectionResourceKind::RepeatNesting =>
        {
            SamplingCompileError::invalid_circuit(format!(
                "repeat nesting exceeds sampler limit {}",
                stab_model::RepeatNestingLimit::HARD_MAX
            ))
        }
        DetectionError::ResourceLimit(limit) => {
            SamplingCompileError::invalid_circuit(limit.to_string())
        }
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

/// Compiles the one program a sampling plan executes.
fn compile_program(
    circuit: &Circuit,
) -> Result<(CircuitProgram, api::SamplingPlanKind), SamplingCompileError> {
    use crate::detection::frame::ProgramOptions;
    let admission = CircuitProgram::admit(circuit, ProgramOptions::SAMPLING, 0, u64::MAX)
        .map_err(sampling_compile_error)?;
    let counts = admission.counts();
    let limit = u128::from(api::MAX_EXPANDED_OPERATIONS_PER_SHOT);
    if counts.measurements > 0 && counts.expanded_instructions > limit {
        return Err(SamplingCompileError::ExpandedOperationLimit {
            actual: crate::ResourceAmount::from_u128(counts.expanded_instructions),
            limit: api::MAX_EXPANDED_OPERATIONS_PER_SHOT,
        });
    }
    let fits_frame = admission.retained_bytes() <= api::MAX_SAMPLING_SESSION_STORAGE_BYTES;
    let kind = if fits_frame {
        api::SamplingPlanKind::PauliFrame
    } else {
        api::SamplingPlanKind::GeneralFrame
    };
    if counts.measurements == 0 {
        // No shot observes any work, so neither executor needs the program.
        return Ok((CircuitProgram::empty(counts), kind));
    }
    if fits_frame {
        let program =
            CircuitProgram::materialize(circuit, &admission).map_err(sampling_compile_error)?;
        return Ok((program, kind));
    }
    // A program above the frame budget falls back to the general stabilizer frame, which keeps
    // its historical per-target work bound.
    if counts.expanded_target_operations > limit {
        return Err(SamplingCompileError::ExpandedOperationLimit {
            actual: crate::ResourceAmount::from_u128(counts.expanded_target_operations),
            limit: api::MAX_EXPANDED_OPERATIONS_PER_SHOT,
        });
    }
    let program = CircuitProgram::compile(circuit, ProgramOptions::GENERAL_SAMPLING)
        .map_err(sampling_compile_error)?;
    Ok((program, kind))
}

#[cfg(test)]
mod tests;
