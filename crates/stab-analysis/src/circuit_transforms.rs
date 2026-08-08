use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, Gate, QubitId, RepeatNestingLimit, Target,
    advanced::CircuitBuilder,
};

use crate::{
    AnalysisError, AnalysisResult, CircuitPass, CircuitPassInput, CircuitPassOutput,
    CircuitPassResources, ResourceLimitError,
};

const MAX_MATERIALIZED_FLATTENED_OPERATIONS: u64 = 1_000_000;
const MAX_MATERIALIZED_FLATTENED_TARGETS: u64 = 32_000_000;
const MAX_MATERIALIZED_FLATTENED_ARGUMENTS: u64 = 16_000_000;
const MAX_MATERIALIZED_FLATTENED_BYTES: u64 = 512 * 1024 * 1024;

/// Resource policy for operation-owned circuit flattening.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircuitFlattenLimits {
    max_expanded_operations: u64,
    max_expanded_targets: u64,
    max_expanded_arguments: u64,
    max_materialized_bytes: u64,
}

impl CircuitFlattenLimits {
    /// The default maximum for owned flattened circuit materialization.
    pub const DEFAULT_MAX_EXPANDED_OPERATIONS: u64 = MAX_MATERIALIZED_FLATTENED_OPERATIONS;
    /// The default maximum number of retained target occurrences.
    pub const DEFAULT_MAX_EXPANDED_TARGETS: u64 = MAX_MATERIALIZED_FLATTENED_TARGETS;
    /// The default maximum number of retained argument values.
    pub const DEFAULT_MAX_EXPANDED_ARGUMENTS: u64 = MAX_MATERIALIZED_FLATTENED_ARGUMENTS;
    /// The default conservative byte ceiling for the materialized result.
    pub const DEFAULT_MAX_MATERIALIZED_BYTES: u64 = MAX_MATERIALIZED_FLATTENED_BYTES;

    /// Returns the configured expanded-operation limit.
    pub const fn max_expanded_operations(self) -> u64 {
        self.max_expanded_operations
    }

    /// Returns the configured retained-target limit.
    pub const fn max_expanded_targets(self) -> u64 {
        self.max_expanded_targets
    }

    /// Returns the configured retained-argument limit.
    pub const fn max_expanded_arguments(self) -> u64 {
        self.max_expanded_arguments
    }

    /// Returns the configured conservative materialized-byte limit.
    pub const fn max_materialized_bytes(self) -> u64 {
        self.max_materialized_bytes
    }

    /// Returns this policy with a different expanded-operation limit.
    #[must_use]
    pub const fn with_max_expanded_operations(mut self, max_expanded_operations: u64) -> Self {
        self.max_expanded_operations = max_expanded_operations;
        self
    }

    /// Returns this policy with a different retained-target limit.
    #[must_use]
    pub const fn with_max_expanded_targets(mut self, max_expanded_targets: u64) -> Self {
        self.max_expanded_targets = max_expanded_targets;
        self
    }

    /// Returns this policy with a different retained-argument limit.
    #[must_use]
    pub const fn with_max_expanded_arguments(mut self, max_expanded_arguments: u64) -> Self {
        self.max_expanded_arguments = max_expanded_arguments;
        self
    }

    /// Returns this policy with a different conservative materialized-byte limit.
    #[must_use]
    pub const fn with_max_materialized_bytes(mut self, max_materialized_bytes: u64) -> Self {
        self.max_materialized_bytes = max_materialized_bytes;
        self
    }
}

impl Default for CircuitFlattenLimits {
    fn default() -> Self {
        Self {
            max_expanded_operations: Self::DEFAULT_MAX_EXPANDED_OPERATIONS,
            max_expanded_targets: Self::DEFAULT_MAX_EXPANDED_TARGETS,
            max_expanded_arguments: Self::DEFAULT_MAX_EXPANDED_ARGUMENTS,
            max_materialized_bytes: Self::DEFAULT_MAX_MATERIALIZED_BYTES,
        }
    }
}

/// Returns a circuit with repeat blocks unrolled and coordinate shifts applied.
///
/// `SHIFT_COORDS` instructions are absorbed into subsequent `QUBIT_COORDS` and `DETECTOR`
/// instructions, matching Stim's materialized `flattened` transform. Because this returns an
/// owned circuit, expansions above one million operations are rejected; use the lazy flattened
/// iterators for raw repeat traversal when materialization is not required.
pub fn flattened_circuit(circuit: &Circuit) -> AnalysisResult<Circuit> {
    flattened_circuit_with_limits(circuit, CircuitFlattenLimits::default())
}

/// Returns a materialized flattened circuit using explicit resource limits.
///
/// The expanded operation count is validated before any output circuit is allocated or mutated.
pub fn flattened_circuit_with_limits(
    circuit: &Circuit,
    limits: CircuitFlattenLimits,
) -> AnalysisResult<Circuit> {
    let estimate = validate_flattening_budget(circuit, limits)?;
    let capacity = validate_materialized_instruction_capacity(estimate.operations)?;
    let mut result = CircuitBuilder::new();
    result.try_reserve_exact(capacity)?;
    visit_flattened_operations(circuit, &mut Vec::new(), |instruction| {
        result
            .try_append_instruction(instruction)
            .map_err(Into::into)
    })?;
    Ok(result.finish())
}

/// Returns owned flattened instructions without Stim-style adjacent-instruction fusion.
///
/// This is the Rust transform counterpart to Stim's deprecated `flattened_operations` surface.
/// It applies coordinate shifts and unrolls repeats, but preserves each yielded operation as an
/// independent instruction.
pub fn flattened_circuit_operations(circuit: &Circuit) -> AnalysisResult<Vec<CircuitInstruction>> {
    flattened_circuit_operations_with_limits(circuit, CircuitFlattenLimits::default())
}

/// Returns owned flattened instructions using explicit resource limits.
///
/// Repeat nesting and the expanded operation count are admitted before output allocation.
pub fn flattened_circuit_operations_with_limits(
    circuit: &Circuit,
    limits: CircuitFlattenLimits,
) -> AnalysisResult<Vec<CircuitInstruction>> {
    let estimate = validate_flattening_budget(circuit, limits)?;
    let capacity = validate_materialized_instruction_capacity(estimate.operations)?;
    let mut result = Vec::new();
    result.try_reserve_exact(capacity).map_err(|error| {
        AnalysisError::invalid_domain_value(
            "flattened circuit allocation",
            format!(
                "unable to reserve {} instruction slots: {error}",
                estimate.operations
            ),
        )
    })?;
    visit_flattened_operations(circuit, &mut Vec::new(), |instruction| {
        result.push(instruction);
        Ok(())
    })?;
    Ok(result)
}

fn validate_materialized_instruction_capacity(count: u64) -> AnalysisResult<usize> {
    let element_size = std::mem::size_of::<CircuitInstruction>().max(1);
    let platform_limit = (isize::MAX as usize) / element_size;
    let platform_limit_u64 = u64::try_from(platform_limit).unwrap_or(u64::MAX);
    if count > platform_limit_u64 {
        return Err(ResourceLimitError::circuit_flatten_materialized_units(
            count,
            platform_limit_u64,
        )
        .into());
    }
    usize::try_from(count).map_err(|_| {
        ResourceLimitError::circuit_flatten_materialized_units(count, platform_limit_u64).into()
    })
}

/// Returns a copy of a circuit with noisy behavior removed while preserving records.
///
/// Ordinary noise instructions are dropped. Noisy measurement probabilities are stripped, and
/// heralded noise instructions become deterministic zero `MPAD` results so measurement-record
/// indexing stays unchanged.
pub fn circuit_without_noise(circuit: &Circuit) -> AnalysisResult<Circuit> {
    circuit_without_noise_impl(circuit, &mut ())
}

/// Built-in pass that removes noisy behavior while preserving measurement records.
#[derive(Clone, Copy, Debug, Default)]
pub struct WithoutNoisePass;

/// Options for [`WithoutNoisePass`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WithoutNoiseOptions;

/// Represented structural changes made by [`WithoutNoisePass`].
///
/// Instructions inside a folded repeat body are counted once, independently of the repeat count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WithoutNoiseReport {
    removed_noise_instructions: u64,
    stripped_measurement_probabilities: u64,
    replaced_heralded_noise_instructions: u64,
}

impl WithoutNoiseReport {
    pub const fn removed_noise_instructions(self) -> u64 {
        self.removed_noise_instructions
    }

    pub const fn stripped_measurement_probabilities(self) -> u64 {
        self.stripped_measurement_probabilities
    }

    pub const fn replaced_heralded_noise_instructions(self) -> u64 {
        self.replaced_heralded_noise_instructions
    }
}

#[derive(Clone, Copy)]
enum WithoutNoiseChange {
    RemovedNoise,
    StrippedMeasurementProbability,
    ReplacedHeraldedNoise,
}

trait WithoutNoiseRecorder {
    fn record(&mut self, change: WithoutNoiseChange) -> AnalysisResult<()>;
}

impl WithoutNoiseRecorder for () {
    #[inline(always)]
    fn record(&mut self, _change: WithoutNoiseChange) -> AnalysisResult<()> {
        Ok(())
    }
}

impl WithoutNoiseRecorder for WithoutNoiseReport {
    fn record(&mut self, change: WithoutNoiseChange) -> AnalysisResult<()> {
        let (counter, kind) = match change {
            WithoutNoiseChange::RemovedNoise => (
                &mut self.removed_noise_instructions,
                "removed noise instruction count",
            ),
            WithoutNoiseChange::StrippedMeasurementProbability => (
                &mut self.stripped_measurement_probabilities,
                "stripped measurement-probability count",
            ),
            WithoutNoiseChange::ReplacedHeraldedNoise => (
                &mut self.replaced_heralded_noise_instructions,
                "replaced heralded-noise instruction count",
            ),
        };
        increment_pass_report_counter(counter, kind)
    }
}

impl CircuitPass for WithoutNoisePass {
    type Options = WithoutNoiseOptions;
    type Report = WithoutNoiseReport;
    type Diagnostic = AnalysisError;

    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        // Removing noise never increases any retained resource dimension.
        Ok(input.resources())
    }

    fn run(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        let mut report = WithoutNoiseReport::default();
        let circuit = circuit_without_noise_impl(input.circuit(), &mut report)?;
        Ok(CircuitPassOutput::new(circuit, report))
    }
}

fn circuit_without_noise_impl<R: WithoutNoiseRecorder>(
    circuit: &Circuit,
    recorder: &mut R,
) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                append_noiseless_instruction(&mut result, instruction, recorder)?
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body = circuit_without_noise_impl(repeat.body(), recorder)?;
                result.append_repeat_block(stab_model::advanced::repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    body,
                    repeat.tag_bytes(),
                ));
            }
        }
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct CircuitFlattenEstimate {
    operations: u64,
    targets: u64,
    arguments: u64,
    tag_bytes: u64,
}

fn validate_flattening_budget(
    circuit: &Circuit,
    limits: CircuitFlattenLimits,
) -> AnalysisResult<CircuitFlattenEstimate> {
    validate_flattened_repeat_nesting(circuit)?;
    let estimate = flattened_materialization_estimate(circuit, 1)?;

    if estimate.operations > limits.max_expanded_operations() {
        return Err(ResourceLimitError::circuit_flatten_expanded_operations(
            estimate.operations,
            limits.max_expanded_operations(),
        )
        .into());
    }
    validate_materialized_instruction_capacity(estimate.operations)?;
    if estimate.targets > limits.max_expanded_targets() {
        return Err(ResourceLimitError::circuit_flatten_target_occurrences(
            estimate.targets,
            limits.max_expanded_targets(),
        )
        .into());
    }
    if estimate.arguments > limits.max_expanded_arguments() {
        return Err(ResourceLimitError::circuit_flatten_argument_values(
            estimate.arguments,
            limits.max_expanded_arguments(),
        )
        .into());
    }

    let materialized_bytes = estimate.materialized_bytes()?;
    if materialized_bytes > limits.max_materialized_bytes() {
        return Err(ResourceLimitError::circuit_flatten_materialized_bytes(
            materialized_bytes,
            limits.max_materialized_bytes(),
        )
        .into());
    }
    Ok(estimate)
}

impl CircuitFlattenEstimate {
    fn checked_add(self, other: Self) -> AnalysisResult<Self> {
        Ok(Self {
            operations: self
                .operations
                .checked_add(other.operations)
                .ok_or_else(flattened_operation_count_error)?,
            targets: checked_flatten_sum(self.targets, other.targets, "target count")?,
            arguments: checked_flatten_sum(self.arguments, other.arguments, "argument count")?,
            tag_bytes: checked_flatten_sum(self.tag_bytes, other.tag_bytes, "tag byte count")?,
        })
    }

    fn materialized_bytes(self) -> AnalysisResult<u64> {
        crate::circuit_pass::projected_circuit_payload_bytes(
            self.operations,
            self.targets,
            self.arguments,
            self.tag_bytes,
        )
        .ok_or_else(|| {
            AnalysisError::invalid_domain_value(
                "flattened circuit resource estimate",
                "byte count overflowed",
            )
        })
    }
}

fn flattened_materialization_estimate(
    circuit: &Circuit,
    multiplier: u64,
) -> AnalysisResult<CircuitFlattenEstimate> {
    let mut estimate = CircuitFlattenEstimate::default();
    for item in circuit.items() {
        let addition = match item {
            CircuitItem::Instruction(instruction)
                if instruction.gate().canonical_name() == "SHIFT_COORDS" =>
            {
                CircuitFlattenEstimate::default()
            }
            CircuitItem::Instruction(instruction) => CircuitFlattenEstimate {
                operations: multiplier,
                targets: checked_flatten_product(
                    usize_to_u64(instruction.targets().len())?,
                    multiplier,
                    "target count",
                )?,
                arguments: checked_flatten_product(
                    usize_to_u64(instruction.args().len())?,
                    multiplier,
                    "argument count",
                )?,
                tag_bytes: checked_flatten_product(
                    usize_to_u64(instruction.tag_bytes().map_or(0, <[u8]>::len))?,
                    multiplier,
                    "tag byte count",
                )?,
            },
            CircuitItem::RepeatBlock(repeat) => {
                let repeated_multiplier = multiplier
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(flattened_operation_count_error)?;
                flattened_materialization_estimate(repeat.body(), repeated_multiplier)?
            }
        };
        estimate = estimate.checked_add(addition)?;
    }
    Ok(estimate)
}

fn checked_flatten_sum(left: u64, right: u64, resource: &'static str) -> AnalysisResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::invalid_domain_value(
            "flattened circuit resource estimate",
            format!("{resource} overflowed"),
        )
    })
}

fn checked_flatten_product(left: u64, right: u64, resource: &'static str) -> AnalysisResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::invalid_domain_value(
            "flattened circuit resource estimate",
            format!("{resource} overflowed"),
        )
    })
}

fn usize_to_u64(value: usize) -> AnalysisResult<u64> {
    u64::try_from(value)
        .map_err(|_| AnalysisError::invalid_domain_value("flattened circuit count", "exceeds u64"))
}

fn validate_flattened_repeat_nesting(circuit: &Circuit) -> AnalysisResult<()> {
    let limit = RepeatNestingLimit::HARD_MAX;
    let mut pending = vec![(circuit, 0usize)];
    while let Some((current, depth)) = pending.pop() {
        for item in current.items() {
            let CircuitItem::RepeatBlock(repeat) = item else {
                continue;
            };
            let next_depth = depth.checked_add(1).ok_or_else(|| {
                AnalysisError::invalid_domain_value("circuit repeat nesting", "depth overflow")
            })?;
            if next_depth > limit {
                return Err(
                    ResourceLimitError::circuit_flatten_repeat_nesting(next_depth, limit).into(),
                );
            }
            pending.push((repeat.body(), next_depth));
        }
    }
    Ok(())
}

fn flattened_operation_count(circuit: &Circuit) -> AnalysisResult<u64> {
    let mut count = 0_u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() != "SHIFT_COORDS" {
                    count = count
                        .checked_add(1)
                        .ok_or_else(flattened_operation_count_error)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = flattened_operation_count(repeat.body())?;
                let repeated_count = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(flattened_operation_count_error)?;
                count = count
                    .checked_add(repeated_count)
                    .ok_or_else(flattened_operation_count_error)?;
            }
        }
    }
    Ok(count)
}

fn visit_flattened_operations(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    mut visitor: impl FnMut(CircuitInstruction) -> AnalysisResult<()>,
) -> AnalysisResult<()> {
    visit_flattened_operations_inner(circuit, shift, &mut visitor)
}

fn visit_flattened_operations_inner(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    visitor: &mut impl FnMut(CircuitInstruction) -> AnalysisResult<()>,
) -> AnalysisResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                visit_flattened_instruction(instruction, shift, visitor)?
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = flattened_operation_count(repeat.body())?;
                if body_count == 0 {
                    let body_shift = coordinate_shift_of(repeat.body())?;
                    add_coordinate_shift_mul(
                        shift,
                        &body_shift,
                        repeat.repeat_count().get() as f64,
                    )?;
                    continue;
                }
                for _ in 0..repeat.repeat_count().get() {
                    visit_flattened_operations_inner(repeat.body(), shift, visitor)?;
                }
            }
        }
    }
    Ok(())
}

fn visit_flattened_instruction(
    instruction: &CircuitInstruction,
    shift: &mut Vec<f64>,
    visitor: &mut impl FnMut(CircuitInstruction) -> AnalysisResult<()>,
) -> AnalysisResult<()> {
    match instruction.gate().canonical_name() {
        "SHIFT_COORDS" => {
            add_coordinate_shift_mul(shift, instruction.args(), 1.0)?;
        }
        "QUBIT_COORDS" | "DETECTOR" => {
            visitor(clone_instruction_with_args(
                instruction,
                shifted_flattened_coordinates(instruction.args(), shift)?,
            )?)?;
        }
        _ => visitor(clone_instruction_with_args(
            instruction,
            try_clone_slice(instruction.args(), "flattened circuit argument allocation")?,
        )?)?,
    }
    Ok(())
}

fn append_noiseless_instruction<R: WithoutNoiseRecorder>(
    result: &mut Circuit,
    instruction: &CircuitInstruction,
    recorder: &mut R,
) -> AnalysisResult<()> {
    let gate = instruction.gate();
    if gate.produces_measurements() {
        let noiseless = if is_heralded_noise(gate) {
            recorder.record(WithoutNoiseChange::ReplacedHeraldedNoise)?;
            stab_model::advanced::circuit_instruction_with_tag_bytes(
                Gate::from_name("MPAD")?,
                Vec::new(),
                vec![Target::qubit(QubitId::new(0)?, false); instruction.targets().len()],
                instruction.tag_bytes(),
            )?
        } else {
            if !instruction.args().is_empty() {
                recorder.record(WithoutNoiseChange::StrippedMeasurementProbability)?;
            }
            clone_instruction_with_args(instruction, Vec::new())?
        };
        result.append_instruction(noiseless);
    } else if !gate.is_noisy() {
        result.append_instruction(clone_instruction_with_args(
            instruction,
            instruction.args().to_vec(),
        )?);
    } else {
        recorder.record(WithoutNoiseChange::RemovedNoise)?;
    }
    Ok(())
}

fn increment_pass_report_counter(counter: &mut u64, kind: &'static str) -> AnalysisResult<()> {
    *counter = counter
        .checked_add(1)
        .ok_or_else(|| AnalysisError::invalid_domain_value(kind, "overflowed"))?;
    Ok(())
}

fn coordinate_shift_of(circuit: &Circuit) -> AnalysisResult<Vec<f64>> {
    let mut shift = Vec::new();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "SHIFT_COORDS" {
                    add_coordinate_shift_mul(&mut shift, instruction.args(), 1.0)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_shift = coordinate_shift_of(repeat.body())?;
                add_coordinate_shift_mul(
                    &mut shift,
                    &body_shift,
                    repeat.repeat_count().get() as f64,
                )?;
            }
        }
    }
    Ok(shift)
}

fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> AnalysisResult<()> {
    if shift.len() < delta.len() {
        let additional = delta.len() - shift.len();
        shift.try_reserve_exact(additional).map_err(|error| {
            AnalysisError::invalid_domain_value(
                "flattened circuit coordinate allocation",
                format!("unable to reserve {additional} coordinate slots: {error}"),
            )
        })?;
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift.get_mut(index).ok_or_else(|| {
            AnalysisError::invalid_result_format("coordinate shift dimension missing")
        })?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(AnalysisError::invalid_result_format(
                "coordinate shift overflowed",
            ));
        }
    }
    Ok(())
}

fn shifted_flattened_coordinates(args: &[f64], shift: &[f64]) -> AnalysisResult<Vec<f64>> {
    let mut shifted = try_clone_slice(args, "flattened circuit coordinate allocation")?;
    for (index, coordinate) in shifted.iter_mut().enumerate() {
        if let Some(offset) = shift.get(index) {
            *coordinate += *offset;
            if !coordinate.is_finite() {
                return Err(AnalysisError::invalid_result_format(
                    "coordinate shift overflowed",
                ));
            }
        }
    }
    Ok(shifted)
}

fn clone_instruction_with_args(
    instruction: &CircuitInstruction,
    args: Vec<f64>,
) -> AnalysisResult<CircuitInstruction> {
    Ok(stab_model::advanced::circuit_instruction_with_tag_bytes(
        instruction.gate(),
        args,
        try_clone_slice(instruction.targets(), "flattened circuit target allocation")?,
        instruction.tag_bytes(),
    )?)
}

fn try_clone_slice<T: Clone>(values: &[T], context: &'static str) -> AnalysisResult<Vec<T>> {
    let mut cloned = Vec::new();
    cloned.try_reserve_exact(values.len()).map_err(|error| {
        AnalysisError::invalid_domain_value(
            context,
            format!("unable to reserve {} values: {error}", values.len()),
        )
    })?;
    cloned.extend_from_slice(values);
    Ok(cloned)
}

fn is_heralded_noise(gate: Gate) -> bool {
    matches!(
        gate.canonical_name(),
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn flattened_operation_count_error() -> AnalysisError {
    AnalysisError::invalid_domain_value("flattened circuit operation count", "overflowed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "the test must fail if an impossible reservation unexpectedly succeeds"
    )]
    fn circuit_item_reservation_failure_is_a_domain_error() {
        let mut circuit = CircuitBuilder::new();
        let error = circuit
            .try_reserve_exact(usize::MAX)
            .expect_err("an impossible vector capacity must not panic");
        assert!(
            error
                .to_string()
                .contains("unable to reserve 18446744073709551615 item slots")
                || error.to_string().contains("unable to reserve")
        );
    }
}
