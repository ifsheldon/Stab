use std::ops::ControlFlow;

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DemTarget,
    DetectorErrorModel, MAX_DEM_REPEAT_NESTING,
};
use crate::{CircuitError, CircuitResult};

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

#[derive(Clone, Debug)]
pub(crate) struct DemBlockSummary {
    detector_shift: CircuitResult<u64>,
    detector_count: CircuitResult<u64>,
    observable_count: u64,
    error_count: CircuitResult<u64>,
    detector_declaration_count: Option<u64>,
    detector_declaration_bounds: CircuitResult<Option<DemDetectorBounds>>,
    compact_search_error_count: CircuitResult<Option<u64>>,
    compact_filter_error_count: CircuitResult<Option<u64>>,
    has_nonzero_probability_error: bool,
    max_repeat_depth: usize,
}

impl DemBlockSummary {
    pub(crate) fn detector_shift(&self) -> CircuitResult<u64> {
        self.detector_shift.clone()
    }

    pub(crate) fn detector_count(&self) -> CircuitResult<u64> {
        self.detector_count.clone()
    }

    pub(crate) const fn observable_count(&self) -> u64 {
        self.observable_count
    }

    pub(crate) fn error_count(&self) -> CircuitResult<u64> {
        self.error_count.clone()
    }

    pub(super) const fn detector_declaration_count(&self) -> Option<u64> {
        self.detector_declaration_count
    }

    pub(super) fn detector_declaration_bounds(&self) -> CircuitResult<Option<DemDetectorBounds>> {
        self.detector_declaration_bounds.clone()
    }

    pub(super) fn compact_search_error_count(&self) -> CircuitResult<Option<u64>> {
        self.compact_search_error_count.clone()
    }

    pub(crate) fn compact_filter_error_count(&self) -> CircuitResult<Option<u64>> {
        self.compact_filter_error_count.clone()
    }

    pub(super) const fn has_nonzero_probability_error(&self) -> bool {
        self.has_nonzero_probability_error
    }

    pub(crate) const fn max_repeat_depth(&self) -> usize {
        self.max_repeat_depth
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FoldedDemTraversal<'a> {
    root: FoldedDemBlock<'a>,
}

impl<'a> FoldedDemTraversal<'a> {
    pub(crate) fn new(model: &'a DetectorErrorModel) -> CircuitResult<Self> {
        Ok(Self {
            root: FoldedDemBlock::new(model)?,
        })
    }

    pub(crate) const fn root(&self) -> &FoldedDemBlock<'a> {
        &self.root
    }

    pub(crate) fn try_visit<V>(&self, visitor: &mut V) -> CircuitResult<ControlFlow<()>>
    where
        V: FoldedDemVisitor,
    {
        let mut state = DemTraversalState::default();
        let mut expansion = ExpansionBudget::default();
        self.root.visit(visitor, &mut state, &mut expansion)
    }

    pub(super) fn try_visit_with_coordinates<V>(
        &self,
        visitor: &mut V,
    ) -> CircuitResult<ControlFlow<()>>
    where
        V: FoldedDemVisitor,
    {
        let mut state = DemTraversalState::with_coordinates();
        let mut expansion = ExpansionBudget::default();
        self.root.visit(visitor, &mut state, &mut expansion)
    }

    pub(crate) fn validate_repeat_depth(&self, context: &'static str) -> CircuitResult<()> {
        let depth = self.root.summary().max_repeat_depth();
        if depth > MAX_DEM_REPEAT_NESTING {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM {context} repeat nesting exceeds current limit {MAX_DEM_REPEAT_NESTING}, got {depth}"
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FoldedDemBlock<'a> {
    items: Vec<FoldedDemItem<'a>>,
    summary: DemBlockSummary,
}

impl<'a> FoldedDemBlock<'a> {
    fn new(model: &'a DetectorErrorModel) -> CircuitResult<Self> {
        let mut items = Vec::with_capacity(model.items().len());
        for item in model.items() {
            items.push(match item {
                DemItem::Instruction(instruction) => FoldedDemItem::Instruction(instruction),
                DemItem::RepeatBlock(repeat) => FoldedDemItem::Repeat {
                    repeat,
                    body: Box::new(Self::new(repeat.body())?),
                },
            });
        }
        let summary = summarize(&items);
        Ok(Self { items, summary })
    }

    pub(crate) fn items(&self) -> &[FoldedDemItem<'a>] {
        &self.items
    }

    pub(crate) const fn summary(&self) -> &DemBlockSummary {
        &self.summary
    }

    pub(super) fn coordinate_shift(&self) -> CircuitResult<Vec<f64>> {
        let mut budget = ExpansionBudget::default();
        self.coordinate_shift_with_budget(&mut budget)
    }

    fn coordinate_shift_with_budget(
        &self,
        budget: &mut ExpansionBudget,
    ) -> CircuitResult<Vec<f64>> {
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
    ) -> CircuitResult<ControlFlow<()>>
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

#[derive(Clone, Debug)]
pub(crate) enum FoldedDemItem<'a> {
    Instruction(&'a DemInstruction),
    Repeat {
        repeat: &'a DemRepeatBlock,
        body: Box<FoldedDemBlock<'a>>,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct DemTraversalState {
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

    pub(crate) const fn detector_offset(&self) -> u64 {
        self.detector_offset
    }

    pub(super) fn coordinate_shift(&self) -> CircuitResult<&[f64]> {
        self.coordinate_shift.as_deref().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "folded traversal coordinate state was not requested",
            )
        })
    }

    pub(crate) const fn folded_repeat_depth(&self) -> usize {
        self.folded_repeat_depth
    }

    pub(super) const fn folded_repeat_multiplicity(&self) -> u64 {
        self.folded_repeat_multiplicity
    }

    fn apply_instruction(
        &mut self,
        instruction: &DemInstruction,
        expansion: &mut ExpansionBudget,
    ) -> CircuitResult<()> {
        if instruction.kind() == DemInstructionKind::ShiftDetectors {
            self.detector_offset = self
                .detector_offset
                .checked_add(instruction.detector_shift()?)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model(
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
    ) -> CircuitResult<Self> {
        let detector_offset = body
            .summary()
            .detector_shift()?
            .checked_mul(iteration)
            .and_then(|shift| self.detector_offset.checked_add(shift))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
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
                    CircuitError::invalid_detector_error_model(
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
                    CircuitError::invalid_detector_error_model(
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
    ) -> CircuitResult<()> {
        self.detector_offset = body
            .summary()
            .detector_shift()?
            .checked_mul(repeat_count)
            .and_then(|shift| self.detector_offset.checked_add(shift))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DemRepeatSelection {
    Skip,
    StructuralOnce,
    FoldOnce,
    Expand {
        max_total_iterations: u64,
        context: &'static str,
    },
    Selected(Vec<u64>),
}

pub(crate) trait FoldedDemVisitor {
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>>;

    fn enter_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> CircuitResult<DemRepeatSelection> {
        Ok(DemRepeatSelection::StructuralOnce)
    }

    fn exit_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>> {
        Ok(ControlFlow::Continue(()))
    }
}

#[derive(Default)]
struct ExpansionBudget {
    used_iterations: u64,
    coordinate_scalars: u64,
}

impl ExpansionBudget {
    fn add_coordinate_scalars(&mut self, count: usize) -> CircuitResult<()> {
        let count = u64::try_from(count).map_err(|_| {
            CircuitError::invalid_detector_error_model(
                "DEM coordinate scalar work does not fit u64",
            )
        })?;
        self.coordinate_scalars = self.coordinate_scalars.checked_add(count).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("DEM coordinate scalar work overflowed")
        })?;
        if self.coordinate_scalars > MAX_DEM_COORDINATE_SCALAR_WORK {
            return Err(CircuitError::invalid_detector_error_model(format!(
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
) -> CircuitResult<ControlFlow<()>>
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
                return Err(CircuitError::invalid_detector_error_model(
                    "folded-once traversal requires a zero detector-shift repeat body",
                ));
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
                .ok_or_else(|| expansion_error(context, max_total_iterations, u64::MAX))?;
            if expansion.used_iterations > max_total_iterations {
                return Err(expansion_error(
                    context,
                    max_total_iterations,
                    expansion.used_iterations,
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
                    return Err(CircuitError::invalid_detector_error_model(
                        "folded traversal selected repeat iterations must be strictly increasing and in range",
                    ));
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

fn expansion_error(context: &'static str, limit: u64, actual: u64) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!(
        "DEM {context} traversal currently supports at most {limit} expanded repeat iterations, got at least {actual}"
    ))
}

fn summarize(items: &[FoldedDemItem<'_>]) -> DemBlockSummary {
    DemBlockSummary {
        detector_shift: summarize_detector_shift(items),
        detector_count: summarize_detector_count(items),
        observable_count: summarize_observable_count(items),
        error_count: summarize_error_count(items),
        detector_declaration_count: summarize_detector_declaration_count(items),
        detector_declaration_bounds: summarize_detector_declaration_bounds(items),
        compact_search_error_count: summarize_compact_search_error_count(items),
        compact_filter_error_count: summarize_compact_filter_error_count(items),
        has_nonzero_probability_error: has_nonzero_probability_error(items),
        max_repeat_depth: summarize_max_repeat_depth(items),
    }
}

fn summarize_detector_shift(items: &[FoldedDemItem<'_>]) -> CircuitResult<u64> {
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

fn summarize_detector_count(items: &[FoldedDemItem<'_>]) -> CircuitResult<u64> {
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
                if body_count > 0 {
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

fn summarize_error_count(items: &[FoldedDemItem<'_>]) -> CircuitResult<u64> {
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
) -> CircuitResult<Option<DemDetectorBounds>> {
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
                if let Some(body_bounds) = body.summary().detector_declaration_bounds()? {
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

fn has_nonzero_probability_error(items: &[FoldedDemItem<'_>]) -> bool {
    items.iter().any(|item| match item {
        FoldedDemItem::Instruction(instruction) => {
            instruction.kind() == DemInstructionKind::Error
                && instruction.args().first().copied().unwrap_or(0.0) != 0.0
        }
        FoldedDemItem::Repeat { body, .. } => body.summary().has_nonzero_probability_error(),
    })
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

fn summarize_compact_search_error_count(items: &[FoldedDemItem<'_>]) -> CircuitResult<Option<u64>> {
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    if instruction.args().first().copied().unwrap_or(0.0) == 0.0 {
                        continue;
                    }
                    let mut has_any_target = false;
                    let mut has_search_target = false;
                    for target in instruction.targets() {
                        has_any_target = true;
                        match target {
                            DemTarget::RelativeDetector(_) | DemTarget::LogicalObservable(_) => {
                                has_search_target = true;
                            }
                            DemTarget::Numeric(_) => return Ok(None),
                            DemTarget::Separator => {}
                        }
                    }
                    if !has_search_target && has_any_target {
                        return Ok(None);
                    }
                    if has_search_target {
                        count = count.checked_add(1).ok_or_else(|| {
                            detector_summary_error(
                                "DEM search compact-repeat error count overflowed",
                            )
                        })?;
                    }
                }
                DemInstructionKind::ShiftDetectors if instruction.detector_shift()? == 0 => {}
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                DemInstructionKind::ShiftDetectors => return Ok(None),
            },
            FoldedDemItem::Repeat { body, .. } => {
                let Some(child_count) = body.summary().compact_search_error_count()? else {
                    return Ok(None);
                };
                if body.summary().detector_shift()? != 0 {
                    return Ok(None);
                }
                count = count.checked_add(child_count).ok_or_else(|| {
                    detector_summary_error("DEM search compact-repeat error count overflowed")
                })?;
            }
        }
    }
    Ok(Some(count))
}

fn summarize_compact_filter_error_count(items: &[FoldedDemItem<'_>]) -> CircuitResult<Option<u64>> {
    let mut count = 0_u64;
    for item in items {
        match item {
            FoldedDemItem::Instruction(instruction) => match instruction.kind() {
                DemInstructionKind::Error => {
                    for target in instruction.targets() {
                        match target {
                            DemTarget::RelativeDetector(_)
                            | DemTarget::LogicalObservable(_)
                            | DemTarget::Separator => {}
                            DemTarget::Numeric(_) => return Ok(None),
                        }
                    }
                    count = count.checked_add(1).ok_or_else(|| {
                        detector_summary_error(
                            "DEM ErrorMatcher filter compact-repeat error count overflowed",
                        )
                    })?;
                }
                DemInstructionKind::ShiftDetectors if instruction.detector_shift()? == 0 => {}
                DemInstructionKind::Detector | DemInstructionKind::LogicalObservable => {}
                DemInstructionKind::ShiftDetectors => return Ok(None),
            },
            FoldedDemItem::Repeat { body, .. } => {
                if body.summary().detector_shift()? != 0 {
                    return Ok(None);
                }
                let Some(child_count) = body.summary().compact_filter_error_count()? else {
                    return Ok(None);
                };
                count = count.checked_add(child_count).ok_or_else(|| {
                    detector_summary_error(
                        "DEM ErrorMatcher filter compact-repeat error count overflowed",
                    )
                })?;
            }
        }
    }
    Ok(Some(count))
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

pub(super) fn shifted_detector(
    detector: DemDetectorId,
    detector_offset: u64,
) -> CircuitResult<DemDetectorId> {
    DemDetectorId::try_new(
        detector
            .get()
            .checked_add(detector_offset)
            .ok_or_else(|| detector_summary_error("relative detector id overflowed"))?,
    )
}

pub(super) fn shifted_targets(
    targets: &[DemTarget],
    detector_offset: u64,
) -> CircuitResult<Vec<DemTarget>> {
    targets
        .iter()
        .map(|target| match *target {
            DemTarget::RelativeDetector(detector) => Ok(DemTarget::RelativeDetector(
                shifted_detector(detector, detector_offset)?,
            )),
            DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
                Ok(*target)
            }
        })
        .collect()
}

pub(super) fn shifted_coordinates(
    coordinates: &[f64],
    coordinate_shift: &[f64],
) -> CircuitResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
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
) -> CircuitResult<()> {
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

fn detector_summary_error(message: &'static str) -> CircuitError {
    CircuitError::invalid_detector_error_model(message)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        clippy::unwrap_used,
        reason = "unit tests use direct assertions for compact traversal diagnostics"
    )]

    use super::*;

    #[test]
    fn folded_summary_handles_nested_repeat_geometry() {
        let model = DetectorErrorModel::from_dem_str(
            "repeat 3 {\n    detector(1) D2\n    error(0.25) D2 L4\n    shift_detectors(5) 4\n    repeat 2 {\n        error(0) D1 L7\n        shift_detectors(2) 1\n    }\n}\n",
        )
        .expect("nested DEM");
        let traversal = FoldedDemTraversal::new(&model).expect("folded traversal");
        let summary = traversal.root().summary();

        assert_eq!(summary.detector_shift().unwrap(), 18);
        assert_eq!(summary.detector_count().unwrap(), 19);
        assert_eq!(traversal.root().coordinate_shift().unwrap(), vec![27.0]);
        assert_eq!(summary.observable_count(), 8);
        assert_eq!(summary.error_count().unwrap(), 9);
        assert_eq!(summary.detector_declaration_count(), Some(3));
        assert_eq!(
            summary.detector_declaration_bounds().unwrap(),
            Some(DemDetectorBounds { min: 2, max: 14 })
        );
        assert!(summary.has_nonzero_probability_error());
    }

    #[test]
    fn visitor_stops_immediately_and_preserves_errors() {
        struct Visitor {
            visits: usize,
            fail: bool,
        }

        impl FoldedDemVisitor for Visitor {
            fn visit_instruction(
                &mut self,
                _instruction: &DemInstruction,
                _state: &DemTraversalState,
            ) -> CircuitResult<ControlFlow<()>> {
                self.visits += 1;
                if self.fail {
                    return Err(CircuitError::invalid_detector_error_model(
                        "sentinel visitor error",
                    ));
                }
                Ok(ControlFlow::Break(()))
            }
        }

        let model = DetectorErrorModel::from_dem_str(
            "repeat 1000000 {\n    error(0.1) D0\n}\nerror(0.2) D1\n",
        )
        .expect("visitor DEM");
        let traversal = FoldedDemTraversal::new(&model).expect("folded traversal");
        let mut visitor = Visitor {
            visits: 0,
            fail: false,
        };
        assert!(traversal.try_visit(&mut visitor).unwrap().is_break());
        assert_eq!(visitor.visits, 1);

        let mut visitor = Visitor {
            visits: 0,
            fail: true,
        };
        let error = traversal
            .try_visit(&mut visitor)
            .expect_err("visitor error");
        assert_eq!(
            error.to_string(),
            "invalid detector error model: sentinel visitor error"
        );
        assert_eq!(visitor.visits, 1);
    }
}
