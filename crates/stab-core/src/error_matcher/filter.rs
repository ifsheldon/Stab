use std::collections::BTreeSet;
use std::ops::ControlFlow;

use crate::dem::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemTraversal, FoldedDemVisitor,
    MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS, MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
    MAX_DEM_FLATTEN_REPEAT_UNROLL,
};
use crate::{
    CircuitError, CircuitResult, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget,
    DetectorErrorModel,
};

pub(super) fn error_keys_from_dem(
    model: &DetectorErrorModel,
) -> CircuitResult<Vec<Vec<DemTarget>>> {
    let traversal = FoldedDemTraversal::new(model)?;
    traversal.validate_repeat_depth("ErrorMatcher filter")?;
    let mut keys = Vec::new();
    let mut visitor = ErrorMatcherFilterVisitor {
        expanded_instructions: 0,
        keys: &mut keys,
    };
    let _ = traversal.try_visit(&mut visitor)?;
    Ok(keys)
}

struct ErrorMatcherFilterVisitor<'a> {
    expanded_instructions: u64,
    keys: &'a mut Vec<Vec<DemTarget>>,
}

impl ErrorMatcherFilterVisitor<'_> {
    fn add_expanded_instruction(&mut self) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions.checked_add(1).ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
                    "DEM ErrorMatcher filter expanded instruction count overflowed",
                )
            })?;
        if self.expanded_instructions > MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM ErrorMatcher filter currently supports at most {MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS} expanded instructions, got at least {}",
                self.expanded_instructions
            )));
        }
        Ok(())
    }
}

impl FoldedDemVisitor for ErrorMatcherFilterVisitor<'_> {
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>> {
        if state.folded_repeat_depth() == 0 || instruction.kind() == DemInstructionKind::Error {
            self.add_expanded_instruction()?;
        }
        if instruction.kind() == DemInstructionKind::Error {
            self.keys.push(canonical_error_key(
                instruction.targets(),
                state.detector_offset(),
            )?);
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> CircuitResult<DemRepeatSelection> {
        if body.summary().error_count()? == 0 {
            return Ok(DemRepeatSelection::Skip);
        }
        if body.summary().compact_filter_error_count()?.is_some() {
            return Ok(DemRepeatSelection::FoldOnce);
        }
        let repeat_count = repeat.repeat_count().get();
        if repeat_count > MAX_DEM_FLATTEN_REPEAT_UNROLL {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM ErrorMatcher filter currently supports repeat counts up to {MAX_DEM_FLATTEN_REPEAT_UNROLL}, got {repeat_count}"
            )));
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: MAX_DEM_FLATTEN_REPEAT_ITERATIONS,
            context: "ErrorMatcher filter",
        })
    }
}

fn canonical_error_key(
    targets: &[DemTarget],
    detector_offset: u64,
) -> CircuitResult<Vec<DemTarget>> {
    let mut toggled = BTreeSet::new();
    for target in targets {
        let shifted = match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector.get().checked_add(detector_offset).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("detector id overflowed")
                })?,
            )?,
            DemTarget::LogicalObservable(_) => *target,
            DemTarget::Separator | DemTarget::Numeric(_) => continue,
        };
        if !toggled.insert(shifted) {
            toggled.remove(&shifted);
        }
    }
    Ok(toggled.into_iter().collect())
}
