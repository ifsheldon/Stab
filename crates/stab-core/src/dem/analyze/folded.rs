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
}
