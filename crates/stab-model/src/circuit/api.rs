use std::collections::{BTreeMap, BTreeSet};

use crate::resource_limit::admit_detector_coordinate_map;
use crate::{
    CircuitDetectorId, Gate, MemoryBudget, ModelDialect, ModelError, ModelResult, QubitId,
    RepeatCount, ValidationError,
};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

mod body_summaries;

use body_summaries::BodySummaries;

impl Circuit {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Parses Stim circuit text and appends the resulting operations to this circuit.
    ///
    /// The text is parsed into a temporary circuit before mutating `self`, so parse failures leave
    /// the existing circuit unchanged. Appended instructions use the normal append path, including
    /// Stim-style fusion with the previous instruction when applicable.
    pub fn append_from_stim_text(&mut self, input: &str) -> ModelResult<()> {
        let mut parsed = Self::from_stim_str(input)?;
        // Circuit implements Drop, so drain instead of moving the items out.
        for item in parsed.items.drain(..) {
            self.append_item(item);
        }
        Ok(())
    }

    /// Compatibility alias matching Stim's Python API name.
    pub fn append_from_stim_program_text(&mut self, input: &str) -> ModelResult<()> {
        self.append_from_stim_text(input)
    }

    /// Appends a copy of another circuit, fusing adjacent compatible instructions at the boundary.
    pub fn append_circuit(&mut self, other: &Self) {
        for item in other.items.iter().cloned() {
            self.append_item(item);
        }
    }

    /// Returns a copy of this circuit followed by a copy of another circuit.
    pub fn concatenated(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.append_circuit(other);
        result
    }

    /// Returns this circuit repeated using Stim's repeat-block special cases.
    pub fn repeated(&self, repetitions: u64) -> ModelResult<Self> {
        if repetitions == 0 {
            return Ok(Self::new());
        }
        if repetitions == 1 {
            return Ok(self.clone());
        }
        if let [CircuitItem::RepeatBlock(repeat)] = self.items() {
            let repeat_count = repeat
                .repeat_count()
                .get()
                .checked_mul(repetitions)
                .ok_or_else(repetition_count_overflow)?;
            return Ok(Self {
                items: vec![CircuitItem::RepeatBlock(RepeatBlock::new(
                    RepeatCount::try_new(repeat_count)?,
                    repeat.body().clone(),
                    None,
                ))],
            });
        }

        Ok(Self {
            items: vec![CircuitItem::RepeatBlock(RepeatBlock::new(
                RepeatCount::try_new(repetitions)?,
                self.clone(),
                None,
            ))],
        })
    }

    /// Mutates this circuit into its repeated form.
    pub fn repeat_in_place(&mut self, repetitions: u64) -> ModelResult<()> {
        *self = self.repeated(repetitions)?;
        Ok(())
    }

    /// Inserts an item, fusing compatible instruction boundaries around the insertion point.
    pub fn insert_item(&mut self, index: usize, item: CircuitItem) -> ModelResult<()> {
        validate_insert_index(index, self.items.len())?;
        self.items.insert(index, item);
        self.fuse_inserted_range(index, 1);
        Ok(())
    }

    /// Inserts an instruction, fusing compatible instruction boundaries around the insertion point.
    pub fn insert_instruction(
        &mut self,
        index: usize,
        instruction: CircuitInstruction,
    ) -> ModelResult<()> {
        self.insert_item(index, CircuitItem::Instruction(instruction))
    }

    /// Inserts a repeat block at the requested top-level item index.
    pub fn insert_repeat_block(&mut self, index: usize, repeat: RepeatBlock) -> ModelResult<()> {
        self.insert_item(index, CircuitItem::RepeatBlock(repeat))
    }

    /// Inserts a copy of another circuit, fusing compatible instruction boundaries.
    pub fn insert_circuit(&mut self, index: usize, other: &Self) -> ModelResult<()> {
        validate_insert_index(index, self.items.len())?;
        let inserted_len = other.items.len();
        if inserted_len == 0 {
            return Ok(());
        }
        self.items.splice(index..index, other.items.iter().cloned());
        self.fuse_inserted_range(index, inserted_len);
        Ok(())
    }

    /// Removes and returns the top-level item at `index`.
    pub fn pop_item(&mut self, index: usize) -> ModelResult<CircuitItem> {
        if index >= self.items.len() {
            return Err(pop_index_error(index));
        }
        Ok(self.items.remove(index))
    }

    /// Removes and returns the last top-level item.
    pub fn pop_last_item(&mut self) -> ModelResult<CircuitItem> {
        let index = self
            .items
            .len()
            .checked_sub(1)
            .ok_or_else(|| pop_index_error("empty"))?;
        self.pop_item(index)
    }

    pub fn count_measurements(&self) -> ModelResult<u64> {
        flat_sum_operations(self, |instruction| -> ModelResult<u64> {
            if instruction.gate().produces_measurements() {
                u64::try_from(instruction.target_group_count())
                    .map_err(|_| circuit_count_overflow())
            } else {
                Ok(0)
            }
        })
    }

    pub fn count_detectors(&self) -> ModelResult<u64> {
        flat_sum_operations(self, |instruction| {
            Ok(u64::from(instruction.gate() == Gate::DETECTOR))
        })
    }

    pub fn count_observables(&self) -> ModelResult<u64> {
        max_operation_property(self, |instruction| {
            if instruction.gate() == Gate::OBSERVABLE_INCLUDE {
                instruction
                    .observable_id_argument()
                    .map(|id| id.map(|id| u64::from(id.get()) + 1))
            } else {
                Ok(None)
            }
        })
    }

    pub fn count_ticks(&self) -> ModelResult<u64> {
        flat_sum_operations(self, |instruction| {
            Ok(u64::from(instruction.gate() == Gate::TICK))
        })
    }

    pub fn count_sweep_bits(&self) -> ModelResult<u64> {
        max_operation_property(self, |instruction| {
            let max_sweep = instruction.targets().iter().filter_map(|target| {
                target
                    .sweep_bit_id()
                    .map(|sweep_bit| u64::from(sweep_bit).saturating_add(1))
            });
            Ok(max_sweep.max())
        })
    }

    pub fn final_coordinate_shift(&self) -> ModelResult<Vec<f64>> {
        coordinate_shift_of(self)
    }

    pub fn final_qubit_coordinates(&self) -> ModelResult<BTreeMap<QubitId, Vec<f64>>> {
        let mut coordinates = BTreeMap::new();
        let mut shift = Vec::new();
        apply_final_qubit_coordinates(self, &mut shift, &mut coordinates)?;
        Ok(coordinates)
    }

    /// Materializes every detector's coordinates within the default memory budget.
    pub fn detector_coordinates(&self) -> ModelResult<BTreeMap<CircuitDetectorId, Vec<f64>>> {
        self.detector_coordinates_with_budget(MemoryBudget::DEFAULT)
    }

    /// Materializes every detector's coordinates after admitting the conservative map size
    /// against `budget`.
    ///
    /// A map beyond `budget` is refused with a [`crate::ResourceLimitError`] of kind
    /// [`crate::ResourceKind::MaterializedBytes`] before anything is allocated, so a huge
    /// `REPEAT` cannot force an allocation proportional to its detector count. Use
    /// [`Self::detector_coordinates_for`] to look up selected detectors without this charge.
    pub fn detector_coordinates_with_budget(
        &self,
        budget: MemoryBudget,
    ) -> ModelResult<BTreeMap<CircuitDetectorId, Vec<f64>>> {
        let detector_count = self.count_detectors()?;
        let max_coordinates = max_operation_property(self, |instruction| {
            Ok((instruction.gate() == Gate::DETECTOR).then(|| instruction.args().len() as u64))
        })?;
        admit_detector_coordinate_map(
            ModelDialect::StimCircuit,
            detector_count,
            usize::try_from(max_coordinates).unwrap_or(usize::MAX),
            budget,
        )?;
        let detectors = (0..detector_count)
            .map(CircuitDetectorId::new)
            .collect::<BTreeSet<_>>();
        self.detector_coordinates_for(detectors)
    }

    pub fn detector_coordinates_for(
        &self,
        detectors: impl IntoIterator<Item = CircuitDetectorId>,
    ) -> ModelResult<BTreeMap<CircuitDetectorId, Vec<f64>>> {
        let detectors = detectors.into_iter().collect::<BTreeSet<_>>();
        let mut scan = DetectorCoordinateScan::new(detectors);
        scan.visit_circuit(self, &mut Vec::new())?;
        if let Some(missing) = scan.next_unresolved_detector() {
            return Err(ValidationError::DetectorIndexOutOfRange {
                index: missing.get(),
                detector_count: self.count_detectors()?,
            }
            .into());
        }
        Ok(scan.out)
    }

    pub fn coordinates_of_detector(&self, detector: CircuitDetectorId) -> ModelResult<Vec<f64>> {
        let mut coordinates = self.detector_coordinates_for([detector])?;
        coordinates
            .remove(&detector)
            .ok_or_else(|| ValidationError::DetectorCoordinateLookupFailed.into())
    }

    fn append_item(&mut self, item: CircuitItem) {
        match item {
            CircuitItem::Instruction(instruction) => self.append_instruction(instruction),
            CircuitItem::RepeatBlock(repeat) => self.append_repeat_block(repeat),
        }
    }

    fn fuse_inserted_range(&mut self, index: usize, inserted_len: usize) {
        if inserted_len == 0 {
            return;
        }
        self.try_fuse_after(index + inserted_len - 1);
        if index > 0 {
            self.try_fuse_after(index - 1);
        }
    }

    fn try_fuse_after(&mut self, index: usize) -> bool {
        let Some(next_index) = index.checked_add(1) else {
            return false;
        };
        if next_index >= self.items.len() {
            return false;
        }
        let can_fuse = match (self.items.get(index), self.items.get(next_index)) {
            (Some(CircuitItem::Instruction(left)), Some(CircuitItem::Instruction(right))) => {
                left.can_fuse(right)
            }
            _ => false,
        };
        if !can_fuse {
            return false;
        }
        let CircuitItem::Instruction(right) = self.items.remove(next_index) else {
            return false;
        };
        let Some(CircuitItem::Instruction(left)) = self.items.get_mut(index) else {
            return false;
        };
        left.targets.extend_from_slice(&right.targets);
        true
    }
}

/// Sums `count_instruction` over the expanded circuit without unrolling or recursing.
///
/// Each repeat body is summed once and multiplied by its count when the body ends, with the same
/// checked arithmetic, in the same order, as a body-first recursion.
fn flat_sum_operations(
    circuit: &Circuit,
    mut count_instruction: impl FnMut(&CircuitInstruction) -> ModelResult<u64>,
) -> ModelResult<u64> {
    // Each frame holds the enclosing block's iterator, its partial sum, and the repeat count
    // of the body being summed.
    let mut enclosing: Vec<(std::slice::Iter<'_, CircuitItem>, u64, u64)> = Vec::new();
    let mut items = circuit.items().iter();
    let mut count = 0_u64;
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => {
                count = count
                    .checked_add(count_instruction(instruction)?)
                    .ok_or_else(circuit_count_overflow)?;
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let parent = std::mem::replace(&mut items, repeat.body().items().iter());
                enclosing.push((parent, count, repeat.repeat_count().get()));
                count = 0;
            }
            None => {
                let Some((parent, parent_count, repeat_count)) = enclosing.pop() else {
                    return Ok(count);
                };
                let repeated_count = count
                    .checked_mul(repeat_count)
                    .ok_or_else(circuit_count_overflow)?;
                count = parent_count
                    .checked_add(repeated_count)
                    .ok_or_else(circuit_count_overflow)?;
                items = parent;
            }
        }
    }
}

/// Visits every instruction of `circuit`, nested ones included, in source order without
/// recursing.
fn try_for_each_instruction<'a>(
    circuit: &'a Circuit,
    mut visit: impl FnMut(&'a CircuitInstruction) -> ModelResult<()>,
) -> ModelResult<()> {
    let mut enclosing = Vec::new();
    let mut items = circuit.items().iter();
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => visit(instruction)?,
            Some(CircuitItem::RepeatBlock(repeat)) => {
                enclosing.push(std::mem::replace(&mut items, repeat.body().items().iter()));
            }
            None => {
                let Some(parent) = enclosing.pop() else {
                    return Ok(());
                };
                items = parent;
            }
        }
    }
}

fn circuit_count_overflow() -> ModelError {
    ValidationError::CircuitCountOverflow.into()
}

fn repetition_count_overflow() -> ModelError {
    ModelError::invalid_domain_value("repetition count", "overflowed")
}

fn validate_insert_index(index: usize, len: usize) -> ModelResult<()> {
    if index > len {
        return Err(ModelError::invalid_domain_value(
            "circuit insertion index",
            index,
        ));
    }
    Ok(())
}

fn pop_index_error(index: impl ToString) -> ModelError {
    ModelError::invalid_domain_value("circuit pop index", index)
}

fn detector_count_overflow() -> ModelError {
    ValidationError::DetectorCountOverflow.into()
}

fn max_operation_property(
    circuit: &Circuit,
    mut instruction_property: impl FnMut(&CircuitInstruction) -> ModelResult<Option<u64>>,
) -> ModelResult<u64> {
    let mut max_value = 0_u64;
    try_for_each_instruction(circuit, |instruction| {
        if let Some(value) = instruction_property(instruction)? {
            max_value = max_value.max(value);
        }
        Ok(())
    })?;
    Ok(max_value)
}

/// Returns the coordinate shift `circuit` applies, without recursing.
///
/// Each repeat body's shift is accumulated from zero and added, times the repeat count, to its
/// parent's shift when the body ends, so the floating-point operations and their order match a
/// body-first recursion exactly.
fn coordinate_shift_of(circuit: &Circuit) -> ModelResult<Vec<f64>> {
    let mut enclosing: Vec<(std::slice::Iter<'_, CircuitItem>, Vec<f64>, u64)> = Vec::new();
    let mut items = circuit.items().iter();
    let mut shift = Vec::new();
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => {
                if instruction.gate() == Gate::SHIFT_COORDS
                    && let Some(args) = instruction.coordinate_arguments()
                {
                    add_coordinate_shift_mul(&mut shift, args, 1.0)?;
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let parent = std::mem::replace(&mut items, repeat.body().items().iter());
                let parent_shift = std::mem::take(&mut shift);
                enclosing.push((parent, parent_shift, repeat.repeat_count().get()));
            }
            None => {
                let Some((parent, parent_shift, repeat_count)) = enclosing.pop() else {
                    return Ok(shift);
                };
                let body_shift = std::mem::replace(&mut shift, parent_shift);
                add_coordinate_shift_mul(&mut shift, &body_shift, repeat_count as f64)?;
                items = parent;
            }
        }
    }
}

/// Records the final coordinates of every `QUBIT_COORDS` target, without recursing.
///
/// Entering a repeat body first advances `shift` by all but one iteration of the body's shift and
/// then visits the body once, as the recursive definition does. Each body is entered once, so
/// its cached shift is released when it is used.
fn apply_final_qubit_coordinates(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    coordinates: &mut BTreeMap<QubitId, Vec<f64>>,
) -> ModelResult<()> {
    let mut bodies = BodySummaries::default();
    let mut enclosing = Vec::new();
    let mut items = circuit.items().iter();
    loop {
        match items.next() {
            Some(CircuitItem::Instruction(instruction)) => {
                let gate = instruction.gate();
                if gate == Gate::QUBIT_COORDS {
                    let args = instruction.coordinate_arguments().unwrap_or_default();
                    for target in instruction.targets() {
                        if let Some(qubit) = target.qubit_id() {
                            coordinates.insert(qubit, shifted_coordinates(args, shift)?);
                        }
                    }
                } else if gate == Gate::SHIFT_COORDS
                    && let Some(args) = instruction.coordinate_arguments()
                {
                    add_coordinate_shift_mul(shift, args, 1.0)?;
                }
            }
            Some(CircuitItem::RepeatBlock(repeat)) => {
                let repeat_count = repeat.repeat_count().get();
                let body_shift = bodies.take_shift(repeat.body())?;
                if repeat_count > 1 {
                    add_coordinate_shift_mul(
                        shift,
                        &body_shift,
                        repeat_count.saturating_sub(1) as f64,
                    )?;
                }
                enclosing.push(std::mem::replace(&mut items, repeat.body().items().iter()));
            }
            None => {
                let Some(parent) = enclosing.pop() else {
                    return Ok(());
                };
                items = parent;
            }
        }
    }
}

/// Progress through one repeat block of a detector-coordinate scan.
struct RepeatScan<'a> {
    body: &'a Circuit,
    body_detector_count: u64,
    body_shift: std::rc::Rc<Vec<f64>>,
    used_repetitions: u64,
    repetitions: u64,
}

struct DetectorCoordinateScan {
    desired: Vec<CircuitDetectorId>,
    desired_cursor: usize,
    next_detector_index: u64,
    out: BTreeMap<CircuitDetectorId, Vec<f64>>,
}

impl DetectorCoordinateScan {
    fn new(desired: BTreeSet<CircuitDetectorId>) -> Self {
        Self {
            desired: desired.into_iter().collect(),
            desired_cursor: 0,
            next_detector_index: 0,
            out: BTreeMap::new(),
        }
    }

    fn next_unresolved_detector(&self) -> Option<CircuitDetectorId> {
        self.desired.get(self.desired_cursor).copied()
    }

    /// Scans `circuit` until every desired detector is resolved, without recursing.
    ///
    /// A repeat block skips whole iterations that cannot contain the next desired detector and
    /// visits its body once per remaining iteration, each visit a stack frame instead of a
    /// recursive call, with the same checked arithmetic in the same order. Body detector counts
    /// and shifts are computed once per scan, so the scan's time is linear in nesting depth.
    fn visit_circuit(&mut self, circuit: &Circuit, shift: &mut Vec<f64>) -> ModelResult<()> {
        let mut bodies = BodySummaries::default();
        let mut enclosing: Vec<(std::slice::Iter<'_, CircuitItem>, RepeatScan<'_>)> = Vec::new();
        let mut items = circuit.items().iter();
        loop {
            if self.next_unresolved_detector().is_none() {
                // Nothing after this point can change the result or fail.
                return Ok(());
            }
            match items.next() {
                Some(CircuitItem::Instruction(instruction)) => {
                    self.visit_instruction(instruction, shift)?;
                }
                Some(CircuitItem::RepeatBlock(repeat)) => {
                    let body = repeat.body();
                    let mut scan = RepeatScan {
                        body,
                        body_detector_count: bodies.detector_count(body)?,
                        body_shift: bodies.shift(body)?,
                        used_repetitions: 0,
                        repetitions: repeat.repeat_count().get(),
                    };
                    if self.start_next_iteration(&mut scan, shift)? {
                        enclosing.push((std::mem::replace(&mut items, body.items().iter()), scan));
                    }
                }
                None => {
                    let Some((parent, mut scan)) = enclosing.pop() else {
                        return Ok(());
                    };
                    scan.used_repetitions = scan
                        .used_repetitions
                        .checked_add(1)
                        .ok_or_else(detector_count_overflow)?;
                    if self.start_next_iteration(&mut scan, shift)? {
                        items = scan.body.items().iter();
                        enclosing.push((parent, scan));
                    } else {
                        items = parent;
                    }
                }
            }
        }
    }

    /// Skips the iterations of `scan` that cannot hold the next desired detector and reports
    /// whether one iteration remains to be visited.
    fn start_next_iteration(
        &mut self,
        scan: &mut RepeatScan<'_>,
        shift: &mut Vec<f64>,
    ) -> ModelResult<bool> {
        if scan.used_repetitions >= scan.repetitions {
            return Ok(false);
        }
        let Some(next_desired) = self.next_unresolved_detector() else {
            return Ok(false);
        };
        let skip = if scan.body_detector_count == 0 {
            scan.repetitions.saturating_sub(scan.used_repetitions)
        } else if next_desired.get() <= self.next_detector_index {
            0
        } else {
            let remaining = scan.repetitions.saturating_sub(scan.used_repetitions);
            let distance = next_desired.get().saturating_sub(self.next_detector_index);
            remaining.min(distance / scan.body_detector_count)
        };

        if skip > 0 {
            let detector_skip = scan
                .body_detector_count
                .checked_mul(skip)
                .ok_or_else(detector_count_overflow)?;
            self.next_detector_index = self
                .next_detector_index
                .checked_add(detector_skip)
                .ok_or_else(detector_count_overflow)?;
            add_coordinate_shift_mul(shift, &scan.body_shift, skip as f64)?;
            scan.used_repetitions = scan
                .used_repetitions
                .checked_add(skip)
                .ok_or_else(detector_count_overflow)?;
        }
        Ok(scan.used_repetitions < scan.repetitions)
    }

    fn visit_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        shift: &mut Vec<f64>,
    ) -> ModelResult<()> {
        let gate = instruction.gate();
        if gate == Gate::SHIFT_COORDS {
            if let Some(args) = instruction.coordinate_arguments() {
                add_coordinate_shift_mul(shift, args, 1.0)?;
            }
        } else if gate == Gate::DETECTOR {
            self.visit_detector(instruction, shift)?;
        }
        Ok(())
    }

    fn visit_detector(
        &mut self,
        instruction: &CircuitInstruction,
        shift: &[f64],
    ) -> ModelResult<()> {
        let detector_id = CircuitDetectorId::new(self.next_detector_index);
        if self
            .next_unresolved_detector()
            .is_some_and(|desired| desired == detector_id)
        {
            self.out.insert(
                detector_id,
                shifted_detector_coordinates(instruction.args(), shift)?,
            );
            self.desired_cursor += 1;
        }
        self.next_detector_index = self
            .next_detector_index
            .checked_add(1)
            .ok_or_else(detector_count_overflow)?;
        Ok(())
    }
}

fn add_coordinate_shift_mul(
    shift: &mut Vec<f64>,
    delta: &[f64],
    multiplier: f64,
) -> ModelResult<()> {
    if shift.len() < delta.len() {
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift
            .get_mut(index)
            .ok_or_else(|| ModelError::from(ValidationError::CoordinateShiftDimensionMissing))?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(ValidationError::CoordinateShiftOverflow.into());
        }
    }
    Ok(())
}

fn shifted_detector_coordinates(coordinates: &[f64], shift: &[f64]) -> ModelResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
    for (index, coordinate) in shifted.iter_mut().enumerate() {
        if let Some(offset) = shift.get(index) {
            *coordinate += *offset;
            if !coordinate.is_finite() {
                return Err(ValidationError::CoordinateShiftOverflow.into());
            }
        }
    }
    Ok(shifted)
}

fn shifted_coordinates(coordinates: &[f64], shift: &[f64]) -> ModelResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
    if shifted.len() < shift.len() {
        shifted.resize(shift.len(), 0.0);
    }
    for (index, value) in shift.iter().enumerate() {
        let coordinate = shifted
            .get_mut(index)
            .ok_or_else(|| ModelError::from(ValidationError::CoordinateShiftDimensionMissing))?;
        *coordinate += *value;
        if !coordinate.is_finite() {
            return Err(ValidationError::CoordinateShiftOverflow.into());
        }
    }
    Ok(shifted)
}
