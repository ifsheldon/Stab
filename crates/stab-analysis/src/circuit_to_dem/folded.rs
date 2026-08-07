use stab_model::Circuit;

use crate::AnalysisResult;

use super::{DetectorErrorModel, ErrorAnalyzerOptions, reverse_fold};

pub(super) struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    pub(super) fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    pub(super) fn analyze(&self, circuit: &Circuit) -> AnalysisResult<DetectorErrorModel> {
        reverse_fold::try_analyze(circuit, self.options)
    }
}
