const MAX_DEM_SAMPLER_BUFFER_UNITS: usize = 64_000_000;
const MAX_DEM_SAMPLER_BUFFER_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS: usize = 64_000_000;
const MAX_DEM_SAMPLER_REPLAY_WORK_UNITS: usize = 64_000_000;

/// Admission limits for DEM sampling work and materialized output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemSamplerLimits {
    max_sampled_error_applications: usize,
    max_replay_work_units: usize,
    max_materialized_units: usize,
    max_materialized_bytes: usize,
}

impl DemSamplerLimits {
    pub const fn max_sampled_error_applications(self) -> usize {
        self.max_sampled_error_applications
    }

    pub const fn max_replay_work_units(self) -> usize {
        self.max_replay_work_units
    }

    pub const fn max_materialized_units(self) -> usize {
        self.max_materialized_units
    }

    pub const fn max_materialized_bytes(self) -> usize {
        self.max_materialized_bytes
    }

    #[must_use]
    pub const fn with_max_sampled_error_applications(
        mut self,
        max_sampled_error_applications: usize,
    ) -> Self {
        self.max_sampled_error_applications = max_sampled_error_applications;
        self
    }

    #[must_use]
    pub const fn with_max_replay_work_units(mut self, max_replay_work_units: usize) -> Self {
        self.max_replay_work_units = max_replay_work_units;
        self
    }

    #[must_use]
    pub const fn with_max_materialized_units(mut self, max_materialized_units: usize) -> Self {
        self.max_materialized_units = max_materialized_units;
        self
    }

    #[must_use]
    pub const fn with_max_materialized_bytes(mut self, max_materialized_bytes: usize) -> Self {
        self.max_materialized_bytes = max_materialized_bytes;
        self
    }
}

impl Default for DemSamplerLimits {
    fn default() -> Self {
        Self {
            max_sampled_error_applications: MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS,
            max_replay_work_units: MAX_DEM_SAMPLER_REPLAY_WORK_UNITS,
            max_materialized_units: MAX_DEM_SAMPLER_BUFFER_UNITS,
            max_materialized_bytes: MAX_DEM_SAMPLER_BUFFER_BYTES,
        }
    }
}
