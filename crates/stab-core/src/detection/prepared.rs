use super::{
    CompiledDetectionConverter, ConversionPlan, DetectionConversionLimits, ReferenceSampleSource,
};
use crate::{Circuit, CircuitResult, CompiledSampler};

pub(super) struct PreparedDetectionSampling {
    pub(super) converter: CompiledDetectionConverter,
    pub(super) sampler: CompiledSampler,
}

impl PreparedDetectionSampling {
    pub(super) fn compile(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> CircuitResult<Self> {
        let plan = ConversionPlan::from_circuit_with_limits(circuit, limits)?;
        let sampler = CompiledSampler::compile_allowing_sweep(circuit)?;
        let reference_sample = sampler.reference_sample();
        let converter = CompiledDetectionConverter::from_plan_and_reference_sample(
            plan,
            ReferenceSampleSource::Static(reference_sample),
        )?;
        Ok(Self { converter, sampler })
    }
}
