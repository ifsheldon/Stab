use crate::{
    CircuitError, CircuitResult, DetectionConversionOutput, DetectionEventRecord,
    DetectorErrorModel, RandomPolicy, ResourceLimitError, Seed, ShotCount,
};

mod buffers;
mod compat_sink;

use buffers::{try_clone_bool_slice, try_clone_detection_record, try_vec_with_capacity};
use compat_sink::{DetectionAndErrorVisitorSink, DetectionVisitorSink, map_run_error};
pub use stab_engine::DemSamplerLimits;
use stab_engine::{
    DemSamplingCompiler, DemSamplingExecutionError, DemSamplingPlan, DemSamplingSession,
};

/// Compatibility materializer over the engine-owned DEM sampling plan and sessions.
#[derive(Clone, Debug, PartialEq)]
pub struct CompiledDemSampler {
    plan: DemSamplingPlan,
}

impl CompiledDemSampler {
    pub fn compile(model: &DetectorErrorModel) -> CircuitResult<Self> {
        Ok(Self {
            plan: DemSamplingCompiler::new()
                .compile(model)
                .map_err(CircuitError::from)?,
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
            .map_err(CircuitError::from)
    }

    fn validate_materialized_bytes_with_limits(
        &self,
        shots: usize,
        include_error_records: bool,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_materialized_bytes_with_limits(shots, include_error_records, limits)
            .map_err(CircuitError::from)
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
            .map_err(CircuitError::from)
    }

    fn replay_work_units_per_shot(&self) -> CircuitResult<usize> {
        self.plan
            .replay_work_units_per_shot()
            .map_err(CircuitError::from)
    }

    fn validate_detector_sample_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_detector_sample_work_units_with_limits(shots, limits)
            .map_err(CircuitError::from)
    }

    fn validate_sampled_error_work_units_with_limits(
        &self,
        shots: usize,
        limits: DemSamplerLimits,
    ) -> CircuitResult<()> {
        self.plan
            .validate_sampled_error_work_units_with_limits(shots, limits)
            .map_err(CircuitError::from)
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
        let session_limits = limits_after_compatibility_sink(&self.plan, true, limits)?;
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
            .map_err(CircuitError::from)?;
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
        let session_limits = limits_after_compatibility_sink(&self.plan, false, limits)?;
        let mut sink = DetectionVisitorSink::<E, F>::try_new(&self.plan, visit)?;
        let mut session = self
            .plan
            .session_with_limits(random_policy(seed), session_limits)
            .map_err(|source| E::from(CircuitError::from(source)))?;
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
        let session_limits = limits_after_compatibility_sink(&self.plan, true, limits)?;
        let mut sink = DetectionAndErrorVisitorSink::<E, F>::try_new(&self.plan, visit)?;
        let mut session = self
            .plan
            .session_with_limits(random_policy(seed), session_limits)
            .map_err(|source| E::from(CircuitError::from(source)))?;
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
            self.plan
                .validate_error_record_width(error_record, Some(shot_index))
                .map_err(|source| E::from(CircuitError::from(source)))?;
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
                record = Some(
                    self.plan
                        .try_reusable_detection_record()
                        .map_err(|source| E::from(CircuitError::from(source)))?,
                );
            }
            let Some(record) = record.as_mut() else {
                return Err(E::from(CircuitError::invalid_sampler_compilation(
                    "DEM replay record allocation did not produce reusable storage",
                )));
            };
            self.plan
                .detection_record_from_error_record_into(error_record, record)
                .map_err(|source| E::from(CircuitError::from(source)))?;
            visit(record, error_record)?;
        }
        Ok(())
    }
}

fn random_policy(seed: Option<u64>) -> RandomPolicy {
    seed.map_or(RandomPolicy::Entropy, |seed| {
        RandomPolicy::Seeded(Seed::new(seed))
    })
}

fn limits_after_compatibility_sink(
    plan: &DemSamplingPlan,
    include_error_records: bool,
    limits: DemSamplerLimits,
) -> CircuitResult<DemSamplerLimits> {
    let reserved_bytes = plan
        .materialized_bytes_per_shot(include_error_records)
        .map_err(CircuitError::from)?;
    if reserved_bytes > limits.max_materialized_bytes() {
        return Err(ResourceLimitError::dem_materialized_bytes(
            reserved_bytes,
            limits.max_materialized_bytes(),
        )
        .into());
    }
    Ok(limits.with_max_materialized_bytes(limits.max_materialized_bytes() - reserved_bytes))
}

fn shot_count_from_usize(shots: usize) -> CircuitResult<ShotCount> {
    u64::try_from(shots).map(ShotCount::new).map_err(|_| {
        CircuitError::invalid_sampler_compilation("DEM sampler shot count does not fit in u64")
    })
}
