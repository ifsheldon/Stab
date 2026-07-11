use crate::{Circuit, CircuitItem, CircuitResult, DetectorErrorModel};

use super::{Analyzer, ErrorAnalyzerOptions, FoldedAnalyzer, reverse_fold};

#[doc(hidden)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ErrorAnalyzerDiagnostics {
    pub used_reverse_fold: bool,
    pub used_bounded_fallback: bool,
    pub recurrence_search_steps: u64,
    pub recurrences_found: u64,
    pub max_recurrence_period: u64,
    pub represented_repeat_iterations: u64,
    pub folded_repeat_iterations: u64,
    pub max_boundary_entries: u64,
    pub emitted_compact_dem_items: u64,
}

#[doc(hidden)]
pub fn __circuit_to_detector_error_model_with_diagnostics(
    circuit: &Circuit,
    options: ErrorAnalyzerOptions,
) -> CircuitResult<(DetectorErrorModel, ErrorAnalyzerDiagnostics)> {
    if options.fold_loops
        && circuit
            .items()
            .iter()
            .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        return FoldedAnalyzer::new(options).analyze_with_diagnostics(circuit);
    }
    let model = Analyzer::new(options).analyze(circuit)?;
    let emitted_compact_dem_items = reverse_fold::compact_dem_item_count(&model);
    Ok((
        model,
        ErrorAnalyzerDiagnostics {
            emitted_compact_dem_items,
            ..ErrorAnalyzerDiagnostics::default()
        },
    ))
}
