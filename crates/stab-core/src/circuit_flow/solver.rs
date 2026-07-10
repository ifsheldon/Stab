use crate::{Circuit, CircuitError, CircuitResult, Flow, PauliBasis, PauliString};

use super::circuit_flow_generators;

/// Finds measurement indices that explain each queried unsigned stabilizer flow.
///
/// The solver reduces flow Pauli terms against the circuit's generator table over GF(2), then
/// returns the corresponding measurement parity. Unsupported generator shapes fail closed through
/// the same typed error path regardless of the circuit's measurement count. When a flow has more
/// than one valid parity, the selected checker-valid solution is deterministic but is not promised
/// to match Stim's internal pre-canonicalization tie break. Like Stim v1.16.0, this operation solves
/// only each query's input and output Pauli projection; measurement and observable terms already
/// present on a query are not constraints on the returned parity.
pub fn solve_for_flow_measurements(
    circuit: &Circuit,
    flows: &[Flow],
) -> CircuitResult<Vec<Option<Vec<i32>>>> {
    validate_non_empty_flow_queries(flows)?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }

    solve_with_generator_rows(circuit, flows)
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
) -> CircuitResult<Vec<Option<Vec<i32>>>> {
    let generators = circuit_flow_generators(circuit)?;
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
            eliminate_implicit_idle_suffix(
                &mut row.vector,
                circuit.count_simulated_qubits(),
                qubit_count,
            );
            reduce_row(&mut row, &basis);
            row.is_zero().then(|| {
                reduce_measurements_with_zero_rows(row.measurements, &zero_measurement_rows)
            })
        })
        .collect();
    Ok(results)
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
    let count = flows
        .iter()
        .fold(circuit.count_simulated_qubits(), |count, flow| {
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

fn eliminate_implicit_idle_suffix(vector: &mut [bool], circuit_qubits: usize, qubit_count: usize) {
    let output_offset = qubit_count.saturating_mul(2);
    for qubit in circuit_qubits..qubit_count {
        for component in 0..2 {
            let input = qubit.saturating_mul(2).saturating_add(component);
            let output = output_offset.saturating_add(input);
            let difference = vector.get(input).copied().unwrap_or(false)
                ^ vector.get(output).copied().unwrap_or(false);
            if let Some(bit) = vector.get_mut(input) {
                *bit = difference;
            }
            if let Some(bit) = vector.get_mut(output) {
                *bit = false;
            }
        }
    }
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
