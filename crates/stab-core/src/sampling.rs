use stab_engine::sampling as engine_sampling;

use crate::{Circuit, CircuitError, CircuitResult};

mod stream;

#[doc(hidden)]
pub(crate) use engine_sampling::ReferenceSampleScratch;
pub use engine_sampling::{
    BackendPreference, PlanFingerprint, RandomPolicy, ReferenceSampleMode, RunError,
    SamplingBackend, SamplingCancellation, SamplingCompileError, SamplingCompileErrorCode,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingRunProgress, SamplingRunStatus,
    SamplingRunSummary, SamplingSession, Seed, ShotCount, SinkFailurePhase,
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
            .compile_allowing_sweep_for_core_detection(circuit)
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

    pub fn count_determined_measurements(&self, unknown_input: bool) -> u64 {
        self.plan.count_determined_measurements(unknown_input)
    }

    pub fn reference_sample(&self) -> Vec<bool> {
        self.plan.reference_sample()
    }

    pub(crate) fn sweep_bit_count(&self) -> usize {
        self.plan.sweep_bit_count_for_core_detection()
    }

    #[cfg(test)]
    pub(crate) fn reference_sample_with_sweep_into(
        &self,
        sweep_record: &[bool],
        output: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        self.plan
            .reference_measurement_record_with_sweep_into_for_core_detection(sweep_record, output)
            .map_err(CircuitError::from)
    }

    pub(crate) fn reference_measurement_record_with_sweep_into(
        &self,
        sweep_record: &[bool],
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        self.plan
            .reference_measurement_record_with_sweep_into_for_core_detection(sweep_record, record)
            .map_err(CircuitError::from)
    }

    pub(crate) fn try_reusable_reference_sample_scratch(
        &self,
    ) -> Result<ReferenceSampleScratch, SamplingExecutionError> {
        self.plan
            .try_reusable_reference_sample_scratch_for_core_detection()
    }

    pub(crate) fn reference_measurement_record_with_sweep_and_scratch_into(
        &self,
        sweep_record: &[bool],
        scratch: &mut ReferenceSampleScratch,
        record: &mut Vec<bool>,
    ) -> CircuitResult<()> {
        self.plan
            .reference_measurement_record_with_sweep_and_scratch_into_for_core_detection(
                sweep_record,
                scratch,
                record,
            )
            .map_err(CircuitError::from)
    }
}

pub fn count_determined_measurements(circuit: &Circuit, unknown_input: bool) -> CircuitResult<u64> {
    engine_sampling::count_determined_measurements(circuit, unknown_input)
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

pub(crate) mod pauli_product {
    use crate::{CircuitError, CircuitResult, PauliBasis};

    pub(crate) fn normalize_terms(
        raw_terms: Vec<(usize, PauliBasis, bool)>,
        base_inverted: bool,
    ) -> CircuitResult<(Vec<(usize, PauliBasis)>, bool)> {
        stab_engine::sampling::normalize_pauli_product_terms_for_core_detection(
            raw_terms,
            base_inverted,
        )
        .map_err(CircuitError::from)
    }
}

#[cfg(test)]
mod tests;
