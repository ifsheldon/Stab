//! Options of detection sessions and streaming runs.

use stab_model::MemoryBudget;

use crate::{LaneWidth, RandomPolicy};

/// Options of one detection-sampling session or streaming run; see
/// [`super::DetectionSamplingPlan::session`].
///
/// The default draws its seed from entropy, samples 256 lanes per batch, and admits its storage
/// against the plan's memory budget.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct DetectionSamplingSessionOptions {
    /// How the session's random stream is initialized.
    pub random: RandomPolicy,
    /// Shots each frame batch samples together. The seeded stream is a function of the plan,
    /// the seed, and this width.
    pub lane_width: LaneWidth,
    /// Budget for the session's storage; `None` admits it against the plan's
    /// [`super::DetectionSamplingCompiler::memory_budget`].
    pub memory_budget: Option<MemoryBudget>,
}

/// Options of one measurement-to-detection session; see
/// [`super::MeasurementToDetectionPlan::session`].
///
/// The default admits the session's storage against the plan's memory budget.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct MeasurementToDetectionSessionOptions {
    /// Budget for the session's reusable storage; `None` admits it against the plan's
    /// [`super::MeasurementToDetectionCompiler::memory_budget`].
    pub memory_budget: Option<MemoryBudget>,
}
