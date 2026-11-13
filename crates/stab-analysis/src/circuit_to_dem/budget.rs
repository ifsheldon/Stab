use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
use stab_model::{Circuit, CircuitItem};

use crate::{AnalysisResult, ResourceKind, ResourceLimitError};

/// Rejects repeat nesting beyond the shared envelope before unrolled analysis recurses.
pub(super) fn validate_analyzer_repeat_nesting(circuit: &Circuit) -> AnalysisResult<()> {
    let mut stack = vec![(circuit, 0_usize)];
    while let Some((block, depth)) = stack.pop() {
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(ResourceLimitError::circuit_to_detector_error_model(
                ResourceKind::RepeatNesting,
                depth as u64,
                MAX_DEM_REPEAT_NESTING as u64,
            )
            .into());
        }
        for item in block.items() {
            if let CircuitItem::RepeatBlock(repeat) = item {
                stack.push((repeat.body(), depth + 1));
            }
        }
    }
    Ok(())
}
