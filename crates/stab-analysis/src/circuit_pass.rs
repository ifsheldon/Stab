use std::error::Error;

use stab_model::{Circuit, CircuitItem, ParseLimits, RepeatNestingLimit, Target};
use thiserror::Error;

use crate::resources::CircuitPassStage;
use crate::{ResourceKind, ResourceLimitError};

const MAX_PROJECTED_PASS_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
const OPAQUE_TAG_STORAGE_MULTIPLIER: u64 = 4;

/// Resource policy applied before and after a circuit pass runs.
///
/// Counts describe the represented, folded circuit. Repeat counts do not multiply item, target,
/// or argument totals. Repeat nesting is always bounded by the model's hard recursive-safety
/// ceiling and may be tightened by callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CircuitPassLimits {
    max_represented_items: u64,
    max_target_occurrences: u64,
    max_argument_values: u64,
    max_projected_payload_bytes: u64,
    repeat_nesting: RepeatNestingLimit,
}

impl CircuitPassLimits {
    pub const DEFAULT_MAX_REPRESENTED_ITEMS: u64 = 1_000_000;
    pub const DEFAULT_MAX_TARGET_OCCURRENCES: u64 = 32_000_000;
    pub const DEFAULT_MAX_ARGUMENT_VALUES: u64 = 16_000_000;
    pub const DEFAULT_MAX_PROJECTED_PAYLOAD_BYTES: u64 = MAX_PROJECTED_PASS_PAYLOAD_BYTES;

    pub const fn new(
        max_represented_items: u64,
        max_target_occurrences: u64,
        max_argument_values: u64,
        max_projected_payload_bytes: u64,
        repeat_nesting: RepeatNestingLimit,
    ) -> Self {
        Self {
            max_represented_items,
            max_target_occurrences,
            max_argument_values,
            max_projected_payload_bytes,
            repeat_nesting,
        }
    }

    /// Returns the widest policy permitted by the fixed repeat-nesting safety envelope.
    pub fn maximal() -> Self {
        Self::new(
            u64::MAX,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            ParseLimits::DEFAULT_REPEAT_NESTING,
        )
    }

    pub const fn max_represented_items(self) -> u64 {
        self.max_represented_items
    }

    pub const fn max_target_occurrences(self) -> u64 {
        self.max_target_occurrences
    }

    pub const fn max_argument_values(self) -> u64 {
        self.max_argument_values
    }

    /// Returns the maximum admitted logical payload, excluding allocator metadata and spare capacity.
    pub const fn max_projected_payload_bytes(self) -> u64 {
        self.max_projected_payload_bytes
    }

    pub const fn repeat_nesting(self) -> RepeatNestingLimit {
        self.repeat_nesting
    }

    #[must_use]
    pub const fn with_max_represented_items(mut self, max_represented_items: u64) -> Self {
        self.max_represented_items = max_represented_items;
        self
    }

    #[must_use]
    pub const fn with_max_target_occurrences(mut self, max_target_occurrences: u64) -> Self {
        self.max_target_occurrences = max_target_occurrences;
        self
    }

    #[must_use]
    pub const fn with_max_argument_values(mut self, max_argument_values: u64) -> Self {
        self.max_argument_values = max_argument_values;
        self
    }

    #[must_use]
    pub const fn with_max_projected_payload_bytes(
        mut self,
        max_projected_payload_bytes: u64,
    ) -> Self {
        self.max_projected_payload_bytes = max_projected_payload_bytes;
        self
    }

    #[must_use]
    pub const fn with_repeat_nesting(mut self, repeat_nesting: RepeatNestingLimit) -> Self {
        self.repeat_nesting = repeat_nesting;
        self
    }
}

impl Default for CircuitPassLimits {
    fn default() -> Self {
        Self::new(
            Self::DEFAULT_MAX_REPRESENTED_ITEMS,
            Self::DEFAULT_MAX_TARGET_OCCURRENCES,
            Self::DEFAULT_MAX_ARGUMENT_VALUES,
            Self::DEFAULT_MAX_PROJECTED_PAYLOAD_BYTES,
            ParseLimits::DEFAULT_REPEAT_NESTING,
        )
    }
}

/// Immutable context shared by circuit-pass implementations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CircuitPassContext {
    limits: CircuitPassLimits,
}

impl CircuitPassContext {
    pub const fn new(limits: CircuitPassLimits) -> Self {
        Self { limits }
    }

    pub const fn limits(self) -> CircuitPassLimits {
        self.limits
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
}

impl<D: Error + 'static> CircuitPassError<D> {
    pub const fn diagnostic(&self) -> Option<&D> {
        match self {
            Self::Diagnostic(diagnostic) => Some(diagnostic),
            Self::ResourceLimit(_) | Self::ProjectionUnderestimated { .. } => None,
        }
    }

    pub const fn resource_limit_error(&self) -> Option<&ResourceLimitError> {
        match self {
            Self::Diagnostic(_) | Self::ProjectionUnderestimated { .. } => None,
            Self::ResourceLimit(error) => Some(error),
        }
    }

    /// Returns the typed framework stage for a circuit-pass resource rejection.
    pub const fn resource_stage(&self) -> Option<CircuitPassStage> {
        match self {
            Self::ResourceLimit(error) => error.circuit_pass_stage(),
            Self::Diagnostic(_) | Self::ProjectionUnderestimated { .. } => None,
        }
    }

    pub const fn projection_underestimate(&self) -> Option<(ResourceKind, u64, u64)> {
        match self {
            Self::ProjectionUnderestimated {
                resource,
                projected,
                actual,
            } => Some((*resource, *projected, *actual)),
            Self::Diagnostic(_) | Self::ResourceLimit(_) => None,
        }
    }
}

/// Executes a pass with common input admission and output validation.
pub fn run_circuit_pass<P: CircuitPass + ?Sized>(
    pass: &P,
    circuit: &Circuit,
    options: &P::Options,
    context: &CircuitPassContext,
) -> Result<CircuitPassOutput<P::Report>, CircuitPassError<P::Diagnostic>> {
    let resources = validate_circuit(circuit, context.limits(), CircuitPassStage::Input)?;
    let input = CircuitPassInput {
        circuit,
        context,
        resources,
    };
    let projected = pass
        .project_output_resources(input, options)
        .map_err(CircuitPassError::Diagnostic)?;
    admit_resources(
        projected,
        context.limits(),
        CircuitPassStage::OutputProjection,
    )?;
    let output = pass
        .run(input, options)
        .map_err(CircuitPassError::Diagnostic)?;
    let actual = validate_circuit(output.circuit(), context.limits(), CircuitPassStage::Output)?;
    ensure_projection_covers(projected, actual)?;
    Ok(output)
}

fn validate_circuit(
    circuit: &Circuit,
    limits: CircuitPassLimits,
    stage: CircuitPassStage,
) -> Result<CircuitPassResources, ResourceLimitError> {
    let mut represented_items = 0;
    let mut target_occurrences = 0;
    let mut argument_values = 0;
    let mut tag_bytes = 0;
    let mut repeat_nesting = 0;
    validate_block(
        circuit,
        0,
        limits,
        stage,
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
            resource_limit(limits, error.resource()),
        )
    })?;
    admit_resources(resources, limits, stage)?;
    Ok(resources)
}

#[allow(
    clippy::too_many_arguments,
    reason = "the recursive validator carries one counter per independently admitted resource"
)]
fn validate_block(
    circuit: &Circuit,
    depth: usize,
    limits: CircuitPassLimits,
    stage: CircuitPassStage,
    represented_items: &mut u64,
    target_occurrences: &mut u64,
    argument_values: &mut u64,
    tag_bytes: &mut u64,
    repeat_nesting: &mut usize,
) -> Result<(), ResourceLimitError> {
    for item in circuit.items() {
        add_and_admit(
            represented_items,
            1,
            limits.max_represented_items(),
            stage,
            ResourceKind::RepresentedItems,
        )?;
        match item {
            CircuitItem::Instruction(instruction) => {
                add_unbounded(tag_bytes, instruction.tag_bytes().map_or(0, <[u8]>::len));
                add_and_admit(
                    target_occurrences,
                    instruction.targets().len(),
                    limits.max_target_occurrences(),
                    stage,
                    ResourceKind::TargetOccurrences,
                )?;
                add_and_admit(
                    argument_values,
                    instruction.args().len(),
                    limits.max_argument_values(),
                    stage,
                    ResourceKind::ArgumentValues,
                )?;
            }
            CircuitItem::RepeatBlock(repeat) => {
                let next_depth = depth.saturating_add(1);
                *repeat_nesting = (*repeat_nesting).max(next_depth);
                add_unbounded(tag_bytes, repeat.tag_bytes().map_or(0, <[u8]>::len));
                let limit = limits.repeat_nesting().get();
                if next_depth > limit {
                    return Err(ResourceLimitError::circuit_pass(
                        stage,
                        ResourceKind::RepeatNesting,
                        next_depth as u64,
                        limit as u64,
                    ));
                }
                validate_block(
                    repeat.body(),
                    next_depth,
                    limits,
                    stage,
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

fn add_and_admit(
    total: &mut u64,
    additional: usize,
    limit: u64,
    stage: CircuitPassStage,
    resource: ResourceKind,
) -> Result<(), ResourceLimitError> {
    let additional = u64::try_from(additional).unwrap_or(u64::MAX);
    *total = total.saturating_add(additional);
    if *total > limit {
        return Err(ResourceLimitError::circuit_pass(
            stage, resource, *total, limit,
        ));
    }
    Ok(())
}

fn admit_resources(
    resources: CircuitPassResources,
    limits: CircuitPassLimits,
    stage: CircuitPassStage,
) -> Result<(), ResourceLimitError> {
    for (resource, actual, limit) in [
        (
            ResourceKind::RepresentedItems,
            resources.represented_items(),
            limits.max_represented_items(),
        ),
        (
            ResourceKind::TargetOccurrences,
            resources.target_occurrences(),
            limits.max_target_occurrences(),
        ),
        (
            ResourceKind::ArgumentValues,
            resources.argument_values(),
            limits.max_argument_values(),
        ),
        (
            ResourceKind::ProjectedPayloadBytes,
            resources.projected_payload_bytes(),
            limits.max_projected_payload_bytes(),
        ),
        (
            ResourceKind::RepeatNesting,
            u64::try_from(resources.repeat_nesting()).unwrap_or(u64::MAX),
            u64::try_from(limits.repeat_nesting().get()).unwrap_or(u64::MAX),
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

const fn resource_limit(limits: CircuitPassLimits, resource: ResourceKind) -> u64 {
    match resource {
        ResourceKind::RepresentedItems => limits.max_represented_items(),
        ResourceKind::TargetOccurrences => limits.max_target_occurrences(),
        ResourceKind::ArgumentValues => limits.max_argument_values(),
        ResourceKind::ProjectedPayloadBytes => limits.max_projected_payload_bytes(),
        ResourceKind::RepeatNesting => limits.repeat_nesting().get() as u64,
        _ => u64::MAX,
    }
}
