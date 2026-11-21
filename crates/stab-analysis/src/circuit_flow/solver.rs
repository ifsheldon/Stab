use stab_algebra::{Flow, PauliBasis, PauliString};
use stab_model::advanced::CancellationCountdown;
use stab_model::{Cancellation, Circuit, MemoryBudget};

use crate::{AnalysisError, AnalysisRequestError, AnalysisResult};
use crate::{AnalysisOperation, FlowGeneratorOptions, ResourceKind, ResourceLimitError};

use super::circuit_flow_generators;

/// Options of [`solve_for_flow_measurements`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FlowMeasurementOptions {
    /// Budget for the circuit's flow generators, admitted as by [`FlowGeneratorOptions`], and
    /// separately for the solver's elimination table and solutions, projected before they are
    /// built.
    pub memory_budget: MemoryBudget,
    /// Signal polled while the flow generators are computed and once per row reduction step.
    pub cancellation: Cancellation,
}

/// Finds measurement indices that explain each queried unsigned stabilizer flow.
///
/// The solver reduces flow Pauli terms against the circuit's generator table over GF(2), then
/// returns the corresponding measurement parity. Unsupported generator shapes fail closed through
/// the same typed error path regardless of the circuit's measurement count. When a flow has more
/// than one valid parity, the selected checker-valid solution is deterministic but is not promised
/// to match Stim's internal pre-canonicalization tie break. Like Stim v1.16.0, this operation solves
/// only each query's input and output Pauli projection; measurement and observable terms already
/// present on a query are not constraints on the returned parity.
///
/// # Errors
///
/// Returns [`AnalysisError::Cancelled`] once the options' signal is observed and
/// [`AnalysisError::ResourceLimit`] when the generators or the elimination table exceed the
/// options' memory budget.
pub fn solve_for_flow_measurements(
    circuit: &Circuit,
    flows: &[Flow],
    options: &FlowMeasurementOptions,
) -> AnalysisResult<Vec<Option<Vec<i32>>>> {
    options.cancellation.check()?;
    validate_non_empty_flow_queries(flows)?;
    if flows.is_empty() {
        return Ok(Vec::new());
    }

    solve_with_generator_rows(circuit, flows, options)
}

fn validate_non_empty_flow_queries(flows: &[Flow]) -> AnalysisResult<()> {
    for flow in flows {
        if flow.input().has_no_pauli_terms() && flow.output().has_no_pauli_terms() {
            return Err(AnalysisRequestError::EmptyFlowQuery.into());
        }
    }
    Ok(())
}

fn solve_with_generator_rows(
    circuit: &Circuit,
    flows: &[Flow],
    options: &FlowMeasurementOptions,
) -> AnalysisResult<Vec<Option<Vec<i32>>>> {
    let generators = circuit_flow_generators(
        circuit,
        &FlowGeneratorOptions {
            memory_budget: options.memory_budget,
            cancellation: options.cancellation.clone(),
        },
    )?;
    let qubit_count = solved_qubit_count(circuit, flows, &generators);
    admit_solver_storage(
        circuit,
        qubit_count,
        generators.len(),
        flows.len(),
        options.memory_budget,
    )?;
    let vector_len = qubit_count.saturating_mul(4);
    let mut reduction = Reduction {
        cancellation: &options.cancellation,
        countdown: CancellationCountdown::new(),
    };
    let mut basis = vec![None; vector_len];
    let mut zero_measurement_rows = Vec::new();
    for generator in generators {
        let row = SolveRow {
            vector: flow_pauli_vector(&generator, qubit_count),
            measurements: generator.measurements().collect(),
        };
        add_basis_row(row, &mut basis, &mut zero_measurement_rows, &mut reduction)?;
    }

    let mut results = Vec::with_capacity(flows.len());
    for flow in flows {
        let mut row = SolveRow {
            vector: flow_pauli_vector(flow, qubit_count),
            measurements: Vec::new(),
        };
        eliminate_implicit_idle_suffix(
            &mut row.vector,
            circuit.simulated_qubit_count(),
            qubit_count,
        );
        reduction.reduce(&mut row, &basis)?;
        results.push(
            row.is_zero().then(|| {
                reduce_measurements_with_zero_rows(row.measurements, &zero_measurement_rows)
            }),
        );
    }
    Ok(results)
}

/// Admits the elimination table and the solutions before they are built: one basis slot per
/// vector bit, and for every generator row and every query a bit vector and at most one
/// measurement index per circuit measurement. The generators themselves are admitted separately
/// by [`circuit_flow_generators`].
fn admit_solver_storage(
    circuit: &Circuit,
    qubit_count: usize,
    generator_rows: usize,
    queries: usize,
    budget: MemoryBudget,
) -> AnalysisResult<()> {
    let vector_len = (qubit_count as u128).saturating_mul(4);
    let measurements = u128::from(circuit.count_measurements()?);
    let slot_bytes = vector_len.saturating_mul(size_of::<Option<SolveRow>>() as u128);
    let row_bytes = (size_of::<SolveRow>() as u128)
        .saturating_add(vector_len)
        .saturating_add(measurements.saturating_mul(size_of::<i32>() as u128));
    let rows = (generator_rows as u128).saturating_add(queries as u128);
    budget
        .admit(slot_bytes.saturating_add(rows.saturating_mul(row_bytes)))
        .map_err(|exceeded| {
            ResourceLimitError::budget_refusal(
                AnalysisOperation::FlowMeasurementSolving,
                ResourceKind::ProjectedPayloadBytes,
                exceeded,
            )
            .into()
        })
}

/// Row reduction against the basis, polling the cancellation signal once per row operation.
struct Reduction<'a> {
    cancellation: &'a Cancellation,
    countdown: CancellationCountdown,
}

impl Reduction<'_> {
    fn reduce(&mut self, row: &mut SolveRow, basis: &[Option<SolveRow>]) -> AnalysisResult<()> {
        let mut pivot = 0;
        while pivot < row.vector.len() {
            if row.vector.get(pivot).copied().unwrap_or(false)
                && let Some(Some(basis_row)) = basis.get(pivot)
            {
                self.countdown.tick(self.cancellation)?;
                row.xor_assign(basis_row);
            }
            pivot += 1;
        }
        Ok(())
    }
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
    reduction: &mut Reduction<'_>,
) -> AnalysisResult<()> {
    reduction.reduce(&mut row, basis)?;
    if row.is_zero() {
        if !row.measurements.is_empty() {
            zero_measurement_rows.push(row.measurements);
        }
        return Ok(());
    }
    if let Some(pivot) = row.pivot() {
        let slot = basis.get_mut(pivot).ok_or_else(|| {
            AnalysisError::internal(
                AnalysisOperation::FlowMeasurementSolving,
                "flow solver pivot row is outside the basis table",
            )
        })?;
        *slot = Some(row);
    }
    Ok(())
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
        .fold(circuit.simulated_qubit_count(), |count, flow| {
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
