use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Bound, ControlFlow, Range, RangeBounds};

use crate::advanced::CancellationCountdown;
use crate::{Cancellation, MemoryBudget, ModelError, ModelResult};

use super::coordinate_scan::{
    MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS, RepeatScanGeometry,
    find_selected_detector_coordinates_in_bounded_flattened_repeat_body,
    flattened_detector_declaration_count_up_to,
};
use super::traversal::{
    CoordinateShifts, DemDetectorBounds, DemRepeatSelection, DemTraversalState, FoldedDemBlock,
    FoldedDemItem, FoldedDemTraversal, FoldedDemVisitor, shifted_coordinates,
};
use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DetectorErrorModel,
};

/// Conservative retained bytes of one all-map detector entry, excluding its coordinate values:
/// a selected-detector set node plus a result-map node holding a vector header.
const COORDINATE_MAP_ENTRY_BYTES: u64 = 96;
/// Conservative retained bytes of one selected repeat-iteration candidate set node.
const SELECTED_COORDINATE_CANDIDATE_BYTES: u64 = 48;
/// Candidate iterations retained by one selected-coordinate query within the default budget.
const MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES: u64 =
    MemoryBudget::DEFAULT.bytes() / SELECTED_COORDINATE_CANDIDATE_BYTES;

#[derive(Clone, Debug)]
pub struct DemFlattenedInstructionIter<'a> {
    stack: Vec<DemFlattenFrame<'a>>,
    detector_offset: u64,
    coordinate_shift: Vec<f64>,
    finished: bool,
    cancellation: Cancellation,
    countdown: CancellationCountdown,
}

impl<'a> DemFlattenedInstructionIter<'a> {
    fn new(model: &'a DetectorErrorModel) -> Self {
        Self {
            stack: vec![DemFlattenFrame::new(model.items())],
            detector_offset: 0,
            coordinate_shift: Vec::new(),
            finished: false,
            cancellation: Cancellation::never(),
            countdown: CancellationCountdown::new(),
        }
    }

    /// Polls `cancellation` once per visited item and repeat iteration, yielding
    /// [`ModelError::Cancelled`] once it is observed.
    ///
    /// A repeat whose body yields nothing, such as one holding only a zero-count repeat, is still
    /// walked once per iteration, so a caller that must stay interruptible sets a signal.
    #[must_use]
    pub fn with_cancellation(mut self, cancellation: Cancellation) -> Self {
        self.cancellation = cancellation;
        self
    }
}

impl Iterator for DemFlattenedInstructionIter<'_> {
    type Item = ModelResult<DemInstruction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }

        match self.next_result() {
            Ok(Some(instruction)) => Some(Ok(instruction)),
            Ok(None) => None,
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl DemFlattenedInstructionIter<'_> {
    fn next_result(&mut self) -> ModelResult<Option<DemInstruction>> {
        while let Some(frame) = self.stack.last_mut() {
            self.countdown.tick(&self.cancellation)?;
            if frame.index == frame.items.len() {
                if frame.start_next_repetition() {
                    continue;
                }
                self.stack.pop();
                continue;
            }

            let item = frame.items.get(frame.index).ok_or_else(|| {
                ModelError::invalid_detector_error_model("DEM flattened iterator index escaped")
            })?;
            frame.index += 1;
            match item {
                DemItem::Instruction(instruction) => match instruction.detector_shift() {
                    Some(shift) => {
                        apply_detector_shift(&mut self.detector_offset, shift)?;
                        add_coordinate_shift_mul(
                            &mut self.coordinate_shift,
                            instruction.args(),
                            1.0,
                        )?;
                    }
                    None => {
                        return Ok(Some(flatten_instruction(
                            instruction,
                            self.detector_offset,
                            &self.coordinate_shift,
                        )?));
                    }
                },
                DemItem::RepeatBlock(repeat) => {
                    if repeat.repeat_count().get() > 0 && !repeat.body().items().is_empty() {
                        self.stack.push(DemFlattenFrame::new_repeated(
                            repeat.body().items(),
                            repeat.repeat_count().get(),
                        ));
                    }
                }
            }
        }
        Ok(None)
    }
}

#[derive(Clone, Debug)]
struct DemFlattenFrame<'a> {
    items: &'a [DemItem],
    index: usize,
    remaining_repetitions: u64,
}

impl<'a> DemFlattenFrame<'a> {
    fn new(items: &'a [DemItem]) -> Self {
        Self::new_repeated(items, 1)
    }

    fn new_repeated(items: &'a [DemItem], repetitions: u64) -> Self {
        Self {
            items,
            index: 0,
            remaining_repetitions: repetitions,
        }
    }

    fn start_next_repetition(&mut self) -> bool {
        if self.remaining_repetitions > 1 {
            self.remaining_repetitions -= 1;
            self.index = 0;
            true
        } else {
            false
        }
    }
}

impl DetectorErrorModel {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn iter_items(&self) -> impl DoubleEndedIterator<Item = &DemItem> + ExactSizeIterator {
        self.items.iter()
    }

    pub fn item_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> ModelResult<impl DoubleEndedIterator<Item = &DemItem> + ExactSizeIterator> {
        Ok(self.item_slice(range)?.iter())
    }

    pub fn instruction_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> ModelResult<impl DoubleEndedIterator<Item = &DemInstruction>> {
        let range = checked_dem_item_range(range, self.items.len())?;
        let items = self.item_slice(range.clone())?;
        for (offset, item) in items.iter().enumerate() {
            if matches!(item, DemItem::RepeatBlock(_)) {
                return Err(ModelError::invalid_detector_error_model(format!(
                    "DEM instruction range contains repeat block at top-level item index {}",
                    range.start + offset
                )));
            }
        }
        Ok(items.iter().filter_map(DemItem::as_instruction))
    }

    pub fn append_from_dem_text(&mut self, input: &str) -> ModelResult<()> {
        let mut parsed = Self::from_dem_str(input)?;
        self.items.append(&mut parsed.items);
        Ok(())
    }

    pub fn final_coordinate_shift(&self) -> ModelResult<Vec<f64>> {
        coordinate_shift_of(self)
    }

    pub fn count_errors(&self) -> ModelResult<u64> {
        count_errors_in(self)
    }

    pub fn iter_flattened_instructions(&self) -> DemFlattenedInstructionIter<'_> {
        DemFlattenedInstructionIter::new(self)
    }

    /// Materializes every detector's coordinates within the default memory budget.
    pub fn detector_coordinates(&self) -> ModelResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        self.detector_coordinates_with_memory_budget(MemoryBudget::DEFAULT)
    }

    /// Materializes every detector's coordinates after admitting the conservative map size
    /// against `budget`.
    pub fn detector_coordinates_with_memory_budget(
        &self,
        budget: MemoryBudget,
    ) -> ModelResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        let traversal = FoldedDemTraversal::new(self)?;
        let count = traversal.root().summary().detector_count()?;
        let entry_bytes = u128::from(COORDINATE_MAP_ENTRY_BYTES)
            + (max_instruction_argument_count(self) as u128).saturating_mul(8);
        let required = u128::from(count).saturating_mul(entry_bytes);
        if !budget.admits(required) {
            return Err(ModelError::invalid_detector_error_model(format!(
                "DEM detector_coordinates needs at least {required} bytes for {count} detectors, exceeding the {}-byte memory budget; raise the memory budget or use detector_coordinates_for for selected detectors",
                budget.bytes()
            )));
        }
        let detectors = (0..count)
            .map(DemDetectorId::try_new)
            .collect::<ModelResult<BTreeSet<_>>>()?;
        detector_coordinates_for_traversal(&traversal, detectors, count)
    }

    pub fn detector_coordinates_for(
        &self,
        detectors: impl IntoIterator<Item = DemDetectorId>,
    ) -> ModelResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        let detector_set: BTreeSet<_> = detectors.into_iter().collect();
        if detector_set.is_empty() {
            return Ok(BTreeMap::new());
        }
        let traversal = FoldedDemTraversal::new(self)?;
        let detector_count = traversal.root().summary().detector_count()?;
        detector_coordinates_for_traversal(&traversal, detector_set, detector_count)
    }

    pub fn coordinates_of_detector(&self, detector: DemDetectorId) -> ModelResult<Vec<f64>> {
        self.detector_coordinates_for([detector])?
            .remove(&detector)
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model(format!(
                    "detector index {} is missing from coordinate result",
                    detector.get()
                ))
            })
    }

    fn item_slice(&self, range: impl RangeBounds<usize>) -> ModelResult<&[DemItem]> {
        let range = checked_dem_item_range(range, self.items.len())?;
        self.items
            .get(range)
            .ok_or_else(|| dem_item_range_error("computed range was outside item list"))
    }
}

fn detector_coordinates_for_traversal(
    traversal: &FoldedDemTraversal<'_>,
    detector_set: BTreeSet<DemDetectorId>,
    detector_count: u64,
) -> ModelResult<BTreeMap<DemDetectorId, Vec<f64>>> {
    for detector in &detector_set {
        if detector.get() >= detector_count {
            return Err(ModelError::invalid_detector_error_model(format!(
                "detector index {} is too big; the detector error model has {detector_count} detectors",
                detector.get()
            )));
        }
    }

    let mut coordinates = BTreeMap::new();
    let mut visitor = SelectedCoordinateVisitor {
        detector_set: &detector_set,
        coordinates: &mut coordinates,
        pending_truncated: Vec::new(),
        shifts: CoordinateShifts::default(),
    };
    let _ = traversal.try_visit_with_coordinates(&mut visitor)?;
    for detector in detector_set {
        coordinates.entry(detector).or_default();
    }
    Ok(coordinates)
}

impl DemItem {
    pub fn as_instruction(&self) -> Option<&DemInstruction> {
        match self {
            Self::Instruction(instruction) => Some(instruction),
            Self::RepeatBlock(_) => None,
        }
    }

    pub fn as_repeat_block(&self) -> Option<&DemRepeatBlock> {
        match self {
            Self::Instruction(_) => None,
            Self::RepeatBlock(repeat) => Some(repeat),
        }
    }
}

pub(super) fn coordinate_shift_of(model: &DetectorErrorModel) -> ModelResult<Vec<f64>> {
    FoldedDemTraversal::new(model)?.root().coordinate_shift()
}

impl DetectorErrorModel {
    /// Mutable item storage for consumers that rebuild a model in place.
    ///
    /// Items carry no cross-item invariants, so any sequence of valid items is a valid model.
    pub(crate) fn items_mut(&mut self) -> &mut Vec<DemItem> {
        &mut self.items
    }
}

impl DemRepeatBlock {
    /// Mutable access to the repeated body.
    pub(crate) fn body_mut(&mut self) -> &mut DetectorErrorModel {
        &mut self.body
    }
}

struct SelectedCoordinateVisitor<'a> {
    detector_set: &'a BTreeSet<DemDetectorId>,
    coordinates: &'a mut BTreeMap<DemDetectorId, Vec<f64>>,
    pending_truncated: Vec<BTreeSet<DemDetectorId>>,
    shifts: CoordinateShifts,
}

impl FoldedDemVisitor for SelectedCoordinateVisitor<'_> {
    type Error = ModelError;

    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> ModelResult<ControlFlow<()>> {
        if instruction.kind() == DemInstructionKind::Detector {
            record_selected_detector_coordinates(
                instruction,
                self.detector_set,
                self.coordinates,
                state.detector_offset(),
                state.coordinate_shift()?,
            )?;
        }
        Ok(if self.coordinates.len() == self.detector_set.len() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        })
    }

    fn enter_repeat(
        &mut self,
        repeat: &DemRepeatBlock,
        body: &FoldedDemBlock<'_>,
        state: &DemTraversalState,
    ) -> ModelResult<DemRepeatSelection> {
        self.pending_truncated.push(BTreeSet::new());
        let Some(body_declared_bounds) = body.summary().detector_declaration_bounds()? else {
            return Ok(DemRepeatSelection::Skip);
        };
        let body_detector_shift = body.summary().detector_shift()?;
        if body_detector_shift == 0 {
            return Ok(
                if self.detector_set.iter().any(|detector| {
                    !self.coordinates.contains_key(detector)
                        && detector_in_repeat_body_bounds(
                            *detector,
                            state.detector_offset(),
                            body_declared_bounds,
                        )
                }) {
                    DemRepeatSelection::StructuralOnce
                } else {
                    DemRepeatSelection::Skip
                },
            );
        }

        if repeat_body_is_flat(body) {
            let body_coordinate_shift = self.shifts.of(body)?.to_vec();
            find_selected_detector_coordinates_in_flat_repeat_body(
                repeat,
                body,
                self.detector_set,
                self.coordinates,
                state.detector_offset(),
                state.coordinate_shift()?,
                RepeatScanGeometry {
                    body_detector_shift,
                    body_coordinate_shift: &body_coordinate_shift,
                },
            )?;
            return Ok(DemRepeatSelection::Skip);
        }
        if flattened_detector_declaration_count_up_to(
            body,
            MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS,
        )?
        .is_some()
        {
            let body_coordinate_shift = self.shifts.of(body)?.to_vec();
            find_selected_detector_coordinates_in_bounded_flattened_repeat_body(
                repeat,
                body,
                self.detector_set,
                self.coordinates,
                state.detector_offset(),
                state.coordinate_shift()?,
                RepeatScanGeometry {
                    body_detector_shift,
                    body_coordinate_shift: &body_coordinate_shift,
                },
            )?;
            return Ok(DemRepeatSelection::Skip);
        }

        let selected = selected_repeat_iterations(
            self.detector_set,
            self.coordinates,
            state.detector_offset(),
            body_detector_shift,
            body_declared_bounds,
            repeat.repeat_count().get(),
        )?;
        let pending = self.pending_truncated.last_mut().ok_or_else(|| {
            ModelError::invalid_detector_error_model(
                "selected coordinate traversal repeat stack is missing",
            )
        })?;
        *pending = selected.truncated_detectors;
        Ok(DemRepeatSelection::Selected(
            selected.iterations.into_iter().collect(),
        ))
    }

    fn exit_repeat(
        &mut self,
        _repeat: &DemRepeatBlock,
        _body: &FoldedDemBlock<'_>,
        _state: &DemTraversalState,
    ) -> ModelResult<ControlFlow<()>> {
        let truncated = self.pending_truncated.pop().ok_or_else(|| {
            ModelError::invalid_detector_error_model(
                "selected coordinate traversal repeat stack underflowed",
            )
        })?;
        if let Some(detector) = truncated
            .iter()
            .find(|detector| !self.coordinates.contains_key(detector))
        {
            return Err(ModelError::invalid_detector_error_model(format!(
                "DEM detector_coordinates_for retains at most {MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES} overlapping repeat candidates within the default memory budget before finding detector {}",
                detector.get()
            )));
        }
        Ok(ControlFlow::Continue(()))
    }
}

fn record_selected_detector_coordinates(
    instruction: &DemInstruction,
    detector_set: &BTreeSet<DemDetectorId>,
    coordinates: &mut BTreeMap<DemDetectorId, Vec<f64>>,
    detector_offset: u64,
    coordinate_shift: &[f64],
) -> ModelResult<()> {
    for detector in instruction
        .targets()
        .iter()
        .filter_map(|target| target.detector())
    {
        {
            let detector =
                DemDetectorId::try_new(detector_offset.checked_add(detector.get()).ok_or_else(
                    || ModelError::invalid_detector_error_model("relative detector id overflowed"),
                )?)?;
            if detector_set.contains(&detector) && !coordinates.contains_key(&detector) {
                coordinates.insert(
                    detector,
                    shifted_coordinates(instruction.args(), coordinate_shift)?,
                );
            }
        }
    }
    Ok(())
}

fn repeat_body_is_flat(body: &FoldedDemBlock<'_>) -> bool {
    body.items()
        .iter()
        .all(|item| matches!(item, FoldedDemItem::Instruction(_)))
}

fn find_selected_detector_coordinates_in_flat_repeat_body(
    repeat: &DemRepeatBlock,
    body: &FoldedDemBlock<'_>,
    detector_set: &BTreeSet<DemDetectorId>,
    coordinates: &mut BTreeMap<DemDetectorId, Vec<f64>>,
    detector_offset: u64,
    coordinate_shift: &[f64],
    geometry: RepeatScanGeometry<'_>,
) -> ModelResult<()> {
    let mut local_detector_offset = 0_u64;
    let mut local_coordinate_shift = Vec::new();
    let mut scan = FlatRepeatScan {
        detector_set,
        existing_coordinates: coordinates,
        best: BTreeMap::new(),
        outer_detector_offset: detector_offset,
        outer_coordinate_shift: coordinate_shift,
        body_detector_shift: geometry.body_detector_shift,
        body_coordinate_shift: geometry.body_coordinate_shift,
        repeat_count: repeat.repeat_count().get(),
    };
    for (body_order, item) in body.items().iter().enumerate() {
        let FoldedDemItem::Instruction(instruction) = item else {
            continue;
        };
        match instruction.kind() {
            DemInstructionKind::Detector => {
                for target in instruction.targets() {
                    if let Some(local_detector) = target.detector() {
                        let local_detector = local_detector_offset
                            .checked_add(local_detector.get())
                            .ok_or_else(|| {
                                ModelError::invalid_detector_error_model(
                                    "relative detector id overflowed",
                                )
                            })?;
                        scan.record_candidates(
                            instruction,
                            local_detector,
                            &local_coordinate_shift,
                            body_order,
                        )?;
                    }
                }
            }
            DemInstructionKind::ShiftDetectors
            | DemInstructionKind::Error
            | DemInstructionKind::LogicalObservable => {}
        }
        if let Some(shift) = instruction.detector_shift() {
            apply_detector_shift(&mut local_detector_offset, shift)?;
            add_coordinate_shift_mul(&mut local_coordinate_shift, instruction.args(), 1.0)?;
        }
    }
    for (detector, (_, detector_coordinates)) in scan.best {
        coordinates.insert(detector, detector_coordinates);
    }
    Ok(())
}

pub(super) struct FlatRepeatScan<'a> {
    pub(super) detector_set: &'a BTreeSet<DemDetectorId>,
    pub(super) existing_coordinates: &'a BTreeMap<DemDetectorId, Vec<f64>>,
    pub(super) best: BTreeMap<DemDetectorId, (FlatRepeatOrder, Vec<f64>)>,
    pub(super) outer_detector_offset: u64,
    pub(super) outer_coordinate_shift: &'a [f64],
    pub(super) body_detector_shift: u64,
    pub(super) body_coordinate_shift: &'a [f64],
    pub(super) repeat_count: u64,
}

impl FlatRepeatScan<'_> {
    fn record_candidates(
        &mut self,
        instruction: &DemInstruction,
        local_detector: u64,
        local_coordinate_shift: &[f64],
        body_order: usize,
    ) -> ModelResult<()> {
        self.record_declaration_with_shift(
            local_detector,
            instruction.args(),
            local_coordinate_shift,
            body_order,
        )
    }

    pub(super) fn record_declaration_with_shift(
        &mut self,
        local_detector: u64,
        detector_coordinates: &[f64],
        local_coordinate_shift: &[f64],
        body_order: usize,
    ) -> ModelResult<()> {
        let relative_end = self
            .body_detector_shift
            .checked_mul(self.repeat_count.saturating_sub(1))
            .and_then(|shift| local_detector.checked_add(shift))
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model("flat repeat detector range overflowed")
            })?;
        let start_detector = DemDetectorId::try_new(
            self.outer_detector_offset
                .checked_add(local_detector)
                .ok_or_else(|| {
                    ModelError::invalid_detector_error_model("flat repeat detector id overflowed")
                })?,
        )?;
        let end_detector = self
            .outer_detector_offset
            .checked_add(relative_end)
            .ok_or_else(|| {
                ModelError::invalid_detector_error_model("flat repeat detector id overflowed")
            })?;

        for detector in self.detector_set.range(start_detector..) {
            if detector.get() > end_detector {
                break;
            }
            if self.existing_coordinates.contains_key(detector) {
                continue;
            }
            let Some(relative_detector) = detector.get().checked_sub(self.outer_detector_offset)
            else {
                continue;
            };
            let Some(delta) = relative_detector.checked_sub(local_detector) else {
                continue;
            };
            if !delta.is_multiple_of(self.body_detector_shift) {
                continue;
            }
            let iteration = delta / self.body_detector_shift;
            if iteration >= self.repeat_count {
                continue;
            }
            let order = FlatRepeatOrder {
                iteration,
                body_order,
            };
            if self
                .best
                .get(detector)
                .is_none_or(|(best_order, _)| order.precedes(best_order))
            {
                let mut candidate_shift = coordinate_shift_with_repeat(
                    self.outer_coordinate_shift,
                    self.body_coordinate_shift,
                    iteration,
                )?;
                add_coordinate_shift_mul(&mut candidate_shift, local_coordinate_shift, 1.0)?;
                let shifted_coordinates =
                    shifted_coordinates(detector_coordinates, &candidate_shift)?;
                self.best.insert(*detector, (order, shifted_coordinates));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct FlatRepeatOrder {
    iteration: u64,
    body_order: usize,
}

impl FlatRepeatOrder {
    fn precedes(&self, other: &Self) -> bool {
        (self.iteration, self.body_order) < (other.iteration, other.body_order)
    }
}

fn detector_in_repeat_body_bounds(
    detector: DemDetectorId,
    detector_offset: u64,
    body_declared_bounds: DemDetectorBounds,
) -> bool {
    detector
        .get()
        .checked_sub(detector_offset)
        .is_some_and(|relative| {
            (body_declared_bounds.min..=body_declared_bounds.max).contains(&relative)
        })
}

fn selected_repeat_iterations(
    detector_set: &BTreeSet<DemDetectorId>,
    coordinates: &BTreeMap<DemDetectorId, Vec<f64>>,
    detector_offset: u64,
    body_detector_shift: u64,
    body_declared_bounds: DemDetectorBounds,
    repeat_count: u64,
) -> ModelResult<SelectedRepeatIterations> {
    let mut iterations = BTreeSet::new();
    let mut candidate_count = 0_u64;
    let mut truncated_detectors = BTreeSet::new();
    for detector in detector_set {
        if coordinates.contains_key(detector) {
            continue;
        }
        let Some(relative) = detector.get().checked_sub(detector_offset) else {
            continue;
        };
        if relative < body_declared_bounds.min {
            continue;
        }
        let min_iteration = if relative <= body_declared_bounds.max {
            0
        } else {
            ceil_div(
                relative
                    .checked_sub(body_declared_bounds.max)
                    .ok_or_else(|| {
                        ModelError::invalid_detector_error_model(
                            "selected detector repeat range underflowed",
                        )
                    })?,
                body_detector_shift,
            )
        };
        if min_iteration >= repeat_count {
            continue;
        }
        let max_iteration =
            ((relative - body_declared_bounds.min) / body_detector_shift).min(repeat_count - 1);
        if min_iteration > max_iteration {
            continue;
        }
        for iteration in min_iteration..=max_iteration {
            if candidate_count >= MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES {
                truncated_detectors.insert(*detector);
                break;
            }
            iterations.insert(iteration);
            candidate_count = candidate_count.checked_add(1).ok_or_else(|| {
                ModelError::invalid_detector_error_model(
                    "selected detector coordinate repeat candidate count overflowed",
                )
            })?;
        }
    }
    Ok(SelectedRepeatIterations {
        iterations,
        truncated_detectors,
    })
}

struct SelectedRepeatIterations {
    iterations: BTreeSet<u64>,
    truncated_detectors: BTreeSet<DemDetectorId>,
}

pub(super) fn detector_offset_with_repeat(
    detector_offset: u64,
    body_detector_shift: u64,
    iteration: u64,
) -> ModelResult<u64> {
    body_detector_shift
        .checked_mul(iteration)
        .and_then(|shift| detector_offset.checked_add(shift))
        .ok_or_else(|| ModelError::invalid_detector_error_model("repeat detector shift overflowed"))
}

pub(super) fn coordinate_shift_with_repeat(
    coordinate_shift: &[f64],
    body_coordinate_shift: &[f64],
    iteration: u64,
) -> ModelResult<Vec<f64>> {
    let mut shifted = coordinate_shift.to_vec();
    add_coordinate_shift_mul(&mut shifted, body_coordinate_shift, iteration as f64)?;
    Ok(shifted)
}

pub(super) fn add_detector_shift_mul(
    detector_offset: &mut u64,
    detector_shift: u64,
    multiplier: u64,
) -> ModelResult<()> {
    *detector_offset = detector_shift
        .checked_mul(multiplier)
        .and_then(|shift| (*detector_offset).checked_add(shift))
        .ok_or_else(|| ModelError::invalid_detector_error_model("detector shift overflowed"))?;
    Ok(())
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    debug_assert!(denominator > 0);
    (numerator / denominator) + u64::from(!numerator.is_multiple_of(denominator))
}

fn count_errors_in(model: &DetectorErrorModel) -> ModelResult<u64> {
    FoldedDemTraversal::new(model)?
        .root()
        .summary()
        .error_count()
}

pub(super) fn apply_detector_shift(detector_offset: &mut u64, shift: u64) -> ModelResult<()> {
    *detector_offset = detector_offset
        .checked_add(shift)
        .ok_or_else(|| ModelError::invalid_detector_error_model("detector shift overflowed"))?;
    Ok(())
}

fn flatten_instruction(
    instruction: &DemInstruction,
    detector_offset: u64,
    coordinate_shift: &[f64],
) -> ModelResult<DemInstruction> {
    let args = if instruction.kind() == DemInstructionKind::Detector {
        shifted_coordinates(instruction.args(), coordinate_shift)?
    } else {
        let mut args = Vec::new();
        args.try_reserve_exact(instruction.args().len())
            .map_err(|_| {
                ModelError::invalid_detector_error_model(
                    "flattened instruction argument allocation failed",
                )
            })?;
        args.extend_from_slice(instruction.args());
        args
    };
    instruction.relocated(args, detector_offset)
}

pub(super) fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> ModelResult<()> {
    super::traversal::add_coordinate_shift_mul(shift, delta, multiplier)
}

fn checked_dem_item_range(range: impl RangeBounds<usize>, len: usize) -> ModelResult<Range<usize>> {
    let start = match range.start_bound() {
        Bound::Included(start) => *start,
        Bound::Excluded(start) => start
            .checked_add(1)
            .ok_or_else(|| dem_item_range_error("excluded start index overflowed"))?,
        Bound::Unbounded => 0,
    };
    let end = match range.end_bound() {
        Bound::Included(end) => end
            .checked_add(1)
            .ok_or_else(|| dem_item_range_error("included end index overflowed"))?,
        Bound::Excluded(end) => *end,
        Bound::Unbounded => len,
    };

    if start > end || end > len {
        return Err(dem_item_range_error(format!(
            "{start}..{end} outside top-level item length {len}",
        )));
    }
    Ok(start..end)
}

fn dem_item_range_error(value: impl ToString) -> ModelError {
    ModelError::invalid_detector_error_model(format!("DEM item range {}", value.to_string()))
}

/// Longest argument list of any instruction, which bounds every coordinate vector length.
fn max_instruction_argument_count(model: &DetectorErrorModel) -> usize {
    let mut longest = 0;
    let mut stack = vec![model.items.as_slice()];
    while let Some(items) = stack.pop() {
        for item in items {
            match item {
                DemItem::Instruction(instruction) => {
                    longest = longest.max(instruction.args().len())
                }
                DemItem::RepeatBlock(repeat) => stack.push(repeat.body().items.as_slice()),
            }
        }
    }
    longest
}
