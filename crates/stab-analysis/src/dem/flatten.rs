use stab_model::advanced::MAX_DEM_REPEAT_NESTING;
use stab_model::{DemInstruction, DemItem, DemTarget, DetectorErrorModel, MemoryBudget};

use crate::{AnalysisError, AnalysisResult, ResourceKind, ResourceLimitError, ResourceOperation};

const OPAQUE_TAG_STORAGE_MULTIPLIER: u64 = 4;

/// Resource policy for materializing a detector error model's folded repeat structure.
///
/// The memory budget bounds the conservative retained size of the flattened model, admitted
/// before output is allocated. It does not constrain compact traversal or
/// [`DetectorErrorModel::iter_flattened_instructions`], whose callers may stop before consuming
/// the represented model. Repeat counts, expanded instructions, and payload counts are not
/// separately capped; repeat nesting remains a fixed parser and model safety invariant.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DemFlattenLimits {
    memory_budget: MemoryBudget,
}

impl DemFlattenLimits {
    pub const fn new(memory_budget: MemoryBudget) -> Self {
        Self { memory_budget }
    }

    /// Returns the budget for the materialized result.
    pub const fn memory_budget(self) -> MemoryBudget {
        self.memory_budget
    }

    /// Sets the budget for the materialized result.
    #[must_use]
    pub const fn with_memory_budget(mut self, memory_budget: MemoryBudget) -> Self {
        self.memory_budget = memory_budget;
        self
    }
}

pub(crate) fn validate_flattening_budget_with_limits(
    model: &DetectorErrorModel,
    context: &'static str,
    limits: DemFlattenLimits,
) -> AnalysisResult<DemFlatteningBudget> {
    let mut budget = DemFlatteningBudget::new(limits);
    validate_flattening_budget_items(model, 1, 0, context, &mut budget)?;
    Ok(budget)
}

fn validate_flattening_budget_items(
    model: &DetectorErrorModel,
    multiplier: u64,
    depth: usize,
    context: &'static str,
    budget: &mut DemFlatteningBudget,
) -> AnalysisResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(AnalysisError::invalid_detector_error_model(format!(
            "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}"
        )));
    }
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => {
                budget.add_instruction(instruction, multiplier, context)?;
            }
            DemItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count == 0 {
                    continue;
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        AnalysisError::invalid_detector_error_model(format!(
                            "DEM {context} repeat expansion count overflowed"
                        ))
                    })?;
                validate_flattening_budget_items(
                    repeat.body(),
                    repeated_multiplier,
                    depth + 1,
                    context,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DemFlatteningBudget {
    limits: DemFlattenLimits,
    expanded_instructions: u64,
    materialized_bytes: u64,
}

impl DemFlatteningBudget {
    const fn new(limits: DemFlattenLimits) -> Self {
        Self {
            limits,
            expanded_instructions: 0,
            materialized_bytes: 0,
        }
    }

    pub(crate) fn materialized_capacity(self) -> AnalysisResult<usize> {
        dem_item_capacity(self.expanded_instructions)
    }

    fn add_instruction(
        &mut self,
        instruction: &DemInstruction,
        multiplier: u64,
        context: &'static str,
    ) -> AnalysisResult<()> {
        self.add_expanded_instructions(multiplier, context)?;
        let tag_bytes = usize_to_dem_u64(instruction.tag_bytes().map_or(0, <[u8]>::len))?;
        let bytes_per_instruction = checked_dem_sum(
            checked_dem_sum(
                usize_to_dem_u64(std::mem::size_of::<DemItem>())?,
                checked_dem_product(
                    usize_to_dem_u64(std::mem::size_of::<DemTarget>())?,
                    usize_to_dem_u64(instruction.targets().len())?,
                    "materialized byte count",
                    context,
                )?,
                "materialized byte count",
                context,
            )?,
            checked_dem_sum(
                checked_dem_product(
                    usize_to_dem_u64(std::mem::size_of::<f64>())?,
                    usize_to_dem_u64(instruction.args().len())?,
                    "materialized byte count",
                    context,
                )?,
                checked_dem_product(
                    tag_bytes,
                    OPAQUE_TAG_STORAGE_MULTIPLIER,
                    "materialized byte count",
                    context,
                )?,
                "materialized byte count",
                context,
            )?,
            "materialized byte count",
            context,
        )?;
        self.materialized_bytes = add_scaled_dem_resource(
            self.materialized_bytes,
            bytes_per_instruction,
            multiplier,
            "materialized byte count",
            context,
        )?;
        let budget = self.limits.memory_budget();
        if !budget.admits(u128::from(self.materialized_bytes)) {
            return Err(ResourceLimitError::memory_budget(
                ResourceOperation::DetectorErrorModelFlatten,
                ResourceKind::MaterializedBytes,
                self.materialized_bytes,
                budget.bytes(),
            )
            .into());
        }
        Ok(())
    }

    fn add_expanded_instructions(
        &mut self,
        count: u64,
        context: &'static str,
    ) -> AnalysisResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    AnalysisError::invalid_detector_error_model(format!(
                        "DEM {context} expanded instruction count overflowed"
                    ))
                })?;
        dem_item_capacity(self.expanded_instructions)?;
        Ok(())
    }
}

fn dem_item_capacity(count: u64) -> AnalysisResult<usize> {
    let element_size = std::mem::size_of::<DemItem>().max(1);
    let platform_limit = (isize::MAX as usize) / element_size;
    let platform_limit_u64 = u64::try_from(platform_limit).unwrap_or(u64::MAX);
    if count > platform_limit_u64 {
        return Err(
            ResourceLimitError::dem_flatten_materialized_units(count, platform_limit_u64).into(),
        );
    }
    usize::try_from(count).map_err(|_| {
        ResourceLimitError::dem_flatten_materialized_units(count, platform_limit_u64).into()
    })
}

fn add_scaled_dem_resource(
    current: u64,
    per_instruction: u64,
    multiplier: u64,
    resource: &'static str,
    context: &'static str,
) -> AnalysisResult<u64> {
    let addition = checked_dem_product(per_instruction, multiplier, resource, context)?;
    checked_dem_sum(current, addition, resource, context)
}

fn checked_dem_sum(
    left: u64,
    right: u64,
    resource: &'static str,
    context: &'static str,
) -> AnalysisResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("DEM {context} {resource} overflowed"))
    })
}

fn checked_dem_product(
    left: u64,
    right: u64,
    resource: &'static str,
    context: &'static str,
) -> AnalysisResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::invalid_detector_error_model(format!("DEM {context} {resource} overflowed"))
    })
}

fn usize_to_dem_u64(value: usize) -> AnalysisResult<u64> {
    u64::try_from(value)
        .map_err(|_| AnalysisError::invalid_detector_error_model("DEM flattened count exceeds u64"))
}
