use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Bound, ControlFlow, Range, RangeBounds};

use crate::{CircuitError, CircuitResult};

use super::coordinate_scan::{
    MAX_DEM_SELECTED_COORDINATE_FLATTENED_DECLARATIONS, RepeatScanGeometry,
    find_selected_detector_coordinates_in_bounded_flattened_repeat_body,
    flattened_detector_declaration_count_up_to,
};
use super::traversal::{
    DemDetectorBounds, DemRepeatSelection, DemTraversalState, FoldedDemBlock, FoldedDemItem,
    FoldedDemTraversal, FoldedDemVisitor, shifted_coordinates,
};
use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DemTarget,
    DetectorErrorModel,
};

const MAX_DEM_COORDINATE_MAP_DETECTORS: u64 = 1_000_000;
const MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES: u64 = 1_000_000;

#[derive(Clone, Debug)]
pub struct DemFlattenedInstructionIter<'a> {
    stack: Vec<DemFlattenFrame<'a>>,
    detector_offset: u64,
    coordinate_shift: Vec<f64>,
    finished: bool,
}

impl<'a> DemFlattenedInstructionIter<'a> {
    fn new(model: &'a DetectorErrorModel) -> Self {
        Self {
            stack: vec![DemFlattenFrame::new(model.items())],
            detector_offset: 0,
            coordinate_shift: Vec::new(),
            finished: false,
        }
    }
}

impl Iterator for DemFlattenedInstructionIter<'_> {
    type Item = CircuitResult<DemInstruction>;

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
    fn next_result(&mut self) -> CircuitResult<Option<DemInstruction>> {
        while let Some(frame) = self.stack.last_mut() {
            if frame.index == frame.items.len() {
                if frame.start_next_repetition() {
                    continue;
                }
                self.stack.pop();
                continue;
            }

            let item = frame.items.get(frame.index).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("DEM flattened iterator index escaped")
            })?;
            frame.index += 1;
            match item {
                DemItem::Instruction(instruction) => match instruction.kind() {
                    DemInstructionKind::ShiftDetectors => {
                        apply_detector_shift(&mut self.detector_offset, instruction)?;
                        add_coordinate_shift_mul(
                            &mut self.coordinate_shift,
                            instruction.args(),
                            1.0,
                        )?;
                    }
                    _ => {
                        return Ok(Some(flatten_instruction(
                            instruction,
                            self.detector_offset,
                            &self.coordinate_shift,
                        )?));
                    }
                },
                DemItem::RepeatBlock(repeat) => {
                    if !repeat.body().items().is_empty() {
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
    ) -> CircuitResult<impl DoubleEndedIterator<Item = &DemItem> + ExactSizeIterator> {
        Ok(self.item_slice(range)?.iter())
    }

    pub fn instruction_range(
        &self,
        range: impl RangeBounds<usize>,
    ) -> CircuitResult<impl DoubleEndedIterator<Item = &DemInstruction>> {
        let range = checked_dem_item_range(range, self.items.len())?;
        let items = self.item_slice(range.clone())?;
        for (offset, item) in items.iter().enumerate() {
            if matches!(item, DemItem::RepeatBlock(_)) {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "DEM instruction range contains repeat block at top-level item index {}",
                    range.start + offset
                )));
            }
        }
        Ok(items.iter().filter_map(DemItem::as_instruction))
    }

    pub fn append_from_dem_text(&mut self, input: &str) -> CircuitResult<()> {
        let mut parsed = Self::from_dem_str(input)?;
        self.items.append(&mut parsed.items);
        Ok(())
    }

    pub fn without_tags(&self) -> Self {
        Self {
            items: self.items.iter().map(DemItem::without_tags).collect(),
        }
    }

    pub fn flattened(&self) -> CircuitResult<Self> {
        self.validate_flattening_budget("flattened")?;
        let mut flattened = Self::new();
        for instruction in self.iter_flattened_instructions() {
            flattened.push_instruction(instruction?);
        }
        Ok(flattened)
    }

    pub fn rounded(&self, digits: u8) -> CircuitResult<Self> {
        Ok(Self {
            items: self
                .items
                .iter()
                .map(|item| item.rounded(digits))
                .collect::<CircuitResult<Vec<_>>>()?,
        })
    }

    pub fn final_coordinate_shift(&self) -> CircuitResult<Vec<f64>> {
        coordinate_shift_of(self)
    }

    pub fn count_errors(&self) -> CircuitResult<u64> {
        count_errors_in(self)
    }

    pub fn iter_flattened_instructions(&self) -> DemFlattenedInstructionIter<'_> {
        DemFlattenedInstructionIter::new(self)
    }

    pub fn detector_coordinates(&self) -> CircuitResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        let traversal = FoldedDemTraversal::new(self)?;
        let count = traversal.root().summary().detector_count()?;
        if count > MAX_DEM_COORDINATE_MAP_DETECTORS {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM detector_coordinates currently supports at most {MAX_DEM_COORDINATE_MAP_DETECTORS} detectors, got {count}; use detector_coordinates_for for selected detectors"
            )));
        }
        let detectors = (0..count)
            .map(DemDetectorId::try_new)
            .collect::<CircuitResult<BTreeSet<_>>>()?;
        detector_coordinates_for_traversal(&traversal, detectors, count)
    }

    pub fn detector_coordinates_for(
        &self,
        detectors: impl IntoIterator<Item = DemDetectorId>,
    ) -> CircuitResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        let detector_set: BTreeSet<_> = detectors.into_iter().collect();
        if detector_set.is_empty() {
            return Ok(BTreeMap::new());
        }
        let traversal = FoldedDemTraversal::new(self)?;
        let detector_count = traversal.root().summary().detector_count()?;
        detector_coordinates_for_traversal(&traversal, detector_set, detector_count)
    }

    pub fn coordinates_of_detector(&self, detector: DemDetectorId) -> CircuitResult<Vec<f64>> {
        self.detector_coordinates_for([detector])?
            .remove(&detector)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model(format!(
                    "detector index {} is missing from coordinate result",
                    detector.get()
                ))
            })
    }

    fn item_slice(&self, range: impl RangeBounds<usize>) -> CircuitResult<&[DemItem]> {
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
) -> CircuitResult<BTreeMap<DemDetectorId, Vec<f64>>> {
    for detector in &detector_set {
        if detector.get() >= detector_count {
            return Err(CircuitError::invalid_detector_error_model(format!(
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

    fn without_tags(&self) -> Self {
        match self {
            Self::Instruction(instruction) => {
                let mut instruction = instruction.clone();
                instruction.tag = None;
                Self::Instruction(instruction)
            }
            Self::RepeatBlock(repeat) => {
                let mut repeat = repeat.clone();
                repeat.tag = None;
                repeat.body = repeat.body.without_tags();
                Self::RepeatBlock(repeat)
            }
        }
    }

    fn rounded(&self, digits: u8) -> CircuitResult<Self> {
        match self {
            Self::Instruction(instruction) => Ok(Self::Instruction(instruction.rounded(digits)?)),
            Self::RepeatBlock(repeat) => Ok(Self::RepeatBlock(repeat.rounded(digits)?)),
        }
    }
}

pub(super) fn coordinate_shift_of(model: &DetectorErrorModel) -> CircuitResult<Vec<f64>> {
    FoldedDemTraversal::new(model)?.root().coordinate_shift()
}

impl DemInstruction {
    fn rounded(&self, digits: u8) -> CircuitResult<Self> {
        if self.kind() != DemInstructionKind::Error {
            return Ok(self.clone());
        }
        let args = self
            .args()
            .iter()
            .map(|arg| rounded_probability_arg(*arg, digits))
            .collect::<Vec<_>>();
        Self::new(
            self.kind(),
            args,
            self.targets().to_vec(),
            self.tag().map(ToOwned::to_owned),
        )
    }
}

impl DemRepeatBlock {
    fn rounded(&self, digits: u8) -> CircuitResult<Self> {
        Ok(Self {
            repeat_count: self.repeat_count,
            body: self.body.rounded(digits)?,
            tag: self.tag.clone(),
        })
    }
}

struct SelectedCoordinateVisitor<'a> {
    detector_set: &'a BTreeSet<DemDetectorId>,
    coordinates: &'a mut BTreeMap<DemDetectorId, Vec<f64>>,
    pending_truncated: Vec<BTreeSet<DemDetectorId>>,
}

impl FoldedDemVisitor for SelectedCoordinateVisitor<'_> {
    fn visit_instruction(
        &mut self,
        instruction: &DemInstruction,
        state: &DemTraversalState,
    ) -> CircuitResult<ControlFlow<()>> {
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
    ) -> CircuitResult<DemRepeatSelection> {
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
            let body_coordinate_shift = body.coordinate_shift()?;
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
            let body_coordinate_shift = body.coordinate_shift()?;
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
            CircuitError::invalid_detector_error_model(
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
    ) -> CircuitResult<ControlFlow<()>> {
        let truncated = self.pending_truncated.pop().ok_or_else(|| {
            CircuitError::invalid_detector_error_model(
                "selected coordinate traversal repeat stack underflowed",
            )
        })?;
        if let Some(detector) = truncated
            .iter()
            .find(|detector| !self.coordinates.contains_key(detector))
        {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "DEM detector_coordinates_for currently supports at most {MAX_DEM_SELECTED_COORDINATE_REPEAT_CANDIDATES} overlapping repeat candidates before finding detector {}",
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
) -> CircuitResult<()> {
    for target in instruction.targets() {
        if let DemTarget::RelativeDetector(detector) = target {
            let detector = DemDetectorId::try_new(
                detector_offset.checked_add(detector.get()).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("relative detector id overflowed")
                })?,
            )?;
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
) -> CircuitResult<()> {
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
                    if let DemTarget::RelativeDetector(local_detector) = target {
                        let local_detector = local_detector_offset
                            .checked_add(local_detector.get())
                            .ok_or_else(|| {
                                CircuitError::invalid_detector_error_model(
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
            DemInstructionKind::ShiftDetectors => {
                apply_detector_shift(&mut local_detector_offset, instruction)?;
                add_coordinate_shift_mul(&mut local_coordinate_shift, instruction.args(), 1.0)?;
            }
            DemInstructionKind::Error | DemInstructionKind::LogicalObservable => {}
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
    ) -> CircuitResult<()> {
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
    ) -> CircuitResult<()> {
        let relative_end = self
            .body_detector_shift
            .checked_mul(self.repeat_count.saturating_sub(1))
            .and_then(|shift| local_detector.checked_add(shift))
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("flat repeat detector range overflowed")
            })?;
        let start_detector = DemDetectorId::try_new(
            self.outer_detector_offset
                .checked_add(local_detector)
                .ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("flat repeat detector id overflowed")
                })?,
        )?;
        let end_detector = self
            .outer_detector_offset
            .checked_add(relative_end)
            .ok_or_else(|| {
                CircuitError::invalid_detector_error_model("flat repeat detector id overflowed")
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
) -> CircuitResult<SelectedRepeatIterations> {
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
                        CircuitError::invalid_detector_error_model(
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
                CircuitError::invalid_detector_error_model(
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
) -> CircuitResult<u64> {
    body_detector_shift
        .checked_mul(iteration)
        .and_then(|shift| detector_offset.checked_add(shift))
        .ok_or_else(|| {
            CircuitError::invalid_detector_error_model("repeat detector shift overflowed")
        })
}

pub(super) fn coordinate_shift_with_repeat(
    coordinate_shift: &[f64],
    body_coordinate_shift: &[f64],
    iteration: u64,
) -> CircuitResult<Vec<f64>> {
    let mut shifted = coordinate_shift.to_vec();
    add_coordinate_shift_mul(&mut shifted, body_coordinate_shift, iteration as f64)?;
    Ok(shifted)
}

pub(super) fn add_detector_shift_mul(
    detector_offset: &mut u64,
    detector_shift: u64,
    multiplier: u64,
) -> CircuitResult<()> {
    *detector_offset = detector_shift
        .checked_mul(multiplier)
        .and_then(|shift| (*detector_offset).checked_add(shift))
        .ok_or_else(|| CircuitError::invalid_detector_error_model("detector shift overflowed"))?;
    Ok(())
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    debug_assert!(denominator > 0);
    (numerator / denominator) + u64::from(!numerator.is_multiple_of(denominator))
}

fn rounded_probability_arg(value: f64, digits: u8) -> f64 {
    let mut scale = 1.0;
    for _ in 0..digits {
        scale *= 10.0;
    }
    (value * scale).round() / scale
}

fn count_errors_in(model: &DetectorErrorModel) -> CircuitResult<u64> {
    FoldedDemTraversal::new(model)?
        .root()
        .summary()
        .error_count()
}

pub(super) fn apply_detector_shift(
    detector_offset: &mut u64,
    instruction: &DemInstruction,
) -> CircuitResult<()> {
    *detector_offset = detector_offset
        .checked_add(instruction.detector_shift()?)
        .ok_or_else(|| CircuitError::invalid_detector_error_model("detector shift overflowed"))?;
    Ok(())
}

fn flatten_instruction(
    instruction: &DemInstruction,
    detector_offset: u64,
    coordinate_shift: &[f64],
) -> CircuitResult<DemInstruction> {
    let args = if instruction.kind() == DemInstructionKind::Detector {
        shifted_coordinates(instruction.args(), coordinate_shift)?
    } else {
        instruction.args().to_vec()
    };
    let targets = instruction
        .targets()
        .iter()
        .map(|target| shifted_target(*target, detector_offset))
        .collect::<CircuitResult<Vec<_>>>()?;
    DemInstruction::new(
        instruction.kind(),
        args,
        targets,
        instruction.tag().map(ToOwned::to_owned),
    )
}

fn shifted_target(target: DemTarget, detector_offset: u64) -> CircuitResult<DemTarget> {
    match target {
        DemTarget::RelativeDetector(detector) => {
            let shifted = detector.get().checked_add(detector_offset).ok_or_else(|| {
                CircuitError::invalid_detector_error_model("relative detector id overflowed")
            })?;
            DemTarget::relative_detector(shifted)
        }
        DemTarget::LogicalObservable(_) | DemTarget::Separator | DemTarget::Numeric(_) => {
            Ok(target)
        }
    }
}

pub(super) fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> CircuitResult<()> {
    super::traversal::add_coordinate_shift_mul(shift, delta, multiplier)
}

fn checked_dem_item_range(
    range: impl RangeBounds<usize>,
    len: usize,
) -> CircuitResult<Range<usize>> {
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

fn dem_item_range_error(value: impl ToString) -> CircuitError {
    CircuitError::invalid_detector_error_model(format!("DEM item range {}", value.to_string()))
}
