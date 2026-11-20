//! Repetition-code memory circuits.

use crate::AnalysisResult;
use stab_model::{Circuit, GateKind};

use super::{
    CodeCircuitParams, GeneratedCircuit, append_begin_round_tick, append_circuit,
    append_instruction, append_measure, append_measure_reset, append_repeated_body, append_reset,
    append_unitary_2, rec_target,
};

/// Generates Stim's `repetition_code` `memory` circuit; the caller has admitted its size.
pub(super) fn generate(params: &CodeCircuitParams) -> AnalysisResult<GeneratedCircuit> {
    let measurement_count = params.distance.get() - 1;
    let qubit_count = measurement_count
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| super::generation_invariant("code distance", params.distance.get()))?;

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
    append_reset(params, &mut full, &all_qubits, 'Z')?;
    append_circuit(&mut full, &cycle_actions);
    append_first_round_detectors(&mut full, measurement_count)?;

    let mut body = cycle_actions;
    append_instruction(
        &mut body,
        GateKind::SHIFT_COORDS,
        vec![0.0, 1.0],
        Vec::new(),
    )?;
    append_repeat_detectors(&mut body, measurement_count)?;
    append_repeated_body(&mut full, body, params.rounds.get().saturating_sub(1))?;

    append_measure(params, &mut full, &data_qubits, 'Z')?;
    append_tail_detectors(&mut full, measurement_count)?;
    append_instruction(
        &mut full,
        GateKind::OBSERVABLE_INCLUDE,
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
    params: &CodeCircuitParams,
    data_qubits: &[u32],
    cnot_targets_1: &[u32],
    cnot_targets_2: &[u32],
    measurement_qubits: &[u32],
) -> AnalysisResult<Circuit> {
    let mut circuit = Circuit::new();
    append_begin_round_tick(params, &mut circuit, data_qubits)?;
    append_unitary_2(params, &mut circuit, GateKind::CX, cnot_targets_1)?;
    append_instruction(&mut circuit, GateKind::TICK, Vec::new(), Vec::new())?;
    append_unitary_2(params, &mut circuit, GateKind::CX, cnot_targets_2)?;
    append_instruction(&mut circuit, GateKind::TICK, Vec::new(), Vec::new())?;
    append_measure_reset(params, &mut circuit, measurement_qubits, 'Z')?;
    Ok(circuit)
}

fn append_first_round_detectors(
    circuit: &mut Circuit,
    measurement_count: u32,
) -> AnalysisResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            GateKind::DETECTOR,
            vec![f64::from(2 * detector + 1), 0.0],
            vec![rec_target(rec)?],
        )?;
    }
    Ok(())
}

fn append_repeat_detectors(circuit: &mut Circuit, measurement_count: u32) -> AnalysisResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            GateKind::DETECTOR,
            vec![f64::from(2 * detector + 1), 0.0],
            vec![
                rec_target(rec)?,
                rec_target(2 * measurement_count - detector)?,
            ],
        )?;
    }
    Ok(())
}

fn append_tail_detectors(circuit: &mut Circuit, measurement_count: u32) -> AnalysisResult<()> {
    for detector in 0..measurement_count {
        let rec = measurement_count - detector;
        append_instruction(
            circuit,
            GateKind::DETECTOR,
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
