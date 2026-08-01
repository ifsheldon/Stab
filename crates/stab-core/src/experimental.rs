//! Reserved for implemented extension contracts that may change before Stab 1.0.
//!
//! Circuit-pass contracts appear here after a built-in transform and an external Stable crate
//! prove the common seam. Dynamic Rust plugins and runtime gate registration remain unsupported.

pub use stab_analysis::{
    CircuitPass, CircuitPassContext, CircuitPassError, CircuitPassInput, CircuitPassLimits,
    CircuitPassOutput, CircuitPassProjectionError, CircuitPassResources, CircuitPassStage,
    WithoutNoiseOptions, WithoutNoisePass, WithoutNoiseReport, run_circuit_pass,
};
