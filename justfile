set fallback

# Rust component checks.
mod rust 'justfiles/rust.just'

# Product dependency and source-boundary checks.
mod architecture 'justfiles/architecture.just'

# Repository maintenance helpers.
mod maintenance 'justfiles/maintenance.just'

# Stim oracle compatibility helpers.
mod oracle 'justfiles/oracle.just'

# Benchmark workflow helpers.
mod bench 'justfiles/bench.just'

# Documentation generation helpers.
mod docs 'justfiles/docs.just'

# Coordinated package and binary release helpers.
mod release 'justfiles/release.just'
