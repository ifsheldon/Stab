//! Stable typed Stim circuit and detector-error-model values.

mod error;
mod ids;
mod target;

pub use error::{ModelError, ModelResult};
pub use ids::{
    CircuitDetectorId, DemRepeatCount, MeasureRecordOffset, MeasureRecordOffsetText, ObservableId,
    Probability, QubitId, RepeatCount,
};
pub use target::{Pauli, Target};

/// Low-level model operations for parsers and admitted algorithms.
pub mod advanced {
    use std::fmt::Display;

    use super::{MeasureRecordOffset, ModelResult, Probability, Target};
    use smallvec::SmallVec;

    /// Inline target storage used by the exact Stim parsers.
    pub type TargetVec = SmallVec<[Target; 4]>;

    /// Stim's exclusive upper bound for encoded target values.
    pub const STIM_TARGET_VALUE_LIMIT: u32 = crate::ids::STIM_TARGET_VALUE_LIMIT;

    /// Constructs a probability after the caller has proved its domain.
    pub fn probability_from_valid(value: f64) -> Probability {
        Probability::from_valid_probability(value)
    }

    /// Preserves Stim's parsed `rec[-0]` spelling.
    pub fn measure_record_offset_from_stim_text(value: i32) -> ModelResult<MeasureRecordOffset> {
        MeasureRecordOffset::from_stim_text(value)
    }

    /// Reports whether an offset preserves Stim's parsed `rec[-0]` spelling.
    pub fn measure_record_offset_is_negative_zero(offset: MeasureRecordOffset) -> bool {
        offset.is_negative_zero()
    }

    /// Formats an offset using Stim target syntax.
    pub fn measure_record_offset_stim_text(offset: MeasureRecordOffset) -> impl Display {
        offset.stim_text()
    }

    /// Parses one possibly combined target token into caller-provided storage.
    pub fn parse_target_token_into(token: &str, targets: &mut TargetVec) -> ModelResult<()> {
        crate::target::parse_target_token_into(token, targets)
    }

    /// Parses the fast path for a whitespace-separated plain-qubit target list.
    pub fn parse_plain_qubit_target_text(text: &str) -> ModelResult<Option<TargetVec>> {
        crate::target::parse_plain_qubit_target_text(text)
    }
}
