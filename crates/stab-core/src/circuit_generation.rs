use crate::{
    Circuit, CircuitError, CircuitInstruction, CircuitResult, Gate, MeasureRecordOffset,
    Probability, QubitId, RepeatBlock, RepeatCount, Target,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CodeDistance(u32);

impl CodeDistance {
    pub fn try_new(value: u32) -> CircuitResult<Self> {
        if !(2..=2047).contains(&value) {
            return Err(CircuitError::invalid_domain_value("code distance", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RoundCount(u64);

impl RoundCount {
    pub fn try_new(value: u64) -> CircuitResult<Self> {
        if value == 0 {
            return Err(CircuitError::invalid_domain_value("round count", value));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepetitionCodeTask {
    Memory,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RepetitionCodeParams {
    rounds: RoundCount,
    distance: CodeDistance,
    task: RepetitionCodeTask,
    before_round_data_depolarization: Probability,
    before_measure_flip_probability: Probability,
    after_reset_flip_probability: Probability,
    after_clifford_depolarization: Probability,
}

impl RepetitionCodeParams {
    pub fn new(
        rounds: RoundCount,
        distance: CodeDistance,
        task: RepetitionCodeTask,
    ) -> CircuitResult<Self> {
        Ok(Self {
            rounds,
            distance,
            task,
            before_round_data_depolarization: Probability::try_new(0.0)?,
            before_measure_flip_probability: Probability::try_new(0.0)?,
            after_reset_flip_probability: Probability::try_new(0.0)?,
            after_clifford_depolarization: Probability::try_new(0.0)?,
        })
    }

    pub fn rounds(&self) -> RoundCount {
        self.rounds
    }

    pub fn distance(&self) -> CodeDistance {
        self.distance
    }

    pub fn task(&self) -> RepetitionCodeTask {
        self.task
    }

    pub fn before_round_data_depolarization(&self) -> Probability {
        self.before_round_data_depolarization
    }

    pub fn before_measure_flip_probability(&self) -> Probability {
        self.before_measure_flip_probability
    }

    pub fn after_reset_flip_probability(&self) -> Probability {
        self.after_reset_flip_probability
    }

    pub fn after_clifford_depolarization(&self) -> Probability {
        self.after_clifford_depolarization
    }

    pub fn with_before_round_data_depolarization(mut self, value: Probability) -> Self {
        self.before_round_data_depolarization = value;
        self
    }

    pub fn with_before_measure_flip_probability(mut self, value: Probability) -> Self {
        self.before_measure_flip_probability = value;
        self
    }

    pub fn with_after_reset_flip_probability(mut self, value: Probability) -> Self {
        self.after_reset_flip_probability = value;
        self
    }

    pub fn with_after_clifford_depolarization(mut self, value: Probability) -> Self {
        self.after_clifford_depolarization = value;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedCircuit {
    circuit: Circuit,
    layout_text: String,
    hint_text: &'static str,
}

impl GeneratedCircuit {
    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    pub fn layout_text(&self) -> &str {
        &self.layout_text
    }

    pub fn hint_text(&self) -> &'static str {
        self.hint_text
    }
}

/// Generates Stim-compatible repetition-code memory circuits for the M7 generator subset.
pub fn generate_repetition_code_circuit(
    params: &RepetitionCodeParams,
) -> CircuitResult<GeneratedCircuit> {
    let RepetitionCodeTask::Memory = params.task;
    let measurement_count = params.distance.get() - 1;
    let qubit_count = measurement_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| {
            CircuitError::invalid_domain_value("code distance", params.distance.get())
        })?;

    let all_qubits = (0..qubit_count).collect::<Vec<_>>();
    let data_qubits = all_qubits
        .iter()
        .copied()
        .filter(|qubit| qubit % 2 == 0)
        .collect::<Vec<_>>();
    let measurement_qubits = all_qubits
        .iter()
        .copied()
        .filter(|qubit| qubit % 2 == 1)
        .collect::<Vec<_>>();
    let cnot_targets_1 = measurement_qubits
        .iter()
        .flat_map(|qubit| [qubit - 1, *qubit])
        .collect::<Vec<_>>();
    let cnot_targets_2 = measurement_qubits
        .iter()
        .flat_map(|qubit| [qubit + 1, *qubit])
        .collect::<Vec<_>>();

    let cycle_actions = repetition_cycle(
        params,
        &data_qubits,
        &cnot_targets_1,
        &cnot_targets_2,
        &measurement_qubits,
    )?;

    let mut full = Circuit::new();
    append_reset(params, &mut full, &all_qubits)?;
    append_circuit(&mut full, &cycle_actions);
    append_first_round_detectors(&mut full, measurement_count)?;

    let mut body = cycle_actions;
    append_instruction(&mut body, "SHIFT_COORDS", vec![0.0, 1.0], Vec::new())?;
    append_repeat_detectors(&mut body, measurement_count)?;
    append_repeated_body(&mut full, body, params.rounds.get().saturating_sub(1))?;

    append_measure(params, &mut full, &data_qubits)?;
    append_tail_detectors(&mut full, measurement_count)?;
    append_instruction(
        &mut full,
        "OBSERVABLE_INCLUDE",
        vec![0.0],
        vec![rec_target(1)?],
    )?;

    Ok(GeneratedCircuit {
        circuit: full,
        layout_text: repetition_layout(qubit_count),
        hint_text: "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     Z# = measurement qubit\n",
    })
}

fn repetition_cycle(
    params: &RepetitionCodeParams,
    data_qubits: &[u32],
    cnot_targets_1: &[u32],
    cnot_targets_2: &[u32],
    measurement_qubits: &[u32],
) -> CircuitResult<Circuit> {
    let mut circuit = Circuit::new();
    append_begin_round_tick(params, &mut circuit, data_qubits)?;
    append_unitary_2(params, &mut circuit, "CX", cnot_targets_1)?;
    append_instruction(&mut circuit, "TICK", Vec::new(), Vec::new())?;
    append_unitary_2(params, &mut circuit, "CX", cnot_targets_2)?;
    append_instruction(&mut circuit, "TICK", Vec::new(), Vec::new())?;
    append_measure_reset(params, &mut circuit, measurement_qubits)?;
    Ok(circuit)
}

fn append_begin_round_tick(
    params: &RepetitionCodeParams,
    circuit: &mut Circuit,
    data_qubits: &[u32],
) -> CircuitResult<()> {
    append_instruction(circuit, "TICK", Vec::new(), Vec::new())?;
    append_probability_instruction(
        circuit,
        "DEPOLARIZE1",
        data_qubits,
        params.before_round_data_depolarization,
    )
}

fn append_unitary_2(
    params: &RepetitionCodeParams,
    circuit: &mut Circuit,
    gate: &'static str,
    targets: &[u32],
) -> CircuitResult<()> {
    append_instruction(circuit, gate, Vec::new(), qubit_targets(targets)?)?;
    append_probability_instruction(
        circuit,
        "DEPOLARIZE2",
        targets,
        params.after_clifford_depolarization,
    )
}

fn append_reset(
    params: &RepetitionCodeParams,
    circuit: &mut Circuit,
    targets: &[u32],
) -> CircuitResult<()> {
    append_instruction(circuit, "R", Vec::new(), qubit_targets(targets)?)?;
    append_probability_instruction(
        circuit,
        "X_ERROR",
        targets,
        params.after_reset_flip_probability,
    )
}

fn append_measure(
    params: &RepetitionCodeParams,
    circuit: &mut Circuit,
    targets: &[u32],
) -> CircuitResult<()> {
    append_probability_instruction(
        circuit,
        "X_ERROR",
        targets,
        params.before_measure_flip_probability,
    )?;
    append_instruction(circuit, "M", Vec::new(), qubit_targets(targets)?)
}

fn append_measure_reset(
    params: &RepetitionCodeParams,
    circuit: &mut Circuit,
    targets: &[u32],
) -> CircuitResult<()> {
    append_probability_instruction(
        circuit,
        "X_ERROR",
        targets,
        params.before_measure_flip_probability,
    )?;
    append_instruction(circuit, "MR", Vec::new(), qubit_targets(targets)?)?;
    append_probability_instruction(
        circuit,
        "X_ERROR",
        targets,
        params.after_reset_flip_probability,
    )
}

fn append_probability_instruction(
    circuit: &mut Circuit,
    gate: &'static str,
    targets: &[u32],
    probability: Probability,
) -> CircuitResult<()> {
    if probability.get() > 0.0 {
        append_instruction(
            circuit,
            gate,
            vec![probability.get()],
            qubit_targets(targets)?,
        )?;
    }
    Ok(())
}

fn append_first_round_detectors(
    circuit: &mut Circuit,
    measurement_count: u32,
) -> CircuitResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            "DETECTOR",
            vec![f64::from(2 * detector + 1), 0.0],
            vec![rec_target(rec)?],
        )?;
    }
    Ok(())
}

fn append_repeat_detectors(circuit: &mut Circuit, measurement_count: u32) -> CircuitResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            "DETECTOR",
            vec![f64::from(2 * detector + 1), 0.0],
            vec![
                rec_target(rec)?,
                rec_target(2 * measurement_count - detector)?,
            ],
        )?;
    }
    Ok(())
}

fn append_tail_detectors(circuit: &mut Circuit, measurement_count: u32) -> CircuitResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            "DETECTOR",
            vec![f64::from(2 * detector + 1), 1.0],
            vec![
                rec_target(rec)?,
                rec_target(rec + 1)?,
                rec_target(2 * measurement_count - detector + 1)?,
            ],
        )?;
    }
    Ok(())
}

fn append_repeated_body(
    target: &mut Circuit,
    body: Circuit,
    repetitions: u64,
) -> CircuitResult<()> {
    match repetitions {
        0 => Ok(()),
        1 => {
            append_circuit(target, &body);
            Ok(())
        }
        count => {
            target.append_repeat_block(RepeatBlock::new(RepeatCount::try_new(count)?, body, None));
            Ok(())
        }
    }
}

fn append_circuit(target: &mut Circuit, source: &Circuit) {
    for item in source.items() {
        match item {
            crate::CircuitItem::Instruction(instruction) => {
                target.append_instruction(instruction.clone());
            }
            crate::CircuitItem::RepeatBlock(repeat) => {
                target.append_repeat_block(repeat.clone());
            }
        }
    }
}

fn append_instruction(
    circuit: &mut Circuit,
    gate: &'static str,
    args: Vec<f64>,
    targets: Vec<Target>,
) -> CircuitResult<()> {
    let gate = Gate::from_name(gate)?;
    circuit.append_instruction(CircuitInstruction::new(gate, args, targets, None)?);
    Ok(())
}

fn qubit_targets(targets: &[u32]) -> CircuitResult<Vec<Target>> {
    targets
        .iter()
        .copied()
        .map(|target| QubitId::new(target).map(|id| Target::qubit(id, false)))
        .collect()
}

fn rec_target(offset: u32) -> CircuitResult<Target> {
    let offset = i32::try_from(offset)
        .map_err(|_| CircuitError::invalid_domain_value("measurement record offset", offset))?;
    Ok(Target::measurement_record(MeasureRecordOffset::try_new(
        -offset,
    )?))
}

fn repetition_layout(qubit_count: u32) -> String {
    let entries = (0..qubit_count)
        .map(|qubit| {
            let prefix = match qubit {
                0 => 'L',
                value if value % 2 == 0 => 'd',
                _ => 'Z',
            };
            format!("{prefix}{qubit}")
        })
        .collect::<Vec<_>>();
    let max_len = entries.iter().map(String::len).max().unwrap_or(0);
    let mut out = String::from("#");
    for entry in entries {
        out.push(' ');
        out.push_str(&entry);
        out.extend(std::iter::repeat_n(
            ' ',
            max_len.saturating_sub(entry.len()),
        ));
    }
    out.push('\n');
    out
}
