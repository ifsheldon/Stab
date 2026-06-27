use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemItem, DemTarget,
    DetectionConversionOutput, DetectionEventRecord, DetectorErrorModel,
    dem::MAX_DEM_REPEAT_NESTING,
};

const MAX_DEM_SAMPLER_REPEAT_UNROLL: u64 = 100_000;
const MAX_DEM_SAMPLER_EXPANDED_INSTRUCTIONS: u64 = 1_000_000;
const MAX_DEM_SAMPLER_REPEAT_ITERATIONS: u64 = 1_000_000;
const MAX_DEM_SAMPLER_BUFFER_UNITS: usize = 64_000_000;
const MAX_DEM_SAMPLER_BUFFER_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDemSampler {
    detector_count: usize,
    observable_count: usize,
    operations: Vec<DemSampleOperation>,
}

impl CompiledDemSampler {
    pub fn compile(model: &DetectorErrorModel) -> CircuitResult<Self> {
        validate_compile_budget(model.items())?;
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
        self.validate_sample_buffer_units(shots, false)?;
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

    pub fn error_count(&self) -> usize {
        self.operations.len()
    }

    pub fn validate_sample_buffer_units(
        &self,
        shots: usize,
        include_error_records: bool,
    ) -> CircuitResult<()> {
        let mut units_per_shot = self
            .detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer size",
                )
            })?;
        if include_error_records {
            units_per_shot = units_per_shot.checked_add(self.operations.len()).ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output and error width overflowed while validating buffer size",
                )
            })?;
        }
        let units_per_shot = units_per_shot.max(1);
        let total_units = shots.checked_mul(units_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer size overflowed")
        })?;
        if total_units > MAX_DEM_SAMPLER_BUFFER_UNITS {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler would require {total_units} buffered units; current limit is {MAX_DEM_SAMPLER_BUFFER_UNITS}"
            )));
        }
        let bytes_per_shot = self.materialized_bytes_per_shot(include_error_records)?;
        let total_bytes = shots.checked_mul(bytes_per_shot).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("DEM sampler buffer byte size overflowed")
        })?;
        if total_bytes > MAX_DEM_SAMPLER_BUFFER_BYTES {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler would require at least {total_bytes} materialized bytes; current limit is {MAX_DEM_SAMPLER_BUFFER_BYTES}"
            )));
        }
        Ok(())
    }

    fn materialized_bytes_per_shot(&self, include_error_records: bool) -> CircuitResult<usize> {
        let detector_observable_bytes = self
            .detector_count
            .checked_add(self.observable_count)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler output width overflowed while validating buffer bytes",
                )
            })?;
        let mut bytes = std::mem::size_of::<DetectionEventRecord>()
            .checked_add(detector_observable_bytes)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler per-shot output byte size overflowed",
                )
            })?;
        if include_error_records {
            bytes = bytes
                .checked_add(std::mem::size_of::<Vec<bool>>())
                .and_then(|value| value.checked_add(self.operations.len()))
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler per-shot error byte size overflowed",
                    )
                })?;
        }
        Ok(bytes.max(1))
    }

    pub fn sample_detection_events_and_errors_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<(DetectionConversionOutput, Vec<Vec<bool>>)> {
        self.validate_sample_buffer_units(shots, true)?;
        let mut rng = dem_sampler_rng(seed);
        let mut records = Vec::with_capacity(shots);
        let mut error_records = Vec::with_capacity(shots);
        for _ in 0..shots {
            let error_record = self.sample_error_record(&mut rng);
            records.push(self.detection_record_from_error_record(&error_record)?);
            error_records.push(error_record);
        }
        Ok((
            DetectionConversionOutput {
                records,
                detector_count: self.detector_count,
                observable_count: self.observable_count,
            },
            error_records,
        ))
    }

    pub fn sample_detection_events_from_error_records(
        &self,
        error_records: &[Vec<bool>],
    ) -> CircuitResult<DetectionConversionOutput> {
        self.validate_sample_buffer_units(error_records.len(), true)?;
        self.validate_error_record_widths(error_records)?;
        let records = error_records
            .iter()
            .map(|error_record| self.detection_record_from_error_record(error_record))
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
        let error_record = self.sample_error_record(rng);
        self.detection_record_from_error_record(&error_record)
    }

    fn sample_error_record<R>(&self, rng: &mut R) -> Vec<bool>
    where
        R: Rng,
    {
        self.operations
            .iter()
            .map(|operation| operation.sample_occurs(rng))
            .collect()
    }

    fn validate_error_record_widths(&self, error_records: &[Vec<bool>]) -> CircuitResult<()> {
        for (shot_index, error_record) in error_records.iter().enumerate() {
            if error_record.len() != self.operations.len() {
                return Err(CircuitError::invalid_result_format(format!(
                    "DEM error record {shot_index} expected {} bits, got {}",
                    self.operations.len(),
                    error_record.len()
                )));
            }
        }
        Ok(())
    }

    fn detection_record_from_error_record(
        &self,
        error_record: &[bool],
    ) -> CircuitResult<DetectionEventRecord> {
        if error_record.len() != self.operations.len() {
            return Err(CircuitError::invalid_result_format(format!(
                "DEM error record expected {} bits, got {}",
                self.operations.len(),
                error_record.len()
            )));
        }
        let mut detectors = vec![false; self.detector_count];
        let mut observables = vec![false; self.observable_count];
        for (operation, occurred) in self.operations.iter().zip(error_record) {
            if !occurred {
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DemSamplerCompileBudget {
    expanded_instructions: u64,
    repeat_iterations: u64,
}

impl DemSamplerCompileBudget {
    fn add_expanded_instructions(&mut self, count: u64) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler expanded instruction count overflowed",
                    )
                })?;
        if self.expanded_instructions > MAX_DEM_SAMPLER_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler currently supports at most {MAX_DEM_SAMPLER_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_sampler_compilation(
                "DEM sampler repeat iteration count overflowed",
            )
        })?;
        if self.repeat_iterations > MAX_DEM_SAMPLER_REPEAT_ITERATIONS {
            return Err(CircuitError::invalid_sampler_compilation(format!(
                "DEM sampler currently supports at most {MAX_DEM_SAMPLER_REPEAT_ITERATIONS} expanded repeat iterations, got at least {}",
                self.repeat_iterations
            )));
        }
        Ok(())
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

fn validate_compile_budget(items: &[DemItem]) -> CircuitResult<()> {
    let mut budget = DemSamplerCompileBudget::default();
    validate_compile_budget_items(items, 1, 0, &mut budget)
}

fn validate_compile_budget_items(
    items: &[DemItem],
    multiplier: u64,
    depth: usize,
    budget: &mut DemSamplerCompileBudget,
) -> CircuitResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_sampler_compilation(format!(
            "DEM repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in items {
        match item {
            DemItem::Instruction(_) => budget.add_expanded_instructions(multiplier)?,
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count > MAX_DEM_SAMPLER_REPEAT_UNROLL {
                    return Err(CircuitError::invalid_sampler_compilation(format!(
                        "DEM sampler currently supports repeat counts up to {MAX_DEM_SAMPLER_REPEAT_UNROLL}, got {repeat_count}"
                    )));
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        CircuitError::invalid_sampler_compilation(
                            "DEM sampler repeat expansion count overflowed",
                        )
                    })?;
                budget.add_repeat_iterations(repeated_multiplier)?;
                validate_compile_budget_items(
                    repeat.body().items(),
                    repeated_multiplier,
                    depth + 1,
                    budget,
                )?;
            }
        }
    }
    Ok(())
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
