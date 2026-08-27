const MAX_DEM_SAMPLER_ACTIVE_BATCH_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS: usize = 64_000_000;
const MAX_DEM_SAMPLER_REPLAY_WORK_UNITS: usize = 64_000_000;

/// Admission limits for DEM sampling work and reusable session storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemSamplerLimits {
    max_sampled_error_applications: usize,
    max_replay_work_units: usize,
    max_active_batch_bytes: usize,
}

impl DemSamplerLimits {
    pub const fn max_sampled_error_applications(self) -> usize {
        self.max_sampled_error_applications
    }

    pub const fn max_replay_work_units(self) -> usize {
        self.max_replay_work_units
    }

    pub const fn max_active_batch_bytes(self) -> usize {
        self.max_active_batch_bytes
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
    pub const fn with_max_active_batch_bytes(mut self, max_active_batch_bytes: usize) -> Self {
        self.max_active_batch_bytes = max_active_batch_bytes;
        self
    }
}

impl Default for DemSamplerLimits {
    fn default() -> Self {
        Self {
            max_sampled_error_applications: MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS,
            max_replay_work_units: MAX_DEM_SAMPLER_REPLAY_WORK_UNITS,
            max_active_batch_bytes: MAX_DEM_SAMPLER_ACTIVE_BATCH_BYTES,
        }
    }
}
