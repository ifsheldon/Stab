use stab_engine as engine_sampling;

use crate::{Circuit, CircuitError, CircuitResult};

mod stream;

pub use engine_sampling::{
    PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError, SamplingCancellation,
    SamplingCompileError, SamplingCompileErrorCode, SamplingCompiler, SamplingExecutionError,
    SamplingPlan, SamplingRunProgress, SamplingRunStatus, SamplingRunSummary, SamplingSession,
    Seed, ShotCount, SinkFailurePhase,
};

#[derive(Clone, Debug)]
pub struct CompiledSampler {
    plan: SamplingPlan,
}

impl PartialEq for CompiledSampler {
    fn eq(&self, other: &Self) -> bool {
        self.plan == other.plan
    }
}

impl CompiledSampler {
    pub fn compile(circuit: &Circuit) -> CircuitResult<Self> {
        let plan = SamplingCompiler::new()
            .compile(circuit)
            .map_err(CircuitError::from)?;
        plan.validate_legacy_adapter_storage_for_core()
            .map_err(CircuitError::from)?;
        Ok(Self { plan })
    }

    pub(crate) fn compile_allowing_sweep(circuit: &Circuit) -> CircuitResult<Self> {
        let plan = SamplingCompiler::new()
            .compile_allowing_sweep_for_core(circuit)
            .map_err(CircuitError::from)?;
        plan.validate_legacy_adapter_storage_for_core()
            .map_err(CircuitError::from)?;
        Ok(Self { plan })
    }

    pub const fn plan(&self) -> &SamplingPlan {
        &self.plan
    }

    pub fn into_plan(self) -> SamplingPlan {
        self.plan
    }

    pub fn count_determined_measurements(&self, unknown_input: bool) -> CircuitResult<u64> {
        self.plan
            .try_count_determined_measurements(unknown_input)
            .map_err(CircuitError::from)
    }

    pub fn reference_sample(&self) -> CircuitResult<Vec<bool>> {
        self.plan.try_reference_sample().map_err(CircuitError::from)
    }

    #[cfg(test)]
    pub(crate) fn reference_sample_with_sweep_into(
        &self,
        sweep_record: &[bool],
        output: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        self.plan
            .reference_measurement_record_with_sweep_into_for_core(sweep_record, output)
            .map_err(CircuitError::from)
    }

    #[cfg(test)]
    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        self.plan
            .reference_measurement_record_with_sweep_into_for_core(sweep_record, record)
            .map_err(CircuitError::from)
    }
}

pub fn count_determined_measurements(circuit: &Circuit, unknown_input: bool) -> CircuitResult<u64> {
    SamplingCompiler::new()
        .compile_allowing_sweep_for_core(circuit)
        .map_err(CircuitError::from)?
        .try_count_determined_measurements(unknown_input)
        .map_err(CircuitError::from)
}

pub(crate) fn legacy_random_policy(seed: Option<u64>) -> RandomPolicy {
    seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    })
}

pub(crate) fn legacy_reference_mode(skip_reference_sample: bool) -> ReferenceSampleMode {
    if skip_reference_sample {
        ReferenceSampleMode::SkipReferenceSample
    } else {
        ReferenceSampleMode::UseReferenceSample
    }
}

pub(crate) fn legacy_shot_count(shots: usize) -> CircuitResult<ShotCount> {
    u64::try_from(shots).map(ShotCount::new).map_err(|_| {
        CircuitError::invalid_sampler_compilation(
            "shot count cannot fit in the sampling session counter",
        )
    })
}

pub(crate) fn legacy_execution_error(error: SamplingExecutionError) -> CircuitError {
    CircuitError::from(error)
}

#[cfg(test)]
mod tests;
