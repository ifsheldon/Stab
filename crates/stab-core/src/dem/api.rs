use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Bound, Range, RangeBounds};

use crate::{CircuitError, CircuitResult};

use super::{
    DemDetectorId, DemInstruction, DemInstructionKind, DemItem, DemRepeatBlock, DemTarget,
    DetectorErrorModel,
};

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
        let count = self.count_detectors()?;
        let detectors = (0..count)
            .map(DemDetectorId::try_new)
            .collect::<CircuitResult<Vec<_>>>()?;
        self.detector_coordinates_for(detectors)
    }

    pub fn detector_coordinates_for(
        &self,
        detectors: impl IntoIterator<Item = DemDetectorId>,
    ) -> CircuitResult<BTreeMap<DemDetectorId, Vec<f64>>> {
        let detector_set: BTreeSet<_> = detectors.into_iter().collect();
        if detector_set.is_empty() {
            return Ok(BTreeMap::new());
        }
        let detector_count = self.count_detectors()?;
        for detector in &detector_set {
            if detector.get() >= detector_count {
                return Err(CircuitError::invalid_detector_error_model(format!(
                    "detector index {} is too big; the detector error model has {detector_count} detectors",
                    detector.get()
                )));
            }
        }

        let mut coordinates = BTreeMap::new();
        for instruction in self.iter_flattened_instructions() {
            let instruction = instruction?;
            if instruction.kind() == DemInstructionKind::Detector {
                for target in instruction.targets() {
                    if let DemTarget::RelativeDetector(detector) = target
                        && detector_set.contains(detector)
                    {
                        coordinates
                            .entry(*detector)
                            .or_insert_with(|| instruction.args().to_vec());
                    }
                }
            }
            if coordinates.len() == detector_set.len() {
                return Ok(coordinates);
            }
        }

        for detector in detector_set {
            coordinates.entry(detector).or_insert_with(Vec::new);
        }
        Ok(coordinates)
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

fn coordinate_shift_of(model: &DetectorErrorModel) -> CircuitResult<Vec<f64>> {
    let mut shift = Vec::new();
    apply_coordinate_shift_of(model, &mut shift)?;
    Ok(shift)
}

fn apply_coordinate_shift_of(
    model: &DetectorErrorModel,
    shift: &mut Vec<f64>,
) -> CircuitResult<()> {
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => {
                if instruction.kind() == DemInstructionKind::ShiftDetectors {
                    add_coordinate_shift_mul(shift, instruction.args(), 1.0)?;
                }
            }
            DemItem::RepeatBlock(repeat) => {
                let body_shift = coordinate_shift_of(repeat.body())?;
                add_coordinate_shift_mul(shift, &body_shift, repeat.repeat_count().get() as f64)?;
            }
        }
    }
    Ok(())
}

fn rounded_probability_arg(value: f64, digits: u8) -> f64 {
    let mut scale = 1.0;
    for _ in 0..digits {
        scale *= 10.0;
    }
    (value * scale).round() / scale
}

fn count_errors_in(model: &DetectorErrorModel) -> CircuitResult<u64> {
    let mut total = 0_u64;
    for item in model.items() {
        match item {
            DemItem::Instruction(instruction) => {
                if instruction.kind() == DemInstructionKind::Error {
                    total = total.checked_add(1).ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("error count overflowed")
                    })?;
                }
            }
            DemItem::RepeatBlock(repeat) => {
                let body_errors = count_errors_in(repeat.body())?;
                let repeated = body_errors
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(|| {
                        CircuitError::invalid_detector_error_model("repeat error count overflowed")
                    })?;
                total = total.checked_add(repeated).ok_or_else(|| {
                    CircuitError::invalid_detector_error_model("error count overflowed")
                })?;
            }
        }
    }
    Ok(total)
}

fn apply_detector_shift(
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
        shifted_detector_coordinates(instruction.args(), coordinate_shift)?
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

fn shifted_detector_coordinates(coordinates: &[f64], shift: &[f64]) -> CircuitResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
    for (index, coordinate) in shifted.iter_mut().enumerate() {
        if let Some(delta) = shift.get(index) {
            *coordinate += delta;
            if !coordinate.is_finite() {
                return Err(CircuitError::invalid_detector_error_model(
                    "detector coordinate overflowed",
                ));
            }
        }
    }
    Ok(shifted)
}

fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> CircuitResult<()> {
    if shift.len() < delta.len() {
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift.get_mut(index).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("coordinate shift dimension missing")
        })?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(CircuitError::invalid_detector_error_model(
                "coordinate shift overflowed",
            ));
        }
    }
    Ok(())
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
