use crate::{Circuit, CircuitResult};

use super::{Analyzer, DetectorErrorModel, ErrorAnalyzerOptions, reverse_fold};

pub(super) struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    pub(super) fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    pub(super) fn analyze(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        if let Some(model) = reverse_fold::try_analyze(circuit, self.options)? {
            return Ok(model);
        }

        let mut fallback_options = self.options;
        fallback_options.fold_loops = false;
        Analyzer::new(fallback_options).analyze(circuit)
    }

    #[cfg(feature = "ops-contracts")]
    pub(super) fn analyze_with_diagnostics(
        &self,
        circuit: &Circuit,
    ) -> CircuitResult<(DetectorErrorModel, super::ErrorAnalyzerDiagnostics)> {
        if let Some((model, diagnostics)) =
            reverse_fold::try_analyze_with_diagnostics(circuit, self.options)?
        {
            return Ok((model, diagnostics.into()));
        }

        let mut fallback_options = self.options;
        fallback_options.fold_loops = false;
        let model = Analyzer::new(fallback_options).analyze(circuit)?;
        let emitted_compact_dem_items = reverse_fold::compact_dem_item_count(&model)?;
        Ok((
            model,
            super::ErrorAnalyzerDiagnostics {
                used_bounded_fallback: true,
                emitted_compact_dem_items,
                ..super::ErrorAnalyzerDiagnostics::default()
            },
        ))
    }
}
