use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, DemItem, RepeatCount,
};

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
        if let Some((prefix, repeat)) = prefixed_single_repeat(circuit) {
            return self.analyze_prefixed_repeat(&prefix, repeat);
        }

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

    fn analyze_prefixed_repeat(
        &self,
        prefix: &[CircuitInstruction],
        repeat: &RepeatBlock,
    ) -> CircuitResult<DetectorErrorModel> {
        let first_iteration = prefixed_body_circuit(prefix, repeat.body());
        let mut body_options = self.options;
        body_options.fold_loops = false;
        let first_result = Analyzer::new(body_options).analyze_with_stats(&first_iteration)?;
        let body_result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        let prefix_dem = subtract_trailing_body_dem(&first_result.dem, &body_result.dem)?;

        let mut dem = DetectorErrorModel::new();
        push_dem_items(&mut dem, prefix_dem.items());
        if repeat.repeat_count().get() > 1 {
            let mut loop_body = body_result.dem.clone();
            push_detector_shift(&mut loop_body, body_result.detector_count)?;
            dem.push_repeat_block(DemRepeatBlock::new(
                RepeatCount::try_new(repeat.repeat_count().get() - 1)?,
                loop_body,
                repeat.tag().map(ToOwned::to_owned),
            ));
        }
        push_dem_items(&mut dem, body_result.dem.items());
        Ok(dem)
    }

    fn analyze_repeat(&self, repeat: &RepeatBlock) -> CircuitResult<DemRepeatBlock> {
        if repeat_body_contains_only_repeats(repeat.body()) {
            return Ok(DemRepeatBlock::new(
                repeat.repeat_count(),
                self.analyze(repeat.body())?,
                repeat.tag().map(ToOwned::to_owned),
            ));
        }

        let mut body_options = self.options;
        body_options.fold_loops = false;
        let mut result = Analyzer::new(body_options).analyze_with_stats(repeat.body())?;
        push_detector_shift(&mut result.dem, result.detector_count)?;
        Ok(DemRepeatBlock::new(
            repeat.repeat_count(),
            result.dem,
            repeat.tag().map(ToOwned::to_owned),
        ))
    }
}

fn prefixed_single_repeat(circuit: &Circuit) -> Option<(Vec<CircuitInstruction>, &RepeatBlock)> {
    let (last, prefix) = circuit.items().split_last()?;
    let CircuitItem::RepeatBlock(repeat) = last else {
        return None;
    };
    if prefix.is_empty() {
        return None;
    }
    let mut instructions = Vec::with_capacity(prefix.len());
    for item in prefix {
        let CircuitItem::Instruction(instruction) = item else {
            return None;
        };
        instructions.push(instruction.clone());
    }
    Some((instructions, repeat))
}

fn repeat_body_contains_only_repeats(body: &Circuit) -> bool {
    !body.items().is_empty()
        && body
            .items()
            .iter()
            .all(|item| matches!(item, CircuitItem::RepeatBlock(_)))
}

fn prefixed_body_circuit(prefix: &[CircuitInstruction], body: &Circuit) -> Circuit {
    let mut circuit = Circuit::new();
    for instruction in prefix {
        circuit.append_instruction(instruction.clone());
    }
    for item in body.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                circuit.append_instruction(instruction.clone())
            }
            CircuitItem::RepeatBlock(repeat) => circuit.append_repeat_block(repeat.clone()),
        }
    }
    circuit
}

fn subtract_trailing_body_dem(
    first_iteration: &DetectorErrorModel,
    body: &DetectorErrorModel,
) -> CircuitResult<DetectorErrorModel> {
    let first_items = first_iteration.items();
    let body_items = body.items();
    let Some(prefix_len) = first_items.len().checked_sub(body_items.len()) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    let Some(trailing_items) = first_items.get(prefix_len..) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    if trailing_items != body_items {
        return Err(prefixed_repeat_unsupported_error());
    }
    let Some(prefix_items) = first_items.get(..prefix_len) else {
        return Err(prefixed_repeat_unsupported_error());
    };
    let mut prefix = DetectorErrorModel::new();
    push_dem_items(&mut prefix, prefix_items);
    Ok(prefix)
}

fn prefixed_repeat_unsupported_error() -> CircuitError {
    CircuitError::invalid_detector_error_model(
        "analyze_errors --fold_loops currently supports prefixed repeats only when the first iteration ends with the standalone loop-body detector error model",
    )
}

fn push_detector_shift(model: &mut DetectorErrorModel, detector_count: u64) -> CircuitResult<()> {
    if detector_count > 0 {
        model.push_instruction(DemInstruction::shift_detectors(
            Vec::new(),
            detector_count,
            None,
        )?);
    }
    Ok(())
}

fn push_dem_items(model: &mut DetectorErrorModel, items: &[DemItem]) {
    for item in items {
        match item {
            DemItem::Instruction(instruction) => model.push_instruction(instruction.clone()),
            DemItem::RepeatBlock(repeat) => model.push_repeat_block(repeat.clone()),
        }
    }
}
