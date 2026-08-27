use stab_algebra::{StabilizerResource, Tableau};
use stab_model::{Circuit, CircuitInstruction, CircuitItem, Gate, GateCategory, QubitId, Target};

use crate::{AnalysisError, AnalysisResult};

/// Converts a circuit after checking its dense Tableau width before materialization.
pub fn circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
) -> AnalysisResult<Tableau> {
    let num_qubits = stab_model::advanced::circuit_simulated_qubit_count(circuit);
    StabilizerResource::TableauQubits
        .ensure(num_qubits)
        .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))?;
    let mut result = stab_algebra::advanced::tableau_identity_unchecked(num_qubits);
    let mut repeat_work = TableauRepeatWork::default();
    apply_circuit_to_tableau(
        circuit,
        ignore_noise,
        ignore_measurement,
        ignore_reset,
        &mut repeat_work,
        &mut result,
    )?;
    Ok(result)
}

fn apply_circuit_to_tableau(
    circuit: &Circuit,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
    repeat_work: &mut TableauRepeatWork,
    result: &mut Tableau,
) -> AnalysisResult<()> {
    for item in circuit.items() {
        match item {
            CircuitItem::Instruction(instruction) => apply_instruction_to_tableau(
                instruction,
                ignore_noise,
                ignore_measurement,
                ignore_reset,
                repeat_work,
                result,
            )?,
            CircuitItem::RepeatBlock(repeat) => {
                let mut body = stab_algebra::advanced::tableau_identity_unchecked(result.len());
                apply_circuit_to_tableau(
                    repeat.body(),
                    ignore_noise,
                    ignore_measurement,
                    ignore_reset,
                    repeat_work,
                    &mut body,
                )?;
                let identity = stab_algebra::advanced::tableau_identity_unchecked(result.len());
                if body != identity {
                    let repeated = tableau_power(&body, repeat.repeat_count().get(), repeat_work)?;
                    if repeated != identity {
                        if *result == identity {
                            *result = repeated;
                        } else {
                            *result = compose_repeat_tableaus(result, &repeated, repeat_work)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn apply_instruction_to_tableau(
    instruction: &CircuitInstruction,
    ignore_noise: bool,
    ignore_measurement: bool,
    ignore_reset: bool,
    repeat_work: &mut TableauRepeatWork,
    result: &mut Tableau,
) -> AnalysisResult<()> {
    let gate = instruction.gate();

    if !ignore_measurement && gate.produces_measurements() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "measurement operation {}",
            gate.canonical_name()
        )));
    }
    if !ignore_reset && gate.is_reset() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "reset operation {}",
            gate.canonical_name()
        )));
    }
    if !ignore_noise && gate.is_noisy() && instruction.args().iter().any(|argument| *argument > 0.0)
    {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "noisy operation {}",
            gate.canonical_name()
        )));
    }

    match gate.category() {
        GateCategory::Annotation
        | GateCategory::Collapsing
        | GateCategory::ControlFlow
        | GateCategory::HeraldedNoise
        | GateCategory::Noise
        | GateCategory::PairMeasurement => Ok(()),
        GateCategory::Controlled
        | GateCategory::HadamardLike
        | GateCategory::Pauli
        | GateCategory::Period3
        | GateCategory::Period4
        | GateCategory::ParityPhasing
        | GateCategory::Swap => {
            for group in instruction.target_groups() {
                apply_unitary_group_to_tableau(gate.canonical_name(), group, result)?;
            }
            Ok(())
        }
        GateCategory::PauliProduct if !gate.is_unitary() => Ok(()),
        GateCategory::PauliProduct => {
            let decomposed = crate::advanced::decomposed_single_instruction(instruction)
                .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))?;
            apply_circuit_to_tableau(
                &decomposed,
                ignore_noise,
                ignore_measurement,
                ignore_reset,
                repeat_work,
                result,
            )
        }
    }
}

fn tableau_power(
    base: &Tableau,
    mut exponent: u64,
    repeat_work: &mut TableauRepeatWork,
) -> AnalysisResult<Tableau> {
    let identity = stab_algebra::advanced::tableau_identity_unchecked(base.len());
    if exponent == 0 || *base == identity {
        return Ok(identity);
    }
    let mut result = identity.clone();
    let mut power = base.clone();
    while exponent > 0 {
        if exponent & 1 == 1 && power != identity {
            result = if result == identity {
                power.clone()
            } else {
                compose_repeat_tableaus(&result, &power, repeat_work)?
            };
        }
        exponent >>= 1;
        if exponent == 0 || power == identity {
            break;
        }
        power = compose_repeat_tableaus(&power, &power, repeat_work)?;
        if power == identity {
            break;
        }
    }
    Ok(result)
}

#[derive(Default)]
struct TableauRepeatWork {
    consumed: usize,
}

impl TableauRepeatWork {
    fn charge_composition(&mut self, width: usize) -> AnalysisResult<()> {
        let width = width.max(1);
        let cost = width.saturating_mul(width);
        let requested = self.consumed.saturating_add(cost);
        StabilizerResource::CircuitTableauRepeatWork
            .ensure(requested)
            .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))?;
        self.consumed = requested;
        Ok(())
    }
}

fn compose_repeat_tableaus(
    first: &Tableau,
    second: &Tableau,
    repeat_work: &mut TableauRepeatWork,
) -> AnalysisResult<Tableau> {
    repeat_work.charge_composition(first.len())?;
    compose_tableaus(first, second)
}

fn compose_tableaus(first: &Tableau, second: &Tableau) -> AnalysisResult<Tableau> {
    first
        .then(second)
        .map_err(|error| AnalysisError::invalid_tableau_conversion(error.to_string()))
}

fn apply_unitary_group_to_tableau(
    gate_name: &str,
    targets: &[Target],
    result: &mut Tableau,
) -> AnalysisResult<()> {
    let target_ids = target_qubit_ids(gate_name, targets)?;
    let local = gate_tableau_for_name(gate_name)?;
    if local.len() != target_ids.len() {
        return Err(AnalysisError::invalid_tableau_conversion(format!(
            "gate {gate_name} expected {} tableau targets but got {}",
            local.len(),
            target_ids.len()
        )));
    }
    let targets = target_ids
        .iter()
        .map(|target| target.get() as usize)
        .collect::<Vec<_>>();
    result.append(&local, &targets).map_err(map_tableau_error)?;
    Ok(())
}

fn target_qubit_ids(gate_name: &str, targets: &[Target]) -> AnalysisResult<Vec<QubitId>> {
    targets
        .iter()
        .map(|target| {
            target.qubit_id().ok_or_else(|| {
                AnalysisError::invalid_tableau_conversion(format!(
                    "gate {gate_name} has non-qubit tableau target {target}"
                ))
            })
        })
        .collect()
}

fn gate_tableau_for_name(gate_name: &str) -> AnalysisResult<Tableau> {
    let gate = Gate::from_name(gate_name).map_err(AnalysisError::from)?;
    crate::gate_tableau(gate)
}

fn map_tableau_error(error: stab_algebra::StabilizerError) -> AnalysisError {
    AnalysisError::invalid_tableau_conversion(error.to_string())
}
