use crate::{Circuit, CircuitError, CircuitResult, Flow, PauliBasis, PauliString};

use super::{
    check_if_circuit_has_unsigned_stabilizer_flows, circuit_flow_generators, single_pauli,
};

const MAX_EXHAUSTIVE_FLOW_SOLVE_MEASUREMENTS: usize = 16;

/// Finds measurement indices that explain each queried unsigned stabilizer flow.
///
/// The promoted Rust scope matches the pinned Stim v1.16.0 examples for unitary, single
/// measurement, extra-idle-qubit, and small composed measurement-rich circuits. Broader composed
/// measurement-rich circuits use a bounded checker fallback until the full generator-table solver is
/// promoted.
pub fn solve_for_flow_measurements(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Vec<Option<Vec<i32>>>> {
    validate_non_empty_flow_queries(flows)?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(results) = solve_with_generator_rows(circuit, flows)? {
        return Ok(results);
    }
    solve_with_bounded_checker(circuit, flows)
}

fn validate_non_empty_flow_queries(flows: &[Flow]) -> CircuitResult<()> {
    for flow in flows {
        if flow.input().has_no_pauli_terms() && flow.output().has_no_pauli_terms() {
            return Err(CircuitError::invalid_tableau_conversion(
                "solve_for_flow_measurements only supports flows with non-empty Pauli input or output",
            ));
        }
    }
    Ok(())
}

fn solve_with_generator_rows(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Option<Vec<Option<Vec<i32>>>>> {
    let Some(generators) = generator_rows_for(circuit, flows)? else {
        return Ok(None);
    };
    let qubit_count = solved_qubit_count(circuit, flows, &generators);
    let vector_len = qubit_count.saturating_mul(4);
    let mut basis = vec![None; vector_len];
    let mut zero_measurement_rows = Vec::new();
    for generator in generators {
        let row = SolveRow {
            vector: flow_pauli_vector(&generator, qubit_count),
            measurements: generator.measurements().collect(),
        };
        add_basis_row(row, &mut basis, &mut zero_measurement_rows)?;
    }

    let results = flows
        .iter()
        .map(|flow| {
            let mut row = SolveRow {
                vector: flow_pauli_vector(flow, qubit_count),
                measurements: Vec::new(),
            };
            reduce_row(&mut row, &basis);
            row.is_zero().then(|| {
                reduce_measurements_with_zero_rows(row.measurements, &zero_measurement_rows)
            })
        })
        .collect();
    Ok(Some(results))
}

fn generator_rows_for(circuit: &Circuit, flows: &[Flow]) -> CircuitResult<Option<Vec<Flow>>> {
    let mut generators = match circuit_flow_generators(circuit) {
        Ok(generators) => generators,
        Err(_) => return Ok(None),
    };
    let required_qubits = flows.iter().fold(circuit.count_qubits(), |count, flow| {
        count.max(flow.input().len()).max(flow.output().len())
    });
    let existing_qubits = generators
        .iter()
        .fold(circuit.count_qubits(), |count, flow| {
            count.max(flow.input().len()).max(flow.output().len())
        });
    for qubit in existing_qubits..required_qubits {
        generators.push(Flow::new(
            single_pauli(required_qubits, qubit, PauliBasis::X),
            single_pauli(required_qubits, qubit, PauliBasis::X),
            [],
            [],
        ));
        generators.push(Flow::new(
            single_pauli(required_qubits, qubit, PauliBasis::Z),
            single_pauli(required_qubits, qubit, PauliBasis::Z),
            [],
            [],
        ));
    }
    Ok(Some(generators))
}

fn solve_with_bounded_checker(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Vec<Option<Vec<i32>>>> {
    let measurement_count = usize::try_from(circuit.count_measurements()?).map_err(|_| {
        CircuitError::invalid_tableau_conversion(
            "circuit measurement count does not fit usize during flow solving",
        )
    })?;
    if measurement_count > MAX_EXHAUSTIVE_FLOW_SOLVE_MEASUREMENTS {
        return Err(CircuitError::invalid_tableau_conversion(format!(
            "solve_for_flow_measurements fallback supports at most {MAX_EXHAUSTIVE_FLOW_SOLVE_MEASUREMENTS} measurements, got {measurement_count}"
        )));
    }

    flows
        .iter()
        .map(|flow| solve_one_with_bounded_checker(circuit, flow, measurement_count))
        .collect()
}

fn solve_one_with_bounded_checker(
    circuit: &Circuit,
    flow: &Flow,
    measurement_count: usize,
) -> CircuitResult<Option<Vec<i32>>> {
    let subset_count = 1usize
        .checked_shl(u32::try_from(measurement_count).map_err(|_| {
            CircuitError::invalid_tableau_conversion(
                "measurement count does not fit u32 during flow solving",
            )
        })?)
        .ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "measurement subset count overflowed during flow solving",
            )
        })?;
    let mut masks = (0..subset_count).collect::<Vec<_>>();
    masks.sort_unstable_by_key(|mask| (mask.count_ones(), *mask));
    for mask in masks {
        let measurements = measurements_from_mask(mask, measurement_count)?;
        let candidate = Flow::new(
            flow.input().clone(),
            flow.output().clone(),
            measurements.iter().copied(),
            [],
        );
        if check_if_circuit_has_unsigned_stabilizer_flows(circuit, &[candidate])
            .first()
            .copied()
            .unwrap_or(false)
        {
            return Ok(Some(measurements));
        }
    }
    Ok(None)
}

fn measurements_from_mask(mask: usize, measurement_count: usize) -> CircuitResult<Vec<i32>> {
    (0..measurement_count)
        .filter(|index| (mask & (1usize << index)) != 0)
        .map(|index| {
            i32::try_from(index).map_err(|_| {
                CircuitError::invalid_tableau_conversion(format!(
                    "measurement index {index} does not fit i32 during flow solving"
                ))
            })
        })
        .collect()
}

#[derive(Clone)]
struct SolveRow {
    vector: Vec<bool>,
    measurements: Vec<i32>,
}

impl SolveRow {
    fn is_zero(&self) -> bool {
        self.vector.iter().all(|bit| !*bit)
    }

    fn pivot(&self) -> Option<usize> {
        self.vector.iter().position(|bit| *bit)
    }

    fn xor_assign(&mut self, rhs: &Self) {
        for (left, right) in self.vector.iter_mut().zip(&rhs.vector) {
            *left ^= *right;
        }
        self.measurements = xor_sorted_i32(&self.measurements, &rhs.measurements);
    }
}

fn add_basis_row(
    mut row: SolveRow,
    basis: &mut [Option<SolveRow>],
    zero_measurement_rows: &mut Vec<Vec<i32>>,
) -> CircuitResult<()> {
    reduce_row(&mut row, basis);
    if row.is_zero() {
        if !row.measurements.is_empty() {
            zero_measurement_rows.push(row.measurements);
        }
        return Ok(());
    }
    if let Some(pivot) = row.pivot() {
        let slot = basis.get_mut(pivot).ok_or_else(|| {
            CircuitError::invalid_tableau_conversion(
                "flow solver pivot row is outside the basis table",
            )
        })?;
        *slot = Some(row);
    }
    Ok(())
}

fn reduce_row(row: &mut SolveRow, basis: &[Option<SolveRow>]) {
    let mut pivot = 0;
    while pivot < row.vector.len() {
        if row.vector.get(pivot).copied().unwrap_or(false)
            && let Some(Some(basis_row)) = basis.get(pivot)
        {
            row.xor_assign(basis_row);
        }
        pivot += 1;
    }
}

fn reduce_measurements_with_zero_rows(
    mut measurements: Vec<i32>,
    zero_measurement_rows: &[Vec<i32>],
) -> Vec<i32> {
    for zero_row in zero_measurement_rows {
        let candidate = xor_sorted_i32(&measurements, zero_row);
        if candidate.len() < measurements.len() {
            measurements = candidate;
        }
    }
    measurements
}

fn solved_qubit_count(circuit: &Circuit, flows: &[Flow], generators: &[Flow]) -> usize {
    let count = flows.iter().fold(circuit.count_qubits(), |count, flow| {
        count.max(flow.input().len()).max(flow.output().len())
    });
    generators.iter().fold(count, |count, flow| {
        count.max(flow.input().len()).max(flow.output().len())
    })
}

fn flow_pauli_vector(flow: &Flow, qubit_count: usize) -> Vec<bool> {
    let mut bits = Vec::with_capacity(qubit_count.saturating_mul(4));
    push_pauli_bits(flow.input(), qubit_count, &mut bits);
    push_pauli_bits(flow.output(), qubit_count, &mut bits);
    bits
}

fn push_pauli_bits(pauli: &PauliString, qubit_count: usize, bits: &mut Vec<bool>) {
    for qubit in 0..qubit_count {
        let basis = pauli.get(qubit).unwrap_or(PauliBasis::I);
        bits.push(basis.x_bit());
        bits.push(basis.z_bit());
    }
}

fn xor_sorted_i32(left: &[i32], right: &[i32]) -> Vec<i32> {
    let mut values = Vec::with_capacity(left.len() + right.len());
    values.extend_from_slice(left);
    values.extend_from_slice(right);
    values.sort_unstable();

    let mut result = Vec::new();
    let mut current = None;
    for value in values {
        match current {
            Some((existing, parity)) if existing == value => {
                current = Some((existing, !parity));
            }
            Some((existing, true)) => {
                result.push(existing);
                current = Some((value, true));
            }
            Some((_existing, false)) => {
                current = Some((value, true));
            }
            None => {
                current = Some((value, true));
            }
        }
    }
    if let Some((value, true)) = current {
        result.push(value);
    }
    result
}
