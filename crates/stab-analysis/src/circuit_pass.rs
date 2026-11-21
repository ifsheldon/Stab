use std::error::Error;

use stab_model::advanced::CancellationCountdown;
use stab_model::{
    Cancellation, Cancelled, Circuit, CircuitItem, MemoryBudget, MemoryBudgetExceeded, ParseLimits,
    RepeatNestingLimit, Target,
};
use thiserror::Error;

use crate::resources::CircuitPassStage;
use crate::{ResourceKind, ResourceLimitError};

const OPAQUE_TAG_STORAGE_MULTIPLIER: u64 = 4;

/// Resource policy and cancellation signal of one circuit-pass execution.
///
/// [`run_circuit_pass`] polls the signal once per folded item while it admits the input and the
/// returned circuit, and before and after the pass runs. A pass whose work is not bounded by a
/// small multiple of its admitted input should poll [`Self::cancellation`] at bounded intervals,
/// for example with `stab_model::advanced::CancellationCountdown`, and return any diagnostic once
/// it observes the signal: the executor reports [`CircuitPassError::Cancelled`] for every failure
/// returned while the signal is set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CircuitPassContext {
    /// Budget for the projected logical payload of the input, of each pass's declared output
    /// projection, and of the returned circuit, excluding allocator metadata and spare capacity.
    /// Counts describe the represented, folded circuit and are reported but not separately
    /// capped.
    pub memory_budget: MemoryBudget,
    /// Deepest repeat nesting admitted, never above the model's hard recursive-safety ceiling.
    pub repeat_nesting: RepeatNestingLimit,
    /// The signal the executor and pass implementations poll.
    pub cancellation: Cancellation,
}

impl Default for CircuitPassContext {
    fn default() -> Self {
        Self {
            memory_budget: MemoryBudget::DEFAULT,
            repeat_nesting: ParseLimits::DEFAULT_REPEAT_NESTING,
            cancellation: Cancellation::never(),
        }
    }
}

/// Folded circuit resources used for pass input admission and output projection.
///
/// `projected_payload_bytes` is a semantic payload estimate covering circuit items, targets,
/// arguments, and both exact and lossy-display bytes for opaque tags. It intentionally excludes
/// allocator metadata and spare collection capacity, so it is an admission proxy for proportional
/// output work rather than an exact resident-memory measurement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircuitPassResources {
    represented_items: u64,
    target_occurrences: u64,
    argument_values: u64,
    tag_bytes: u64,
    repeat_nesting: usize,
    projected_payload_bytes: u64,
}

impl CircuitPassResources {
    pub fn try_new(
        represented_items: u64,
        target_occurrences: u64,
        argument_values: u64,
        tag_bytes: u64,
        repeat_nesting: usize,
    ) -> Result<Self, CircuitPassProjectionError> {
        let projected_payload_bytes = projected_payload_bytes(
            represented_items,
            target_occurrences,
            argument_values,
            tag_bytes,
        )?;
        Ok(Self {
            represented_items,
            target_occurrences,
            argument_values,
            tag_bytes,
            repeat_nesting,
            projected_payload_bytes,
        })
    }

    pub const fn represented_items(self) -> u64 {
        self.represented_items
    }

    pub const fn target_occurrences(self) -> u64 {
        self.target_occurrences
    }

    pub const fn argument_values(self) -> u64 {
        self.argument_values
    }

    pub const fn tag_bytes(self) -> u64 {
        self.tag_bytes
    }

    pub const fn repeat_nesting(self) -> usize {
        self.repeat_nesting
    }

    pub const fn projected_payload_bytes(self) -> u64 {
        self.projected_payload_bytes
    }

    /// Projects additional logical output payload without allocating it.
    pub fn checked_with_additional(
        self,
        represented_items: u64,
        target_occurrences: u64,
        argument_values: u64,
        tag_bytes: u64,
    ) -> Result<Self, CircuitPassProjectionError> {
        Self::try_new(
            checked_resource_add(
                self.represented_items,
                represented_items,
                ResourceKind::RepresentedItems,
            )?,
            checked_resource_add(
                self.target_occurrences,
                target_occurrences,
                ResourceKind::TargetOccurrences,
            )?,
            checked_resource_add(
                self.argument_values,
                argument_values,
                ResourceKind::ArgumentValues,
            )?,
            checked_resource_add(
                self.tag_bytes,
                tag_bytes,
                ResourceKind::ProjectedPayloadBytes,
            )?,
            self.repeat_nesting,
        )
    }

    #[must_use]
    pub const fn with_repeat_nesting(mut self, repeat_nesting: usize) -> Self {
        self.repeat_nesting = repeat_nesting;
        self
    }
}

/// Arithmetic failure while projecting pass output before allocation.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("circuit pass resource projection overflowed {resource:?}")]
pub struct CircuitPassProjectionError {
    resource: ResourceKind,
}

impl CircuitPassProjectionError {
    pub const fn resource(self) -> ResourceKind {
        self.resource
    }
}

/// Framework-admitted input supplied to a [`CircuitPass`] implementation.
///
/// The fields are private so external callers cannot bypass the common input admission performed
/// by [`run_circuit_pass`].
#[derive(Clone, Copy, Debug)]
pub struct CircuitPassInput<'a> {
    circuit: &'a Circuit,
    context: &'a CircuitPassContext,
    resources: CircuitPassResources,
}

impl<'a> CircuitPassInput<'a> {
    pub const fn circuit(self) -> &'a Circuit {
        self.circuit
    }

    pub const fn context(self) -> &'a CircuitPassContext {
        self.context
    }

    pub const fn resources(self) -> CircuitPassResources {
        self.resources
    }
}

/// Candidate circuit and pass-specific report returned by a pass implementation.
#[derive(Clone, Debug, PartialEq)]
pub struct CircuitPassOutput<R> {
    circuit: Circuit,
    report: R,
}

impl<R> CircuitPassOutput<R> {
    pub const fn new(circuit: Circuit, report: R) -> Self {
        Self { circuit, report }
    }

    pub const fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    pub const fn report(&self) -> &R {
        &self.report
    }

    pub fn into_parts(self) -> (Circuit, R) {
        (self.circuit, self.report)
    }
}

/// Typed circuit transform implemented by built-in and external crates.
pub trait CircuitPass {
    type Options;
    type Report;
    type Diagnostic: Error + Send + Sync + 'static;

    /// Returns a conservative upper bound for logical output payload before output allocation.
    ///
    /// Implementations may inspect admitted input and typed options, but this method must not
    /// allocate storage proportional to the projected output. The common executor admits the
    /// projection before calling [`Self::run`] and rejects an underestimated result afterward.
    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic>;

    /// Produces a candidate output from framework-admitted input.
    ///
    /// Callers execute passes through [`run_circuit_pass`], which validates the returned circuit
    /// before exposing it as an accepted result.
    fn run(
        &self,
        input: CircuitPassInput<'_>,
        options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic>;
}

/// Failure from common resource admission or a pass-specific diagnostic.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CircuitPassError<D: Error + 'static> {
    #[error("circuit pass rejected the transform: {0}")]
    Diagnostic(#[source] D),
    #[error(transparent)]
    ResourceLimit(#[from] ResourceLimitError),
    #[error(
        "circuit pass output projection underestimated {resource:?}: projected {projected}, actual {actual}"
    )]
    ProjectionUnderestimated {
        resource: ResourceKind,
        projected: u64,
        actual: u64,
    },
    /// The context's cancellation signal was observed; no output is returned.
    #[error(transparent)]
    Cancelled(#[from] Cancelled),
}

impl<D: Error + 'static> CircuitPassError<D> {
    pub const fn diagnostic(&self) -> Option<&D> {
        match self {
            Self::Diagnostic(diagnostic) => Some(diagnostic),
            Self::ResourceLimit(_) | Self::ProjectionUnderestimated { .. } | Self::Cancelled(_) => {
                None
            }
        }
    }

    pub const fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::Diagnostic(_) | Self::ProjectionUnderestimated { .. } | Self::Cancelled(_) => {
                None
            }
            Self::ResourceLimit(error) => Some(error),
        }
    }

    /// The memory-budget refusal of the framework's resource admission; a pass-specific
    /// diagnostic is the pass's own type and is not inspected.
    pub const fn memory_budget_exceeded(&self) -> Option<&MemoryBudgetExceeded> {
        match self {
            Self::ResourceLimit(error) => error.memory_budget_exceeded(),
            Self::Diagnostic(_) | Self::ProjectionUnderestimated { .. } | Self::Cancelled(_) => {
                None
            }
        }
    }

    /// Returns the typed framework stage for a circuit-pass resource rejection.
    pub const fn resource_stage(&self) -> Option<CircuitPassStage> {
        match self {
            Self::ResourceLimit(error) => error.circuit_pass_stage(),
            Self::Diagnostic(_) | Self::ProjectionUnderestimated { .. } | Self::Cancelled(_) => {
                None
            }
        }
    }

    pub const fn projection_underestimate(&self) -> Option<(ResourceKind, u64, u64)> {
        match self {
            Self::ProjectionUnderestimated {
                resource,
                projected,
                actual,
            } => Some((*resource, *projected, *actual)),
            Self::Diagnostic(_) | Self::ResourceLimit(_) | Self::Cancelled(_) => None,
        }
    }
}

/// Executes a pass with common input admission and output validation.
///
/// The context's cancellation signal is polled once per folded item of the input and of the
/// returned circuit, and before and after each pass method; a failure the pass returns while the
/// signal is set is reported as [`CircuitPassError::Cancelled`].
pub fn run_circuit_pass<P: CircuitPass + ?Sized>(
    pass: &P,
    circuit: &Circuit,
    options: &P::Options,
    context: &CircuitPassContext,
) -> Result<CircuitPassOutput<P::Report>, CircuitPassError<P::Diagnostic>> {
    let cancellation = &context.cancellation;
    cancellation.check()?;
    let resources = validate_circuit(circuit, context, CircuitPassStage::Input)?;
    let input = CircuitPassInput {
        circuit,
        context,
        resources,
    };
    let projected = pass
        .project_output_resources(input, options)
        .map_err(|diagnostic| pass_failure(diagnostic, cancellation))?;
    admit_resources(projected, context, CircuitPassStage::OutputProjection)?;
    cancellation.check()?;
    let output = pass
        .run(input, options)
        .map_err(|diagnostic| pass_failure(diagnostic, cancellation))?;
    let actual = validate_circuit(output.circuit(), context, CircuitPassStage::Output)?;
    ensure_projection_covers(projected, actual)?;
    Ok(output)
}

/// Reports a pass failure as cancellation when the signal is set, since a pass that observes the
/// signal stops through its own diagnostic type.
fn pass_failure<D: Error + 'static>(
    diagnostic: D,
    cancellation: &Cancellation,
) -> CircuitPassError<D> {
    match cancellation.check() {
        Ok(()) => CircuitPassError::Diagnostic(diagnostic),
        Err(cancelled) => CircuitPassError::Cancelled(cancelled),
    }
}

/// Failure while admitting a circuit: a resource limit or the context's signal.
enum ValidationFailure {
    ResourceLimit(ResourceLimitError),
    Cancelled(Cancelled),
}

impl From<ResourceLimitError> for ValidationFailure {
    fn from(error: ResourceLimitError) -> Self {
        Self::ResourceLimit(error)
    }
}

impl From<Cancelled> for ValidationFailure {
    fn from(cancelled: Cancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

impl<D: Error + 'static> From<ValidationFailure> for CircuitPassError<D> {
    fn from(failure: ValidationFailure) -> Self {
        match failure {
            ValidationFailure::ResourceLimit(error) => Self::ResourceLimit(error),
            ValidationFailure::Cancelled(cancelled) => Self::Cancelled(cancelled),
        }
    }
}

/// The context's limits and signal, with the countdown that spaces polls across folded items.
struct ValidationScope<'a> {
    limits: &'a CircuitPassContext,
    stage: CircuitPassStage,
    countdown: CancellationCountdown,
}

fn validate_circuit(
    circuit: &Circuit,
    context: &CircuitPassContext,
    stage: CircuitPassStage,
) -> Result<CircuitPassResources, ValidationFailure> {
    let mut scope = ValidationScope {
        limits: context,
        stage,
        countdown: CancellationCountdown::new(),
    };
    let mut represented_items = 0;
    let mut target_occurrences = 0;
    let mut argument_values = 0;
    let mut tag_bytes = 0;
    let mut repeat_nesting = 0;
    validate_block(
        circuit,
        0,
        &mut scope,
        &mut represented_items,
        &mut target_occurrences,
        &mut argument_values,
        &mut tag_bytes,
        &mut repeat_nesting,
    )?;
    let resources = CircuitPassResources::try_new(
        represented_items,
        target_occurrences,
        argument_values,
        tag_bytes,
        repeat_nesting,
    )
    .map_err(|error| {
        ResourceLimitError::circuit_pass(
            stage,
            error.resource(),
            u64::MAX,
            resource_limit(context, error.resource()),
        )
    })?;
    admit_resources(resources, context, stage)?;
    Ok(resources)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the recursive validator carries one counter per independently admitted resource"
)]
fn validate_block(
    circuit: &Circuit,
    depth: usize,
    scope: &mut ValidationScope<'_>,
    represented_items: &mut u64,
    target_occurrences: &mut u64,
    argument_values: &mut u64,
    tag_bytes: &mut u64,
    repeat_nesting: &mut usize,
) -> Result<(), ValidationFailure> {
    let (limits, stage) = (scope.limits, scope.stage);
    for item in circuit.items() {
        scope.countdown.tick(&scope.limits.cancellation)?;
        add_unbounded(represented_items, 1);
        match item {
            CircuitItem::Instruction(instruction) => {
                add_unbounded(tag_bytes, instruction.tag_bytes().map_or(0, <[u8]>::len));
                add_unbounded(target_occurrences, instruction.targets().len());
                add_unbounded(argument_values, instruction.args().len());
            }
            CircuitItem::RepeatBlock(repeat) => {
                let next_depth = depth.saturating_add(1);
                *repeat_nesting = (*repeat_nesting).max(next_depth);
                add_unbounded(tag_bytes, repeat.tag_bytes().map_or(0, <[u8]>::len));
                let limit = limits.repeat_nesting.get();
                if next_depth > limit {
                    return Err(ResourceLimitError::circuit_pass(
                        stage,
                        ResourceKind::RepeatNesting,
                        next_depth as u64,
                        limit as u64,
                    )
                    .into());
                }
                validate_block(
                    repeat.body(),
                    next_depth,
                    scope,
                    represented_items,
                    target_occurrences,
                    argument_values,
                    tag_bytes,
                    repeat_nesting,
                )?;
            }
        }
    }
    Ok(())
}

fn add_unbounded(total: &mut u64, additional: usize) {
    *total = total.saturating_add(u64::try_from(additional).unwrap_or(u64::MAX));
}

fn admit_resources(
    resources: CircuitPassResources,
    limits: &CircuitPassContext,
    stage: CircuitPassStage,
) -> Result<(), ResourceLimitError> {
    for (resource, actual, limit) in [
        (
            ResourceKind::ProjectedPayloadBytes,
            resources.projected_payload_bytes(),
            limits.memory_budget.bytes(),
        ),
        (
            ResourceKind::RepeatNesting,
            u64::try_from(resources.repeat_nesting()).unwrap_or(u64::MAX),
            u64::try_from(limits.repeat_nesting.get()).unwrap_or(u64::MAX),
        ),
    ] {
        if actual > limit {
            return Err(ResourceLimitError::circuit_pass(
                stage, resource, actual, limit,
            ));
        }
    }
    Ok(())
}

fn ensure_projection_covers<D: Error + 'static>(
    projected: CircuitPassResources,
    actual: CircuitPassResources,
) -> Result<(), CircuitPassError<D>> {
    for (resource, projected, actual) in [
        (
            ResourceKind::RepresentedItems,
            projected.represented_items(),
            actual.represented_items(),
        ),
        (
            ResourceKind::TargetOccurrences,
            projected.target_occurrences(),
            actual.target_occurrences(),
        ),
        (
            ResourceKind::ArgumentValues,
            projected.argument_values(),
            actual.argument_values(),
        ),
        (
            ResourceKind::ProjectedPayloadBytes,
            projected.projected_payload_bytes(),
            actual.projected_payload_bytes(),
        ),
        (
            ResourceKind::RepeatNesting,
            u64::try_from(projected.repeat_nesting()).unwrap_or(u64::MAX),
            u64::try_from(actual.repeat_nesting()).unwrap_or(u64::MAX),
        ),
    ] {
        if actual > projected {
            return Err(CircuitPassError::ProjectionUnderestimated {
                resource,
                projected,
                actual,
            });
        }
    }
    Ok(())
}

fn projected_payload_bytes(
    represented_items: u64,
    target_occurrences: u64,
    argument_values: u64,
    tag_bytes: u64,
) -> Result<u64, CircuitPassProjectionError> {
    projected_circuit_payload_bytes(
        represented_items,
        target_occurrences,
        argument_values,
        tag_bytes,
    )
    .ok_or(CircuitPassProjectionError {
        resource: ResourceKind::ProjectedPayloadBytes,
    })
}

/// Projects the logical payload bytes for folded circuit content before allocation.
///
/// This is the one owner of the payload-byte estimate shared by the circuit-pass framework and
/// the flatten transform: represented items, target occurrences, argument floats, and opaque-tag
/// storage (exact bytes plus lossy-display copies) each contribute their in-memory size, using
/// checked arithmetic throughout. Returns `None` when the projection overflows `u64`; callers map
/// that to their own typed overflow error.
pub(crate) fn projected_circuit_payload_bytes(
    represented_items: u64,
    target_occurrences: u64,
    argument_values: u64,
    tag_bytes: u64,
) -> Option<u64> {
    let item_bytes = represented_items.checked_mul(element_size_u64::<CircuitItem>())?;
    let target_bytes = target_occurrences.checked_mul(element_size_u64::<Target>())?;
    let argument_bytes = argument_values.checked_mul(element_size_u64::<f64>())?;
    let tag_bytes = tag_bytes.checked_mul(OPAQUE_TAG_STORAGE_MULTIPLIER)?;
    item_bytes
        .checked_add(target_bytes)?
        .checked_add(argument_bytes)?
        .checked_add(tag_bytes)
}

/// Converts an element size to the estimator's `u64` domain, saturating so that an
/// unrepresentable size can only overreport and fail closed at the overflow checks.
fn element_size_u64<T>() -> u64 {
    u64::try_from(std::mem::size_of::<T>()).unwrap_or(u64::MAX)
}

fn checked_resource_add(
    left: u64,
    right: u64,
    resource: ResourceKind,
) -> Result<u64, CircuitPassProjectionError> {
    left.checked_add(right)
        .ok_or(CircuitPassProjectionError { resource })
}

const fn resource_limit(limits: &CircuitPassContext, resource: ResourceKind) -> u64 {
    match resource {
        ResourceKind::ProjectedPayloadBytes => limits.memory_budget.bytes(),
        ResourceKind::RepeatNesting => limits.repeat_nesting.get() as u64,
        _ => u64::MAX,
    }
}
