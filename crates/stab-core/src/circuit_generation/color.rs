#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "Color-code coordinates are bounded by CodeDistance and the generator indexes closed coordinate maps constructed in the same function."
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{Circuit, CircuitError, CircuitResult};

use super::{
    ColorCodeParams, ColorCodeTask, GeneratedCircuit, append_begin_round_tick, append_circuit,
    append_instruction, append_measure, append_measure_reset, append_repeated_body, append_reset,
    append_unitary_1, append_unitary_2, layout_text, qubit_targets, rec_targets,
};

/// Generates Stim-compatible triangular color-code memory circuits.
pub fn generate_color_code_circuit(params: &ColorCodeParams) -> CircuitResult<GeneratedCircuit> {
    let ColorCodeTask::MemoryXyz = params.task;
    if params.rounds().get() < 2 {
        return Err(CircuitError::invalid_domain_value(
            "color code round count",
            params.rounds().get(),
        ));
    }
    if params.distance().get() < 3 || params.distance().get().is_multiple_of(2) {
        return Err(CircuitError::invalid_domain_value(
            "color code distance",
            params.distance().get(),
        ));
    }

    let distance = params.distance().get();
    let width = distance + (distance - 1) / 2;
    let mut data_coords = BTreeSet::new();
    let mut measure_coords = BTreeSet::new();
    let mut p2q = BTreeMap::new();
    let mut data_qubits = Vec::new();
    let mut measurement_qubits = Vec::new();
    for y in 0..width {
        for x in 0..(width - y) {
            let coord = ColorCoord::new((2 * x + y) as i32, y as i32);
            let qubit = u32::try_from(p2q.len()).map_err(|_| {
                CircuitError::invalid_domain_value("color code qubit count", p2q.len())
            })?;
            p2q.insert(coord, qubit);
            if (x + 2 * y) % 3 == 2 {
                measure_coords.insert(coord);
                measurement_qubits.push(qubit);
            } else {
                data_coords.insert(coord);
                data_qubits.push(qubit);
            }
        }
    }

    let mut all_qubits = data_qubits.clone();
    all_qubits.extend_from_slice(&measurement_qubits);
    all_qubits.sort_unstable();
    data_qubits.sort_unstable();
    measurement_qubits.sort_unstable();

    let mut q2p = BTreeMap::new();
    for (coord, qubit) in &p2q {
        q2p.insert(*qubit, *coord);
    }
    let mut data_coord_to_order = BTreeMap::new();
    let mut measure_coord_to_order = BTreeMap::new();
    for qubit in &data_qubits {
        let order = u32::try_from(data_coord_to_order.len()).map_err(|_| {
            CircuitError::invalid_domain_value("data qubit count", data_qubits.len())
        })?;
        data_coord_to_order.insert(q2p[qubit], order);
    }
    for qubit in &measurement_qubits {
        let order = u32::try_from(measure_coord_to_order.len()).map_err(|_| {
            CircuitError::invalid_domain_value("measurement qubit count", measurement_qubits.len())
        })?;
        measure_coord_to_order.insert(q2p[qubit], order);
    }

    let deltas = [
        ColorCoord::new(2, 0),
        ColorCoord::new(1, 1),
        ColorCoord::new(1, -1),
        ColorCoord::new(-2, 0),
        ColorCoord::new(-1, 1),
        ColorCoord::new(-1, -1),
    ];
    let mut cnot_targets: [Vec<u32>; 6] = std::array::from_fn(|_| Vec::new());
    for (index, delta) in deltas.iter().enumerate() {
        for measure in &measure_coords {
            let data = measure.plus(*delta);
            if let Some(data_qubit) = p2q.get(&data) {
                cnot_targets[index].push(*data_qubit);
                cnot_targets[index].push(p2q[measure]);
            }
        }
    }

    let common = params.common();
    let mut cycle_actions = Circuit::new();
    append_begin_round_tick(common, &mut cycle_actions, &data_qubits)?;
    append_unitary_1(common, &mut cycle_actions, "C_XYZ", &data_qubits)?;
    for targets in &cnot_targets {
        append_instruction(&mut cycle_actions, "TICK", Vec::new(), Vec::new())?;
        append_unitary_2(common, &mut cycle_actions, "CX", targets)?;
    }
    append_instruction(&mut cycle_actions, "TICK", Vec::new(), Vec::new())?;
    append_measure_reset(common, &mut cycle_actions, &measurement_qubits, 'Z')?;

    let mut head = Circuit::new();
    for qubit in &all_qubits {
        append_instruction(
            &mut head,
            "QUBIT_COORDS",
            q2p[qubit].args(),
            qubit_targets(&[*qubit])?,
        )?;
    }
    append_reset(common, &mut head, &all_qubits, 'Z')?;
    append_repeated_body(&mut head, cycle_actions.clone(), 2)?;

    let measurement_count = u32::try_from(measurement_qubits.len()).map_err(|_| {
        CircuitError::invalid_domain_value("measurement qubit count", measurement_qubits.len())
    })?;
    for k in (0..measurement_count).rev() {
        let index = usize::try_from(measurement_count - k - 1)
            .map_err(|_| CircuitError::invalid_domain_value("measurement index", k))?;
        let coord = q2p[&measurement_qubits[index]];
        append_instruction(
            &mut head,
            "DETECTOR",
            coord.detector_args(0.0),
            rec_targets(&[k + 1, k + 1 + measurement_count])?,
        )?;
    }

    let mut body = cycle_actions;
    append_instruction(&mut body, "SHIFT_COORDS", vec![0.0, 0.0, 1.0], Vec::new())?;
    for k in (0..measurement_count).rev() {
        let index = usize::try_from(measurement_count - k - 1)
            .map_err(|_| CircuitError::invalid_domain_value("measurement index", k))?;
        let coord = q2p[&measurement_qubits[index]];
        append_instruction(
            &mut body,
            "DETECTOR",
            coord.detector_args(0.0),
            rec_targets(&[
                k + 1,
                k + 1 + measurement_count,
                k + 1 + 2 * measurement_count,
            ])?,
        )?;
    }

    let mut tail = Circuit::new();
    let tail_basis = match params.rounds().get() % 3 {
        0 => 'Z',
        1 => 'X',
        _ => 'Y',
    };
    append_measure(common, &mut tail, &data_qubits, tail_basis)?;
    let data_count = u32::try_from(data_qubits.len())
        .map_err(|_| CircuitError::invalid_domain_value("data qubit count", data_qubits.len()))?;
    for measurement_qubit in &measurement_qubits {
        let measure = q2p[measurement_qubit];
        let mut detectors = Vec::new();
        for delta in &deltas {
            let data = measure.plus(*delta);
            if p2q.contains_key(&data) {
                detectors.push(data_count - data_coord_to_order[&data]);
            }
        }
        let previous_measurement =
            data_count + measurement_count - measure_coord_to_order[&measure];
        match params.rounds().get() % 3 {
            0 => detectors.push(previous_measurement),
            1 => detectors.push(previous_measurement + measurement_count),
            _ => {
                detectors.push(previous_measurement);
                detectors.push(previous_measurement + measurement_count);
            }
        }
        detectors.sort_unstable();
        append_instruction(
            &mut tail,
            "DETECTOR",
            measure.detector_args(1.0),
            rec_targets(&detectors)?,
        )?;
    }
    let mut obs_inc = data_coords
        .iter()
        .filter(|coord| coord.y == 0)
        .map(|coord| data_count - data_coord_to_order[coord])
        .collect::<Vec<_>>();
    obs_inc.sort_unstable();
    append_instruction(
        &mut tail,
        "OBSERVABLE_INCLUDE",
        vec![0.0],
        rec_targets(&obs_inc)?,
    )?;

    let mut full = head;
    append_repeated_body(&mut full, body, params.rounds().get().saturating_sub(2))?;
    append_circuit(&mut full, &tail);

    let mut layout = BTreeMap::new();
    for coord in &data_coords {
        let marker = if coord.y == 0 { 'L' } else { 'd' };
        layout.insert((coord.x2 as u32, coord.y as u32), (marker, p2q[coord]));
    }
    let rgb = ['R', 'G', 'B'];
    for coord in &measure_coords {
        let index = usize::try_from((coord.x2 + coord.y).rem_euclid(3))
            .map_err(|_| CircuitError::invalid_domain_value("color code layout color", coord.x2))?;
        layout.insert((coord.x2 as u32, coord.y as u32), (rgb[index], p2q[coord]));
    }

    Ok(GeneratedCircuit {
        circuit: full,
        layout_text: layout_text(&layout),
        hint_text: "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     R# = measurement qubit (red hex)\n#     G# = measurement qubit (green hex)\n#     B# = measurement qubit (blue hex)\n",
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct ColorCoord {
    x2: i32,
    y: i32,
}

impl ColorCoord {
    fn new(x2: i32, y: i32) -> Self {
        Self { x2, y }
    }

    fn plus(self, other: Self) -> Self {
        Self::new(self.x2 + other.x2, self.y + other.y)
    }

    fn args(self) -> Vec<f64> {
        vec![f64::from(self.x2) / 2.0, f64::from(self.y)]
    }

    fn detector_args(self, t: f64) -> Vec<f64> {
        vec![f64::from(self.x2) / 2.0, f64::from(self.y), t]
    }
}
