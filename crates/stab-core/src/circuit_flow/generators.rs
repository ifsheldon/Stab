use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitItem, CircuitResult, Flow, GateCategory,
    Pauli, PauliBasis, PauliSign, PauliString, Target,
};

const MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS: usize = 4096;

/// Returns unsigned stabilizer-flow generators for the supported tableau and PFM5 measurement subset.
///
/// Repeat-contained measurement-rich circuits use bounded flattened operations plus a flow-row cap; broader semantics fail closed.
pub fn circuit_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    if circuit_needs_measurement_rich_generators(circuit) {
        return simple_measurement_rich_flow_generators(circuit)?
            .ok_or_else(|| unsupported_flow_generator_error(circuit));
    }
    unitary_flow_generators(circuit)
}

fn unitary_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    let tableau = circuit.to_tableau(false, false, false)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in (0..tableau.len()).rev() {
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
    }
    Ok(flows)
}

fn circuit_needs_measurement_rich_generators(circuit: &Circuit) -> bool {
    circuit.items().iter().any(|item| match item {
        CircuitItem::Instruction(instruction) => {
            instruction.gate().produces_measurements()
                || matches!(
                    instruction.gate().category(),
                    GateCategory::Collapsing | GateCategory::PairMeasurement
                )
        }
        CircuitItem::RepeatBlock(repeat) => {
            circuit_needs_measurement_rich_generators(repeat.body())
        }
    })
}

fn simple_measurement_rich_flow_generators(circuit: &Circuit) -> CircuitResult<Option<Vec<Flow>>> {
    if let [CircuitItem::Instruction(instruction)] = circuit.items() {
        return Ok(match instruction.gate().canonical_name() {
            "M" => simple_measurement_flows(instruction, PauliBasis::Z)?,
            "MX" => simple_measurement_flows(instruction, PauliBasis::X)?,
            "MY" => simple_measurement_flows(instruction, PauliBasis::Y)?,
            "R" => simple_reset_flows(instruction, PauliBasis::Z)?,
            "RX" => simple_reset_flows(instruction, PauliBasis::X)?,
            "RY" => simple_reset_flows(instruction, PauliBasis::Y)?,
            "MR" => simple_measure_reset_flows(instruction, PauliBasis::Z)?,
            "MRX" => simple_measure_reset_flows(instruction, PauliBasis::X)?,
            "MRY" => simple_measure_reset_flows(instruction, PauliBasis::Y)?,
            "MXX" => simple_pair_measurement_flows(instruction, PauliBasis::X)?,
            "MYY" => simple_pair_measurement_flows(instruction, PauliBasis::Y)?,
            "MZZ" => simple_pair_measurement_flows(instruction, PauliBasis::Z)?,
            "MPP" => simple_pauli_product_measurement_flows(instruction)?,
            "MPAD" => Some(measurement_pad_flows(instruction)?),
            _ => None,
        });
    }
    scoped_composed_measurement_flow_generators(circuit)
}

fn simple_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    validate_measurement_rich_flow_generator_rows(qubit_count, instruction.targets().len())?;
    let mut flows = Vec::new();
    let mut last_records_by_qubit = vec![None; qubit_count];
    for (record_index, target) in instruction.targets().iter().enumerate() {
        let Some(qubit) = plain_target_index(target) else {
            return Ok(None);
        };
        let Some(slot) = last_records_by_qubit.get_mut(qubit) else {
            return Ok(None);
        };
        if let Some(previous_record) = *slot {
            flows.push(record_equality_flow(previous_record, record_index)?);
        }
        *slot = Some(record_index);
    }

    for (qubit, record_index) in last_records_by_qubit.into_iter().enumerate() {
        if let Some(record_index) = record_index {
            flows.push(output_measurement_flow(
                qubit_count,
                qubit,
                basis,
                record_index,
            )?);
            flows.push(input_measurement_flow(
                qubit_count,
                qubit,
                basis,
                record_index,
            )?);
        }
    }
    Ok(Some(flows))
}

fn simple_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    validate_measurement_rich_flow_generator_rows(qubit_count, 0)?;
    let qubits = match unique_plain_target_indices(instruction) {
        Some(qubits) => qubits,
        None => return Ok(None),
    };
    Ok(Some(
        qubits
            .into_iter()
            .map(|qubit| reset_flow(qubit_count, qubit, basis))
            .collect(),
    ))
}

fn simple_measure_reset_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let qubits = match unique_plain_target_indices(instruction) {
        Some(qubits) => qubits,
        None => return Ok(None),
    };
    validate_measurement_rich_flow_generator_rows(qubit_count, qubits.len())?;
    let mut flows = Vec::with_capacity(instruction.targets().len() * 2);
    for &qubit in &qubits {
        flows.push(reset_flow(qubit_count, qubit, basis));
    }
    for (record_index, qubit) in qubits.into_iter().enumerate() {
        flows.push(input_measurement_flow(
            qubit_count,
            qubit,
            basis,
            record_index,
        )?);
    }
    Ok(Some(flows))
}

fn simple_pair_measurement_flows(
    instruction: &CircuitInstruction,
    basis: PauliBasis,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let groups = instruction.target_groups();
    validate_measurement_rich_flow_generator_rows(qubit_count, groups.len())?;
    let mut measured_pairs = Vec::with_capacity(groups.len());
    for (record_index, group) in groups.iter().enumerate() {
        let [left, right] = *group else {
            return Ok(None);
        };
        measured_pairs.push((
            pair_measurement_target_index(left)?,
            pair_measurement_target_index(right)?,
            record_index_i32(record_index)?,
        ));
    }

    let mut flows = identity_flow_rows(qubit_count);
    for &((left, left_inverted), (right, right_inverted), record_index) in
        measured_pairs.iter().rev()
    {
        remove_pair_anticommutations(&mut flows, left, right, basis)?;
        flows.push(Flow::new(
            pair_pauli(
                qubit_count,
                left,
                right,
                basis,
                if left_inverted ^ right_inverted {
                    PauliSign::Minus
                } else {
                    PauliSign::Plus
                },
            ),
            PauliString::identity(qubit_count),
            [record_index],
            [],
        ));
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_pairs.len())?;
    Ok(Some(flows))
}

fn simple_pauli_product_measurement_flows(
    instruction: &CircuitInstruction,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = instruction_qubit_count(instruction);
    let mut measured_products = Vec::with_capacity(instruction.target_groups().len());
    for (record_index, group) in instruction.target_groups().into_iter().enumerate() {
        let product =
            measured_pauli_product(instruction.gate().canonical_name(), qubit_count, group)?;
        measured_products.push((product, record_index_i32(record_index)?));
    }
    validate_measurement_rich_flow_generator_rows(qubit_count, measured_products.len())?;

    let mut flows = identity_flow_rows(qubit_count);
    for (product, record_index) in measured_products.iter().rev() {
        add_pauli_product_measurement_flow(&mut flows, product, *record_index, qubit_count)?;
    }
    final_canonicalize_measurement_generators(&mut flows, qubit_count, measured_products.len())?;
    Ok(Some(flows))
}

fn measurement_pad_flows(instruction: &CircuitInstruction) -> CircuitResult<Vec<Flow>> {
    validate_measurement_rich_flow_generator_rows(0, instruction.targets().len())?;
    let mut positive_records = Vec::new();
    let mut negative_records = Vec::new();
    for (record_index, target) in instruction.targets().iter().enumerate() {
        match target.qubit_id().map(|id| id.get()) {
            Some(0) => positive_records.push(record_index),
            Some(1) => negative_records.push(record_index),
            _ => {
                return Err(CircuitError::invalid_tableau_conversion(format!(
                    "MPAD flow generator has invalid pad target {target}"
                )));
            }
        }
    }
    let mut flows = Vec::with_capacity(positive_records.len() + negative_records.len());
    for record in positive_records {
        flows.push(positive_record_flow(record)?);
    }
    for record in negative_records {
        flows.push(negative_record_flow(record)?);
    }
    Ok(flows)
}

fn scoped_composed_measurement_flow_generators(
    circuit: &Circuit,
) -> CircuitResult<Option<Vec<Flow>>> {
    let qubit_count = circuit.count_qubits();
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "circuit measurement count does not fit usize during flow generation",
        )
    })?;
    validate_measurement_rich_flow_generator_rows(qubit_count, measurement_count)?;
    let instructions = flattened_measurement_generator_instructions(circuit)?;
    if !instructions.iter().any(|instruction| {
        instruction.gate().produces_measurements()
            || matches!(
                instruction.gate().category(),
                GateCategory::Collapsing | GateCategory::PairMeasurement
            )
    }) {
        return Ok(None);
    }

    let mut solver = MeasurementFeedbackFlowSolver::new(qubit_count, measurement_count);
    for instruction in instructions.iter().rev() {
        if !solver.undo_instruction(instruction)? {
            return Ok(None);
        }
    }
    solver.finalize().map(Some)
}

fn validate_measurement_rich_flow_generator_rows(
    qubit_count: usize,
    measurement_count: usize,
) -> CircuitResult<()> {
    let rows = qubit_count
        .checked_mul(2)
        .and_then(|rows| rows.checked_add(measurement_count))
        .ok_or_else(|| {
            CircuitError::invalid_domain_value("measurement-rich flow-generator rows", "overflowed")
        })?;
    if rows > MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS {
        return Err(CircuitError::invalid_domain_value(
            "measurement-rich flow-generator rows",
            format!("{rows} exceeds current limit {MAX_MEASUREMENT_RICH_FLOW_GENERATOR_ROWS}"),
        ));
    }
    Ok(())
}

fn flattened_measurement_generator_instructions(
    circuit: &Circuit,
) -> CircuitResult<Vec<CircuitInstruction>> {
    if circuit
        .items()
        .iter()
        .any(|item| matches!(item, CircuitItem::RepeatBlock(_)))
    {
        circuit.flattened_operations()
    } else {
        Ok(circuit
            .items()
            .iter()
            .filter_map(|item| match item {
                CircuitItem::Instruction(instruction) => Some(instruction.clone()),
                CircuitItem::RepeatBlock(_) => None,
            })
            .collect())
    }
}

struct MeasurementFeedbackFlowSolver {
    flows: Vec<Flow>,
    qubit_count: usize,
    measurement_count: usize,
    measurements_in_past: usize,
}

impl MeasurementFeedbackFlowSolver {
    fn new(qubit_count: usize, measurement_count: usize) -> Self {
        Self {
            flows: identity_flow_rows(qubit_count),
            qubit_count,
            measurement_count,
            measurements_in_past: measurement_count,
        }
    }

    fn undo_instruction(&mut self, instruction: &CircuitInstruction) -> CircuitResult<bool> {
        match instruction.gate().canonical_name() {
            "M" => self.undo_measurement(instruction, PauliBasis::Z),
            "MX" => self.undo_measurement(instruction, PauliBasis::X),
            "MY" => self.undo_measurement(instruction, PauliBasis::Y),
            "R" => self.undo_reset(instruction, PauliBasis::Z),
            "RX" => self.undo_reset(instruction, PauliBasis::X),
            "RY" => self.undo_reset(instruction, PauliBasis::Y),
            "MR" => self.undo_measure_reset(instruction, PauliBasis::Z),
            "MRX" => self.undo_measure_reset(instruction, PauliBasis::X),
            "MRY" => self.undo_measure_reset(instruction, PauliBasis::Y),
            "MXX" => self.undo_pair_measurement(instruction, PauliBasis::X),
            "MYY" => self.undo_pair_measurement(instruction, PauliBasis::Y),
            "MZZ" => self.undo_pair_measurement(instruction, PauliBasis::Z),
            "MPP" => self.undo_pauli_product_measurement(instruction),
            "MPAD" => self.undo_measurement_pad(instruction),
            "HERALDED_ERASE" | "HERALDED_PAULI_CHANNEL_1" => {
                self.undo_heralded_flow_records(instruction)
            }
            "TICK" => Ok(true),
            _ if feedback_measurement_basis(instruction).is_some() => {
                self.undo_measurement_feedback(instruction)
            }
            _ => Ok(false),
        }
    }

    fn undo_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let mut targets = Vec::with_capacity(instruction.targets().len());
        for target in instruction.targets() {
            targets.push(pair_measurement_target_index(target)?);
        }
        for (&(qubit, inverted), record_index) in targets.iter().rev().zip(
            measurement_indices_reversed(&mut self.measurements_in_past, targets.len())?,
        ) {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            self.flows.push(Flow::new(
                single_pauli_with_sign(
                    self.qubit_count,
                    qubit,
                    basis,
                    if inverted {
                        PauliSign::Minus
                    } else {
                        PauliSign::Plus
                    },
                ),
                PauliString::identity(self.qubit_count),
                [record_index],
                [],
            ));
        }
        Ok(true)
    }

    fn undo_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let qubits = match unique_plain_target_indices(instruction) {
            Some(qubits) => qubits,
            None => return Ok(false),
        };
        for qubit in qubits {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            clear_input_term(&mut self.flows, qubit)?;
        }
        Ok(true)
    }

    fn undo_measure_reset(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let qubits = match unique_plain_target_indices(instruction) {
            Some(qubits) => qubits,
            None => return Ok(false),
        };
        for (&qubit, record_index) in qubits.iter().rev().zip(measurement_indices_reversed(
            &mut self.measurements_in_past,
            qubits.len(),
        )?) {
            remove_single_anticommutations(&mut self.flows, qubit, basis)?;
            clear_input_term(&mut self.flows, qubit)?;
            self.flows.push(Flow::new(
                single_pauli(self.qubit_count, qubit, basis),
                PauliString::identity(self.qubit_count),
                [record_index],
                [],
            ));
        }
        Ok(true)
    }

    fn undo_pair_measurement(
        &mut self,
        instruction: &CircuitInstruction,
        basis: PauliBasis,
    ) -> CircuitResult<bool> {
        let groups = instruction.target_groups();
        let mut pairs = Vec::with_capacity(groups.len());
        for group in &groups {
            let [left, right] = *group else {
                return Ok(false);
            };
            pairs.push((
                pair_measurement_target_index(left)?,
                pair_measurement_target_index(right)?,
            ));
        }
        for (&((left, left_inverted), (right, right_inverted)), record_index) in
            pairs.iter().rev().zip(measurement_indices_reversed(
                &mut self.measurements_in_past,
                pairs.len(),
            )?)
        {
            remove_pair_anticommutations(&mut self.flows, left, right, basis)?;
            self.flows.push(Flow::new(
                pair_pauli(
                    self.qubit_count,
                    left,
                    right,
                    basis,
                    if left_inverted ^ right_inverted {
                        PauliSign::Minus
                    } else {
                        PauliSign::Plus
                    },
                ),
                PauliString::identity(self.qubit_count),
                [record_index],
                [],
            ));
        }
        Ok(true)
    }

    fn undo_pauli_product_measurement(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        let mut products = Vec::with_capacity(instruction.target_groups().len());
        for group in instruction.target_groups() {
            let product = measured_pauli_product(
                instruction.gate().canonical_name(),
                self.qubit_count,
                group,
            )?;
            products.push(product);
        }
        for (product, record_index) in products.iter().rev().zip(measurement_indices_reversed(
            &mut self.measurements_in_past,
            products.len(),
        )?) {
            add_pauli_product_measurement_flow(
                &mut self.flows,
                product,
                record_index,
                self.qubit_count,
            )?;
        }
        Ok(true)
    }

    fn undo_measurement_pad(&mut self, instruction: &CircuitInstruction) -> CircuitResult<bool> {
        for (target, record_index) in
            instruction
                .targets()
                .iter()
                .rev()
                .zip(measurement_indices_reversed(
                    &mut self.measurements_in_past,
                    instruction.targets().len(),
                )?)
        {
            match target.qubit_id().map(|id| id.get()) {
                Some(0) => self.flows.push(Flow::new(
                    PauliString::identity(self.qubit_count),
                    PauliString::identity(self.qubit_count),
                    [record_index],
                    [],
                )),
                Some(1) => self.flows.push(Flow::new(
                    PauliString::identity(self.qubit_count),
                    PauliString::from_bases(
                        PauliSign::Minus,
                        vec![PauliBasis::I; self.qubit_count],
                    ),
                    [record_index],
                    [],
                )),
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    fn undo_heralded_flow_records(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        for (target, record_index) in
            instruction
                .targets()
                .iter()
                .rev()
                .zip(measurement_indices_reversed(
                    &mut self.measurements_in_past,
                    instruction.targets().len(),
                )?)
        {
            if target.qubit_id().is_none() || target.is_inverted_result_target() {
                return Ok(false);
            }
            self.flows.push(Flow::new(
                PauliString::identity(self.qubit_count),
                PauliString::identity(self.qubit_count),
                [record_index],
                [],
            ));
        }
        Ok(true)
    }

    fn undo_measurement_feedback(
        &mut self,
        instruction: &CircuitInstruction,
    ) -> CircuitResult<bool> {
        let basis = feedback_measurement_basis(instruction)
            .ok_or_else(|| internal_flow_error("missing feedback basis"))?;
        for group in instruction.target_groups().into_iter().rev() {
            let [left, right] = group else {
                return Ok(false);
            };
            let feedback = match (
                left.measurement_record_offset(),
                right.measurement_record_offset(),
            ) {
                (Some(record), None) => Some((record.get(), right)),
                (None, Some(record)) => Some((record.get(), left)),
                _ => None,
            };
            let Some((record_offset, target)) = feedback else {
                return Ok(false);
            };
            let Some(qubit) = target.qubit_id().map(|qubit| qubit.get() as usize) else {
                return Ok(false);
            };
            let record_index = absolute_record_index(self.measurements_in_past, record_offset)?;
            for row in rows_matching(&self.flows, |flow| {
                anticommutes_with_single_measurement(flow.input(), qubit, basis)
            }) {
                let updated = flow_with_toggled_measurement(
                    self.flows
                        .get(row)
                        .ok_or_else(|| internal_flow_error("feedback row is out of bounds"))?,
                    record_index,
                );
                if let Some(slot) = self.flows.get_mut(row) {
                    *slot = updated;
                }
            }
        }
        Ok(true)
    }

    fn finalize(mut self) -> CircuitResult<Vec<Flow>> {
        final_canonicalize_measurement_generators(
            &mut self.flows,
            self.qubit_count,
            self.measurement_count,
        )?;
        Ok(self.flows)
    }
}

fn instruction_qubit_count(instruction: &CircuitInstruction) -> usize {
    instruction
        .targets()
        .iter()
        .filter_map(Target::qubit_id)
        .map(|qubit| qubit.get() as usize + 1)
        .max()
        .unwrap_or(0)
}

fn plain_target_index(target: &Target) -> Option<usize> {
    if target.is_inverted_result_target() {
        return None;
    }
    target.qubit_id().map(|qubit| qubit.get() as usize)
}

fn unique_plain_target_indices(instruction: &CircuitInstruction) -> Option<Vec<usize>> {
    let mut qubits = Vec::with_capacity(instruction.targets().len());
    for target in instruction.targets() {
        let qubit = plain_target_index(target)?;
        if qubits.contains(&qubit) {
            return None;
        }
        qubits.push(qubit);
    }
    Some(qubits)
}

fn measurement_indices_reversed(
    measurements_in_past: &mut usize,
    count: usize,
) -> CircuitResult<Vec<i32>> {
    let mut indices = Vec::with_capacity(count);
    for _ in 0..count {
        *measurements_in_past = measurements_in_past.checked_sub(1).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "measurement count underflowed during flow generation",
            )
        })?;
        indices.push(record_index_i32(*measurements_in_past)?);
    }
    Ok(indices)
}

fn pair_measurement_target_index(target: &Target) -> CircuitResult<(usize, bool)> {
    let qubit = target.qubit_id().ok_or_else(|| {
        CircuitError::invalid_tableau_conversion(format!(
            "pair-measurement flow generator target {target} does not identify a qubit"
        ))
    })?;
    Ok((qubit.get() as usize, target.is_inverted_result_target()))
}

fn measured_pauli_product(
    gate_name: &'static str,
    qubit_count: usize,
    targets: &[Target],
) -> CircuitResult<PauliString> {
    let mut product = PauliString::identity(qubit_count);
    for target in targets {
        if target.is_combiner() {
            continue;
        }
        let qubit = target.qubit_id().ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} does not identify a qubit"
            ))
        })?;
        let pauli = target.pauli_type().ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator target {target} is not a Pauli target"
            ))
        })?;
        let term = single_pauli_with_sign(
            qubit_count,
            qubit.get() as usize,
            pauli_basis(pauli),
            if target.is_inverted_result_target() {
                PauliSign::Minus
            } else {
                PauliSign::Plus
            },
        );
        product = product.multiply_real(&term).map_err(|error| {
            CircuitError::invalid_tableau_conversion(format!(
                "{gate_name} flow generator Pauli product is not Hermitian: {error}"
            ))
        })?;
    }
    Ok(product)
}

fn identity_flow_rows(qubit_count: usize) -> Vec<Flow> {
    let mut flows = Vec::with_capacity(qubit_count.saturating_mul(2));
    for qubit in 0..qubit_count {
        flows.push(Flow::new(
            single_pauli(qubit_count, qubit, PauliBasis::X),
            single_pauli(qubit_count, qubit, PauliBasis::X),
            [],
            [],
        ));
        flows.push(Flow::new(
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            single_pauli(qubit_count, qubit, PauliBasis::Z),
            [],
            [],
        ));
    }
    flows
}

fn remove_single_anticommutations(
    flows: &mut Vec<Flow>,
    qubit: usize,
    basis: PauliBasis,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_single_measurement(flow.input(), qubit, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("single-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("single-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn remove_pair_anticommutations(
    flows: &mut Vec<Flow>,
    left: usize,
    right: usize,
    basis: PauliBasis,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pair_measurement(flow.input(), left, right, basis)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("pair-measurement pivot row is out of bounds"))?;
    for &row in rest {
        let target = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("pair-measurement target row is out of bounds"))?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn add_pauli_product_measurement_flow(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
    record_index: i32,
    qubit_count: usize,
) -> CircuitResult<()> {
    if measured_product.has_no_pauli_terms() {
        flows.push(Flow::new(
            PauliString::identity(qubit_count),
            PauliString::from_bases(measured_product.sign(), vec![PauliBasis::I; qubit_count]),
            [record_index],
            [],
        ));
        return Ok(());
    }
    remove_pauli_product_anticommutations(flows, measured_product)?;
    flows.push(Flow::new(
        measured_product.clone(),
        PauliString::identity(qubit_count),
        [record_index],
        [],
    ));
    Ok(())
}

fn remove_pauli_product_anticommutations(
    flows: &mut Vec<Flow>,
    measured_product: &PauliString,
) -> CircuitResult<()> {
    let anticommuting_rows = rows_matching(flows, |flow| {
        anticommutes_with_pauli_product(flow.input(), measured_product)
    });
    let Some((&pivot, rest)) = anticommuting_rows.split_first() else {
        return Ok(());
    };
    let pivot_flow = flows.get(pivot).cloned().ok_or_else(|| {
        internal_flow_error("Pauli-product measurement pivot row is out of bounds")
    })?;
    for &row in rest {
        let target = flows.get(row).ok_or_else(|| {
            internal_flow_error("Pauli-product measurement target row is out of bounds")
        })?;
        let multiplied = target
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.remove(pivot);
    Ok(())
}

fn anticommutes_with_pauli_product(input: &PauliString, measured_product: &PauliString) -> bool {
    (0..input.len().max(measured_product.len())).fold(false, |anticommutes, qubit| {
        let input_basis = input.get(qubit).unwrap_or(PauliBasis::I);
        let measured_basis = measured_product.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes
            ^ (input_basis.x_bit() & measured_basis.z_bit())
            ^ (input_basis.z_bit() & measured_basis.x_bit())
    })
}

fn clear_input_term(flows: &mut [Flow], qubit: usize) -> CircuitResult<()> {
    for flow in flows {
        let mut input = flow.input().clone();
        if qubit < input.len() {
            input
                .set(qubit, PauliBasis::I)
                .map_err(stabilizer_to_circuit_error)?;
        }
        *flow = Flow::new(
            input,
            flow.output().clone(),
            flow.measurements(),
            flow.observables(),
        );
    }
    Ok(())
}

fn final_canonicalize_measurement_generators(
    flows: &mut [Flow],
    qubit_count: usize,
    measurement_count: usize,
) -> CircuitResult<()> {
    let mut eliminated = 0;
    for qubit in 0..qubit_count {
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.input().get(qubit).is_some_and(|basis| basis.x_bit())
        })?;
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.input().get(qubit).is_some_and(|basis| basis.z_bit())
        })?;
    }
    for qubit in 0..qubit_count {
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.output().get(qubit).is_some_and(|basis| basis.x_bit())
        })?;
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.output().get(qubit).is_some_and(|basis| basis.z_bit())
        })?;
    }
    for measurement in 0..measurement_count {
        let measurement = record_index_i32(measurement)?;
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.measurements().any(|record| record == measurement)
        })?;
    }

    for flow in flows.iter_mut() {
        *flow = flow_with_final_sign_and_trimmed_identities(flow);
    }
    flows.sort();
    Ok(())
}

fn eliminate_rows_with(
    flows: &mut [Flow],
    eliminated: &mut usize,
    predicate: impl Fn(&Flow) -> bool,
) -> CircuitResult<()> {
    let matching_rows = rows_matching(flows, predicate);
    let Some(pivot) = matching_rows
        .iter()
        .copied()
        .find(|&row| row >= *eliminated && row < flows.len())
    else {
        return Ok(());
    };
    let pivot_flow = flows
        .get(pivot)
        .cloned()
        .ok_or_else(|| internal_flow_error("canonical pivot row is out of bounds"))?;
    for row in matching_rows {
        if row == pivot {
            continue;
        }
        let multiplied = flows
            .get(row)
            .ok_or_else(|| internal_flow_error("canonical target row is out of bounds"))?
            .multiply(&pivot_flow)
            .map_err(stabilizer_to_circuit_error)?;
        if let Some(slot) = flows.get_mut(row) {
            *slot = multiplied;
        }
    }
    flows.swap(pivot, *eliminated);
    *eliminated += 1;
    Ok(())
}

fn rows_matching(flows: &[Flow], predicate: impl Fn(&Flow) -> bool) -> Vec<usize> {
    flows
        .iter()
        .enumerate()
        .filter_map(|(index, flow)| predicate(flow).then_some(index))
        .collect()
}

fn anticommutes_with_pair_measurement(
    input: &PauliString,
    left: usize,
    right: usize,
    measured_basis: PauliBasis,
) -> bool {
    let x = measured_basis.x_bit();
    let z = measured_basis.z_bit();
    let mut anticommutes = false;
    for qubit in [left, right] {
        let basis = input.get(qubit).unwrap_or(PauliBasis::I);
        anticommutes ^= basis.x_bit() & z;
        anticommutes ^= basis.z_bit() & x;
    }
    anticommutes
}

fn anticommutes_with_single_measurement(
    input: &PauliString,
    qubit: usize,
    measured_basis: PauliBasis,
) -> bool {
    let basis = input.get(qubit).unwrap_or(PauliBasis::I);
    (basis.x_bit() & measured_basis.z_bit()) ^ (basis.z_bit() & measured_basis.x_bit())
}

fn pair_pauli(
    qubit_count: usize,
    left: usize,
    right: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    PauliString::from_bases(
        sign,
        (0..qubit_count).map(|qubit| {
            if qubit == left || qubit == right {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn single_pauli_with_sign(
    len: usize,
    index: usize,
    basis: PauliBasis,
    sign: PauliSign,
) -> PauliString {
    PauliString::from_bases(
        sign,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn feedback_measurement_basis(instruction: &CircuitInstruction) -> Option<PauliBasis> {
    match instruction.gate().canonical_name() {
        "CX" | "XCZ" => Some(PauliBasis::X),
        "CY" | "YCZ" => Some(PauliBasis::Y),
        "CZ" => Some(PauliBasis::Z),
        _ => None,
    }
}

fn absolute_record_index(measurements_in_past: usize, record_offset: i32) -> CircuitResult<i32> {
    let measurements_in_past = i64::try_from(measurements_in_past).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "measurement count does not fit i64 during feedback flow generation",
        )
    })?;
    let index = measurements_in_past
        .checked_add(i64::from(record_offset))
        .ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "measurement record index overflowed during feedback flow generation",
            )
        })?;
    if index < 0 || index >= measurements_in_past {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "measurement record offset {record_offset} is outside the flow generation history"
        )));
    }
    i32::try_from(index).map_err(|_| {
        CircuitError::invalid_tableau_conversion(format!(
            "flow measurement record index {index} does not fit i32"
        ))
    })
}

fn flow_with_toggled_measurement(flow: &Flow, record_index: i32) -> Flow {
    Flow::new(
        flow.input().clone(),
        flow.output().clone(),
        flow.measurements().chain([record_index]),
        flow.observables(),
    )
}

fn flow_with_final_sign_and_trimmed_identities(flow: &Flow) -> Flow {
    let output_sign = xor_sign(flow.output().sign(), flow.input().sign());
    Flow::new(
        trimmed_pauli_with_sign(flow.input(), PauliSign::Plus),
        trimmed_pauli_with_sign(flow.output(), output_sign),
        flow.measurements(),
        flow.observables(),
    )
}

fn trimmed_pauli_with_sign(pauli: &PauliString, sign: PauliSign) -> PauliString {
    if pauli.has_no_pauli_terms() {
        PauliString::from_bases(sign, [])
    } else {
        pauli.with_sign(sign)
    }
}

fn xor_sign(left: PauliSign, right: PauliSign) -> PauliSign {
    if left.is_negative() ^ right.is_negative() {
        PauliSign::Minus
    } else {
        PauliSign::Plus
    }
}

fn record_equality_flow(left_record: usize, right_record: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::identity(0),
        [
            record_index_i32(left_record)?,
            record_index_i32(right_record)?,
        ],
        [],
    ))
}

fn output_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        single_pauli(qubit_count, qubit, basis),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn input_measurement_flow(
    qubit_count: usize,
    qubit: usize,
    basis: PauliBasis,
    record_index: usize,
) -> CircuitResult<Flow> {
    Ok(Flow::new(
        single_pauli(qubit_count, qubit, basis),
        PauliString::identity(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn reset_flow(qubit_count: usize, qubit: usize, basis: PauliBasis) -> Flow {
    Flow::new(
        PauliString::identity(0),
        single_pauli(qubit_count, qubit, basis),
        [],
        [],
    )
}

fn positive_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::identity(0),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn negative_record_flow(record_index: usize) -> CircuitResult<Flow> {
    Ok(Flow::new(
        PauliString::identity(0),
        PauliString::from_bases(PauliSign::Minus, []),
        [record_index_i32(record_index)?],
        [],
    ))
}

fn pauli_basis(pauli: Pauli) -> PauliBasis {
    match pauli {
        Pauli::X => PauliBasis::X,
        Pauli::Y => PauliBasis::Y,
        Pauli::Z => PauliBasis::Z,
    }
}

fn record_index_i32(record_index: usize) -> CircuitResult<i32> {
    i32::try_from(record_index).map_err(|_| {
        CircuitError::invalid_tableau_conversion(format!(
            "flow measurement record index {record_index} does not fit i32"
        ))
    })
}

fn unsupported_flow_generator_error(circuit: &Circuit) -> CircuitError {
    CircuitError::invalid_tableau_conversion(format!(
        "circuit_flow_generators only supports unitary tableau circuits, supported measurement/reset/pair-measurement/MPP/MPAD circuits, scoped composed measurement-rich circuits, scoped measurement-record feedback circuits, and scoped heralded-noise record circuits; got {} top-level item(s)",
        circuit.items().len()
    ))
}

pub(super) fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn stabilizer_to_circuit_error(error: crate::StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}

fn internal_flow_error(message: &'static str) -> CircuitError {
    CircuitError::invalid_tableau_conversion(message)
}
