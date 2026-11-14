//! One checked, cancellation-aware walk of a folded circuit and the counts it summarizes.
//!
//! Analyses that admit work before unrolling repeat blocks need the same facts: how many times
//! each instruction runs, how many repeat iterations execute, and how deep blocks nest. This walk
//! visits every folded item once, multiplies repeat counts with overflow kept as a value instead
//! of a panic, bounds recursion by a [`RepeatNestingLimit`], and polls one [`Cancellation`]. It
//! never allocates in proportion to a repeat count.

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};
use crate::cancellation::CancellationCountdown;
use crate::{Cancellation, Cancelled, GateKind, ModelResult, RepeatNestingLimit, ValidationError};

/// An expanded count through repeat blocks, or the fact that it exceeds `u64`.
///
/// Products are zero-absorbing: a zero factor yields zero even when the other factor has
/// already overflowed, so a consumer that scales a zero-sized contribution never fails.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldedCount(Option<u64>);

impl FoldedCount {
    pub const ZERO: Self = Self(Some(0));
    pub const ONE: Self = Self(Some(1));

    /// Returns an exact count.
    pub const fn new(value: u64) -> Self {
        Self(Some(value))
    }

    /// Returns the count, or `None` when it exceeds `u64`.
    pub const fn exact(self) -> Option<u64> {
        self.0
    }

    /// Returns the count, or a typed overflow error when it exceeds `u64`.
    pub fn get(self) -> ModelResult<u64> {
        self.0
            .ok_or_else(|| ValidationError::CircuitCountOverflow.into())
    }

    /// Returns the count, or `u64::MAX` when it exceeds `u64`.
    pub const fn saturating(self) -> u64 {
        match self.0 {
            Some(value) => value,
            None => u64::MAX,
        }
    }

    /// Multiplies two counts; zero times anything is zero.
    #[must_use]
    pub const fn times(self, other: Self) -> Self {
        match (self.0, other.0) {
            (Some(0), _) | (_, Some(0)) => Self::ZERO,
            (Some(left), Some(right)) => Self(left.checked_mul(right)),
            _ => Self(None),
        }
    }

    /// Adds two counts.
    #[must_use]
    pub const fn plus(self, other: Self) -> Self {
        match (self.0, other.0) {
            (Some(left), Some(right)) => Self(left.checked_add(right)),
            _ => Self(None),
        }
    }

    fn of_len(len: usize) -> Self {
        Self(u64::try_from(len).ok())
    }
}

/// Why a folded walk stopped before visiting every item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoldedWalkError {
    /// A repeat body is nested deeper than the walk's limit; `depth` is the first rejected
    /// depth, where the body of a top-level `REPEAT` is depth one.
    RepeatNesting {
        depth: usize,
        limit: usize,
    },
    Cancelled(Cancelled),
}

/// One folded item, visited once, with how many times it runs in the expanded circuit.
#[derive(Clone, Copy, Debug)]
pub enum FoldedCircuitEvent<'a> {
    Instruction {
        instruction: &'a CircuitInstruction,
        /// Product of the repeat counts of every enclosing block.
        executions: FoldedCount,
    },
    /// A repeat block, visited before its body.
    Repeat {
        repeat: &'a RepeatBlock,
        /// Expanded iterations of this block: its repeat count times its own executions.
        iterations: FoldedCount,
        /// Nesting depth of the block's body.
        body_depth: usize,
    },
}

/// The error of [`visit_folded_circuit`]: the walk's own refusal or the visitor's error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FoldedVisitError<E> {
    Walk(FoldedWalkError),
    Visitor(E),
}

/// Visits every folded item of `circuit` in source order with its expanded multiplicity.
///
/// A repeat block is reported before its body; entering a body deeper than `nesting` stops the
/// walk with [`FoldedWalkError::RepeatNesting`], so recursion is bounded by the limit, which is
/// at most [`RepeatNestingLimit::HARD_MAX`]. The cancellation signal is polled once every
/// 1024 visited items.
pub fn visit_folded_circuit<E>(
    circuit: &Circuit,
    nesting: RepeatNestingLimit,
    cancellation: &Cancellation,
    mut visitor: impl FnMut(FoldedCircuitEvent<'_>) -> Result<(), E>,
) -> Result<(), FoldedVisitError<E>> {
    let mut walk = FoldedWalk {
        limit: nesting.get(),
        cancellation,
        countdown: CancellationCountdown::default(),
    };
    walk.visit(circuit, FoldedCount::ONE, 0, &mut visitor)
}

struct FoldedWalk<'a> {
    limit: usize,
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl FoldedWalk<'_> {
    fn visit<E>(
        &mut self,
        circuit: &Circuit,
        executions: FoldedCount,
        depth: usize,
        visitor: &mut impl FnMut(FoldedCircuitEvent<'_>) -> Result<(), E>,
    ) -> Result<(), FoldedVisitError<E>> {
        if depth > self.limit {
            return Err(FoldedVisitError::Walk(FoldedWalkError::RepeatNesting {
                depth,
                limit: self.limit,
            }));
        }
        for item in circuit.items() {
            self.countdown
                .tick(self.cancellation)
                .map_err(|cancelled| {
                    FoldedVisitError::Walk(FoldedWalkError::Cancelled(cancelled))
                })?;
            match item {
                CircuitItem::Instruction(instruction) => visitor(FoldedCircuitEvent::Instruction {
                    instruction,
                    executions,
                })
                .map_err(FoldedVisitError::Visitor)?,
                CircuitItem::RepeatBlock(repeat) => {
                    let iterations =
                        executions.times(FoldedCount::new(repeat.repeat_count().get()));
                    let body_depth = depth.saturating_add(1);
                    visitor(FoldedCircuitEvent::Repeat {
                        repeat,
                        iterations,
                        body_depth,
                    })
                    .map_err(FoldedVisitError::Visitor)?;
                    self.visit(repeat.body(), iterations, body_depth, visitor)?;
                }
            }
        }
        Ok(())
    }
}

/// Expanded totals for one gate kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KindTally {
    instructions: FoldedCount,
    targets: FoldedCount,
    arguments: FoldedCount,
    tag_bytes: FoldedCount,
    measurements: FoldedCount,
}

impl KindTally {
    const ZERO: Self = Self {
        instructions: FoldedCount::ZERO,
        targets: FoldedCount::ZERO,
        arguments: FoldedCount::ZERO,
        tag_bytes: FoldedCount::ZERO,
        measurements: FoldedCount::ZERO,
    };
}

/// Counts of a folded circuit as if every repeat block were unrolled, from one folded walk.
///
/// Every expanded count is a [`FoldedCount`], so a consumer reports an overflow of the count it
/// needs with its own error instead of the walk failing for counts it never reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoldedCircuitSummary {
    folded_items: u64,
    repeat_iterations: FoldedCount,
    max_repeat_depth: usize,
    kinds: Box<[KindTally; GateKind::COUNT]>,
}

impl FoldedCircuitSummary {
    /// Summarizes `circuit`, rejecting nesting deeper than `nesting`.
    pub fn of(
        circuit: &Circuit,
        nesting: RepeatNestingLimit,
        cancellation: &Cancellation,
    ) -> Result<Self, FoldedWalkError> {
        let mut summary = Self {
            folded_items: 0,
            repeat_iterations: FoldedCount::ZERO,
            max_repeat_depth: 0,
            kinds: Box::new([KindTally::ZERO; GateKind::COUNT]),
        };
        let walked = visit_folded_circuit(circuit, nesting, cancellation, |event| {
            summary.add(event);
            Ok::<(), std::convert::Infallible>(())
        });
        match walked {
            Ok(()) => Ok(summary),
            Err(FoldedVisitError::Walk(error)) => Err(error),
            Err(FoldedVisitError::Visitor(never)) => match never {},
        }
    }

    fn add(&mut self, event: FoldedCircuitEvent<'_>) {
        self.folded_items = self.folded_items.saturating_add(1);
        match event {
            FoldedCircuitEvent::Instruction {
                instruction,
                executions,
            } => {
                let Some(tally) = self.kinds.get_mut(instruction.gate().kind() as usize) else {
                    return;
                };
                let scaled = |len: usize| FoldedCount::of_len(len).times(executions);
                tally.instructions = tally.instructions.plus(executions);
                tally.targets = tally.targets.plus(scaled(instruction.targets().len()));
                tally.arguments = tally.arguments.plus(scaled(instruction.args().len()));
                tally.tag_bytes = tally
                    .tag_bytes
                    .plus(scaled(instruction.tag_bytes().map_or(0, <[u8]>::len)));
                tally.measurements = tally
                    .measurements
                    .plus(scaled(instruction.measurement_result_count()));
            }
            FoldedCircuitEvent::Repeat {
                iterations,
                body_depth,
                ..
            } => {
                self.repeat_iterations = self.repeat_iterations.plus(iterations);
                self.max_repeat_depth = self.max_repeat_depth.max(body_depth);
            }
        }
    }

    fn total(&self, field: impl Fn(&KindTally) -> FoldedCount) -> FoldedCount {
        self.kinds
            .iter()
            .fold(FoldedCount::ZERO, |total, tally| total.plus(field(tally)))
    }

    fn of_kind(&self, kind: GateKind, field: impl Fn(&KindTally) -> FoldedCount) -> FoldedCount {
        self.kinds
            .get(kind as usize)
            .map_or(FoldedCount::ZERO, field)
    }

    /// Instructions and repeat blocks as written, without multiplying by repeat counts.
    pub fn folded_items(&self) -> u64 {
        self.folded_items
    }

    /// Expanded iterations of every repeat block, nested blocks included.
    pub fn repeat_iterations(&self) -> FoldedCount {
        self.repeat_iterations
    }

    /// The deepest repeat body; zero for a circuit without repeat blocks.
    pub fn max_repeat_depth(&self) -> usize {
        self.max_repeat_depth
    }

    /// Expanded instructions of every gate kind.
    pub fn instructions(&self) -> FoldedCount {
        self.total(|tally| tally.instructions)
    }

    /// Expanded instructions of one gate kind.
    pub fn instructions_of(&self, kind: GateKind) -> FoldedCount {
        self.of_kind(kind, |tally| tally.instructions)
    }

    /// Expanded instruction targets, combiners and record targets included.
    pub fn targets(&self) -> FoldedCount {
        self.total(|tally| tally.targets)
    }

    /// Expanded parenthesized instruction arguments.
    pub fn arguments(&self) -> FoldedCount {
        self.total(|tally| tally.arguments)
    }

    /// Expanded instruction tag bytes.
    pub fn tag_bytes(&self) -> FoldedCount {
        self.total(|tally| tally.tag_bytes)
    }

    /// Expanded measurement-record results, heralds included.
    pub fn measurements(&self) -> FoldedCount {
        self.total(|tally| tally.measurements)
    }

    /// Expanded measurement-record results of one gate kind.
    pub fn measurements_of(&self, kind: GateKind) -> FoldedCount {
        self.of_kind(kind, |tally| tally.measurements)
    }

    /// Expanded `DETECTOR` instructions.
    pub fn detectors(&self) -> FoldedCount {
        self.instructions_of(GateKind::DETECTOR)
    }

    /// Expanded `TICK` instructions.
    pub fn ticks(&self) -> FoldedCount {
        self.instructions_of(GateKind::TICK)
    }
}

impl From<FoldedWalkError> for crate::ModelError {
    fn from(error: FoldedWalkError) -> Self {
        match error {
            FoldedWalkError::RepeatNesting { depth, limit } => Self::invalid_domain_value(
                "circuit repeat nesting",
                format!("depth {depth} exceeds the {limit}-level limit"),
            ),
            FoldedWalkError::Cancelled(cancelled) => Self::Cancelled(cancelled),
        }
    }
}
