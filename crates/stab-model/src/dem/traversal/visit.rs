//! The folded visit loop and the per-visit coordinate-shift cache.
//!
//! The visit walks the folded tree with an explicit stack of open repeat bodies, one frame per
//! body iteration being visited, so it uses constant native stack at any nesting depth. Its
//! callbacks, state updates, checked arithmetic, and errors occur in the same order as a
//! body-first recursion.

use std::ops::ControlFlow;

use super::{
    DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem, FoldedDemVisitor,
    add_coordinate_shift_mul,
};
use crate::dem::{DemInstructionKind, DemRepeatBlock};
use crate::{ModelError, ModelResult};

/// Coordinate shifts of repeat bodies, each computed at most once.
///
/// A body's shift is accumulated from zero exactly as [`FoldedDemBlock::coordinate_shift`] does;
/// computing it also records the shift of every body nested inside, and a cached nested shift
/// is added in place of revisiting that body, which performs the same floating-point operations.
/// Repeated coordinate queries over nested bodies therefore cost time linear in the compact
/// model instead of quadratic in its nesting depth. The cache holds at most one vector per
/// compact block, each no longer than the widest coordinate shift nested inside it.
#[derive(Default)]
pub(in crate::dem) struct CoordinateShifts {
    by_block: Vec<Option<Vec<f64>>>,
}

impl CoordinateShifts {
    /// Returns the coordinate shift `block` applies, computing and caching it on first use.
    pub(in crate::dem) fn of(&mut self, block: &FoldedDemBlock<'_>) -> ModelResult<&[f64]> {
        if self.cached(block.compact_id).is_none() {
            let shift = self.accumulate(block, true)?;
            self.store(block.compact_id, shift)?;
        }
        self.cached(block.compact_id).ok_or_else(|| {
            ModelError::invalid_detector_error_model(
                "DEM coordinate shift cache lost a computed block",
            )
        })
    }

    /// Accumulates the coordinate shift of `block` without recursing, reusing cached nested
    /// shifts and, when `retain` is set, caching every nested shift it computes.
    pub(super) fn accumulate(
        &mut self,
        block: &FoldedDemBlock<'_>,
        retain: bool,
    ) -> ModelResult<Vec<f64>> {
        let mut enclosing: Vec<(
            std::slice::Iter<'_, FoldedDemItem<'_>>,
            Vec<f64>,
            u64,
            usize,
        )> = Vec::new();
        let mut items = block.items.iter();
        let mut shift = Vec::new();
        loop {
            match items.next() {
                Some(FoldedDemItem::Instruction(instruction))
                    if instruction.kind() == DemInstructionKind::ShiftDetectors =>
                {
                    add_coordinate_shift_mul(&mut shift, instruction.args(), 1.0)?;
                }
                Some(FoldedDemItem::Instruction(_)) => {}
                Some(FoldedDemItem::Repeat { repeat, body }) => {
                    let repeat_count = repeat.repeat_count().get();
                    if let Some(body_shift) = self.cached(body.compact_id) {
                        add_coordinate_shift_mul(&mut shift, body_shift, repeat_count as f64)?;
                        continue;
                    }
                    let parent = std::mem::replace(&mut items, body.items.iter());
                    let parent_shift = std::mem::take(&mut shift);
                    enclosing.push((parent, parent_shift, repeat_count, body.compact_id));
                }
                None => {
                    let Some((parent, parent_shift, repeat_count, body_id)) = enclosing.pop()
                    else {
                        return Ok(shift);
                    };
                    let body_shift = std::mem::replace(&mut shift, parent_shift);
                    add_coordinate_shift_mul(&mut shift, &body_shift, repeat_count as f64)?;
                    if retain {
                        self.store(body_id, body_shift)?;
                    }
                    items = parent;
                }
            }
        }
    }

    fn cached(&self, compact_id: usize) -> Option<&[f64]> {
        self.by_block.get(compact_id)?.as_deref()
    }

    fn store(&mut self, compact_id: usize, shift: Vec<f64>) -> ModelResult<()> {
        if self.by_block.len() <= compact_id {
            let additional = compact_id - self.by_block.len() + 1;
            self.by_block.try_reserve(additional).map_err(|_| {
                ModelError::invalid_detector_error_model(
                    "DEM coordinate shift cache allocation failed",
                )
            })?;
            self.by_block.resize_with(compact_id + 1, || None);
        }
        let slot = self.by_block.get_mut(compact_id).ok_or_else(|| {
            ModelError::invalid_detector_error_model("DEM coordinate shift cache slot is missing")
        })?;
        *slot = Some(shift);
        Ok(())
    }
}

#[derive(Default)]
struct ExpansionBudget {
    used_iterations: u64,
}

/// The iterations of one repeat that remain to be visited.
enum RepeatIterations {
    /// At most one iteration, index 0, without polling cancellation.
    Once { pending: bool },
    /// Every iteration in `next..end`, polling cancellation before each.
    Expanded { next: u64, end: u64 },
    /// Visitor-chosen iterations, validated and polled one at a time.
    Selected {
        iterations: std::vec::IntoIter<u64>,
        previous: Option<u64>,
    },
}

impl RepeatIterations {
    fn next_iteration<V: FoldedDemVisitor>(
        &mut self,
        repeat_count: u64,
        visitor: &mut V,
    ) -> Result<Option<u64>, V::Error> {
        match self {
            Self::Once { pending } => Ok(std::mem::take(pending).then_some(0)),
            Self::Expanded { next, end } => {
                if *next >= *end {
                    return Ok(None);
                }
                visitor.poll_cancellation()?;
                let iteration = *next;
                *next += 1;
                Ok(Some(iteration))
            }
            Self::Selected {
                iterations,
                previous,
            } => {
                let Some(iteration) = iterations.next() else {
                    return Ok(None);
                };
                if iteration >= repeat_count || previous.is_some_and(|value| value >= iteration) {
                    return Err(ModelError::invalid_detector_error_model(
                        "folded traversal selected repeat iterations must be strictly increasing and in range",
                    )
                    .into());
                }
                *previous = Some(iteration);
                visitor.poll_cancellation()?;
                Ok(Some(iteration))
            }
        }
    }
}

/// A repeat whose selected body iterations are being visited.
struct OpenRepeat<'b, 'a> {
    repeat: &'a DemRepeatBlock,
    body: &'b FoldedDemBlock<'a>,
    iterations: RepeatIterations,
    folded: bool,
}

/// One block being visited: the root, or one iteration of a repeat body.
struct VisitFrame<'b, 'a> {
    items: std::slice::Iter<'b, FoldedDemItem<'a>>,
    state: DemTraversalState,
    /// The repeat in this block whose body is being visited above this frame.
    open: Option<OpenRepeat<'b, 'a>>,
}

/// Visits `root` from `state`.
pub(super) fn visit<V>(
    root: &FoldedDemBlock<'_>,
    visitor: &mut V,
    state: DemTraversalState,
) -> Result<ControlFlow<()>, V::Error>
where
    V: FoldedDemVisitor,
{
    let mut expansion = ExpansionBudget::default();
    let mut shifts = CoordinateShifts::default();
    let mut frames = vec![VisitFrame {
        items: root.items.iter(),
        state,
        open: None,
    }];
    loop {
        let frame = frames.last_mut().ok_or_else(lost_frame)?;
        match frame.items.next() {
            Some(FoldedDemItem::Instruction(instruction)) => {
                if visitor
                    .visit_instruction(instruction, &frame.state)?
                    .is_break()
                {
                    return Ok(ControlFlow::Break(()));
                }
                frame.state.apply_instruction(instruction)?;
            }
            Some(FoldedDemItem::Repeat { repeat, body }) => {
                let repeat_count = repeat.repeat_count().get();
                if repeat_count == 0 {
                    continue;
                }
                let selection = visitor.enter_repeat(repeat, body, &frame.state)?;
                let (iterations, folded) =
                    start_selection(selection, body, repeat_count, visitor, &mut expansion)?;
                frame.open = Some(OpenRepeat {
                    repeat,
                    body,
                    iterations,
                    folded,
                });
                if step_open_repeat(&mut frames, visitor, &mut shifts)?.is_break() {
                    return Ok(ControlFlow::Break(()));
                }
            }
            None => {
                if frames.len() == 1 {
                    return Ok(ControlFlow::Continue(()));
                }
                frames.pop();
                if step_open_repeat(&mut frames, visitor, &mut shifts)?.is_break() {
                    return Ok(ControlFlow::Break(()));
                }
            }
        }
    }
}

/// Checks a visitor's selection and returns the iterations it visits and whether they are
/// folded.
fn start_selection<V: FoldedDemVisitor>(
    selection: DemRepeatSelection,
    body: &FoldedDemBlock<'_>,
    repeat_count: u64,
    visitor: &mut V,
    expansion: &mut ExpansionBudget,
) -> Result<(RepeatIterations, bool), V::Error> {
    Ok(match selection {
        DemRepeatSelection::Skip => (RepeatIterations::Once { pending: false }, false),
        DemRepeatSelection::StructuralOnce => (RepeatIterations::Once { pending: true }, false),
        DemRepeatSelection::FoldOnce => {
            if body.summary().detector_shift()? != 0 {
                return Err(ModelError::invalid_detector_error_model(
                    "folded-once traversal requires a zero detector-shift repeat body",
                )
                .into());
            }
            (RepeatIterations::Once { pending: true }, true)
        }
        DemRepeatSelection::Expand {
            max_total_iterations,
            context,
        } => {
            expansion.used_iterations = expansion
                .used_iterations
                .checked_add(repeat_count)
                .ok_or_else(|| {
                    visitor.repeat_expansion_limit_error(context, u64::MAX, max_total_iterations)
                })?;
            if expansion.used_iterations > max_total_iterations {
                return Err(visitor.repeat_expansion_limit_error(
                    context,
                    expansion.used_iterations,
                    max_total_iterations,
                ));
            }
            (
                RepeatIterations::Expanded {
                    next: 0,
                    end: repeat_count,
                },
                false,
            )
        }
        DemRepeatSelection::Selected(iterations) => (
            RepeatIterations::Selected {
                iterations: iterations.into_iter(),
                previous: None,
            },
            false,
        ),
    })
}

/// Starts the next selected iteration of the top frame's open repeat, or, when none remains,
/// closes the repeat and advances the top frame past it.
fn step_open_repeat<V: FoldedDemVisitor>(
    frames: &mut Vec<VisitFrame<'_, '_>>,
    visitor: &mut V,
    shifts: &mut CoordinateShifts,
) -> Result<ControlFlow<()>, V::Error> {
    let parent = frames.last_mut().ok_or_else(lost_frame)?;
    let open = parent.open.as_mut().ok_or_else(|| {
        ModelError::invalid_detector_error_model("folded traversal lost its open repeat")
    })?;
    let repeat_count = open.repeat.repeat_count().get();
    if let Some(iteration) = open.iterations.next_iteration(repeat_count, visitor)? {
        let body = open.body;
        let state =
            parent
                .state
                .at_iteration(body, iteration, open.folded, repeat_count, shifts)?;
        frames.push(VisitFrame {
            items: body.items.iter(),
            state,
            open: None,
        });
        return Ok(ControlFlow::Continue(()));
    }
    let open = parent.open.take().ok_or_else(|| {
        ModelError::invalid_detector_error_model("folded traversal lost its open repeat")
    })?;
    if visitor
        .exit_repeat(open.repeat, open.body, &parent.state)?
        .is_break()
    {
        return Ok(ControlFlow::Break(()));
    }
    parent
        .state
        .advance_repeat(open.body, repeat_count, shifts)?;
    Ok(ControlFlow::Continue(()))
}

fn lost_frame() -> ModelError {
    ModelError::invalid_detector_error_model("folded traversal visit stack became empty")
}
