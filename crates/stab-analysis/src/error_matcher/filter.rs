use std::collections::{BTreeSet, HashMap};
use std::ops::ControlFlow;

use stab_model::advanced::{
    CancellationCountdown, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
    FoldedDemTraversal, FoldedDemVisitor,
};
use stab_model::{
    Cancellation, DemInstruction, DemInstructionKind, DemRepeatBlock, DemTarget,
    DetectorErrorModel, MemoryBudget,
};

use crate::{AnalysisError, AnalysisResult};

/// Retained error keys are admitted against the default memory budget.
const ERROR_MATCHER_FILTER_MEMORY_BUDGET: MemoryBudget = MemoryBudget::DEFAULT;

pub(super) fn error_keys_from_dem(
    model: &DetectorErrorModel,
    cancellation: &Cancellation,
) -> AnalysisResult<Vec<Vec<DemTarget>>> {
    let traversal = FoldedDemTraversal::new(model)?;
    traversal.validate_repeat_depth("ErrorMatcher filter")?;
    let block_policies = FilterBlockPolicies::new(traversal.root());
    let mut keys = Vec::new();
    let mut visitor = ErrorMatcherFilterVisitor {
        retained_key_bytes: 0,
        block_policies,
        keys: &mut keys,
        cancellation,
        countdown: CancellationCountdown::new(),
    };
    let _ = traversal.try_visit(&mut visitor)?;
    Ok(keys)
}

struct ErrorMatcherFilterVisitor<'a> {
    retained_key_bytes: u64,
    block_policies: FilterBlockPolicies,
    keys: &'a mut Vec<Vec<DemTarget>>,
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl ErrorMatcherFilterVisitor<'_> {
    /// Charges one retained error key before it is retained.
    fn admit_key(&mut self, target_count: usize) -> AnalysisResult<()> {
        let key_bytes = (size_of::<Vec<DemTarget>>() as u64)
            .saturating_add((target_count as u64).saturating_mul(size_of::<DemTarget>() as u64));
        let next = self.retained_key_bytes.saturating_add(key_bytes);
        if !ERROR_MATCHER_FILTER_MEMORY_BUDGET.admits(u128::from(next)) {
            return Err(AnalysisError::invalid_detector_error_model(format!(
                "DEM ErrorMatcher filter needs at least {next} bytes of error keys, exceeding the {}-byte memory budget",
                ERROR_MATCHER_FILTER_MEMORY_BUDGET.bytes()
            )));
        }
        self.retained_key_bytes = next;
        Ok(())
    }
}

impl FoldedDemVisitor for ErrorMatcherFilterVisitor<'_> {
    type Error = AnalysisError;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> AnalysisResult<ControlFlow<()>> {
        self.countdown.tick(self.cancellation)?;
        if instruction.kind() == DemInstructionKind::Error {
            self.admit_key(instruction.targets().len())?;
            self.keys.push(canonical_error_key(
                instruction.targets(),
                state.detector_offset(),
            )?);
        }
        Ok(ControlFlow::Continue(()))
    }

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> AnalysisResult<DemRepeatSelection> {
        if body.summary().error_count()? == 0 {
            return Ok(DemRepeatSelection::Skip);
        }
        if self
            .block_policies
            .policy_for(body)?
            .compact_error_count
            .clone()?
            .is_some()
        {
            return Ok(DemRepeatSelection::FoldOnce);
        }
        Ok(DemRepeatSelection::Expand {
            max_total_iterations: u64::MAX,
            context: "ErrorMatcher filter",
        })
    }

    fn poll_cancellation(&mut self) -> AnalysisResult<()> {
        Ok(self.countdown.tick(self.cancellation)?)
    }
}

#[derive(Clone, Debug)]
struct FilterBlockPolicy {
    compact_error_count: AnalysisResult<Option<u64>>,
}

#[derive(Debug)]
struct FilterBlockPolicies {
    by_block: HashMap<usize, FilterBlockPolicy>,
}

impl FilterBlockPolicies {
    fn new(root: &FoldedDemBlock<'_>) -> Self {
        let mut by_block = HashMap::new();
        summarize_filter_block_policy(root, &mut by_block);
        Self { by_block }
    }

    fn policy_for(&self, block: &FoldedDemBlock<'_>) -> AnalysisResult<&FilterBlockPolicy> {
        self.by_block.get(&block.compact_id()).ok_or_else(|| {
            AnalysisError::invalid_detector_error_model(
                "DEM ErrorMatcher filter compact policy is missing a folded block",
            )
        })
    }
}

fn summarize_filter_block_policy(
    block: &FoldedDemBlock<'_>,
    by_block: &mut HashMap<usize, FilterBlockPolicy>,
) -> FilterBlockPolicy {
    let mut compact_error_count = Ok(Some(0_u64));

    for item in block.items() {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                let Some(count) = active_compact_count(&compact_error_count) else {
                    continue;
                };
                compact_error_count = update_filter_instruction_count(count, instruction);
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let child = summarize_filter_block_policy(body, by_block);
                let Some(count) = active_compact_count(&compact_error_count) else {
                    continue;
                };
                if repeat.repeat_count().get() == 0 {
                    continue;
                }
                compact_error_count = body
                    .summary()
                    .detector_shift()
                    .map_err(AnalysisError::from)
                    .and_then(|detector_shift| {
                        if detector_shift != 0 {
                            return Ok(None);
                        }
                        child.compact_error_count.clone().and_then(|child_count| {
                            let Some(child_count) = child_count else {
                                return Ok(None);
                            };
                            count.checked_add(child_count).map(Some).ok_or_else(|| {
                                AnalysisError::invalid_detector_error_model(
                                    "DEM ErrorMatcher filter compact-repeat error count overflowed",
                                )
                            })
                        })
                    });
            }
        }
    }

    let policy = FilterBlockPolicy {
        compact_error_count,
    };
    by_block.insert(block.compact_id(), policy.clone());
    policy
}

fn active_compact_count(count: &AnalysisResult<Option<u64>>) -> Option<u64> {
    match count {
        Ok(Some(count)) => Some(*count),
        Ok(None) | Err(_) => None,
    }
}

fn update_filter_instruction_count(
    count: u64,
    instruction: &DemInstruction,
) -> AnalysisResult<Option<u64>> {
    match instruction.kind() {
        DemInstructionKind::Error => {
            if instruction
                .targets()
                .iter()
                .any(|target| matches!(target, DemTarget::Numeric(_)))
            {
                return Ok(None);
            }
            count.checked_add(1).map(Some).ok_or_else(|| {
                AnalysisError::invalid_detector_error_model(
                    "DEM ErrorMatcher filter compact-repeat error count overflowed",
                )
            })
        }
        DemInstructionKind::ShiftDetectors
            if stab_model::advanced::dem_instruction_detector_shift(instruction)? == 0 =>
        {
            Ok(Some(count))
        }
        DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => Ok(Some(count)),
        DemInstructionKind::ShiftDetectors => Ok(None),
    }
}

fn canonical_error_key(
    targets: &[DemTarget],
    detector_offset: u64,
) -> AnalysisResult<Vec<DemTarget>> {
    let mut toggled = BTreeSet::new();
    for target in targets {
        let shifted = match *target {
            DemTarget::RelativeDetector(detector) => DemTarget::relative_detector(
                detector.get().checked_add(detector_offset).ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model("detector id overflowed")
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

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "unit tests use fixed valid DEM fixtures for compact policy diagnostics"
    )]

    use super::*;

    #[test]
    fn filter_policy_cache_scales_with_compact_nested_repeats() {
        let repeated = DetectorErrorModel::from_dem_str(
            "repeat 1000000000 {\n    repeat 1000000000 {\n        error(0.1) D0 D0 D1 ^ L0\n        error(0.2) L1\n        shift_detectors 0\n    }\n}\n",
        )
        .unwrap();
        let compact = DetectorErrorModel::from_dem_str(
            "error(0.1) D0 D0 D1 ^ L0\nerror(0.2) L1\nshift_detectors 0\n",
        )
        .unwrap();
        let traversal = FoldedDemTraversal::new(&repeated).unwrap();
        let policies = FilterBlockPolicies::new(traversal.root());

        assert_eq!(policies.by_block.len(), 3);
        assert_eq!(
            error_keys_from_dem(&repeated, &Cancellation::never()).unwrap(),
            error_keys_from_dem(&compact, &Cancellation::never()).unwrap()
        );
    }
}
