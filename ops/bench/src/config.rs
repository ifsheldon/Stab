use std::time::Duration;

pub(crate) const PREFIX: &str = "stab-bench";
pub(crate) const STIM_TAG: &str = "v1.16.0";
pub(crate) const STIM_COMMIT: &str = "e2fc1eca7fd21684d433aa5f10f4504ea4860d07";
pub(crate) const DEFAULT_STIM_PATH: &str = "vendor/stim";
pub(crate) const BUILD_DIR: &str = "target/benchmarks/stim-v1.16.0";
pub(crate) const DEFAULT_BASELINE_DIR: &str = "target/benchmarks/baseline/latest";
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);
