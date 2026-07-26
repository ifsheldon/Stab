use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufWriter, Read, Write},
    path::Path,
};

use crate::{
    CircuitDetectorId, CircuitError, CircuitResult, GateTargetGroupKind, QubitId, RepeatCount,
};

use super::{Circuit, CircuitInstruction, CircuitItem, RepeatBlock};

const MAX_CIRCUIT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CIRCUIT_FILE_BYTES_USIZE: usize = 64 * 1024 * 1024;
const CIRCUIT_FILE_READ_LIMIT: u64 = MAX_CIRCUIT_FILE_BYTES + 1;

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
    pub fn append_from_stim_text(&mut self, input: &str) -> CircuitResult<()> {
        let parsed = Self::from_stim_str(input)?;
        for item in parsed.items {
            self.append_item(item);
        }
        Ok(())
    }

    /// Compatibility alias matching Stim's Python API name.
    pub fn append_from_stim_program_text(&mut self, input: &str) -> CircuitResult<()> {
        self.append_from_stim_text(input)
    }

    /// Reads a `.stim` circuit file from a filesystem path.
    ///
    /// Files larger than 64 MiB are rejected while the parser remains string-backed.
    pub fn from_stim_file(path: impl AsRef<Path>) -> CircuitResult<Self> {
        let path = path.as_ref();
        let file = File::open(path).map_err(|error| CircuitError::circuit_io("read", error))?;
        let metadata = file
            .metadata()
            .map_err(|error| CircuitError::circuit_io("read", error))?;
        if metadata.len() > MAX_CIRCUIT_FILE_BYTES {
            return Err(circuit_file_size_error(metadata.len()));
        }

        let mut bytes = Vec::new();
        file.take(CIRCUIT_FILE_READ_LIMIT)
            .read_to_end(&mut bytes)
            .map_err(|error| CircuitError::circuit_io("read", error))?;
        if bytes.len() > MAX_CIRCUIT_FILE_BYTES_USIZE {
            return Err(circuit_file_size_error(CIRCUIT_FILE_READ_LIMIT));
        }

        let input = String::from_utf8(bytes).map_err(|error| {
            CircuitError::circuit_io(
                "read",
                std::io::Error::new(std::io::ErrorKind::InvalidData, error),
            )
        })?;
        Self::from_stim_str(&input)
    }

    /// Writes this circuit as canonical `.stim` text to a filesystem path.
    pub fn write_stim_file(&self, path: impl AsRef<Path>) -> CircuitResult<()> {
        let file = File::create(path.as_ref())
            .map_err(|error| CircuitError::circuit_io("write", error))?;
        let mut writer = BufWriter::new(file);
        self.write_stim_io(&mut writer)
            .and_then(|()| writer.flush())
            .map_err(|error| CircuitError::circuit_io("write", error))
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
    pub fn repeated(&self, repetitions: u64) -> CircuitResult<Self> {
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
    pub fn repeat_in_place(&mut self, repetitions: u64) -> CircuitResult<()> {
        *self = self.repeated(repetitions)?;
        Ok(())
    }

    /// Inserts an item, fusing compatible instruction boundaries around the insertion point.
    pub fn insert_item(&mut self, index: usize, item: CircuitItem) -> CircuitResult<()> {
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
    ) -> CircuitResult<()> {
        self.insert_item(index, CircuitItem::Instruction(instruction))
    }

    /// Inserts a repeat block at the requested top-level item index.
    pub fn insert_repeat_block(&mut self, index: usize, repeat: RepeatBlock) -> CircuitResult<()> {
        self.insert_item(index, CircuitItem::RepeatBlock(repeat))
    }

    /// Inserts a copy of another circuit, fusing compatible instruction boundaries.
    pub fn insert_circuit(&mut self, index: usize, other: &Self) -> CircuitResult<()> {
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
    pub fn pop_item(&mut self, index: usize) -> CircuitResult<CircuitItem> {
        if index >= self.items.len() {
            return Err(pop_index_error(index));
        }
        Ok(self.items.remove(index))
    }

    /// Removes and returns the last top-level item.
    pub fn pop_last_item(&mut self) -> CircuitResult<CircuitItem> {
        let index = self
            .items
            .len()
            .checked_sub(1)
            .ok_or_else(|| pop_index_error("empty"))?;
        self.pop_item(index)
    }

    pub fn count_measurements(&self) -> CircuitResult<u64> {
        flat_sum_operations(self, |instruction| -> CircuitResult<u64> {
            if instruction.gate().produces_measurements() {
                u64::try_from(instruction_target_group_count(instruction))
                    .map_err(|_| circuit_count_overflow())
            } else {
                Ok(0)
            }
        })
    }

    pub fn count_detectors(&self) -> CircuitResult<u64> {
        flat_sum_operations(self, |instruction| {
            Ok(u64::from(instruction.gate().canonical_name() == "DETECTOR"))
        })
    }

    pub fn count_observables(&self) -> CircuitResult<u64> {
        max_operation_property(self, |instruction| {
            if instruction.gate().canonical_name() == "OBSERVABLE_INCLUDE" {
                instruction
                    .observable_id_argument()
                    .map(|id| id.map(|id| id.get().saturating_add(1)))
            } else {
                Ok(None)
            }
        })
    }

    pub fn count_ticks(&self) -> CircuitResult<u64> {
        flat_sum_operations(self, |instruction| {
            Ok(u64::from(instruction.gate().canonical_name() == "TICK"))
        })
    }

    pub fn count_sweep_bits(&self) -> CircuitResult<u64> {
        max_operation_property(self, |instruction| {
            let max_sweep = instruction.targets().iter().filter_map(|target| {
                target
                    .sweep_bit_id()
                    .map(|sweep_bit| u64::from(sweep_bit).saturating_add(1))
            });
            Ok(max_sweep.max())
        })
    }

    pub fn final_coordinate_shift(&self) -> CircuitResult<Vec<f64>> {
        coordinate_shift_of(self)
    }

    pub fn final_qubit_coordinates(&self) -> CircuitResult<BTreeMap<QubitId, Vec<f64>>> {
        let mut coordinates = BTreeMap::new();
        let mut shift = Vec::new();
        apply_final_qubit_coordinates(self, &mut shift, &mut coordinates)?;
        Ok(coordinates)
    }

    pub fn detector_coordinates(&self) -> CircuitResult<BTreeMap<CircuitDetectorId, Vec<f64>>> {
        let detector_count = self.count_detectors()?;
        let detectors = (0..detector_count)
            .map(CircuitDetectorId::new)
            .collect::<BTreeSet<_>>();
        self.detector_coordinates_for(detectors)
    }

    pub fn detector_coordinates_for(
        &self,
        detectors: impl IntoIterator<Item = CircuitDetectorId>,
    ) -> CircuitResult<BTreeMap<CircuitDetectorId, Vec<f64>>> {
        let detectors = detectors.into_iter().collect::<BTreeSet<_>>();
        let mut scan = DetectorCoordinateScan::new(detectors);
        scan.visit_circuit(self, &mut Vec::new())?;
        if let Some(missing) = scan.next_unresolved_detector() {
            return Err(CircuitError::invalid_detector_error_model(format!(
                "Detector index {} is too big. The circuit has {} detectors",
                missing.get(),
                self.count_detectors()?
            )));
        }
        Ok(scan.out)
    }

    pub fn coordinates_of_detector(&self, detector: CircuitDetectorId) -> CircuitResult<Vec<f64>> {
        let mut coordinates = self.detector_coordinates_for([detector])?;
        coordinates.remove(&detector).ok_or_else(|| {
            CircuitError::invalid_detector_error_model("detector coordinate lookup failed")
        })
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
        left.try_fuse(&right)
    }
}

fn flat_sum_operations(
    circuit: &Circuit,
    mut count_instruction: impl FnMut(&CircuitInstruction) -> CircuitResult<u64>,
) -> CircuitResult<u64> {
    fn visit(
        circuit: &Circuit,
        multiplier: u64,
        count_instruction: &mut impl FnMut(&CircuitInstruction) -> CircuitResult<u64>,
    ) -> CircuitResult<u64> {
        let mut count = 0_u64;
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    let item_count = count_instruction(instruction)?.checked_mul(multiplier);
                    count = count
                        .checked_add(item_count.ok_or_else(circuit_count_overflow)?)
                        .ok_or_else(circuit_count_overflow)?;
                }
                CircuitItem::RepeatBlock(repeat) => {
                    let repeated_multiplier = multiplier
                        .checked_mul(repeat.repeat_count().get())
                        .ok_or_else(circuit_count_overflow)?;
                    count = count
                        .checked_add(visit(
                            repeat.body(),
                            repeated_multiplier,
                            count_instruction,
                        )?)
                        .ok_or_else(circuit_count_overflow)?;
                }
            }
        }
        Ok(count)
    }

    visit(circuit, 1, &mut count_instruction)
}

fn circuit_count_overflow() -> CircuitError {
    CircuitError::invalid_result_format("circuit count overflowed")
}

fn repetition_count_overflow() -> CircuitError {
    CircuitError::invalid_domain_value("repetition count", "overflowed")
}

fn circuit_file_size_error(size: u64) -> CircuitError {
    CircuitError::invalid_domain_value(
        "circuit file size",
        format!("{size} bytes exceeds {MAX_CIRCUIT_FILE_BYTES} byte limit"),
    )
}

fn validate_insert_index(index: usize, len: usize) -> CircuitResult<()> {
    if index > len {
        return Err(CircuitError::invalid_domain_value(
            "circuit insertion index",
            index,
        ));
    }
    Ok(())
}

fn pop_index_error(index: impl ToString) -> CircuitError {
    CircuitError::invalid_domain_value("circuit pop index", index)
}

fn detector_count_overflow() -> CircuitError {
    CircuitError::invalid_detector_error_model("detector count overflowed")
}

fn instruction_target_group_count(instruction: &CircuitInstruction) -> usize {
    match instruction.gate().target_group_kind() {
        GateTargetGroupKind::None => 0,
        GateTargetGroupKind::Singles => instruction.targets().len(),
        GateTargetGroupKind::Pairs => instruction.targets().len() / 2,
        GateTargetGroupKind::PauliProducts => pauli_product_target_group_count(instruction),
        GateTargetGroupKind::AllTargets => usize::from(!instruction.targets().is_empty()),
    }
}

fn pauli_product_target_group_count(instruction: &CircuitInstruction) -> usize {
    let mut group_count = 0;
    let mut previous_was_combiner = false;
    for target in instruction.targets() {
        if target.is_combiner() {
            previous_was_combiner = true;
        } else {
            if !previous_was_combiner {
                group_count += 1;
            }
            previous_was_combiner = false;
        }
    }
    group_count
}

fn max_operation_property(
    circuit: &Circuit,
    mut instruction_property: impl FnMut(&CircuitInstruction) -> CircuitResult<Option<u64>>,
) -> CircuitResult<u64> {
    fn visit(
        circuit: &Circuit,
        instruction_property: &mut impl FnMut(&CircuitInstruction) -> CircuitResult<Option<u64>>,
    ) -> CircuitResult<u64> {
        let mut max_value = 0_u64;
        for item in circuit.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    if let Some(value) = instruction_property(instruction)? {
                        max_value = max_value.max(value);
                    }
                }
                CircuitItem::RepeatBlock(repeat) => {
                    max_value = max_value.max(visit(repeat.body(), instruction_property)?);
                }
            }
        }
        Ok(max_value)
    }

    visit(circuit, &mut instruction_property)
}

fn coordinate_shift_of(circuit: &Circuit) -> CircuitResult<Vec<f64>> {
    let mut shift = Vec::new();
    apply_coordinate_shift_of(circuit, &mut shift)?;
    Ok(shift)
}

fn apply_coordinate_shift_of(circuit: &Circuit, shift: &mut Vec<f64>) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "SHIFT_COORDS"
                    && let Some(args) = instruction.coordinate_arguments()
                {
                    add_coordinate_shift_mul(shift, args, 1.0)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_shift = coordinate_shift_of(repeat.body())?;
                add_coordinate_shift_mul(shift, &body_shift, repeat.repeat_count().get() as f64)?;
            }
        }
    }
    Ok(())
}

fn apply_final_qubit_coordinates(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    coordinates: &mut BTreeMap<QubitId, Vec<f64>>,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => match instruction.gate().canonical_name() {
                "QUBIT_COORDS" => {
                    let args = instruction.coordinate_arguments().unwrap_or_default();
                    for target in instruction.targets() {
                        if let Some(qubit) = target.qubit_id() {
                            coordinates.insert(qubit, shifted_coordinates(args, shift)?);
                        }
                    }
                }
                "SHIFT_COORDS" => {
                    if let Some(args) = instruction.coordinate_arguments() {
                        add_coordinate_shift_mul(shift, args, 1.0)?;
                    }
                }
                _ => {}
            },
            CircuitItem::RepeatBlock(repeat) => {
                let repeat_count = repeat.repeat_count().get();
                let body_shift = coordinate_shift_of(repeat.body())?;
                if repeat_count > 1 {
                    add_coordinate_shift_mul(
                        shift,
                        &body_shift,
                        repeat_count.saturating_sub(1) as f64,
                    )?;
                }
                apply_final_qubit_coordinates(repeat.body(), shift, coordinates)?;
            }
        }
    }
    Ok(())
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

    fn visit_circuit(&mut self, circuit: &Circuit, shift: &mut Vec<f64>) -> CircuitResult<()> {
        for item in circuit.items() {
            if self.next_unresolved_detector().is_none() {
                return Ok(());
            }
            match item {
                CircuitItem::Instruction(instruction) => {
                    self.visit_instruction(instruction, shift)?
                }
                CircuitItem::RepeatBlock(repeat) => self.visit_repeat(repeat, shift)?,
            }
        }
        Ok(())
    }

    fn visit_repeat(&mut self, repeat: &RepeatBlock, shift: &mut Vec<f64>) -> CircuitResult<()> {
        let body = repeat.body();
        let body_detector_count = body.count_detectors()?;
        let body_shift = coordinate_shift_of(body)?;
        let mut used_repetitions = 0_u64;
        let repetitions = repeat.repeat_count().get();

        while used_repetitions < repetitions {
            let Some(next_desired) = self.next_unresolved_detector() else {
                return Ok(());
            };
            let skip = if body_detector_count == 0 {
                repetitions.saturating_sub(used_repetitions)
            } else if next_desired.get() <= self.next_detector_index {
                0
            } else {
                let remaining = repetitions.saturating_sub(used_repetitions);
                let distance = next_desired.get().saturating_sub(self.next_detector_index);
                remaining.min(distance / body_detector_count)
            };

            if skip > 0 {
                let detector_skip = body_detector_count
                    .checked_mul(skip)
                    .ok_or_else(detector_count_overflow)?;
                self.next_detector_index = self
                    .next_detector_index
                    .checked_add(detector_skip)
                    .ok_or_else(detector_count_overflow)?;
                add_coordinate_shift_mul(shift, &body_shift, skip as f64)?;
                used_repetitions = used_repetitions
                    .checked_add(skip)
                    .ok_or_else(detector_count_overflow)?;
            }

            if used_repetitions < repetitions {
                self.visit_circuit(body, shift)?;
                used_repetitions = used_repetitions
                    .checked_add(1)
                    .ok_or_else(detector_count_overflow)?;
            }
        }
        Ok(())
    }

    fn visit_instruction(
        &mut self,
        instruction: &CircuitInstruction,
        shift: &mut Vec<f64>,
    ) -> CircuitResult<()> {
        match instruction.gate().canonical_name() {
            "SHIFT_COORDS" => {
                if let Some(args) = instruction.coordinate_arguments() {
                    add_coordinate_shift_mul(shift, args, 1.0)?;
                }
            }
            "DETECTOR" => self.visit_detector(instruction, shift)?,
            _ => {}
        }
        Ok(())
    }

    fn visit_detector(
        &mut self,
        instruction: &CircuitInstruction,
        shift: &[f64],
    ) -> CircuitResult<()> {
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
) -> CircuitResult<()> {
    if shift.len() < delta.len() {
        shift.resize(delta.len(), 0.0);
    }
    for (index, value) in delta.iter().enumerate() {
        let coordinate = shift.get_mut(index).ok_or_else(|| {
            CircuitError::invalid_result_format("coordinate shift dimension missing")
        })?;
        *coordinate += value * multiplier;
        if !coordinate.is_finite() {
            return Err(CircuitError::invalid_result_format(
                "coordinate shift overflowed",
            ));
        }
    }
    Ok(())
}

fn shifted_detector_coordinates(coordinates: &[f64], shift: &[f64]) -> CircuitResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
    for (index, coordinate) in shifted.iter_mut().enumerate() {
        if let Some(offset) = shift.get(index) {
            *coordinate += *offset;
            if !coordinate.is_finite() {
                return Err(CircuitError::invalid_result_format(
                    "coordinate shift overflowed",
                ));
            }
        }
    }
    Ok(shifted)
}

fn shifted_coordinates(coordinates: &[f64], shift: &[f64]) -> CircuitResult<Vec<f64>> {
    let mut shifted = coordinates.to_vec();
    if shifted.len() < shift.len() {
        shifted.resize(shift.len(), 0.0);
    }
    for (index, value) in shift.iter().enumerate() {
        let coordinate = shifted.get_mut(index).ok_or_else(|| {
            CircuitError::invalid_result_format("coordinate shift dimension missing")
        })?;
        *coordinate += *value;
        if !coordinate.is_finite() {
            return Err(CircuitError::invalid_result_format(
                "coordinate shift overflowed",
            ));
        }
    }
    Ok(shifted)
}
