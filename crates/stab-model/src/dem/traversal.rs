//! Model-owned traversal over compact detector error models.
//!
//! This module is the advanced model boundary shared by DEM analysis and execution. It
//! builds a checked tree proportional to the compact model and leaves repeat handling to an
//! explicit visitor policy. Consumers can therefore skip, inspect, fold, selectively visit, or
//! boundedly expand repeats without maintaining independent recursion or silently materializing
//! the represented instruction stream.
//!
//! The boundary is deliberately advanced instead of stable top-level API. The current visitor
//! exposes execution-oriented tree views that may evolve as analysis and execution crates split.
//! Public callers should use operation-specific DEM APIs instead.

use std::ops::ControlFlow;

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DemTarget,
    DetectorErrorModel, MAX_DEM_REPEAT_NESTING,
};
use crate::{ModelError, ModelResult};

const MAX_DEM_COORDINATE_SCALAR_WORK: u64 = 8_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DemDetectorBounds {
    pub(super) min: u64,
    pub(super) max: u64,
}

impl DemDetectorBounds {
    fn include(&mut self, detector: u64) {
        self.min = self.min.min(detector);
        self.max = self.max.max(detector);
    }
}

/// Checked scalar facts about one compact DEM block.
///
/// Each fallible metric retains its own error so an overflow in an unrelated query does not make
/// the whole folded tree unusable. Coordinate vectors are not cached here; coordinate-aware
/// consumers opt into their separate bounded traversal state.
#[derive(Clone, Debug)]
pub struct DemBlockSummary {
    detector_shift: ModelResult<u64>,
    detector_count: ModelResult<u64>,
    observable_count: u64,
    error_count: ModelResult<u64>,
    detector_declaration_count: Option<u64>,
    detector_declaration_bounds: ModelResult<Option<DemDetectorBounds>>,
    max_repeat_depth: usize,
}

impl DemBlockSummary {
    pub fn detector_shift(&self) -> ModelResult<u64> {
        self.detector_shift.clone()
    }

    pub fn detector_count(&self) -> ModelResult<u64> {
        self.detector_count.clone()
    }

    pub const fn observable_count(&self) -> u64 {
        self.observable_count
    }

    pub fn error_count(&self) -> ModelResult<u64> {
        self.error_count.clone()
    }

    pub(super) const fn detector_declaration_count(&self) -> Option<u64> {
        self.detector_declaration_count
    }

    pub(super) fn detector_declaration_bounds(&self) -> ModelResult<Option<DemDetectorBounds>> {
        self.detector_declaration_bounds.clone()
    }

    pub const fn max_repeat_depth(&self) -> usize {
        self.max_repeat_depth
    }
}

/// Checked folded representation of a [`DetectorErrorModel`].
///
/// Construction creates one node per compact model item and performs work proportional to compact
/// structure. Repeat counts affect checked summaries, but do not duplicate body nodes. Analysis
/// and execution share this representation instead of implementing consumer-specific repeat
/// recursion.
#[derive(Debug)]
pub struct FoldedDemTraversal<'a> {
    root: FoldedDemBlock<'a>,
}

impl<'a> FoldedDemTraversal<'a> {
    /// Builds a folded tree without expanding repeat bodies.
    pub fn new(model: &'a DetectorErrorModel) -> ModelResult<Self> {
        let mut next_block_id = 0;
        Ok(Self {
            root: FoldedDemBlock::new(model, &mut next_block_id)?,
        })
    }

    /// Returns the compact root block and its checked summaries.
    pub const fn root(&self) -> &FoldedDemBlock<'a> {
        &self.root
    }

    /// Traverses the model with coordinate-free state.
    ///
    /// Visitor errors and [`ControlFlow::Break`] propagate immediately. Repeat expansion occurs
    /// only when the visitor explicitly returns [`DemRepeatSelection::Expand`], whose cumulative
    /// iteration limit is enforced by the traversal.
    pub fn try_visit<V>(&self, visitor: &mut V) -> Result<ControlFlow<()>, V::Error>
    where
        V: FoldedDemVisitor,
    {
        let mut state = DemTraversalState::default();
        let mut expansion = ExpansionBudget::default();
        self.root.visit(visitor, &mut state, &mut expansion)
    }

    /// Traverses the model with opt-in coordinate-shift state.
    ///
    /// Coordinate updates have a separate aggregate scalar-work limit. Consumers that do not need
    /// coordinates must use [`Self::try_visit`] so wide coordinate annotations cannot inflate
    /// their retained state or work.
    pub fn try_visit_with_coordinates<V>(
        &self,
        visitor: &mut V,
    ) -> Result<ControlFlow<()>, V::Error>
    where
        V: FoldedDemVisitor,
    {
        let mut state = DemTraversalState::with_coordinates();
        let mut expansion = ExpansionBudget::default();
        self.root.visit(visitor, &mut state, &mut expansion)
    }

    /// Applies a consumer-owned repeat-depth admission limit.
    ///
    /// Depth validation is not part of construction because compact model queries and consumers
    /// historically have different accepted depth contracts.
    pub fn validate_repeat_depth(&self, context: &'static str) -> ModelResult<()> {
        let depth = self.root.summary().max_repeat_depth();
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(ModelError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}, got {depth}"
            )));
        }
        Ok(())
    }
}

/// One compact block in a folded traversal tree.
///
/// Blocks borrow instructions and repeat declarations from the source model. Their child vectors
/// mirror compact syntax and never contain one entry per represented repeat iteration.
#[derive(Debug)]
pub struct FoldedDemBlock<'a> {
    compact_id: usize,
    items: Vec<FoldedDemItem<'a>>,
    summary: DemBlockSummary,
}

impl<'a> FoldedDemBlock<'a> {
    fn new(model: &'a DetectorErrorModel, next_block_id: &mut usize) -> ModelResult<Self> {
        let root_id = take_next_block_id(next_block_id)?;
        let mut stack = vec![FoldedDemBuildFrame::new(model, root_id, None)];
        loop {
            let action = {
                let frame = stack.last_mut().ok_or_else(|| {
                    ModelError::invalid_detector_error_model(
                        "DEM folded traversal build stack became empty",
                    )
                })?;
                match frame.model.items().get(frame.next_item) {
                    Some(DemItem::Instruction(instruction)) => {
                        frame.next_item = frame.next_item.saturating_add(1);
                        FoldedDemBuildAction::Instruction(instruction)
                    }
                    Some(DemItem::RepeatBlock(repeat)) => {
                        frame.next_item = frame.next_item.saturating_add(1);
                        FoldedDemBuildAction::Repeat(repeat)
                    }
                    None => FoldedDemBuildAction::Finish,
                }
            };
            match action {
                FoldedDemBuildAction::Instruction(instruction) => {
                    let frame = stack.last_mut().ok_or_else(|| {
                        ModelError::invalid_detector_error_model(
                            "DEM folded traversal lost its instruction parent",
                        )
                    })?;
                    frame.items.push(FoldedDemItem::Instruction(instruction));
                }
                FoldedDemBuildAction::Repeat(repeat) => {
                    let compact_id = take_next_block_id(next_block_id)?;
                    stack.push(FoldedDemBuildFrame::new(
                        repeat.body(),
                        compact_id,
                        Some(repeat),
                    ));
                }
                FoldedDemBuildAction::Finish => {
                    let frame = stack.pop().ok_or_else(|| {
                        ModelError::invalid_detector_error_model(
                            "DEM folded traversal lost its completed block",
                        )
                    })?;
                    let parent_repeat = frame.parent_repeat;
                    let block = Self {
                        compact_id: frame.compact_id,
                        summary: summarize(&frame.items),
                        items: frame.items,
                    };
                    let Some(parent) = stack.last_mut() else {
                        return Ok(block);
                    };
                    let repeat = parent_repeat.ok_or_else(|| {
                        ModelError::invalid_detector_error_model(
                            "DEM folded traversal child has no repeat declaration",
                        )
                    })?;
                    parent.items.push(FoldedDemItem::Repeat {
                        repeat,
                        body: Box::new(block),
                    });
                }
            }
        }
    }

    /// Returns the traversal-local identity of this compact block.
    ///
    /// IDs are assigned in deterministic pre-order and remain stable for the lifetime of the
    /// folded tree. They identify compact structure only and are not persistent model IDs.
    pub const fn compact_id(&self) -> usize {
        self.compact_id
    }

    /// Returns compact child items in source order.
    pub fn items(&self) -> &[FoldedDemItem<'a>] {
        &self.items
    }

    /// Returns checked scalar facts for this compact block.
    pub const fn summary(&self) -> &DemBlockSummary {
        &self.summary
    }

    pub(super) fn coordinate_shift(&self) -> ModelResult<Vec<f64>> {
        let mut budget = ExpansionBudget::default();
        self.coordinate_shift_with_budget(&mut budget)
    }

    fn coordinate_shift_with_budget(&self, budget: &mut ExpansionBudget) -> ModelResult<Vec<f64>> {
        let mut shift = Vec::new();
        for item in &self.items {
            match item {
                FoldedDemItem::Instruction(instruction)
                    if instruction.kind() == DemInstructionKind::ShiftDetectors =>
                {
                    budget.add_coordinate_scalars(instruction.args().len())?;
                    add_coordinate_shift_mul(&mut shift, instruction.args(), 1.0)?;
                }
                FoldedDemItem::Repeat { repeat, body } => {
                    let body_shift = body.coordinate_shift_with_budget(budget)?;
                    budget.add_coordinate_scalars(body_shift.len())?;
                    add_coordinate_shift_mul(
                        &mut shift,
                        &body_shift,
                        repeat.repeat_count().get() as f64,
                    )?;
                }
                FoldedDemItem::Instruction(_) => {}
            }
        }
        Ok(shift)
    }

    fn visit<V>(
        &self,
        visitor: &mut V,
        state: &mut DemTraversalState,
        expansion: &mut ExpansionBudget,
    ) -> Result<ControlFlow<()>, V::Error>
    where
        V: FoldedDemVisitor,
    {
        for item in &self.items {
            match item {
                FoldedDemItem::Instruction(instruction) => {
                    if visitor.visit_instruction(instruction, state)?.is_break() {
                        return Ok(ControlFlow::Break(()));
                    }
                    state.apply_instruction(instruction, expansion)?;
                }
                FoldedDemItem::Repeat { repeat, body } => {
                    if repeat.repeat_count().get() == 0 {
                        continue;
                    }
                    let selection = visitor.enter_repeat(repeat, body, state)?;
                    if visit_repeat_selection(repeat, body, selection, visitor, state, expansion)?
                        .is_break()
                    {
                        return Ok(ControlFlow::Break(()));
                    }
                    if visitor.exit_repeat(repeat, body, state)?.is_break() {
                        return Ok(ControlFlow::Break(()));
                    }
                    state.advance_repeat(body, repeat.repeat_count().get(), expansion)?;
                }
            }
        }
        Ok(ControlFlow::Continue(()))
    }
}

impl Drop for FoldedDemBlock<'_> {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        take_child_blocks(&mut self.items, &mut pending);
        while let Some(mut block) = pending.pop() {
            take_child_blocks(&mut block.items, &mut pending);
        }
    }
}

struct FoldedDemBuildFrame<'a> {
    model: &'a DetectorErrorModel,
    compact_id: usize,
    next_item: usize,
    items: Vec<FoldedDemItem<'a>>,
    parent_repeat: Option<&'a DemRepeatBlock>,
}

impl<'a> FoldedDemBuildFrame<'a> {
    fn new(
        model: &'a DetectorErrorModel,
        compact_id: usize,
        parent_repeat: Option<&'a DemRepeatBlock>,
    ) -> Self {
        Self {
            model,
            compact_id,
            next_item: 0,
            items: Vec::with_capacity(model.items().len()),
            parent_repeat,
        }
    }
}

enum FoldedDemBuildAction<'a> {
    Instruction(&'a DemInstruction),
    Repeat(&'a DemRepeatBlock),
    Finish,
}

fn take_next_block_id(next_block_id: &mut usize) -> ModelResult<usize> {
    let compact_id = *next_block_id;
    *next_block_id = (*next_block_id).checked_add(1).ok_or_else(|| {
        ModelError::invalid_detector_error_model("DEM compact folded-block identifier overflowed")
    })?;
    Ok(compact_id)
}

fn take_child_blocks<'a>(
    items: &mut Vec<FoldedDemItem<'a>>,
    pending: &mut Vec<Box<FoldedDemBlock<'a>>>,
) {
    for item in std::mem::take(items) {
        if let FoldedDemItem::Repeat { body, .. } = item {
            pending.push(body);
        }
    }
}

/// Borrowed compact item exposed to advanced analysis and execution consumers.
#[derive(Debug)]
pub enum FoldedDemItem<'a> {
    Instruction(&'a DemInstruction),
    Repeat {
        repeat: &'a DemRepeatBlock,
        body: Box<FoldedDemBlock<'a>>,
    },
}

/// Semantic position supplied to folded traversal callbacks.
///
/// The state describes the position immediately before the current instruction or repeat. Detector
/// shifts are applied after an instruction callback continues. Folded depth and multiplicity are
/// nontrivial only inside bodies selected with [`DemRepeatSelection::FoldOnce`].
#[derive(Clone, Debug)]
pub struct DemTraversalState {
    detector_offset: u64,
    coordinate_shift: Option<Vec<f64>>,
    folded_repeat_depth: usize,
    folded_repeat_multiplicity: u64,
}

impl Default for DemTraversalState {
    fn default() -> Self {
        Self {
            detector_offset: 0,
            coordinate_shift: None,
            folded_repeat_depth: 0,
            folded_repeat_multiplicity: 1,
        }
    }
}

impl DemTraversalState {
    fn with_coordinates() -> Self {
        Self {
            coordinate_shift: Some(Vec::new()),
            ..Self::default()
        }
    }

    /// Returns the absolute detector offset at the current compact position.
    pub const fn detector_offset(&self) -> u64 {
        self.detector_offset
    }

    pub fn coordinate_shift(&self) -> ModelResult<&[f64]> {
        self.coordinate_shift.as_deref().ok_or_else(|| {
            ModelError::invalid_detector_error_model(
                "folded traversal coordinate state was not requested",
            )
        })
    }

    /// Returns the number of enclosing repeats represented through one folded body visit.
    pub const fn folded_repeat_depth(&self) -> usize {
        self.folded_repeat_depth
    }

    pub const fn folded_repeat_multiplicity(&self) -> u64 {
        self.folded_repeat_multiplicity
    }

    fn apply_instruction(
        &mut self,
        instruction: &DemInstruction,
        expansion: &mut ExpansionBudget,
    ) -> ModelResult<()> {
        if instruction.kind() == DemInstructionKind::ShiftDetectors {
            self.detector_offset = self
                .detector_offset
                .checked_add(instruction.detector_shift()?)
                .ok_or_else(|| {
                    ModelError::invalid_detector_error_model(
                        "folded traversal detector offset overflowed",
                    )
                })?;
            if let Some(coordinate_shift) = &mut self.coordinate_shift {
                expansion.add_coordinate_scalars(instruction.args().len())?;
                add_coordinate_shift_mul(coordinate_shift, instruction.args(), 1.0)?;
            }
        }
        Ok(())
    }

    fn at_iteration(
        &self,
        body: &FoldedDemBlock<'_>,
        iteration: u64,
        folded: bool,
        repeat_count: u64,
        expansion: &mut ExpansionBudget,
    ) -> ModelResult<Self> {
        let detector_offset = body
            .summary()
            .detector_shift()?
            .checked_mul(iteration)
            .and_then(|shift| self.detector_offset.checked_add(shift))
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model(
                    "folded traversal repeat detector offset overflowed",
                )
            })?;
        let mut coordinate_shift = self.coordinate_shift.clone();
        if let Some(coordinate_shift) = &mut coordinate_shift {
            let body_shift = body.coordinate_shift_with_budget(expansion)?;
            expansion.add_coordinate_scalars(body_shift.len())?;
            add_coordinate_shift_mul(coordinate_shift, &body_shift, iteration as f64)?;
        }
        let folded_repeat_multiplicity = if folded {
            self.folded_repeat_multiplicity
                .checked_mul(repeat_count)
                .ok_or_else(|| {
                    ModelError::invalid_detector_error_model(
                        "folded traversal repeat multiplicity overflowed",
                    )
                })?
        } else {
            self.folded_repeat_multiplicity
        };
        Ok(Self {
            detector_offset,
            coordinate_shift,
            folded_repeat_depth: if folded {
                self.folded_repeat_depth.checked_add(1).ok_or_else(|| {
                    ModelError::invalid_detector_error_model(
                        "folded traversal folded-repeat depth overflowed",
                    )
                })?
            } else {
                self.folded_repeat_depth
            },
            folded_repeat_multiplicity,
        })
    }

    fn advance_repeat(
        &mut self,
        body: &FoldedDemBlock<'_>,
        repeat_count: u64,
        expansion: &mut ExpansionBudget,
    ) -> ModelResult<()> {
        self.detector_offset = body
            .summary()
            .detector_shift()?
            .checked_mul(repeat_count)
            .and_then(|shift| self.detector_offset.checked_add(shift))
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model(
                    "folded traversal repeat detector offset overflowed",
                )
            })?;
        if let Some(coordinate_shift) = &mut self.coordinate_shift {
            let body_shift = body.coordinate_shift_with_budget(expansion)?;
            expansion.add_coordinate_scalars(body_shift.len())?;
            add_coordinate_shift_mul(coordinate_shift, &body_shift, repeat_count as f64)?;
        }
        Ok(())
    }
}

/// Policy chosen by a visitor when entering a nonempty repeat block.
///
/// Every selection controls body callbacks only. When the selected body visits and
/// [`FoldedDemVisitor::exit_repeat`] complete with [`ControlFlow::Continue`], traversal advances
/// outer detector and coordinate state by the repeat's full semantic effect. An error or
/// [`ControlFlow::Break`] stops before that internal state update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DemRepeatSelection {
    /// Do not visit the body.
    Skip,
    /// Visit the first structural body once without marking it as folded.
    StructuralOnce,
    /// Visit a zero-detector-shift body once with folded depth and multiplicity in the state.
    FoldOnce,
    /// Visit every represented iteration under a cumulative traversal-owned limit.
    Expand {
        /// Maximum total expanded iterations across all `Expand` selections in this traversal.
        max_total_iterations: u64,
        /// Consumer-readable operation context used by the default validation error.
        context: &'static str,
    },
    /// Visit selected iteration indexes in the supplied order.
    ///
    /// The traversal rejects indexes that are out of range or not strictly increasing. The visitor
    /// owns any admission policy for constructing this selection.
    Selected(Vec<u64>),
}

/// Advanced policy interface for checked folded DEM traversal.
///
/// Callbacks borrow model-owned values and may stop immediately with an error or
/// [`ControlFlow::Break`]. `visit_instruction` observes pre-instruction state. `enter_repeat` and
/// `exit_repeat` observe the outer state; zero-count repeats produce neither callback. Consumers
/// choose repeat handling explicitly through [`DemRepeatSelection`] and must preserve their own
/// output-size or algorithmic-work limits in addition to traversal-owned arithmetic checks.
///
/// This trait is an advanced boundary by design. It is shared by analysis and execution
/// implementations, but it is not a stable downstream cursor API.
pub trait FoldedDemVisitor {
    type Error: From<ModelError>;

    /// Visits one instruction with semantic state immediately before that instruction.
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> Result<ControlFlow<()>, Self::Error>;

    /// Chooses how a nonempty repeat body is visited.
    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> Result<DemRepeatSelection, Self::Error> {
        Ok(DemRepeatSelection::StructuralOnce)
    }

    /// Observes completion of the selected body visits before outer state advances.
    fn exit_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> Result<ControlFlow<()>, Self::Error> {
        Ok(ControlFlow::Continue(()))
    }

    /// Constructs the consumer-owned failure for cumulative repeat expansion.
    fn repeat_expansion_limit_error(
        &mut self,
        context: &'static str,
        actual: u64,
        limit: u64,
    ) -> Self::Error {
        ModelError::invalid_detector_error_model(format!(
            "DEM {context} traversal currently supports at most {limit} expanded repeat iterations, got at least {actual}"
        ))
        .into()
    }
}

#[derive(Default)]
struct ExpansionBudget {
    used_iterations: u64,
    coordinate_scalars: u64,
}

impl ExpansionBudget {
    fn add_coordinate_scalars(&mut self, count: usize) -> ModelResult<()> {
        let count = u64::try_from(count).map_err(|_| {
            ModelError::invalid_detector_error_model("DEM coordinate scalar work does not fit u64")
        })?;
        self.coordinate_scalars = self.coordinate_scalars.checked_add(count).ok_or_else(|| {
            ModelError::invalid_detector_error_model("DEM coordinate scalar work overflowed")
        })?;
        if self.coordinate_scalars > MAX_DEM_COORDINATE_SCALAR_WORK {
            return Err(ModelError::invalid_detector_error_model(format!(
                "DEM coordinate traversal currently supports at most {MAX_DEM_COORDINATE_SCALAR_WORK} coordinate scalar updates, got at least {}",
                self.coordinate_scalars
            )));
        }
        Ok(())
    }
}

fn visit_repeat_selection<V>(
    repeat: &DemRepeatBlock,
    body: &FoldedDemBlock<'_>,
    selection: DemRepeatSelection,
    visitor: &mut V,
    state: &DemTraversalState,
    expansion: &mut ExpansionBudget,
) -> Result<ControlFlow<()>, V::Error>
where
    V: FoldedDemVisitor,
{
    let repeat_count = repeat.repeat_count().get();
    match selection {
        DemRepeatSelection::Skip => Ok(ControlFlow::Continue(())),
        DemRepeatSelection::StructuralOnce => {
            let mut iteration_state =
                state.at_iteration(body, 0, false, repeat_count, expansion)?;
            body.visit(visitor, &mut iteration_state, expansion)
        }
        DemRepeatSelection::FoldOnce => {
            if body.summary().detector_shift()? != 0 {
                return Err(ModelError::invalid_detector_error_model(
                    "folded-once traversal requires a zero detector-shift repeat body",
                )
                .into());
            }
            let mut iteration_state = state.at_iteration(body, 0, true, repeat_count, expansion)?;
            body.visit(visitor, &mut iteration_state, expansion)
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
            for iteration in 0..repeat_count {
                let mut iteration_state =
                    state.at_iteration(body, iteration, false, repeat_count, expansion)?;
                if body
                    .visit(visitor, &mut iteration_state, expansion)?
                    .is_break()
                {
                    return Ok(ControlFlow::Break(()));
                }
            }
            Ok(ControlFlow::Continue(()))
        }
        DemRepeatSelection::Selected(iterations) => {
            let mut previous = None;
            for iteration in iterations {
                if iteration >= repeat_count || previous.is_some_and(|value| value >= iteration) {
                    return Err(ModelError::invalid_detector_error_model(
                        "folded traversal selected repeat iterations must be strictly increasing and in range",
                    )
                    .into());
                }
                previous = Some(iteration);
                let mut iteration_state =
                    state.at_iteration(body, iteration, false, repeat_count, expansion)?;
                if body
                    .visit(visitor, &mut iteration_state, expansion)?
                    .is_break()
                {
                    return Ok(ControlFlow::Break(()));
                }
            }
            Ok(ControlFlow::Continue(()))
        }
    }
}

fn summarize(items: &[FoldedDemItem<'_>]) -> DemBlockSummary {
    DemBlockSummary {
        detector_shift: summarize_detector_shift(items),
        detector_count: summarize_detector_count(items),
        observable_count: summarize_observable_count(items),
        error_count: summarize_error_count(items),
        detector_declaration_count: summarize_detector_declaration_count(items),
        detector_declaration_bounds: summarize_detector_declaration_bounds(items),
        max_repeat_depth: summarize_max_repeat_depth(items),
    }
}

fn summarize_detector_shift(items: &[FoldedDemItem<'_>]) -> ModelResult<u64> {
    let mut shift = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction)
                if instruction.kind() == DemInstructionKind::ShiftDetectors =>
            {
                shift = shift
                    .checked_add(instruction.detector_shift()?)
                    .ok_or_else(|| detector_summary_error("detector shift overflowed"))?;
            }
            FoldedDemItem::Repeat { repeat, body } => {
                shift = body
                    .summary()
                    .detector_shift()?
                    .checked_mul(repeat.repeat_count().get())
                    .and_then(|repeated| shift.checked_add(repeated))
                    .ok_or_else(|| detector_summary_error("repeat detector shift overflowed"))?;
            }
            FoldedDemItem::Instruction(_) => {}
        }
    }
    Ok(shift)
}

fn summarize_detector_count(items: &[FoldedDemItem<'_>]) -> ModelResult<u64> {
    let mut offset = 0_u64;
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                for target in instruction.targets() {
                    if let DemTarget::RelativeDetector(detector) = target {
                        count = count.max(
                            offset
                                .checked_add(detector.get())
                                .and_then(|value| value.checked_add(1))
                                .ok_or_else(|| {
                                    detector_summary_error("detector count overflowed")
                                })?,
                        );
                    }
                }
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    offset = offset
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| detector_summary_error("detector shift overflowed"))?;
                }
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let repeat_count = repeat.repeat_count().get();
                let body_shift = body.summary().detector_shift()?;
                let body_count = body.summary().detector_count()?;
                if repeat_count > 0 && body_count > 0 {
                    count = count.max(
                        body_shift
                            .checked_mul(repeat_count.saturating_sub(1))
                            .and_then(|shift| offset.checked_add(shift))
                            .and_then(|start| start.checked_add(body_count))
                            .ok_or_else(|| {
                                detector_summary_error("repeat detector count overflowed")
                            })?,
                    );
                }
                offset = body_shift
                    .checked_mul(repeat_count)
                    .and_then(|shift| offset.checked_add(shift))
                    .ok_or_else(|| detector_summary_error("repeat detector shift overflowed"))?;
            }
        }
    }
    Ok(count)
}

fn summarize_observable_count(items: &[FoldedDemItem<'_>]) -> u64 {
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                for target in instruction.targets() {
                    if let DemTarget::LogicalObservable(observable) = target {
                        count = count.max(observable.get().saturating_add(1));
                    }
                }
            }
            FoldedDemItem::Repeat { body, .. } => {
                count = count.max(body.summary().observable_count());
            }
        }
    }
    count
}

fn summarize_error_count(items: &[FoldedDemItem<'_>]) -> ModelResult<u64> {
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction)
                if instruction.kind() == DemInstructionKind::Error =>
            {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| detector_summary_error("error count overflowed"))?;
            }
            FoldedDemItem::Repeat { repeat, body } => {
                count = body
                    .summary()
                    .error_count()?
                    .checked_mul(repeat.repeat_count().get())
                    .and_then(|repeated| count.checked_add(repeated))
                    .ok_or_else(|| detector_summary_error("repeat error count overflowed"))?;
            }
            FoldedDemItem::Instruction(_) => {}
        }
    }
    Ok(count)
}

fn summarize_detector_declaration_count(items: &[FoldedDemItem<'_>]) -> Option<u64> {
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction)
                if instruction.kind() == DemInstructionKind::Detector =>
            {
                let targets = instruction
                    .targets()
                    .iter()
                    .filter(|target| matches!(target, DemTarget::RelativeDetector(_)))
                    .count();
                let targets = u64::try_from(targets).ok()?;
                count = count.checked_add(targets)?;
            }
            FoldedDemItem::Repeat { repeat, body } => {
                count = body
                    .summary()
                    .detector_declaration_count()?
                    .checked_mul(repeat.repeat_count().get())
                    .and_then(|repeated| count.checked_add(repeated))?;
            }
            FoldedDemItem::Instruction(_) => {}
        }
    }
    Some(count)
}

fn summarize_detector_declaration_bounds(
    items: &[FoldedDemItem<'_>],
) -> ModelResult<Option<DemDetectorBounds>> {
    let mut offset = 0_u64;
    let mut bounds = None;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction) => {
                if instruction.kind() == DemInstructionKind::Detector {
                    for target in instruction.targets() {
                        if let DemTarget::RelativeDetector(detector) = target {
                            include_bound(
                                &mut bounds,
                                offset.checked_add(detector.get()).ok_or_else(|| {
                                    detector_summary_error("detector declaration id overflowed")
                                })?,
                            );
                        }
                    }
                }
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    offset = offset
                        .checked_add(instruction.detector_shift()?)
                        .ok_or_else(|| detector_summary_error("detector shift overflowed"))?;
                }
            }
            FoldedDemItem::Repeat { repeat, body } => {
                let repeat_count = repeat.repeat_count().get();
                let body_shift = body.summary().detector_shift()?;
                if repeat_count > 0
                    && let Some(body_bounds) = body.summary().detector_declaration_bounds()?
                {
                    let last_offset = body_shift
                        .checked_mul(repeat_count.saturating_sub(1))
                        .and_then(|shift| offset.checked_add(shift))
                        .ok_or_else(|| {
                            detector_summary_error("repeat detector declaration shift overflowed")
                        })?;
                    for detector in [
                        offset.checked_add(body_bounds.min),
                        offset.checked_add(body_bounds.max),
                        last_offset.checked_add(body_bounds.min),
                        last_offset.checked_add(body_bounds.max),
                    ] {
                        include_bound(
                            &mut bounds,
                            detector.ok_or_else(|| {
                                detector_summary_error("repeat detector declaration id overflowed")
                            })?,
                        );
                    }
                }
                offset = body_shift
                    .checked_mul(repeat_count)
                    .and_then(|shift| offset.checked_add(shift))
                    .ok_or_else(|| detector_summary_error("repeat detector shift overflowed"))?;
            }
        }
    }
    Ok(bounds)
}

fn summarize_max_repeat_depth(items: &[FoldedDemItem<'_>]) -> usize {
    items
        .iter()
        .filter_map(|item| match item {
            FoldedDemItem::Repeat { body, .. } => {
                Some(body.summary().max_repeat_depth().saturating_add(1))
            }
            FoldedDemItem::Instruction(_) => None,
        })
        .max()
        .unwrap_or(0)
}

fn include_bound(bounds: &mut Option<DemDetectorBounds>, detector: u64) {
    match bounds {
        Some(bounds) => bounds.include(detector),
        None => {
            *bounds = Some(DemDetectorBounds {
                min: detector,
                max: detector,
            });
        }
    }
}

pub fn shifted_detector(
    detector: DemDetectorId,
    detector_offset: u64,
) -> ModelResult<DemDetectorId> {
    DemDetectorId::try_new(
        detector
            .get()
            .checked_add(detector_offset)
            .ok_or_else(|| detector_summary_error("relative detector id overflowed"))?,
    )
}

pub fn shifted_targets(targets: &[DemTarget], detector_offset: u64) -> ModelResult<Vec<DemTarget>> {
    let mut shifted = Vec::new();
    shifted
        .try_reserve_exact(targets.len())
        .map_err(|_| detector_summary_error("target allocation failed"))?;
    for target in targets {
        let target = match *target {
            DemTarget::RelativeDetector(detector) => {
                DemTarget::RelativeDetector(shifted_detector(detector, detector_offset)?)
            }
            DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                *target
            }
        };
        shifted.push(target);
    }
    Ok(shifted)
}

pub fn shifted_coordinates(coordinates: &[f64], coordinate_shift: &[f64]) -> ModelResult<Vec<f64>> {
    let mut shifted = Vec::new();
    shifted
        .try_reserve_exact(coordinates.len())
        .map_err(|_| detector_summary_error("coordinate allocation failed"))?;
    shifted.extend_from_slice(coordinates);
    for (index, coordinate) in shifted.iter_mut().enumerate() {
        if let Some(delta) = coordinate_shift.get(index) {
            *coordinate += delta;
            if !coordinate.is_finite() {
                return Err(detector_summary_error("detector coordinate overflowed"));
            }
        }
    }
    Ok(shifted)
}

pub(super) fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> ModelResult<()> {
    if shift.len() < delta.len() {
        shift
            .try_reserve(delta.len() - shift.len())
            .map_err(|_| detector_summary_error("coordinate shift allocation failed"))?;
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift
            .get_mut(index)
            .ok_or_else(|| detector_summary_error("coordinate shift dimension is missing"))?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(detector_summary_error("coordinate shift overflowed"));
        }
    }
    Ok(())
}

fn detector_summary_error(message: &'static str) -> ModelError {
    ModelError::invalid_detector_error_model(message)
}
