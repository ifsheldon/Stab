use std::time::Duration;

pub(crate) const PREFIX: &str = "stab-bench";
pub(crate) const DEFAULT_STIM_PATH: &str = "vendor/stim";
pub(crate) const BUILD_DIR: &str = "target/benchmarks/stim-v1.16.0";
pub(crate) const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);
