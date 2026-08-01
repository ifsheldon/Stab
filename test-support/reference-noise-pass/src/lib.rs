//! Deterministic external noise-insertion pass built only from Stab's public Stable APIs.

use std::collections::TryReserveError;
use std::error::Error;
use std::fmt::{Display, Formatter};

use stab_analysis::{
    CircuitPass, CircuitPassInput, CircuitPassOutput, CircuitPassProjectionError,
    CircuitPassResources,
};
use stab_model::advanced::{
    CircuitBuilder, circuit_instruction_with_tag_bytes, repeat_block_with_tag_bytes,
};
use stab_model::{
    Circuit, CircuitInstruction, CircuitItem, Gate, GateTargetGroupKind, ModelError, Probability,
    Target,
};

/// Options for [`XErrorAfterSingleQubitUnitariesPass`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct XErrorAfterSingleQubitUnitariesOptions {
    probability: Probability,
}

impl XErrorAfterSingleQubitUnitariesOptions {
    pub const fn new(probability: Probability) -> Self {
        Self { probability }
    }

    pub const fn probability(self) -> Probability {
        self.probability
    }
}

/// Represented structural work performed by [`XErrorAfterSingleQubitUnitariesPass`].
///
/// Insertions inside a folded repeat body are counted once, independently of the repeat count.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct XErrorAfterSingleQubitUnitariesReport {
    inserted_represented_instruction_count: u64,
    affected_target_count: u64,
}

impl XErrorAfterSingleQubitUnitariesReport {
    pub const fn inserted_represented_instruction_count(self) -> u64 {
        self.inserted_represented_instruction_count
    }

    pub const fn affected_target_count(self) -> u64 {
        self.affected_target_count
    }
}

/// A failure while constructing the transformed closed-Stim circuit.
#[derive(Debug)]
#[non_exhaustive]
pub enum XErrorAfterSingleQubitUnitariesDiagnostic {
    /// A public model constructor rejected a reconstructed instruction.
    Model(ModelError),
    /// Conservative output-resource projection overflowed before allocation.
    Projection(CircuitPassProjectionError),
    /// A fallible output allocation could not be satisfied.
    Allocation(TryReserveError),
    /// The represented insertion count exceeded `u64`.
    InsertedInstructionCountOverflow,
    /// The affected-target count exceeded `u64`.
    AffectedTargetCountOverflow,
    /// The transformed block's represented item count exceeded `usize`.
    OutputItemCountOverflow,
}

impl Display for XErrorAfterSingleQubitUnitariesDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Model(error) => {
                write!(formatter, "unable to construct closed-Stim output: {error}")
            }
            Self::Projection(error) => Display::fmt(error, formatter),
            Self::Allocation(error) => {
                write!(formatter, "unable to reserve circuit-pass output: {error}")
            }
            Self::InsertedInstructionCountOverflow => {
                formatter.write_str("inserted represented-instruction count exceeds u64")
            }
            Self::AffectedTargetCountOverflow => {
                formatter.write_str("affected-target count exceeds u64")
            }
            Self::OutputItemCountOverflow => {
                formatter.write_str("transformed represented-item count exceeds usize")
            }
        }
    }
}

impl Error for XErrorAfterSingleQubitUnitariesDiagnostic {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Projection(error) => Some(error),
            Self::Allocation(error) => Some(error),
            Self::InsertedInstructionCountOverflow
            | Self::AffectedTargetCountOverflow
            | Self::OutputItemCountOverflow => None,
        }
    }
}

impl From<ModelError> for XErrorAfterSingleQubitUnitariesDiagnostic {
    fn from(error: ModelError) -> Self {
        Self::Model(error)
    }
}

impl From<CircuitPassProjectionError> for XErrorAfterSingleQubitUnitariesDiagnostic {
    fn from(error: CircuitPassProjectionError) -> Self {
        Self::Projection(error)
    }
}

impl From<TryReserveError> for XErrorAfterSingleQubitUnitariesDiagnostic {
    fn from(error: TryReserveError) -> Self {
        Self::Allocation(error)
    }
}

/// Inserts independent `X_ERROR(p)` noise after represented single-target-group unitaries.
///
/// Repeat blocks remain folded. Each source instruction is reconstructed without fusion so its
/// arguments, targets, opaque tag, and represented boundary are preserved exactly. Inserted noise
/// is untagged and uses the source instruction's complete target list.
#[derive(Clone, Copy, Debug, Default)]
pub struct XErrorAfterSingleQubitUnitariesPass;

impl CircuitPass for XErrorAfterSingleQubitUnitariesPass {
    type Options = XErrorAfterSingleQubitUnitariesOptions;
    type Report = XErrorAfterSingleQubitUnitariesReport;
    type Diagnostic = XErrorAfterSingleQubitUnitariesDiagnostic;

    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        let (insertions, affected_targets) = projected_insertions(input.circuit())?;
        input
            .resources()
            .checked_with_additional(insertions, affected_targets, insertions, 0)
            .map_err(Into::into)
    }

    fn run(
        &self,
        input: CircuitPassInput<'_>,
        options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        let x_error = Gate::from_name("X_ERROR")?;
        let mut report = XErrorAfterSingleQubitUnitariesReport::default();
        let circuit =
            transform_block(input.circuit(), options.probability(), x_error, &mut report)?;
        Ok(CircuitPassOutput::new(circuit, report))
    }
}

fn projected_insertions(
    source: &Circuit,
) -> Result<(u64, u64), XErrorAfterSingleQubitUnitariesDiagnostic> {
    let mut insertions = 0_u64;
    let mut affected_targets = 0_u64;
    for item in source.items() {
        match item {
            CircuitItem::Instruction(instruction) if instruction_receives_noise(instruction) => {
                insertions = insertions.checked_add(1).ok_or(
                    XErrorAfterSingleQubitUnitariesDiagnostic::InsertedInstructionCountOverflow,
                )?;
                let targets = u64::try_from(instruction.targets().len()).map_err(|_| {
                    XErrorAfterSingleQubitUnitariesDiagnostic::AffectedTargetCountOverflow
                })?;
                affected_targets = affected_targets.checked_add(targets).ok_or(
                    XErrorAfterSingleQubitUnitariesDiagnostic::AffectedTargetCountOverflow,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let (body_insertions, body_targets) = projected_insertions(repeat.body())?;
                insertions = insertions.checked_add(body_insertions).ok_or(
                    XErrorAfterSingleQubitUnitariesDiagnostic::InsertedInstructionCountOverflow,
                )?;
                affected_targets = affected_targets.checked_add(body_targets).ok_or(
                    XErrorAfterSingleQubitUnitariesDiagnostic::AffectedTargetCountOverflow,
                )?;
            }
            CircuitItem::Instruction(_) => {}
        }
    }
    Ok((insertions, affected_targets))
}

fn transform_block(
    source: &Circuit,
    probability: Probability,
    x_error: Gate,
    report: &mut XErrorAfterSingleQubitUnitariesReport,
) -> Result<Circuit, XErrorAfterSingleQubitUnitariesDiagnostic> {
    let insertion_count = source
        .items()
        .iter()
        .filter(|item| {
            item.as_instruction()
                .is_some_and(instruction_receives_noise)
        })
        .count();
    let output_item_count = source
        .items()
        .len()
        .checked_add(insertion_count)
        .ok_or(XErrorAfterSingleQubitUnitariesDiagnostic::OutputItemCountOverflow)?;
    let mut items = Vec::new();
    items.try_reserve_exact(output_item_count)?;

    for item in source.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                let copied = copy_instruction(instruction)?;
                items.push(CircuitItem::Instruction(copied));
                if instruction_receives_noise(instruction) {
                    let noise = noise_instruction(x_error, probability, instruction.targets())?;
                    items.push(CircuitItem::Instruction(noise));
                    record_insertion(report, instruction.targets().len())?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body = transform_block(repeat.body(), probability, x_error, report)?;
                items.push(CircuitItem::RepeatBlock(repeat_block_with_tag_bytes(
                    repeat.repeat_count(),
                    body,
                    repeat.tag_bytes(),
                )));
            }
        }
    }

    Ok(CircuitBuilder::from_unfused_items(items).finish())
}

fn instruction_receives_noise(instruction: &CircuitInstruction) -> bool {
    instruction.gate().is_unitary()
        && instruction.gate().target_group_kind() == GateTargetGroupKind::Singles
}

fn copy_instruction(
    instruction: &CircuitInstruction,
) -> Result<CircuitInstruction, XErrorAfterSingleQubitUnitariesDiagnostic> {
    circuit_instruction_with_tag_bytes(
        instruction.gate(),
        copy_slice(instruction.args())?,
        copy_slice(instruction.targets())?,
        instruction.tag_bytes(),
    )
    .map_err(Into::into)
}

fn noise_instruction(
    x_error: Gate,
    probability: Probability,
    source_targets: &[Target],
) -> Result<CircuitInstruction, XErrorAfterSingleQubitUnitariesDiagnostic> {
    let mut args = Vec::new();
    args.try_reserve_exact(1)?;
    args.push(probability.get());
    circuit_instruction_with_tag_bytes(x_error, args, copy_slice(source_targets)?, None)
        .map_err(Into::into)
}

fn copy_slice<T: Clone>(source: &[T]) -> Result<Vec<T>, TryReserveError> {
    let mut copy = Vec::new();
    copy.try_reserve_exact(source.len())?;
    copy.extend_from_slice(source);
    Ok(copy)
}

fn record_insertion(
    report: &mut XErrorAfterSingleQubitUnitariesReport,
    affected_targets: usize,
) -> Result<(), XErrorAfterSingleQubitUnitariesDiagnostic> {
    report.inserted_represented_instruction_count = report
        .inserted_represented_instruction_count
        .checked_add(1)
        .ok_or(XErrorAfterSingleQubitUnitariesDiagnostic::InsertedInstructionCountOverflow)?;
    let affected_targets = u64::try_from(affected_targets)
        .map_err(|_| XErrorAfterSingleQubitUnitariesDiagnostic::AffectedTargetCountOverflow)?;
    report.affected_target_count = report
        .affected_target_count
        .checked_add(affected_targets)
        .ok_or(XErrorAfterSingleQubitUnitariesDiagnostic::AffectedTargetCountOverflow)?;
    Ok(())
}
