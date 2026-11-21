use stab_model::{
    Cancellation, Circuit, CircuitInstruction, CircuitItem, FoldedCircuitSummary, FoldedWalkError,
    Gate, GateKind, MemoryBudget, ParseLimits, QubitId, Target,
    advanced::{CancellationCountdown, CircuitBuilder, FoldedCircuitEvent},
};

use crate::folded_walk::{check_repeat_nesting, walk_folded};

use crate::{
    AnalysisError, AnalysisOperation, AnalysisResult, CircuitPass, CircuitPassInput,
    CircuitPassOutput, CircuitPassResources, ResourceKind, ResourceLimitError,
};

/// Options of [`flattened_circuit`] and [`flattened_circuit_operations`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CircuitFlattenOptions {
    /// Budget for the conservative retained size of the materialized circuit, admitted before the
    /// output is allocated. Expanded operation, target, and argument counts are not separately
    /// capped; repeat nesting stays the shared fixed recursion envelope.
    pub memory_budget: MemoryBudget,
    /// Signal polled once per materialized instruction.
    pub cancellation: Cancellation,
}

/// Returns a circuit with repeat blocks unrolled and coordinate shifts applied.
///
/// `SHIFT_COORDS` instructions are absorbed into subsequent `QUBIT_COORDS` and `DETECTOR`
/// instructions, matching Stim's materialized `flattened` transform. Because this returns an
/// owned circuit, expansions whose conservative size exceeds the options' memory budget are
/// rejected before any output circuit is allocated or mutated; use the lazy flattened iterators
/// for raw repeat traversal when materialization is not required.
pub fn flattened_circuit(
    circuit: &Circuit,
    options: &CircuitFlattenOptions,
) -> AnalysisResult<Circuit> {
    let estimate = validate_flattening_budget(circuit, options)?;
    let capacity = validate_materialized_instruction_capacity(estimate.operations)?;
    let mut result = CircuitBuilder::new();
    result.try_reserve_exact(capacity)?;
    let mut countdown = CancellationCountdown::new();
    visit_flattened_operations(circuit, &mut Vec::new(), |instruction| {
        countdown.tick(&options.cancellation)?;
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
/// independent instruction. Repeat nesting and the expanded operation count are admitted before
/// output allocation.
pub fn flattened_circuit_operations(
    circuit: &Circuit,
    options: &CircuitFlattenOptions,
) -> AnalysisResult<Vec<CircuitInstruction>> {
    let estimate = validate_flattening_budget(circuit, options)?;
    let capacity = validate_materialized_instruction_capacity(estimate.operations)?;
    let mut result = Vec::new();
    result.try_reserve_exact(capacity).map_err(|error| {
        AnalysisError::allocation_failed(
            AnalysisOperation::CircuitFlatten,
            format!(
                "flattened circuit allocation value unable to reserve {} instruction slots: {error}",
                estimate.operations
            ),
        )
    })?;
    let mut countdown = CancellationCountdown::new();
    visit_flattened_operations(circuit, &mut Vec::new(), |instruction| {
        countdown.tick(&options.cancellation)?;
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
    let never = Cancellation::never();
    let mut polls = PassPolls {
        cancellation: &never,
        countdown: CancellationCountdown::new(),
    };
    circuit_without_noise_impl(circuit, &mut (), &mut polls)
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
        let mut polls = PassPolls {
            cancellation: &input.context().cancellation,
            countdown: CancellationCountdown::new(),
        };
        let circuit = circuit_without_noise_impl(input.circuit(), &mut report, &mut polls)?;
        Ok(CircuitPassOutput::new(circuit, report))
    }
}

/// The pass context's signal, polled once per folded item.
struct PassPolls<'a> {
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

fn circuit_without_noise_impl<R: WithoutNoiseRecorder>(
    circuit: &Circuit,
    recorder: &mut R,
    polls: &mut PassPolls<'_>,
) -> AnalysisResult<Circuit> {
    let mut result = Circuit::new();
    for item in circuit.items() {
        polls.countdown.tick(polls.cancellation)?;
        match item {
            CircuitItem::Instruction(instruction) => {
                append_noiseless_instruction(&mut result, instruction, recorder)?
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body = circuit_without_noise_impl(repeat.body(), recorder, polls)?;
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
    options: &CircuitFlattenOptions,
) -> AnalysisResult<CircuitFlattenEstimate> {
    validate_flattened_repeat_nesting(circuit)?;
    let estimate = flattened_materialization_estimate(circuit)?;
    validate_materialized_instruction_capacity(estimate.operations)?;
    let materialized_bytes = estimate.materialized_bytes()?;
    let budget = options.memory_budget;
    if !budget.admits(u128::from(materialized_bytes)) {
        return Err(ResourceLimitError::memory_budget(
            AnalysisOperation::CircuitFlatten,
            ResourceKind::MaterializedBytes,
            materialized_bytes,
            budget,
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
            AnalysisError::count_overflow(
                AnalysisOperation::CircuitFlatten,
                "flattened circuit resource estimate value byte count overflowed",
            )
        })
    }
}

fn flattened_materialization_estimate(circuit: &Circuit) -> AnalysisResult<CircuitFlattenEstimate> {
    let mut estimate = CircuitFlattenEstimate::default();
    walk_folded(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
        |depth, limit| {
            ResourceLimitError::circuit_flatten_repeat_nesting(
                usize::try_from(depth).unwrap_or(usize::MAX),
                usize::try_from(limit).unwrap_or(usize::MAX),
            )
            .into()
        },
        |event| {
            let addition = match event {
                FoldedCircuitEvent::Instruction { instruction, .. }
                    if instruction.gate().kind() == GateKind::SHIFT_COORDS =>
                {
                    return Ok(());
                }
                FoldedCircuitEvent::Instruction {
                    instruction,
                    executions,
                } => {
                    let multiplier = executions
                        .exact()
                        .ok_or_else(flattened_operation_count_error)?;
                    CircuitFlattenEstimate {
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
                    }
                }
                // Flattening multiplies through every block, so an iteration count beyond `u64`
                // is refused even when the block's body is empty.
                FoldedCircuitEvent::Repeat { iterations, .. } => {
                    iterations
                        .exact()
                        .ok_or_else(flattened_operation_count_error)?;
                    return Ok(());
                }
            };
            estimate = estimate.checked_add(addition)?;
            Ok(())
        },
    )?;
    Ok(estimate)
}

fn checked_flatten_sum(left: u64, right: u64, resource: &'static str) -> AnalysisResult<u64> {
    left.checked_add(right).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::CircuitFlatten,
            format!("flattened circuit resource estimate value {resource} overflowed"),
        )
    })
}

fn checked_flatten_product(left: u64, right: u64, resource: &'static str) -> AnalysisResult<u64> {
    left.checked_mul(right).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::CircuitFlatten,
            format!("flattened circuit resource estimate value {resource} overflowed"),
        )
    })
}

fn usize_to_u64(value: usize) -> AnalysisResult<u64> {
    u64::try_from(value).map_err(|_| {
        AnalysisError::count_overflow(
            AnalysisOperation::CircuitFlatten,
            "flattened circuit count value exceeds u64",
        )
    })
}

fn validate_flattened_repeat_nesting(circuit: &Circuit) -> AnalysisResult<()> {
    check_repeat_nesting(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        |depth, limit| {
            ResourceLimitError::circuit_flatten_repeat_nesting(
                usize::try_from(depth).unwrap_or(usize::MAX),
                usize::try_from(limit).unwrap_or(usize::MAX),
            )
            .into()
        },
    )
}

/// Counts the instructions flattening emits, which excludes `SHIFT_COORDS`.
fn flattened_operation_count(circuit: &Circuit) -> AnalysisResult<u64> {
    let summary = FoldedCircuitSummary::of(
        circuit,
        ParseLimits::DEFAULT_REPEAT_NESTING,
        &Cancellation::never(),
    )
    .map_err(|error| match error {
        FoldedWalkError::RepeatNesting { depth, limit } => {
            ResourceLimitError::circuit_flatten_repeat_nesting(depth, limit).into()
        }
        FoldedWalkError::Cancelled(cancelled) => AnalysisError::from(cancelled),
    })?;
    GateKind::ALL
        .into_iter()
        .filter(|kind| *kind != GateKind::SHIFT_COORDS)
        .try_fold(0_u64, |count, kind| {
            summary
                .instructions_of(kind)
                .exact()
                .and_then(|instructions| count.checked_add(instructions))
                .ok_or_else(flattened_operation_count_error)
        })
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
    match instruction.gate().kind() {
        GateKind::SHIFT_COORDS => {
            add_coordinate_shift_mul(shift, instruction.args(), 1.0)?;
        }
        GateKind::QUBIT_COORDS | GateKind::DETECTOR => {
            visitor(clone_instruction_with_args(
                instruction,
                shifted_flattened_coordinates(instruction.args(), shift)?,
            )?)?;
        }
        GateKind::OBSERVABLE_INCLUDE
        | GateKind::TICK
        | GateKind::REPEAT
        | GateKind::MPAD
        | GateKind::MX
        | GateKind::MY
        | GateKind::M
        | GateKind::MRX
        | GateKind::MRY
        | GateKind::MR
        | GateKind::RX
        | GateKind::RY
        | GateKind::R
        | GateKind::XCX
        | GateKind::XCY
        | GateKind::XCZ
        | GateKind::YCX
        | GateKind::YCY
        | GateKind::YCZ
        | GateKind::CX
        | GateKind::CY
        | GateKind::CZ
        | GateKind::H
        | GateKind::H_XY
        | GateKind::H_YZ
        | GateKind::H_NXY
        | GateKind::H_NXZ
        | GateKind::H_NYZ
        | GateKind::DEPOLARIZE1
        | GateKind::DEPOLARIZE2
        | GateKind::X_ERROR
        | GateKind::Y_ERROR
        | GateKind::Z_ERROR
        | GateKind::I_ERROR
        | GateKind::II_ERROR
        | GateKind::PAULI_CHANNEL_1
        | GateKind::PAULI_CHANNEL_2
        | GateKind::E
        | GateKind::ELSE_CORRELATED_ERROR
        | GateKind::HERALDED_ERASE
        | GateKind::HERALDED_PAULI_CHANNEL_1
        | GateKind::I
        | GateKind::X
        | GateKind::Y
        | GateKind::Z
        | GateKind::C_XYZ
        | GateKind::C_ZYX
        | GateKind::C_NXYZ
        | GateKind::C_XNYZ
        | GateKind::C_XYNZ
        | GateKind::C_NZYX
        | GateKind::C_ZNYX
        | GateKind::C_ZYNX
        | GateKind::SQRT_X
        | GateKind::SQRT_X_DAG
        | GateKind::SQRT_Y
        | GateKind::SQRT_Y_DAG
        | GateKind::S
        | GateKind::S_DAG
        | GateKind::II
        | GateKind::SQRT_XX
        | GateKind::SQRT_XX_DAG
        | GateKind::SQRT_YY
        | GateKind::SQRT_YY_DAG
        | GateKind::SQRT_ZZ
        | GateKind::SQRT_ZZ_DAG
        | GateKind::MPP
        | GateKind::SPP
        | GateKind::SPP_DAG
        | GateKind::SWAP
        | GateKind::ISWAP
        | GateKind::CXSWAP
        | GateKind::SWAPCX
        | GateKind::CZSWAP
        | GateKind::ISWAP_DAG
        | GateKind::MXX
        | GateKind::MYY
        | GateKind::MZZ => visitor(clone_instruction_with_args(
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
                GateKind::MPAD.gate(),
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
    *counter = counter.checked_add(1).ok_or_else(|| {
        AnalysisError::count_overflow(
            AnalysisOperation::CircuitPass,
            format!("{kind} value overflowed"),
        )
    })?;
    Ok(())
}

fn coordinate_shift_of(circuit: &Circuit) -> AnalysisResult<Vec<f64>> {
    let mut shift = Vec::new();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().kind() == GateKind::SHIFT_COORDS {
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
            AnalysisError::allocation_failed(
                AnalysisOperation::CircuitFlatten,
                format!(
                    "flattened circuit coordinate allocation value unable to reserve {additional} coordinate slots: {error}"
                ),
            )
        })?;
        shift.resize(delta.len(), 0.0);
    }
    // `shift` now has a dimension for every value of `delta`.
    for (coordinate, value) in shift.iter_mut().zip(delta) {
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(AnalysisError::NonFiniteCoordinate {
                operation: AnalysisOperation::CircuitFlatten,
            });
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
                return Err(AnalysisError::NonFiniteCoordinate {
                    operation: AnalysisOperation::CircuitFlatten,
                });
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
        AnalysisError::allocation_failed(
            AnalysisOperation::CircuitFlatten,
            format!(
                "{context} value unable to reserve {} values: {error}",
                values.len()
            ),
        )
    })?;
    cloned.extend_from_slice(values);
    Ok(cloned)
}

fn is_heralded_noise(gate: Gate) -> bool {
    gate.produces_heralded_results()
}

fn flattened_operation_count_error() -> AnalysisError {
    AnalysisError::count_overflow(
        AnalysisOperation::CircuitFlatten,
        "flattened circuit operation count value overflowed",
    )
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
