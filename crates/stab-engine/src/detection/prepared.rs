use super::error::DetectionResult;
use super::{
    CompiledDetectionConverter, ConversionPlan, DetectionConversionLimits, ReferenceSampleSource,
};
use crate::{SamplingCompiler, SamplingPlan};
use stab_model::Circuit;

pub(super) struct PreparedDetectionSampling {
    pub(super) converter: CompiledDetectionConverter,
    pub(super) sampling: SamplingPlan,
}

impl PreparedDetectionSampling {
    pub(super) fn compile(
        circuit: &Circuit,
        limits: DetectionConversionLimits,
    ) -> DetectionResult<Self> {
        let plan = ConversionPlan::from_circuit_with_limits(circuit, limits)?;
        let sampling = SamplingCompiler::new().compile_allowing_sweep(circuit)?;
        let reference_sample = sampling.try_reference_sample()?;
        let converter = CompiledDetectionConverter::from_plan_and_reference_sample(
            plan,
            ReferenceSampleSource::Static(reference_sample),
        )?;
        Ok(Self {
            converter,
            sampling,
        })
    }
}
