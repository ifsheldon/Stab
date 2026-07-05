use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Gate, QubitId,
    RepeatBlock, Target,
};

const MAX_MATERIALIZED_FLATTENED_OPERATIONS: u64 = 1_000_000;

impl Circuit {
    /// Returns this circuit with repeat blocks unrolled and coordinate shifts applied.
    ///
    /// `SHIFT_COORDS` instructions are absorbed into subsequent `QUBIT_COORDS` and `DETECTOR`
    /// instructions, matching Stim's materialized `flattened` transform. Because this returns an
    /// owned circuit, expansions above one million operations are rejected; use the lazy flattened
    /// iterators for raw repeat traversal when materialization is not required.
    pub fn flattened(&self) -> CircuitResult<Self> {
        validate_flattened_operation_limit(self)?;
        let mut result = Self::new();
        visit_flattened_operations(self, &mut Vec::new(), |instruction| {
            result.append_instruction(instruction);
            Ok(())
        })?;
        Ok(result)
    }

    /// Returns owned flattened instructions without Stim-style adjacent-instruction fusion.
    ///
    /// This is the Rust transform counterpart to Stim's deprecated `flattened_operations`
    /// surface. It applies coordinate shifts and unrolls repeats, but preserves each yielded
    /// operation as an independent instruction.
    pub fn flattened_operations(&self) -> CircuitResult<Vec<CircuitInstruction>> {
        validate_flattened_operation_limit(self)?;
        let count = flattened_operation_count(self)?;
        let capacity = usize::try_from(count).map_err(|_| flattened_operation_count_error())?;
        let mut result = Vec::with_capacity(capacity);
        visit_flattened_operations(self, &mut Vec::new(), |instruction| {
            result.push(instruction);
            Ok(())
        })?;
        Ok(result)
    }

    /// Returns a copy of this circuit with noisy behavior removed while preserving records.
    ///
    /// Ordinary noise instructions are dropped. Noisy measurement probabilities are stripped, and
    /// heralded noise instructions become deterministic zero `MPAD` results so measurement-record
    /// indexing stays unchanged.
    pub fn without_noise(&self) -> CircuitResult<Self> {
        let mut result = Self::new();
        for item in self.items() {
            match item {
                CircuitItem::Instruction(instruction) => {
                    append_noiseless_instruction(&mut result, instruction)?
                }
                CircuitItem::RepeatBlock(repeat) => {
                    let body = repeat.body().without_noise()?;
                    result.append_repeat_block(RepeatBlock::new(
                        repeat.repeat_count(),
                        body,
                        repeat.tag().map(str::to_owned),
                    ));
                }
            }
        }
        Ok(result)
    }

    /// Returns the currently supported H/S/CX/M/R decomposition.
    ///
    /// This is Stab's Rust counterpart to Stim's `Circuit.decomposed()` surface for the current
    /// RPF2-owned gate families. It decomposes supported single-qubit, two-qubit, pair-measurement,
    /// MPP, SPP, and SPP_DAG operations while preserving noise, annotations, `MPAD`, repeat
    /// structure, and selected measurement-rich flow-generator semantics for decomposed MPP and
    /// pair-measurement cases.
    pub fn decomposed(&self) -> CircuitResult<Self> {
        crate::decomposed_circuit(self)
    }

    /// Returns the currently supported transform with measurement feedback inlined.
    ///
    /// This is Stab's scoped Rust counterpart to Stim's feedback-removal transform. The current
    /// implementation handles top-level single-control Pauli feedback, including the supported MPP
    /// measurement case, selected `XCZ`/`YCZ` measurement-record feedback, selected bounded
    /// repeat-loop refolding, and a selected nested bounded-repeat detector-parity feedback case,
    /// and rejects excessive repeat work or
    /// unsupported classical controlled gates with precise domain errors instead of claiming full
    /// feedback-transform parity.
    pub fn with_inlined_feedback(&self) -> CircuitResult<Self> {
        crate::circuit_with_inlined_feedback(self)
    }
}

fn validate_flattened_operation_limit(circuit: &Circuit) -> CircuitResult<()> {
    let count = flattened_operation_count(circuit)?;
    if count > MAX_MATERIALIZED_FLATTENED_OPERATIONS {
        return Err(CircuitError::invalid_domain_value(
            "flattened circuit operation count",
            format!(
                "{count} exceeds current materialized limit {MAX_MATERIALIZED_FLATTENED_OPERATIONS}"
            ),
        ));
    }
    Ok(())
}

fn flattened_operation_count(circuit: &Circuit) -> CircuitResult<u64> {
    let mut count = 0_u64;
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() != "SHIFT_COORDS" {
                    count = count
                        .checked_add(1)
                        .ok_or_else(flattened_operation_count_error)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = flattened_operation_count(repeat.body())?;
                let repeated_count = body_count
                    .checked_mul(repeat.repeat_count().get())
                    .ok_or_else(flattened_operation_count_error)?;
                count = count
                    .checked_add(repeated_count)
                    .ok_or_else(flattened_operation_count_error)?;
            }
        }
    }
    Ok(count)
}

fn visit_flattened_operations(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    mut visitor: impl FnMut(CircuitInstruction) -> CircuitResult<()>,
) -> CircuitResult<()> {
    visit_flattened_operations_inner(circuit, shift, &mut visitor)
}

fn visit_flattened_operations_inner(
    circuit: &Circuit,
    shift: &mut Vec<f64>,
    visitor: &mut impl FnMut(CircuitInstruction) -> CircuitResult<()>,
) -> CircuitResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                visit_flattened_instruction(instruction, shift, visitor)?
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_count = flattened_operation_count(repeat.body())?;
                if body_count == 0 {
                    let body_shift = coordinate_shift_of(repeat.body())?;
                    add_coordinate_shift_mul(
                        shift,
                        &body_shift,
                        repeat.repeat_count().get() as f64,
                    )?;
                    continue;
                }
                for _ in 0..repeat.repeat_count().get() {
                    visit_flattened_operations_inner(repeat.body(), shift, visitor)?;
                }
            }
        }
    }
    Ok(())
}

fn visit_flattened_instruction(
    instruction: &CircuitInstruction,
    shift: &mut Vec<f64>,
    visitor: &mut impl FnMut(CircuitInstruction) -> CircuitResult<()>,
) -> CircuitResult<()> {
    match instruction.gate().canonical_name() {
        "SHIFT_COORDS" => {
            add_coordinate_shift_mul(shift, instruction.args(), 1.0)?;
        }
        "QUBIT_COORDS" | "DETECTOR" => {
            visitor(clone_instruction_with_args(
                instruction,
                shifted_flattened_coordinates(instruction.args(), shift)?,
            )?)?;
        }
        _ => visitor(clone_instruction_with_args(
            instruction,
            instruction.args().to_vec(),
        )?)?,
    }
    Ok(())
}

fn append_noiseless_instruction(
    result: &mut Circuit,
    instruction: &CircuitInstruction,
) -> CircuitResult<()> {
    let gate = instruction.gate();
    if gate.produces_measurements() {
        let noiseless = if is_heralded_noise(gate) {
            CircuitInstruction::new(
                Gate::from_name("MPAD")?,
                Vec::new(),
                vec![Target::qubit(QubitId::new(0)?, false); instruction.targets().len()],
                instruction.tag().map(str::to_owned),
            )?
        } else {
            clone_instruction_with_args(instruction, Vec::new())?
        };
        result.append_instruction(noiseless);
    } else if !gate.is_noisy() {
        result.append_instruction(clone_instruction_with_args(
            instruction,
            instruction.args().to_vec(),
        )?);
    }
    Ok(())
}

fn coordinate_shift_of(circuit: &Circuit) -> CircuitResult<Vec<f64>> {
    let mut shift = Vec::new();
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => {
                if instruction.gate().canonical_name() == "SHIFT_COORDS" {
                    add_coordinate_shift_mul(&mut shift, instruction.args(), 1.0)?;
                }
            }
            CircuitItem::RepeatBlock(repeat) => {
                let body_shift = coordinate_shift_of(repeat.body())?;
                add_coordinate_shift_mul(
                    &mut shift,
                    &body_shift,
                    repeat.repeat_count().get() as f64,
                )?;
            }
        }
    }
    Ok(shift)
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

fn shifted_flattened_coordinates(args: &[f64], shift: &[f64]) -> CircuitResult<Vec<f64>> {
    let mut shifted = args.to_vec();
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

fn clone_instruction_with_args(
    instruction: &CircuitInstruction,
    args: Vec<f64>,
) -> CircuitResult<CircuitInstruction> {
    CircuitInstruction::new(
        instruction.gate(),
        args,
        instruction.targets().to_vec(),
        instruction.tag().map(str::to_owned),
    )
}

fn is_heralded_noise(gate: Gate) -> bool {
    matches!(
        gate.canonical_name(),
        "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1"
    )
}

fn flattened_operation_count_error() -> CircuitError {
    CircuitError::invalid_domain_value("flattened circuit operation count", "overflowed")
}
