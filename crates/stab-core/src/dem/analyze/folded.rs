use crate::{Circuit, CircuitError, CircuitItem, CircuitResult};

use super::{
    Analyzer, DemInstruction, DemRepeatBlock, DetectorErrorModel, ErrorAnalyzerOptions, RepeatBlock,
};

pub(super) struct FoldedAnalyzer {
    options: ErrorAnalyzerOptions,
}

impl FoldedAnalyzer {
    pub(super) fn new(options: ErrorAnalyzerOptions) -> Self {
        Self { options }
    }

    pub(super) fn analyze(&self, circuit: &Circuit) -> CircuitResult<DetectorErrorModel> {
        let mut dem = DetectorErrorModel::new();
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(_) => {
                    return Err(CircuitError::invalid_detector_error_model(
                        "analyze_errors --fold_loops currently supports top-level repeat blocks only",
                    ));
                }
                CircuitItem::RepeatBlock(repeat) => {
                    dem.push_repeat_block(self.analyze_repeat(repeat)?);
                }
            }
        }
        Ok(dem)
    }

    fn analyze_repeat(&self, repeat: &RepeatBlock) -> CircuitResult<DemRepeatBlock> {
        let mut body_options = self.options;
        body_options.fold_loops = false;
        let mut result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        if result.detector_count > 0 {
            result.dem.push_instruction(DemInstruction::shift_detectors(
                Vec::new(),
                result.detector_count,
                None,
            )?);
        }
        Ok(DemRepeatBlock::new(
            repeat.repeat_count(),
            result.dem,
            repeat.tag().map(ToOwned::to_owned),
        ))
    }
}
