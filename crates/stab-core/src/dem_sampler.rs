use rand::rngs::SmallRng;
use rand::{Rng, RngExt as _, SeedableRng as _};

use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemTarget,
    DetectionConversionOutput, DetectionEventRecord, DetectorErrorModel, RandomPolicy,
    ResourceLimitError, Seed, ShotCount,
    dem::{FoldedDemBlock, FoldedDemItem},
};

mod buffers;
mod compat_sink;
mod limits;
mod plan;
mod session;

use buffers::{try_clone_bool_slice, try_clone_detection_record, try_vec_with_capacity};
use compat_sink::{DetectionAndErrorVisitorSink, DetectionVisitorSink, map_run_error};
pub use limits::DemSamplerLimits;
#[allow(
    unreachable_pub,
    reason = "the scoped A5 core change precedes facade reexports"
)]
pub use plan::{DemSamplingCompiler, DemSamplingPlan};
#[allow(
    unreachable_pub,
    unused_imports,
    reason = "the scoped A5 core change precedes facade reexports"
)]
pub use session::{
    DemReplayBatchStatus, DemReplaySession, DemSamplingCancellation, DemSamplingExecutionError,
    DemSamplingRunError, DemSamplingRunProgress, DemSamplingRunStatus, DemSamplingRunSummary,
    DemSamplingSession,
};

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDemSampler {
    plan: DemSamplingPlan,
}

impl CompiledDemSampler {
    pub fn compile(model: &DetectorErrorModel) -> CircuitResult<Self> {
        Ok(Self {
            plan: DemSamplingCompiler::new().compile(model)?,
        })
    }

    pub fn plan(&self) -> DemSamplingPlan {
        self.plan.clone()
    }

    pub fn session(
        &self,
        random_policy: RandomPolicy,
    ) -> Result<DemSamplingSession, DemSamplingExecutionError> {
        self.plan.session(random_policy)
    }

    pub fn session_with_limits(
        &self,
        random_policy: RandomPolicy,
        limits: DemSamplerLimits,
    ) -> Result<DemSamplingSession, DemSamplingExecutionError> {
        self.plan.session_with_limits(random_policy, limits)
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
        self.sample_detection_events_with_seed_and_limits(shots, seed, DemSamplerLimits::default())
    }

    pub fn sample_detection_events_with_seed_and_limits(
        &self,
        shots: usize,
        seed: Option<u64>,
        limits: DemSamplerLimits,
    ) -> CircuitResult<DetectionConversionOutput> {
        self.validate_sample_buffer_units_with_limits(shots, false, limits)?;
        self.validate_detector_sample_work_units_with_limits(shots, limits)?;
        let mut records = try_vec_with_capacity(shots, "DEM detection record container")?;
        self.try_for_each_detection_event_with_seed_and_limits(shots, seed, limits, |record| {
            records.push(try_clone_detection_record(record)?);
            Ok::<(), CircuitError>(())
        })?;
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.plan.detector_count(),
            observable_count: self.plan.observable_count(),
        })
    }

    pub fn error_count(&self) -> usize {
        self.plan.error_count()
    }

    pub fn validate_sample_buffer_units(
        &self,
        shots: usize,
        include_error_records: bool,
    ) -> CircuitResult<()> {
        self.validate_sample_buffer_units_with_limits(
            shots,
            include_error_records,
            DemSamplerLimits::default(),
        )
    }

    pub fn validate_sample_buffer_units_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_sample_buffer_units_with_limits(shots, include_error_records, limits)
    }

    fn validate_materialized_bytes_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_materialized_bytes_with_limits(shots, include_error_records, limits)
    }

    pub fn validate_replay_work_units(&self, shots: usize) -> CircuitResult<()> {
        self.validate_replay_work_units_with_limits(shots, DemSamplerLimits::default())
    }

    pub fn validate_replay_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_replay_work_units_with_limits(shots, limits)
    }

    fn replay_work_units_per_shot(&self) -> CircuitResult<usize> {
        self.plan.replay_work_units_per_shot()
    }

    fn validate_detector_sample_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_detector_sample_work_units_with_limits(shots, limits)
    }

    fn validate_sampled_error_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_sampled_error_work_units_with_limits(shots, limits)
    }

    pub fn sample_detection_events_and_errors_with_seed(
        &self,
        shots: usize,
        seed: Option<u64>,
    ) -> CircuitResult<(DetectionConversionOutput, Vec<Vec<bool>>)> {
        self.sample_detection_events_and_errors_with_seed_and_limits(
            shots,
            seed,
            DemSamplerLimits::default(),
        )
    }

    pub fn sample_detection_events_and_errors_with_seed_and_limits(
        &self,
        shots: usize,
        seed: Option<u64>,
        limits: DemSamplerLimits,
    ) -> CircuitResult<(DetectionConversionOutput, Vec<Vec<bool>>)> {
        self.validate_sample_buffer_units_with_limits(shots, true, limits)?;
        self.validate_sampled_error_work_units_with_limits(shots, limits)?;
        let mut records = try_vec_with_capacity(shots, "DEM detection record container")?;
        let mut error_records = try_vec_with_capacity(shots, "DEM sampled-error record container")?;
        self.try_for_each_detection_event_and_error_with_seed_and_limits(
            shots,
            seed,
            limits,
            |record, error_record| {
                records.push(try_clone_detection_record(record)?);
                error_records.push(try_clone_bool_slice(
                    error_record,
                    "DEM sampled-error record",
                )?);
                Ok::<(), CircuitError>(())
            },
        )?;
        Ok((
            DetectionConversionOutput {
                records,
                detector_count: self.plan.detector_count(),
                observable_count: self.plan.observable_count(),
            },
            error_records,
        ))
    }

    pub fn sample_detection_events_from_error_records(
        &self,
        error_records: &[Vec<bool>],
    ) -> CircuitResult<DetectionConversionOutput> {
        self.sample_detection_events_from_error_records_with_limits(
            error_records,
            DemSamplerLimits::default(),
        )
    }

    pub fn sample_detection_events_from_error_records_with_limits(
        &self,
        error_records: &[Vec<bool>],
        limits: DemSamplerLimits,
    ) -> CircuitResult<DetectionConversionOutput> {
        self.validate_replay_work_units_with_limits(error_records.len(), limits)?;
        self.validate_sample_buffer_units_with_limits(error_records.len(), false, limits)?;
        self.validate_materialized_bytes_with_limits(error_records.len(), true, limits)?;
        let mut records =
            try_vec_with_capacity(error_records.len(), "DEM detection record container")?;
        if error_records.is_empty() {
            return Ok(DetectionConversionOutput {
                records,
                detector_count: self.plan.detector_count(),
                observable_count: self.plan.observable_count(),
            });
        }
        let session_limits = self.plan.limits_after_compatibility_sink(true, limits)?;
        let mut sink = DetectionAndErrorVisitorSink::<CircuitError, _>::try_new(
            &self.plan,
            |record: &DetectionEventRecord, _error_record: &[bool]| {
                records.push(try_clone_detection_record(record)?);
                Ok(())
            },
        )?;
        let mut session = self
            .plan
            .session_with_limits(RandomPolicy::Seeded(Seed::new(0)), session_limits)
            .map_err(DemSamplingExecutionError::into_circuit_error)?;
        session
            .replay(error_records, &mut sink)
            .map_err(map_run_error)?;
        drop(sink);
        Ok(DetectionConversionOutput {
            records,
            detector_count: self.plan.detector_count(),
            observable_count: self.plan.observable_count(),
        })
    }

    pub fn try_for_each_detection_event_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        self.try_for_each_detection_event_with_seed_and_limits(
            shots,
            seed,
            DemSamplerLimits::default(),
            visit,
        )
    }

    pub fn try_for_each_detection_event_with_seed_and_limits<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        limits: DemSamplerLimits,
        visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord) -> Result<(), E>,
    {
        if shots == 0 {
            return Ok(());
        }
        self.validate_detector_sample_work_units_with_limits(shots, limits)?;
        self.validate_sample_buffer_units_with_limits(1, false, limits)?;
        let session_limits = self.plan.limits_after_compatibility_sink(false, limits)?;
        let mut sink = DetectionVisitorSink::<E, F>::try_new(&self.plan, visit)?;
        let mut session = self
            .plan
            .session_with_limits(random_policy(seed), session_limits)
            .map_err(|source| E::from(source.into_circuit_error()))?;
        let shots = shot_count_from_usize(shots).map_err(E::from)?;
        session.run(shots, &mut sink).map_err(map_run_error)?;
        Ok(())
    }

    pub fn try_for_each_detection_event_and_error_with_seed<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        self.try_for_each_detection_event_and_error_with_seed_and_limits(
            shots,
            seed,
            DemSamplerLimits::default(),
            visit,
        )
    }

    pub fn try_for_each_detection_event_and_error_with_seed_and_limits<E, F>(
        &self,
        shots: usize,
        seed: Option<u64>,
        limits: DemSamplerLimits,
        visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        if shots == 0 {
            return Ok(());
        }
        self.validate_sample_buffer_units_with_limits(1, true, limits)?;
        self.validate_sampled_error_work_units_with_limits(shots, limits)?;
        let session_limits = self.plan.limits_after_compatibility_sink(true, limits)?;
        let mut sink = DetectionAndErrorVisitorSink::<E, F>::try_new(&self.plan, visit)?;
        let mut session = self
            .plan
            .session_with_limits(random_policy(seed), session_limits)
            .map_err(|source| E::from(source.into_circuit_error()))?;
        let shots = shot_count_from_usize(shots).map_err(E::from)?;
        session
            .run_with_sampled_errors(shots, &mut sink)
            .map_err(map_run_error)?;
        Ok(())
    }

    pub fn try_for_each_detection_event_from_error_records<'a, E, I, F>(
        &self,
        error_records: I,
        visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        self.try_for_each_detection_event_from_error_records_with_limits(
            error_records,
            DemSamplerLimits::default(),
            visit,
        )
    }

    pub fn try_for_each_detection_event_from_error_records_with_limits<'a, E, I, F>(
        &self,
        error_records: I,
        limits: DemSamplerLimits,
        mut visit: F,
    ) -> Result<(), E>
    where
        E: From<CircuitError>,
        I: IntoIterator<Item = &'a [bool]>,
        F: FnMut(&DetectionEventRecord, &[bool]) -> Result<(), E>,
    {
        let units_per_shot = self.replay_work_units_per_shot()?;
        let mut replay_work_units = 0_usize;
        let mut record = None;
        for (shot_index, error_record) in error_records.into_iter().enumerate() {
            self.validate_error_record_width(error_record, Some(shot_index))?;
            replay_work_units = replay_work_units
                .checked_add(units_per_shot)
                .ok_or_else(|| {
                    E::from(CircuitError::invalid_sampler_compilation(
                        "DEM sampler replay work overflowed",
                    ))
                })?;
            if replay_work_units > limits.max_replay_work_units() {
                return Err(E::from(
                    ResourceLimitError::dem_replay_work_units(
                        replay_work_units,
                        limits.max_replay_work_units(),
                    )
                    .into(),
                ));
            }
            if record.is_none() {
                self.validate_sample_buffer_units_with_limits(1, false, limits)?;
                record = Some(self.try_reusable_detection_record()?);
            }
            let Some(record) = record.as_mut() else {
                return Err(E::from(CircuitError::invalid_sampler_compilation(
                    "DEM replay record allocation did not produce reusable storage",
                )));
            };
            self.detection_record_from_error_record_into(error_record, record)?;
            visit(record, error_record)?;
        }
        Ok(())
    }

    fn try_reusable_detection_record(&self) -> CircuitResult<DetectionEventRecord> {
        self.plan.try_reusable_detection_record()
    }

    fn validate_error_record_width(
        &self,
        error_record: &[bool],
        shot_index: Option<usize>,
    ) -> CircuitResult<()> {
        self.plan
            .validate_error_record_width(error_record, shot_index)
    }

    fn detection_record_from_error_record_into(
        &self,
        error_record: &[bool],
        record: &mut DetectionEventRecord,
    ) -> CircuitResult<()> {
        self.plan
            .detection_record_from_error_record_into(error_record, record)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct DemSampleBlock {
    operations: Vec<DemSampleOperation>,
    detector_shift: u64,
    error_count: usize,
    direct_sample_effect_count: usize,
    direct_sample_work_count: usize,
    direct_sample_has_stochastic_error: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum DemSampleOperation {
    Error(DemSampleError),
    Repeat(DemSampleRepeat),
}

#[derive(Clone, Debug, PartialEq)]
struct DemSampleError {
    probability: f64,
    detectors: Vec<u64>,
    observables: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq)]
struct DemSampleRepeat {
    start_detector_shift: u64,
    repeat_count: u64,
    body: DemSampleBlock,
    folded_zero_shift_errors: Option<Vec<DemSampleError>>,
}

impl DemSampleError {
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

fn compile_block(source: &FoldedDemBlock<'_>) -> CircuitResult<DemSampleBlock> {
    let mut block = DemSampleBlock::default();
    let mut current_shift = 0;
    for item in source.items() {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                compile_instruction(instruction, current_shift, &mut block)?;
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    current_shift = current_shift
                        .checked_add(crate::dem::dem_instruction_detector_shift(instruction)?)
                        .ok_or_else(|| {
                            CircuitError::invalid_sampler_compilation(
                                "DEM sampler detector shift overflowed",
                            )
                        })?;
                }
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count == 0 {
                    continue;
                }
                let body = compile_block(body)?;
                let repeat_start_shift = current_shift;
                let repeated_shift =
                    body.detector_shift
                        .checked_mul(repeat_count)
                        .ok_or_else(|| {
                            CircuitError::invalid_sampler_compilation(
                                "DEM sampler repeat detector shift overflowed",
                            )
                        })?;
                current_shift = current_shift.checked_add(repeated_shift).ok_or_else(|| {
                    CircuitError::invalid_sampler_compilation(
                        "DEM sampler detector shift overflowed",
                    )
                })?;
                block.error_count = checked_repeated_count(
                    block.error_count,
                    body.error_count,
                    repeat_count,
                    "DEM sampler error count",
                )?;
                block.direct_sample_effect_count = checked_repeated_count(
                    block.direct_sample_effect_count,
                    body.direct_sample_effect_count,
                    repeat_count,
                    "DEM sampler direct sample effect count",
                )?;
                block.direct_sample_has_stochastic_error |= body.direct_sample_has_stochastic_error;
                let folded_zero_shift_errors =
                    folded_zero_shift_repeat_errors(&body, repeat_count)?;
                let repeated_work = folded_direct_sample_repeat_work_count(
                    &body,
                    repeat_count,
                    folded_zero_shift_errors.as_deref(),
                )?;
                block.direct_sample_work_count = block
                    .direct_sample_work_count
                    .checked_add(repeated_work)
                    .ok_or_else(|| {
                        CircuitError::invalid_sampler_compilation(
                            "DEM sampler direct sample work count overflowed",
                        )
                    })?;
                block
                    .operations
                    .push(DemSampleOperation::Repeat(DemSampleRepeat {
                        start_detector_shift: repeat_start_shift,
                        repeat_count,
                        body,
                        folded_zero_shift_errors,
                    }));
            }
        }
    }
    block.detector_shift = current_shift;
    Ok(block)
}

fn compile_instruction(
    instruction: &DemInstruction,
    detector_shift: u64,
    block: &mut DemSampleBlock,
) -> CircuitResult<()> {
    if instruction.kind() != DemInstructionKind::Error {
        return Ok(());
    }
    let probability =
        instruction.args().first().copied().ok_or_else(|| {
            CircuitError::invalid_sampler_compilation("error is missing probability")
        })?;
    let mut operation = DemSampleError {
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
                operation.detectors.push(shifted);
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
    block.error_count = block.error_count.checked_add(1).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation("DEM sampler error count overflowed")
    })?;
    block.direct_sample_work_count =
        block
            .direct_sample_work_count
            .checked_add(1)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler direct sample work count overflowed",
                )
            })?;
    if probability > 0.0 {
        block.direct_sample_effect_count = block
            .direct_sample_effect_count
            .checked_add(1)
            .ok_or_else(|| {
                CircuitError::invalid_sampler_compilation(
                    "DEM sampler direct sample effect count overflowed",
                )
            })?;
    }
    if probability > 0.0 && probability < 1.0 {
        block.direct_sample_has_stochastic_error = true;
    }
    block.operations.push(DemSampleOperation::Error(operation));
    Ok(())
}

enum SampledErrorOutput<'a> {
    Discard,
    Record(&'a mut Vec<bool>),
}

impl SampledErrorOutput<'_> {
    fn is_discard(&self) -> bool {
        matches!(self, Self::Discard)
    }
}

fn sample_block_into<R>(
    block: &DemSampleBlock,
    detector_shift: u64,
    rng: &mut R,
    record: &mut DetectionEventRecord,
    mut error_output: SampledErrorOutput<'_>,
) -> CircuitResult<()>
where
    R: Rng,
{
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                let occurred = error.sample_occurs(rng);
                if let SampledErrorOutput::Record(error_record) = &mut error_output {
                    error_record.push(occurred);
                }
                if occurred {
                    apply_error_to_record(error, detector_shift, record)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                if error_output.is_discard() {
                    if repeat.body.direct_sample_effect_count == 0 {
                        continue;
                    }
                    if repeat.body.detector_shift == 0
                        && !repeat.body.direct_sample_has_stochastic_error
                    {
                        if repeat.repeat_count.is_multiple_of(2) {
                            continue;
                        }
                        let iteration_shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        sample_block_into(
                            &repeat.body,
                            iteration_shift,
                            rng,
                            record,
                            SampledErrorOutput::Discard,
                        )?;
                        continue;
                    }
                    if let Some(folded_errors) = repeat.folded_zero_shift_errors.as_deref() {
                        let iteration_shift = detector_shift
                            .checked_add(repeat.start_detector_shift)
                            .ok_or_else(detector_shift_overflow_error)?;
                        sample_folded_repeat_errors(folded_errors, iteration_shift, rng, record)?;
                        continue;
                    }
                }
                let mut iteration_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    sample_block_into(
                        &repeat.body,
                        iteration_shift,
                        rng,
                        record,
                        match &mut error_output {
                            SampledErrorOutput::Discard => SampledErrorOutput::Discard,
                            SampledErrorOutput::Record(error_record) => {
                                SampledErrorOutput::Record(error_record)
                            }
                        },
                    )?;
                    iteration_shift = iteration_shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_error_record_block(
    block: &DemSampleBlock,
    detector_shift: u64,
    error_record: &[bool],
    cursor: &mut usize,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()> {
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                let occurred = *error_record.get(*cursor).ok_or_else(|| {
                    CircuitError::invalid_result_format("DEM error record ended early")
                })?;
                *cursor = cursor.checked_add(1).ok_or_else(|| {
                    CircuitError::invalid_result_format("DEM error record cursor overflowed")
                })?;
                if occurred {
                    apply_error_to_record(error, detector_shift, record)?;
                }
            }
            DemSampleOperation::Repeat(repeat) => {
                let mut iteration_shift = detector_shift
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                for _ in 0..repeat.repeat_count {
                    apply_error_record_block(
                        &repeat.body,
                        iteration_shift,
                        error_record,
                        cursor,
                        record,
                    )?;
                    iteration_shift = iteration_shift
                        .checked_add(repeat.body.detector_shift)
                        .ok_or_else(detector_shift_overflow_error)?;
                }
            }
        }
    }
    Ok(())
}

fn apply_error_to_record(
    error: &DemSampleError,
    detector_shift: u64,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()> {
    for detector in &error.detectors {
        let shifted = detector_shift
            .checked_add(*detector)
            .ok_or_else(|| detector_index_overflow_error("detector"))?;
        toggle_bit(
            &mut record.detectors,
            usize_from_u64(shifted, "detector index")?,
            "detector",
        )?;
    }
    for observable in &error.observables {
        toggle_bit(&mut record.observables, *observable, "observable")?;
    }
    Ok(())
}

fn checked_repeated_count(
    current: usize,
    body_count: usize,
    repeat_count: u64,
    kind: &'static str,
) -> CircuitResult<usize> {
    let repeat_count = usize_from_u64(repeat_count, "DEM sampler repeat count")?;
    let repeated = body_count.checked_mul(repeat_count).ok_or_else(|| {
        CircuitError::invalid_sampler_compilation(format!("repeated {kind} overflowed"))
    })?;
    current
        .checked_add(repeated)
        .ok_or_else(|| CircuitError::invalid_sampler_compilation(format!("{kind} overflowed")))
}

fn folded_direct_sample_repeat_work_count(
    body: &DemSampleBlock,
    repeat_count: u64,
    folded_zero_shift_errors: Option<&[DemSampleError]>,
) -> CircuitResult<usize> {
    if body.direct_sample_effect_count == 0 {
        return Ok(0);
    }
    if let Some(errors) = folded_zero_shift_errors {
        return Ok(errors.len());
    }
    if body.detector_shift == 0 && !body.direct_sample_has_stochastic_error {
        if repeat_count.is_multiple_of(2) {
            return Ok(0);
        }
        return Ok(body.direct_sample_work_count);
    }
    if flat_zero_shift_error_block(body) {
        return Ok(body.direct_sample_work_count);
    }
    checked_repeated_count(
        0,
        body.direct_sample_work_count,
        repeat_count,
        "DEM sampler direct sample work count",
    )
}

fn flat_zero_shift_error_block(block: &DemSampleBlock) -> bool {
    block.detector_shift == 0
        && !block.operations.is_empty()
        && block
            .operations
            .iter()
            .all(|operation| matches!(operation, DemSampleOperation::Error(_)))
}

fn folded_zero_shift_repeat_errors(
    block: &DemSampleBlock,
    repeat_count: u64,
) -> CircuitResult<Option<Vec<DemSampleError>>> {
    if !block.direct_sample_has_stochastic_error {
        return Ok(None);
    }
    let mut one_pass_errors = Vec::new();
    if !collect_zero_shift_effect_errors(block, 0, &mut one_pass_errors)? {
        return Ok(None);
    }
    let mut folded_errors = Vec::with_capacity(one_pass_errors.len());
    for mut folded_error in one_pass_errors {
        folded_error.probability = odd_parity_probability(folded_error.probability, repeat_count);
        if folded_error.probability > 0.0 {
            folded_errors.push(folded_error);
        }
    }
    Ok(Some(folded_errors))
}

fn collect_zero_shift_effect_errors(
    block: &DemSampleBlock,
    detector_offset: u64,
    errors: &mut Vec<DemSampleError>,
) -> CircuitResult<bool> {
    if block.detector_shift != 0 {
        return Ok(false);
    }
    for operation in &block.operations {
        match operation {
            DemSampleOperation::Error(error) => {
                errors.push(shifted_sample_error(error, detector_offset)?);
            }
            DemSampleOperation::Repeat(repeat) => {
                if repeat.body.detector_shift != 0 {
                    return Ok(false);
                }
                let nested_offset = detector_offset
                    .checked_add(repeat.start_detector_shift)
                    .ok_or_else(detector_shift_overflow_error)?;
                if repeat.body.direct_sample_has_stochastic_error {
                    let Some(nested_errors) =
                        folded_zero_shift_repeat_errors(&repeat.body, repeat.repeat_count)?
                    else {
                        return Ok(false);
                    };
                    for nested_error in nested_errors {
                        errors.push(shifted_sample_error(&nested_error, nested_offset)?);
                    }
                } else if !repeat.repeat_count.is_multiple_of(2)
                    && !collect_zero_shift_effect_errors(&repeat.body, nested_offset, errors)?
                {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn shifted_sample_error(
    error: &DemSampleError,
    detector_offset: u64,
) -> CircuitResult<DemSampleError> {
    if detector_offset == 0 {
        return Ok(error.clone());
    }
    let detectors = error
        .detectors
        .iter()
        .map(|detector| {
            detector
                .checked_add(detector_offset)
                .ok_or_else(|| detector_index_overflow_error("detector"))
        })
        .collect::<CircuitResult<Vec<_>>>()?;
    Ok(DemSampleError {
        probability: error.probability,
        detectors,
        observables: error.observables.clone(),
    })
}

fn sample_folded_repeat_errors<R>(
    errors: &[DemSampleError],
    detector_shift: u64,
    rng: &mut R,
    record: &mut DetectionEventRecord,
) -> CircuitResult<()>
where
    R: Rng,
{
    for error in errors {
        if sample_probability(error.probability, rng) {
            apply_error_to_record(error, detector_shift, record)?;
        }
    }
    Ok(())
}

fn odd_parity_probability(probability: f64, repeat_count: u64) -> f64 {
    if repeat_count == 0 || probability <= 0.0 {
        return 0.0;
    }
    if probability >= 1.0 {
        return if repeat_count.is_multiple_of(2) {
            0.0
        } else {
            1.0
        };
    }
    if probability == 0.5 {
        return 0.5;
    }

    if probability < 0.5 {
        let log_bias = (repeat_count as f64) * (-2.0 * probability).ln_1p();
        return (-0.5 * log_bias.exp_m1()).clamp(0.0, 0.5);
    }

    let complement = 1.0 - probability;
    let log_magnitude = (repeat_count as f64) * (-2.0 * complement).ln_1p();
    if repeat_count.is_multiple_of(2) {
        (-0.5 * log_magnitude.exp_m1()).clamp(0.0, 0.5)
    } else {
        (1.0 + 0.5 * log_magnitude.exp_m1()).clamp(0.5, 1.0)
    }
}

fn sample_probability<R>(probability: f64, rng: &mut R) -> bool
where
    R: Rng,
{
    if probability <= 0.0 {
        return false;
    }
    if probability >= 1.0 {
        return true;
    }
    rng.random::<f64>() < probability
}

fn reset_detection_record(
    record: &mut DetectionEventRecord,
    detector_count: usize,
    observable_count: usize,
) {
    record.detectors.clear();
    record.detectors.resize(detector_count, false);
    record.observables.clear();
    record.observables.resize(observable_count, false);
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

fn detector_shift_overflow_error() -> CircuitError {
    CircuitError::invalid_sampler_compilation("DEM sampler detector shift overflowed")
}

fn dem_sampler_rng(seed: Option<u64>) -> SmallRng {
    SmallRng::seed_from_u64(seed.unwrap_or_else(rand::random))
}

fn random_policy(seed: Option<u64>) -> RandomPolicy {
    match seed {
        Some(seed) => RandomPolicy::Seeded(Seed::new(seed)),
        None => RandomPolicy::Entropy,
    }
}

fn shot_count_from_usize(shots: usize) -> CircuitResult<ShotCount> {
    u64::try_from(shots).map(ShotCount::new).map_err(|_| {
        CircuitError::invalid_sampler_compilation("DEM sampler shot count does not fit in u64")
    })
}

#[cfg(test)]
mod tests;
