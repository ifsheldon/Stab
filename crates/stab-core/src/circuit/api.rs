use std::collections::BTreeMap;

use crate::{CircuitError, CircuitResult, GateTargetGroupKind, QubitId};

use super::{Circuit, CircuitInstruction, CircuitItem};

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
            match item {
                CircuitItem::Instruction(instruction) => self.append_instruction(instruction),
                CircuitItem::RepeatBlock(repeat) => self.append_repeat_block(repeat),
            }
        }
        Ok(())
    }

    /// Compatibility alias matching Stim's Python API name.
    pub fn append_from_stim_program_text(&mut self, input: &str) -> CircuitResult<()> {
        self.append_from_stim_text(input)
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
