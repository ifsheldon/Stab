const MAX_DEM_SAMPLER_ACTIVE_BATCH_BYTES: usize = 64 * 1024 * 1024;
const MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS_PER_SHOT: usize = 64_000_000;
const MAX_DEM_SAMPLER_REPLAY_WORK_UNITS_PER_SHOT: usize = 64_000_000;

/// Admission limits for DEM sampling work and reusable session storage.
///
/// Work limits bound one shot. Sessions stream shots through bounded batches,
/// so the total requested shot count is never a resource limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemSamplerLimits {
    max_sampled_error_applications_per_shot: usize,
    max_replay_work_units_per_shot: usize,
    max_active_batch_bytes: usize,
}

impl DemSamplerLimits {
    pub const fn max_sampled_error_applications_per_shot(self) -> usize {
        self.max_sampled_error_applications_per_shot
    }

    pub const fn max_replay_work_units_per_shot(self) -> usize {
        self.max_replay_work_units_per_shot
    }

    pub const fn max_active_batch_bytes(self) -> usize {
        self.max_active_batch_bytes
    }

    #[must_use]
    pub const fn with_max_sampled_error_applications_per_shot(
        mut self,
        max_sampled_error_applications_per_shot: usize,
    ) -> Self {
        self.max_sampled_error_applications_per_shot = max_sampled_error_applications_per_shot;
        self
    }

    #[must_use]
    pub const fn with_max_replay_work_units_per_shot(
        mut self,
        max_replay_work_units_per_shot: usize,
    ) -> Self {
        self.max_replay_work_units_per_shot = max_replay_work_units_per_shot;
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
            max_sampled_error_applications_per_shot:
                MAX_DEM_SAMPLER_SAMPLE_ERROR_APPLICATIONS_PER_SHOT,
            max_replay_work_units_per_shot: MAX_DEM_SAMPLER_REPLAY_WORK_UNITS_PER_SHOT,
            max_active_batch_bytes: MAX_DEM_SAMPLER_ACTIVE_BATCH_BYTES,
        }
    }
}
