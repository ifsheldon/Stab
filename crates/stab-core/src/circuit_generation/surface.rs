#![allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::indexing_slicing,
    reason = "Surface-code coordinates are bounded by CodeDistance and the generator indexes closed coordinate maps constructed in the same function."
)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{Circuit, CircuitError, CircuitResult};

use super::{
    CircuitGenParams, GeneratedCircuit, SurfaceCodeParams, SurfaceCodeTask,
    append_begin_round_tick, append_circuit, append_instruction, append_measure,
    append_measure_reset, append_repeated_body, append_reset, append_unitary_1, append_unitary_2,
    layout_text, qubit_targets, rec_target, rec_targets,
};

/// Generates Stim-compatible rotated and unrotated surface-code memory circuits.
pub fn generate_surface_code_circuit(
    params: &SurfaceCodeParams,
) -> CircuitResult<GeneratedCircuit> {
    match params.task {
        SurfaceCodeTask::RotatedMemoryX => generate_rotated_surface_code_circuit(params, true),
        SurfaceCodeTask::RotatedMemoryZ => generate_rotated_surface_code_circuit(params, false),
        SurfaceCodeTask::UnrotatedMemoryX => generate_unrotated_surface_code_circuit(params, true),
        SurfaceCodeTask::UnrotatedMemoryZ => generate_unrotated_surface_code_circuit(params, false),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct SurfaceCoord {
    x: i32,
    y: i32,
}

impl SurfaceCoord {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn plus(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    fn args(self) -> Vec<f64> {
        vec![f64::from(self.x), f64::from(self.y)]
    }
}

fn generate_rotated_surface_code_circuit(
    params: &SurfaceCodeParams,
    is_memory_x: bool,
) -> CircuitResult<GeneratedCircuit> {
    let distance = params.distance().get();
    let mut data_coords = BTreeSet::new();
    let mut x_observable = Vec::new();
    let mut z_observable = Vec::new();
    for x in 1..=distance {
        for y in 1..=distance {
            let coord = SurfaceCoord::new((2 * x - 1) as i32, (2 * y - 1) as i32);
            data_coords.insert(coord);
            if y == 1 {
                z_observable.push(coord);
            }
            if x == 1 {
                x_observable.push(coord);
            }
        }
    }

    let mut x_measure_coords = BTreeSet::new();
    let mut z_measure_coords = BTreeSet::new();
    for x in 0..=distance {
        for y in 0..=distance {
            let on_boundary_1 = x == 0 || x == distance;
            let on_boundary_2 = y == 0 || y == distance;
            let parity = x % 2 != y % 2;
            if on_boundary_1 && parity {
                continue;
            }
            if on_boundary_2 && !parity {
                continue;
            }
            let coord = SurfaceCoord::new((2 * x) as i32, (2 * y) as i32);
            if parity {
                x_measure_coords.insert(coord);
            } else {
                z_measure_coords.insert(coord);
            }
        }
    }

    let z_order = [
        SurfaceCoord::new(1, 1),
        SurfaceCoord::new(1, -1),
        SurfaceCoord::new(-1, 1),
        SurfaceCoord::new(-1, -1),
    ];
    let x_order = [
        SurfaceCoord::new(1, 1),
        SurfaceCoord::new(-1, 1),
        SurfaceCoord::new(1, -1),
        SurfaceCoord::new(-1, -1),
    ];

    finish_surface_code_circuit(
        |coord| rotated_surface_coord_to_index(coord, distance),
        data_coords,
        x_measure_coords,
        z_measure_coords,
        params.common(),
        &x_order,
        &z_order,
        x_observable,
        z_observable,
        is_memory_x,
    )
}

fn generate_unrotated_surface_code_circuit(
    params: &SurfaceCodeParams,
    is_memory_x: bool,
) -> CircuitResult<GeneratedCircuit> {
    let distance = params.distance().get();
    let mut data_coords = BTreeSet::new();
    let mut x_measure_coords = BTreeSet::new();
    let mut z_measure_coords = BTreeSet::new();
    let mut x_observable = Vec::new();
    let mut z_observable = Vec::new();
    for x in 0..(2 * distance - 1) {
        for y in 0..(2 * distance - 1) {
            let coord = SurfaceCoord::new(x as i32, y as i32);
            let parity = x % 2 != y % 2;
            if parity {
                if x % 2 == 0 {
                    z_measure_coords.insert(coord);
                } else {
                    x_measure_coords.insert(coord);
                }
            } else {
                data_coords.insert(coord);
                if x == 0 {
                    x_observable.push(coord);
                }
                if y == 0 {
                    z_observable.push(coord);
                }
            }
        }
    }

    let order = [
        SurfaceCoord::new(1, 0),
        SurfaceCoord::new(0, 1),
        SurfaceCoord::new(0, -1),
        SurfaceCoord::new(-1, 0),
    ];

    finish_surface_code_circuit(
        |coord| unrotated_surface_coord_to_index(coord, distance),
        data_coords,
        x_measure_coords,
        z_measure_coords,
        params.common(),
        &order,
        &order,
        x_observable,
        z_observable,
        is_memory_x,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The shared surface-code finisher mirrors Stim's generator boundary and keeps the rotated and unrotated setup code small."
)]
fn finish_surface_code_circuit(
    coord_to_index: impl Fn(SurfaceCoord) -> CircuitResult<u32>,
    data_coords: BTreeSet<SurfaceCoord>,
    x_measure_coords: BTreeSet<SurfaceCoord>,
    z_measure_coords: BTreeSet<SurfaceCoord>,
    params: &CircuitGenParams,
    x_order: &[SurfaceCoord; 4],
    z_order: &[SurfaceCoord; 4],
    x_observable: Vec<SurfaceCoord>,
    z_observable: Vec<SurfaceCoord>,
    is_memory_x: bool,
) -> CircuitResult<GeneratedCircuit> {
    let chosen_basis_observable = if is_memory_x {
        &x_observable
    } else {
        &z_observable
    };
    let chosen_basis_measure_coords = if is_memory_x {
        &x_measure_coords
    } else {
        &z_measure_coords
    };
    let measurement_basis = if is_memory_x { 'X' } else { 'Z' };

    let mut p2q = BTreeMap::new();
    for coord in &data_coords {
        p2q.insert(*coord, coord_to_index(*coord)?);
    }
    for coord in &x_measure_coords {
        p2q.insert(*coord, coord_to_index(*coord)?);
    }
    for coord in &z_measure_coords {
        p2q.insert(*coord, coord_to_index(*coord)?);
    }

    let mut q2p = BTreeMap::new();
    for (coord, qubit) in &p2q {
        q2p.insert(*qubit, *coord);
    }

    let mut data_qubits = data_coords
        .iter()
        .map(|coord| p2q[coord])
        .collect::<Vec<_>>();
    let mut measurement_qubits = Vec::new();
    let mut x_measurement_qubits = Vec::new();
    for coord in &x_measure_coords {
        let qubit = p2q[coord];
        measurement_qubits.push(qubit);
        x_measurement_qubits.push(qubit);
    }
    for coord in &z_measure_coords {
        measurement_qubits.push(p2q[coord]);
    }
    let mut all_qubits = data_qubits.clone();
    all_qubits.extend_from_slice(&measurement_qubits);
    all_qubits.sort_unstable();
    data_qubits.sort_unstable();
    measurement_qubits.sort_unstable();
    x_measurement_qubits.sort_unstable();

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

    let mut cnot_targets: [Vec<u32>; 4] = std::array::from_fn(|_| Vec::new());
    for k in 0..4 {
        for measure in &x_measure_coords {
            let data = measure.plus(x_order[k]);
            if let Some(data_qubit) = p2q.get(&data) {
                cnot_targets[k].push(p2q[measure]);
                cnot_targets[k].push(*data_qubit);
            }
        }
        for measure in &z_measure_coords {
            let data = measure.plus(z_order[k]);
            if let Some(data_qubit) = p2q.get(&data) {
                cnot_targets[k].push(*data_qubit);
                cnot_targets[k].push(p2q[measure]);
            }
        }
    }

    let mut cycle_actions = Circuit::new();
    append_begin_round_tick(params, &mut cycle_actions, &data_qubits)?;
    append_unitary_1(params, &mut cycle_actions, "H", &x_measurement_qubits)?;
    for targets in &cnot_targets {
        append_instruction(&mut cycle_actions, "TICK", Vec::new(), Vec::new())?;
        append_unitary_2(params, &mut cycle_actions, "CX", targets)?;
    }
    append_instruction(&mut cycle_actions, "TICK", Vec::new(), Vec::new())?;
    append_unitary_1(params, &mut cycle_actions, "H", &x_measurement_qubits)?;
    append_instruction(&mut cycle_actions, "TICK", Vec::new(), Vec::new())?;
    append_measure_reset(params, &mut cycle_actions, &measurement_qubits, 'Z')?;

    let mut head = Circuit::new();
    for (qubit, coord) in &q2p {
        append_instruction(
            &mut head,
            "QUBIT_COORDS",
            coord.args(),
            qubit_targets(&[*qubit])?,
        )?;
    }
    append_reset(params, &mut head, &data_qubits, measurement_basis)?;
    append_reset(params, &mut head, &measurement_qubits, 'Z')?;
    append_circuit(&mut head, &cycle_actions);
    for measure in chosen_basis_measure_coords {
        append_instruction(
            &mut head,
            "DETECTOR",
            vec![f64::from(measure.x), f64::from(measure.y), 0.0],
            vec![rec_target(
                u32::try_from(measurement_qubits.len()).map_err(|_| {
                    CircuitError::invalid_domain_value(
                        "measurement qubit count",
                        measurement_qubits.len(),
                    )
                })? - measure_coord_to_order[measure],
            )?],
        )?;
    }

    let mut body = cycle_actions;
    let measurement_count = u32::try_from(measurement_qubits.len()).map_err(|_| {
        CircuitError::invalid_domain_value("measurement qubit count", measurement_qubits.len())
    })?;
    append_instruction(&mut body, "SHIFT_COORDS", vec![0.0, 0.0, 1.0], Vec::new())?;
    for qubit in &measurement_qubits {
        let coord = q2p[qubit];
        let k = measurement_count - measure_coord_to_order[&coord] - 1;
        append_instruction(
            &mut body,
            "DETECTOR",
            vec![f64::from(coord.x), f64::from(coord.y), 0.0],
            rec_targets(&[k + 1, k + 1 + measurement_count])?,
        )?;
    }

    let mut tail = Circuit::new();
    append_measure(params, &mut tail, &data_qubits, measurement_basis)?;
    let data_count = u32::try_from(data_qubits.len())
        .map_err(|_| CircuitError::invalid_domain_value("data qubit count", data_qubits.len()))?;
    for measure in chosen_basis_measure_coords {
        let mut detectors = Vec::new();
        for delta in z_order {
            let data = measure.plus(*delta);
            if p2q.contains_key(&data) {
                detectors.push(data_count - data_coord_to_order[&data]);
            }
        }
        detectors.push(data_count + measurement_count - measure_coord_to_order[measure]);
        detectors.sort_unstable();
        append_instruction(
            &mut tail,
            "DETECTOR",
            vec![f64::from(measure.x), f64::from(measure.y), 1.0],
            rec_targets(&detectors)?,
        )?;
    }
    let mut obs_inc = chosen_basis_observable
        .iter()
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
    append_repeated_body(&mut full, body, params.rounds.get().saturating_sub(1))?;
    append_circuit(&mut full, &tail);

    let mut layout = BTreeMap::new();
    for coord in &data_coords {
        layout.insert((coord.x as u32, coord.y as u32), ('d', p2q[coord]));
    }
    for coord in &x_measure_coords {
        layout.insert((coord.x as u32, coord.y as u32), ('X', p2q[coord]));
    }
    for coord in &z_measure_coords {
        layout.insert((coord.x as u32, coord.y as u32), ('Z', p2q[coord]));
    }
    for coord in chosen_basis_observable {
        if let Some(entry) = layout.get_mut(&(coord.x as u32, coord.y as u32)) {
            entry.0 = 'L';
        }
    }

    Ok(GeneratedCircuit {
        circuit: full,
        layout_text: layout_text(&layout),
        hint_text: "# Legend:\n#     d# = data qubit\n#     L# = data qubit with logical observable crossing\n#     X# = measurement qubit (X stabilizer)\n#     Z# = measurement qubit (Z stabilizer)\n",
    })
}

fn rotated_surface_coord_to_index(coord: SurfaceCoord, distance: u32) -> CircuitResult<u32> {
    let adjusted_y = coord.y - coord.x.rem_euclid(2);
    let width = distance
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| CircuitError::invalid_domain_value("surface code distance", distance))?;
    let index = i64::from(coord.x) + i64::from(adjusted_y / 2) * i64::from(width);
    u32::try_from(index)
        .map_err(|_| CircuitError::invalid_domain_value("surface code qubit index", index))
}

fn unrotated_surface_coord_to_index(coord: SurfaceCoord, distance: u32) -> CircuitResult<u32> {
    let width = distance
        .checked_mul(2)
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| CircuitError::invalid_domain_value("surface code distance", distance))?;
    let index = i64::from(coord.x) + i64::from(coord.y) * i64::from(width);
    u32::try_from(index)
        .map_err(|_| CircuitError::invalid_domain_value("surface code qubit index", index))
}
