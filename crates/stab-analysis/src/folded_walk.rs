//! Adapts the model's folded circuit walk to analysis errors.

use stab_model::advanced::{FoldedCircuitEvent, FoldedVisitError, visit_folded_circuit};
use stab_model::{Cancellation, Circuit, FoldedWalkError, RepeatNestingLimit};

use crate::{AnalysisError, AnalysisResult};

/// Visits every folded item of `circuit` with its expanded multiplicity, reporting nesting
/// deeper than `limit` through the caller's operation-specific `nesting_error(depth, limit)`.
pub(crate) fn walk_folded(
    circuit: &Circuit,
    limit: RepeatNestingLimit,
    cancellation: &Cancellation,
    nesting_error: impl FnOnce(u64, u64) -> AnalysisError,
    visitor: impl FnMut(FoldedCircuitEvent<'_>) -> AnalysisResult<()>,
) -> AnalysisResult<()> {
    match visit_folded_circuit(circuit, limit, cancellation, visitor) {
        Ok(()) => Ok(()),
        Err(FoldedVisitError::Walk(FoldedWalkError::RepeatNesting { depth, limit })) => Err(
            nesting_error(u64::try_from(depth).unwrap_or(u64::MAX), limit as u64),
        ),
        Err(FoldedVisitError::Walk(FoldedWalkError::Cancelled(cancelled))) => Err(cancelled.into()),
        Err(FoldedVisitError::Visitor(error)) => Err(error),
    }
}

/// Rejects repeat nesting deeper than `limit` without visiting instructions further.
pub(crate) fn check_repeat_nesting(
    circuit: &Circuit,
    limit: RepeatNestingLimit,
    nesting_error: impl FnOnce(u64, u64) -> AnalysisError,
) -> AnalysisResult<()> {
    walk_folded(
        circuit,
        limit,
        &Cancellation::never(),
        nesting_error,
        |_| Ok(()),
    )
}
