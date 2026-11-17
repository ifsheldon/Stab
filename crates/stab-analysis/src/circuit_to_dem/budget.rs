use stab_model::{Circuit, ParseLimits};

use crate::folded_walk::check_repeat_nesting;
use crate::{AnalysisResult, ResourceKind, ResourceLimitError};

/// Rejects repeat nesting beyond the shared envelope before folded or unrolled analysis recurses.
pub(super) fn validate_analyzer_repeat_nesting(circuit: &Circuit) -> AnalysisResult<()> {
    check_repeat_nesting(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        |depth, limit| {
            ResourceLimitError::circuit_to_detector_error_model(
                ResourceKind::RepeatNesting,
                depth,
                limit,
            )
            .into()
        },
    )
}
