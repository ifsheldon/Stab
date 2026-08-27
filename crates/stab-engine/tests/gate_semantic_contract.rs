#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    reason = "the migrated exhaustive gate contract uses fixed valid fixtures and explicit failures"
)]

pub(crate) use stab_algebra::{Flow, PauliBasis, PauliPhase, PauliSign, PauliString};
pub(crate) use stab_analysis as analysis;
pub(crate) use stab_engine as execution;
pub(crate) use stab_model::{
    Circuit, DetectorErrorModel, Gate, GateArgumentRule, GateCategory, MeasureRecordOffset, Pauli,
    Probability, QubitId, Target,
};
pub(crate) use stab_records::{
    DetectionBatchView, DetectionSink, MeasurementBatchView, MeasurementSink,
};

#[path = "gate_semantic_contract/gate.rs"]
mod gate;
