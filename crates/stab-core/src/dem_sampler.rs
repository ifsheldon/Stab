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
        let mut records = Vec::with_capacity(shots);
        self.try_for_each_detection_event_with_seed(shots, seed, |record| {
            records.push(record.clone());
            Ok::<(), CircuitError>(())
        })?;
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
        let mut records = Vec::with_capacity(shots);
        let mut error_records = Vec::with_capacity(shots);
        self.try_for_each_detection_event_and_error_with_seed(
            shots,
            seed,
            |record, error_record| {
                records.push(record.clone());
                error_records.push(error_record.to_vec());
                Ok::<(), CircuitError>(())
            },
        )?;
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
        let mut records = Vec::with_capacity(error_records.len());
        self.try_for_each_detection_event_from_error_records(
            error_records.iter().map(Vec::as_slice),
            |record, _error_record| {
                records.push(record.clone());
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.detector_count,
            observable_count: self.observable_count,
        })
    }

    pub fn try_for_each_detection_event_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        self.try_for_each_detection_event_and_error_with_seed(shots, seed, |record, _error| {
            visit(record)
        })
    }

    pub fn try_for_each_detection_event_and_error_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        let mut rng = dem_sampler_rng(seed);
        let mut error_record = Vec::with_capacity(self.operations.len());
        let mut record = self.reusable_detection_record();
        for _ in 0..shots {
            self.sample_error_record_into(&mut rng, &mut error_record);
            self.detection_record_from_error_record_into(&error_record, &mut record)?;
            visit(&record, &error_record)?;
        }
        Ok(())
    }

    pub fn try_for_each_detection_event_from_error_records<'a, E, I, F>(
        &self,
        error_records: I,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        let mut record = self.reusable_detection_record();
        for (shot_index, error_record) in error_records.into_iter().enumerate() {
            self.validate_error_record_width(error_record, Some(shot_index))?;
            self.detection_record_from_error_record_into(error_record, &mut record)?;
            visit(&record, error_record)?;
        }
        Ok(())
    }

    fn reusable_detection_record(&self) -> DetectionEventRecord {
        DetectionEventRecord {
            detectors: vec![false; self.detector_count],
            observables: vec![false; self.observable_count],
        }
    }

    fn sample_error_record_into<R>(&self, rng: &mut R, error_record: &mut Vec<bool>)
    where
        R: Rng,
    {
        error_record.clear();
        error_record.extend(
            self.operations
                .iter()
                .map(|operation| operation.sample_occurs(rng)),
        );
    }

    fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        if error_record.len() == self.operations.len() {
            return Ok(());
        }
        if let Some(shot_index) = shot_index {
            return Err(CircuitError::invalid_result_format(format!(
                "DEM error record {shot_index} expected {} bits, got {}",
                self.operations.len(),
                error_record.len()
            )));
        }
        Err(CircuitError::invalid_result_format(format!(
            "DEM error record expected {} bits, got {}",
            self.operations.len(),
            error_record.len()
        )))
    }

    fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.validate_error_record_width(error_record, None)?;
        record.detectors.clear();
        record.detectors.resize(self.detector_count, false);
        record.observables.clear();
        record.observables.resize(self.observable_count, false);
        for (operation, occurred) in self.operations.iter().zip(error_record) {
            if !occurred {
                continue;
            }
            for detector in &operation.detectors {
                toggle_bit(&mut record.detectors, *detector, "detector")?;
            }
            for observable in &operation.observables {
                toggle_bit(&mut record.observables, *observable, "observable")?;
            }
        }
        Ok(())
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "DEM sampler tests use direct fixture assertions for compact diagnostics"
    )]

    use super::*;

    fn collect_streamed_samples(
        sampler: &CompiledDemSampler,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<(Vec<DetectionEventRecord>, Vec<Vec<bool>>)> {
        let mut records = Vec::new();
        let mut errors = Vec::new();
        sampler.try_for_each_detection_event_and_error_with_seed(
            shots,
            seed,
            |record, error_record| {
                records.push(record.clone());
                errors.push(error_record.to_vec());
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok((records, errors))
    }

    #[test]
    fn dem_streaming_samples_match_materialized_seeded_samples() {
        for dem_text in [
            "error(1) D0\n",
            "error(0.25) D0\n",
            "error(0.25) L2\n",
            "error(0.25) D0 D2\nerror(0.25) D2 D3\n",
            "error(0.25) D0\nshift_detectors 1\nrepeat 2 {\n    error(0.25) D0\n    shift_detectors 1\n}\nerror(0) D0\n",
        ] {
            let model = DetectorErrorModel::from_dem_str(dem_text).expect("parse DEM");
            let sampler = CompiledDemSampler::compile(&model).expect("compile DEM sampler");
            let (materialized, materialized_errors) = sampler
                .sample_detection_events_and_errors_with_seed(65, Some(7))
                .expect("materialized samples");
            let (streamed, streamed_errors) =
                collect_streamed_samples(&sampler, 65, Some(7)).expect("streamed samples");

            assert_eq!(streamed, materialized.records);
            assert_eq!(streamed_errors, materialized_errors);
            let replayed = sampler
                .sample_detection_events_from_error_records(&streamed_errors)
                .expect("materialized replay");
            let mut streamed_replay = Vec::new();
            sampler
                .try_for_each_detection_event_from_error_records(
                    streamed_errors.iter().map(Vec::as_slice),
                    |record, _error_record| {
                        streamed_replay.push(record.clone());
                        Ok::<(), CircuitError>(())
                    },
                )
                .expect("streamed replay");
            assert_eq!(streamed_replay, replayed.records);
        }
    }
}
