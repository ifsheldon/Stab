use super::{
    DemInstruction, DemItem, DemTarget, DetectorErrorModel, MAX_DEM_FLATTEN_ARGUMENT_VALUES,
    MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS, MAX_DEM_FLATTEN_MATERIALIZED_BYTES,
    MAX_DEM_FLATTEN_REPEAT_ITERATIONS, MAX_DEM_FLATTEN_REPEAT_UNROLL,
    MAX_DEM_FLATTEN_TARGET_OCCURRENCES, MAX_DEM_REPEAT_NESTING,
};
use crate::{CircuitError, CircuitResult, ResourceLimitError};

const OPAQUE_TAG_STORAGE_MULTIPLIER: u64 = 4;

/// Resource policy for materializing a detector error model's folded repeat structure.
///
/// These limits belong specifically to DEM flattening. They do not constrain compact traversal
/// or [`DetectorErrorModel::iter_flattened_instructions`], whose callers may stop before consuming
/// the represented model. Repeat nesting remains a fixed parser and model safety invariant instead
/// of a configurable flattening policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DemFlattenLimits {
    max_repeat_unroll: u64,
    max_expanded_instructions: u64,
    max_repeat_iterations: u64,
    max_target_occurrences: u64,
    max_argument_values: u64,
    max_materialized_bytes: u64,
}

impl DemFlattenLimits {
    pub const DEFAULT_MAX_REPEAT_UNROLL: u64 = MAX_DEM_FLATTEN_REPEAT_UNROLL;
    pub const DEFAULT_MAX_EXPANDED_INSTRUCTIONS: u64 = MAX_DEM_FLATTEN_EXPANDED_INSTRUCTIONS;
    pub const DEFAULT_MAX_REPEAT_ITERATIONS: u64 = MAX_DEM_FLATTEN_REPEAT_ITERATIONS;
    pub const DEFAULT_MAX_TARGET_OCCURRENCES: u64 = MAX_DEM_FLATTEN_TARGET_OCCURRENCES;
    pub const DEFAULT_MAX_ARGUMENT_VALUES: u64 = MAX_DEM_FLATTEN_ARGUMENT_VALUES;
    pub const DEFAULT_MAX_MATERIALIZED_BYTES: u64 = MAX_DEM_FLATTEN_MATERIALIZED_BYTES;

    /// Returns the largest repeat count accepted for one repeat block.
    pub const fn max_repeat_unroll(self) -> u64 {
        self.max_repeat_unroll
    }

    /// Returns the largest total number of materialized instructions.
    pub const fn max_expanded_instructions(self) -> u64 {
        self.max_expanded_instructions
    }

    /// Returns the largest aggregate number of expanded repeat iterations.
    ///
    /// Nested repeats contribute at every represented repeat level. For example, an outer repeat
    /// of two containing an inner repeat of three contributes `2 + 2 * 3 = 8` iterations.
    pub const fn max_repeat_iterations(self) -> u64 {
        self.max_repeat_iterations
    }

    /// Returns the largest total number of retained target occurrences.
    pub const fn max_target_occurrences(self) -> u64 {
        self.max_target_occurrences
    }

    /// Returns the largest total number of retained argument values.
    pub const fn max_argument_values(self) -> u64 {
        self.max_argument_values
    }

    /// Returns the conservative byte ceiling for the materialized result.
    pub const fn max_materialized_bytes(self) -> u64 {
        self.max_materialized_bytes
    }

    /// Sets the largest repeat count accepted for one repeat block.
    #[must_use]
    pub const fn with_max_repeat_unroll(mut self, limit: u64) -> Self {
        self.max_repeat_unroll = limit;
        self
    }

    /// Sets the largest total number of materialized instructions.
    #[must_use]
    pub const fn with_max_expanded_instructions(mut self, limit: u64) -> Self {
        self.max_expanded_instructions = limit;
        self
    }

    /// Sets the largest aggregate number of expanded repeat iterations.
    #[must_use]
    pub const fn with_max_repeat_iterations(mut self, limit: u64) -> Self {
        self.max_repeat_iterations = limit;
        self
    }

    /// Sets the largest total number of retained target occurrences.
    #[must_use]
    pub const fn with_max_target_occurrences(mut self, limit: u64) -> Self {
        self.max_target_occurrences = limit;
        self
    }

    /// Sets the largest total number of retained argument values.
    #[must_use]
    pub const fn with_max_argument_values(mut self, limit: u64) -> Self {
        self.max_argument_values = limit;
        self
    }

    /// Sets the conservative byte ceiling for the materialized result.
    #[must_use]
    pub const fn with_max_materialized_bytes(mut self, limit: u64) -> Self {
        self.max_materialized_bytes = limit;
        self
    }
}

impl Default for DemFlattenLimits {
    fn default() -> Self {
        Self {
            max_repeat_unroll: Self::DEFAULT_MAX_REPEAT_UNROLL,
            max_expanded_instructions: Self::DEFAULT_MAX_EXPANDED_INSTRUCTIONS,
            max_repeat_iterations: Self::DEFAULT_MAX_REPEAT_ITERATIONS,
            max_target_occurrences: Self::DEFAULT_MAX_TARGET_OCCURRENCES,
            max_argument_values: Self::DEFAULT_MAX_ARGUMENT_VALUES,
            max_materialized_bytes: Self::DEFAULT_MAX_MATERIALIZED_BYTES,
        }
    }
}

pub(crate) fn validate_flattening_budget_with_limits(
    model: &DetectorErrorModel,
    context: &'static str,
    limits: DemFlattenLimits,
) -> CircuitResult<DemFlatteningBudget> {
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
) -> CircuitResult<()> {
    if depth > MAX_DEM_REPEAT_NESTING {
        return Err(CircuitError::invalid_detector_error_model(format!(
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
                if repeat_count > budget.limits.max_repeat_unroll {
                    return Err(ResourceLimitError::dem_flatten_repeat_count(
                        repeat_count,
                        budget.limits.max_repeat_unroll,
                    )
                    .into());
                }
                let repeated_multiplier =
                    multiplier.checked_mul(repeat_count).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model(format!(
                            "DEM {context} repeat expansion count overflowed"
                        ))
                    })?;
                budget.add_repeat_iterations(repeated_multiplier, context)?;
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
    repeat_iterations: u64,
    target_occurrences: u64,
    argument_values: u64,
    materialized_bytes: u64,
}

impl DemFlatteningBudget {
    const fn new(limits: DemFlattenLimits) -> Self {
        Self {
            limits,
            expanded_instructions: 0,
            repeat_iterations: 0,
            target_occurrences: 0,
            argument_values: 0,
            materialized_bytes: 0,
        }
    }

    pub(crate) fn materialized_capacity(self) -> CircuitResult<usize> {
        dem_item_capacity(self.expanded_instructions)
    }

    fn add_instruction(
        &mut self,
        instruction: &DemInstruction,
        multiplier: u64,
        context: &'static str,
    ) -> CircuitResult<()> {
        self.add_expanded_instructions(multiplier, context)?;
        self.target_occurrences = add_scaled_dem_resource(
            self.target_occurrences,
            usize_to_dem_u64(instruction.targets().len())?,
            multiplier,
            "target occurrence count",
            context,
        )?;
        if self.target_occurrences > self.limits.max_target_occurrences {
            return Err(ResourceLimitError::dem_flatten_target_occurrences(
                self.target_occurrences,
                self.limits.max_target_occurrences,
            )
            .into());
        }

        self.argument_values = add_scaled_dem_resource(
            self.argument_values,
            usize_to_dem_u64(instruction.args().len())?,
            multiplier,
            "argument value count",
            context,
        )?;
        if self.argument_values > self.limits.max_argument_values {
            return Err(ResourceLimitError::dem_flatten_argument_values(
                self.argument_values,
                self.limits.max_argument_values,
            )
            .into());
        }

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
        if self.materialized_bytes > self.limits.max_materialized_bytes {
            return Err(ResourceLimitError::dem_flatten_materialized_bytes(
                self.materialized_bytes,
                self.limits.max_materialized_bytes,
            )
            .into());
        }
        Ok(())
    }

    fn add_expanded_instructions(
        &mut self,
        count: u64,
        context: &'static str,
    ) -> CircuitResult<()> {
        self.expanded_instructions =
            self.expanded_instructions
                .checked_add(count)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(format!(
                        "DEM {context} expanded instruction count overflowed"
                    ))
                })?;
        if self.expanded_instructions > self.limits.max_expanded_instructions {
            return Err(ResourceLimitError::dem_flatten_expanded_instructions(
                self.expanded_instructions,
                self.limits.max_expanded_instructions,
            )
            .into());
        }
        dem_item_capacity(self.expanded_instructions)?;
        Ok(())
    }

    fn add_repeat_iterations(&mut self, count: u64, context: &'static str) -> CircuitResult<()> {
        self.repeat_iterations = self.repeat_iterations.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat iteration count overflowed"
            ))
        })?;
        if self.repeat_iterations > self.limits.max_repeat_iterations {
            return Err(ResourceLimitError::dem_flatten_repeat_iterations(
                self.repeat_iterations,
                self.limits.max_repeat_iterations,
            )
            .into());
        }
        Ok(())
    }
}

fn dem_item_capacity(count: u64) -> CircuitResult<usize> {
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
) -> CircuitResult<u64> {
    let addition = checked_dem_product(per_instruction, multiplier, resource, context)?;
    checked_dem_sum(current, addition, resource, context)
}

fn checked_dem_sum(
    left: u64,
    right: u64,
    resource: &'static str,
    context: &'static str,
) -> CircuitResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("DEM {context} {resource} overflowed"))
    })
}

fn checked_dem_product(
    left: u64,
    right: u64,
    resource: &'static str,
    context: &'static str,
) -> CircuitResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        CircuitError::invalid_detector_error_model(format!("DEM {context} {resource} overflowed"))
    })
}

fn usize_to_dem_u64(value: usize) -> CircuitResult<u64> {
    u64::try_from(value)
        .map_err(|_| CircuitError::invalid_detector_error_model("DEM flattened count exceeds u64"))
}
