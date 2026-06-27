use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemTarget,
    DetectionConversionOutput, DetectionEventRecord, DetectorErrorModel,
};

const MAX_DEM_SAMPLER_REPEAT_UNROLL: u64 = 100_000;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDemSampler {
    detector_count: usize,
    observable_count: usize,
    operations: Vec<DemSampleOperation>,
}

impl CompiledDemSampler {
    pub fn compile(model: &DetectorErrorModel) -> CircuitResult<Self> {
        let detector_count = usize_from_u64(model.count_detectors()?, "detector count")?;
        let observable_count = usize_from_u64(model.count_observables()?, "observable count")?;
        let mut operations = Vec::new();
        compile_items(model.items(), 0, &mut operations)?;
        Ok(Self {
            detector_count,
            observable_count,
            operations,
        })
    }

    pub fn sample_detection_events(
        &self,
        shots: usize,
    ) -> CircuitResult<DetectionConversionOutput> {
        self.sample_detection_events_with_seed(shots, None)
    }

    pub fn sample_detection_events_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<DetectionConversionOutput> {
        let mut rng = dem_sampler_rng(seed);
        let records = (0..shots)
            .map(|_| self.sample_record(&mut rng))
            .collect::<CircuitResult<Vec<_>>>()?;
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.detector_count,
            observable_count: self.observable_count,
        })
    }

    fn sample_record<R>(&self, rng: &mut R) -> CircuitResult<DetectionEventRecord>
    where
        R: Rng,
    {
        let mut detectors = vec![false; self.detector_count];
        let mut observables = vec![false; self.observable_count];
        for operation in &self.operations {
            if !operation.sample_occurs(rng) {
                continue;
            }
            for detector in &operation.detectors {
                toggle_bit(&mut detectors, *detector, "detector")?;
            }
            for observable in &operation.observables {
                toggle_bit(&mut observables, *observable, "observable")?;
            }
        }
        Ok(DetectionEventRecord {
            detectors,
            observables,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DemSampleOperation {
    probability: f64,
    detectors: Vec<usize>,
    observables: Vec<usize>,
}

impl DemSampleOperation {
    fn sample_occurs<R>(&self, rng: &mut R) -> bool
    where
        R: Rng,
    {
        if self.probability <= 0.0 {
            return false;
        }
        if self.probability >= 1.0 {
            return true;
        }
        rng.random::<f64>() < self.probability
    }
}

fn compile_items(
    items: &[DemItem],
    detector_shift: u64,
    operations: &mut Vec<DemSampleOperation>,
) -> CircuitResult<u64> {
    let mut current_shift = detector_shift;
    for item in items {
        match item {
            DemItem::Instruction(instruction) => {
                compile_instruction(instruction, current_shift, operations)?;
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    current_shift = current_shift
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| {
                            CircuitError::invalid_sampler_compilation(
                                "DEM sampler detector shift overflowed",
                            )
                        })?;
                }
            }
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_DEM_SAMPLER_REPEAT_UNROLL {
                    return Err(CircuitError::invalid_sampler_compilation(format!(
                        "DEM sampler currently supports repeat counts up to {MAX_DEM_SAMPLER_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                for _ in 0..repeat_count {
                    current_shift =
                        compile_items(repeat.body().items(), current_shift, operations)?;
                }
            }
        }
    }
    Ok(current_shift)
}

fn compile_instruction(
    instruction: &DemInstruction,
    detector_shift: u64,
    operations: &mut Vec<DemSampleOperation>,
) -> CircuitResult<()> {
    if instruction.kind() != DemInstructionKind::Error {
        return Ok(());
    }
    let probability =
        instruction.args().first().copied().ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("error is missing probability")
        })?;
    let mut operation = DemSampleOperation {
        probability,
        detectors: Vec::new(),
        observables: Vec::new(),
    };
    for target in instruction.targets() {
        match target {
            DemTarget::RelativeDetector(detector) => {
                let shifted = detector_shift
                    .checked_add(detector.get())
                    .ok_or_else(|| detector_index_overflow_error("detector"))?;
                operation
                    .detectors
                    .push(usize_from_u64(shifted, "detector index")?);
            }
            DemTarget::LogicalObservable(observable) => {
                operation
                    .observables
                    .push(usize_from_u64(observable.get(), "observable index")?);
            }
            DemTarget::Separator => {}
            DemTarget::Numeric(_) => {
                return Err(CircuitError::invalid_sampler_compilation(
                    "error targets cannot include numeric DEM targets",
                ));
            }
        }
    }
    operations.push(operation);
    Ok(())
}

fn toggle_bit(bits: &mut [bool], index: usize, kind: &'static str) -> CircuitResult<()> {
    let bit = bits.get_mut(index).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation(format!("{kind} index {index} is out of range"))
    })?;
    *bit = !*bit;
    Ok(())
}

fn usize_from_u64(value: u64, kind: &'static str) -> CircuitResult<usize> {
    usize::try_from(value).map_err(|_| {
        CircuitError::invalid_sampler_compilation(format!("{kind} {value} does not fit in usize"))
    })
}

fn detector_index_overflow_error(kind: &'static str) -> CircuitError {
    CircuitError::invalid_sampler_compilation(format!("{kind} index overflowed"))
}

fn dem_sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}
