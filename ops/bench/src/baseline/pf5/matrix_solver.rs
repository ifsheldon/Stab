use std::{collections::BTreeSet, hint::black_box};

use stab_core::{
    Circuit, CircuitItem, Flow, PauliBasis, check_if_circuit_has_unsigned_stabilizer_flows,
    circuit_flow_generators, solve_for_flow_measurements,
};

use crate::error::BenchError;
use crate::manifest::BenchmarkRow;
use crate::report::Measurement;

use super::super::{measure_stab_iterations, stab_runner_error};

const ROW_ID: &str = "pfm-b4-flow-solve-matrix-sizes";

const DENSE_SMALL_BITS: &str = "stab_pfm_b4_flow_solve_dense_32x64_input_bits";
const DENSE_SMALL_QUERIES: &str = "stab_pfm_b4_flow_solve_dense_32x64_queries";
const DENSE_MEDIUM_BITS: &str = "stab_pfm_b4_flow_solve_dense_128x256_input_bits";
const DENSE_MEDIUM_QUERIES: &str = "stab_pfm_b4_flow_solve_dense_128x256_queries";
const SPARSE_LARGE_BITS: &str = "stab_pfm_b4_flow_solve_sparse_512x1024_input_bits";
const SPARSE_LARGE_QUERIES: &str = "stab_pfm_b4_flow_solve_sparse_512x1024_queries";

const DENSE_SMALL_QUBITS: usize = 16;
const DENSE_SMALL_QUERY_COUNT: usize = 17;
const DENSE_MEDIUM_QUBITS: usize = 64;
const DENSE_MEDIUM_QUERY_COUNT: usize = 65;
const SPARSE_LARGE_QUBITS: usize = 256;
const SPARSE_ACTIVE_QUBITS: usize = 32;
const SPARSE_LARGE_QUERY_COUNT: usize = 33;
const QUERY_COMPOSITION_WEIGHT: usize = 3;
const MIN_DENSE_PAULI_BIT_DENSITY: f64 = 0.15;
const MAX_SPARSE_PAULI_BIT_DENSITY: f64 = 0.08;
const MIN_ACTIVE_PAULI_BIT_DENSITY: f64 = 0.15;
const MAX_ACTIVE_PAULI_BIT_DENSITY: f64 = 0.85;

#[cfg(not(test))]
const MATRIX_COMPARE_ITERATIONS: usize = 4;
#[cfg(test)]
const MATRIX_COMPARE_ITERATIONS: usize = 1;

struct MatrixCase {
    bits_name: &'static str,
    queries_name: &'static str,
    circuit: Circuit,
    queries: Vec<Flow>,
}

#[derive(Clone, Copy)]
struct MatrixCaseSpec {
    bits_name: &'static str,
    queries_name: &'static str,
    qubit_count: usize,
    active_count: usize,
    measurement_count: usize,
    query_count: usize,
}

const PRODUCTION_CASE_SPECS: [MatrixCaseSpec; 3] = [
    MatrixCaseSpec {
        bits_name: DENSE_SMALL_BITS,
        queries_name: DENSE_SMALL_QUERIES,
        qubit_count: DENSE_SMALL_QUBITS,
        active_count: DENSE_SMALL_QUBITS,
        measurement_count: 7,
        query_count: DENSE_SMALL_QUERY_COUNT,
    },
    MatrixCaseSpec {
        bits_name: DENSE_MEDIUM_BITS,
        queries_name: DENSE_MEDIUM_QUERIES,
        qubit_count: DENSE_MEDIUM_QUBITS,
        active_count: DENSE_MEDIUM_QUBITS,
        measurement_count: 24,
        query_count: DENSE_MEDIUM_QUERY_COUNT,
    },
    MatrixCaseSpec {
        bits_name: SPARSE_LARGE_BITS,
        queries_name: SPARSE_LARGE_QUERIES,
        qubit_count: SPARSE_LARGE_QUBITS,
        active_count: SPARSE_ACTIVE_QUBITS,
        measurement_count: 12,
        query_count: SPARSE_LARGE_QUERY_COUNT,
    },
];

#[cfg(test)]
const TEST_CASE_SPECS: [MatrixCaseSpec; 3] = [
    MatrixCaseSpec {
        bits_name: DENSE_SMALL_BITS,
        queries_name: DENSE_SMALL_QUERIES,
        qubit_count: 8,
        active_count: 8,
        measurement_count: 3,
        query_count: 5,
    },
    MatrixCaseSpec {
        bits_name: DENSE_MEDIUM_BITS,
        queries_name: DENSE_MEDIUM_QUERIES,
        qubit_count: 12,
        active_count: 12,
        measurement_count: 4,
        query_count: 9,
    },
    MatrixCaseSpec {
        bits_name: SPARSE_LARGE_BITS,
        queries_name: SPARSE_LARGE_QUERIES,
        qubit_count: 16,
        active_count: 6,
        measurement_count: 3,
        query_count: 5,
    },
];

pub(super) fn run(row: &BenchmarkRow) -> Result<Vec<Measurement>, BenchError> {
    let cases = matrix_cases(&row.id)?;
    let mut measurements = Vec::with_capacity(cases.len() * 2);
    for case in cases {
        let measured = measure_stab_iterations(case.bits_name, MATRIX_COMPARE_ITERATIONS, || {
            measure_case(row, &case)
        })?;
        let mut query_rate = measured.clone();
        query_rate.name = case.queries_name.to_string();
        measurements.push(measured);
        measurements.push(query_rate);
    }
    Ok(measurements)
}

pub(super) fn measurement_work(row_id: &str, name: &str) -> Option<(f64, &'static str)> {
    if row_id != ROW_ID {
        return None;
    }
    match name {
        DENSE_SMALL_BITS => Some((
            query_inclusive_input_bits(DENSE_SMALL_QUBITS, DENSE_SMALL_QUERY_COUNT) as f64,
            "query-inclusive-input-bits/s",
        )),
        DENSE_SMALL_QUERIES => Some((DENSE_SMALL_QUERY_COUNT as f64, "queries/s")),
        DENSE_MEDIUM_BITS => Some((
            query_inclusive_input_bits(DENSE_MEDIUM_QUBITS, DENSE_MEDIUM_QUERY_COUNT) as f64,
            "query-inclusive-input-bits/s",
        )),
        DENSE_MEDIUM_QUERIES => Some((DENSE_MEDIUM_QUERY_COUNT as f64, "queries/s")),
        SPARSE_LARGE_BITS => Some((
            query_inclusive_input_bits(SPARSE_LARGE_QUBITS, SPARSE_LARGE_QUERY_COUNT) as f64,
            "query-inclusive-input-bits/s",
        )),
        SPARSE_LARGE_QUERIES => Some((SPARSE_LARGE_QUERY_COUNT as f64, "queries/s")),
        _ => None,
    }
}

pub(super) fn compare_note(row_id: &str) -> Option<&'static str> {
    (row_id == ROW_ID).then_some(
        "contract-only: Stab measures end-to-end public solve_for_flow_measurements GF(2) generator construction and query reduction for deterministic measurement-rich scrambled dense 32x64 and 128x256 Pauli bases with exact 7- and 24-singleton measurement-signature sets and 17 and 65 three-row-composed queries plus a 512x1024 high-qubit Pauli basis with an exact 12-singleton measurement-signature set whose scrambling is confined to exactly 32 sparse active qubits and 33 three-row-composed queries. Every circuit includes one mixed classical-feedback and plain-quantum controlled-Pauli instruction that the pre-PFM-B4 generator path rejected; the medium case combines that shape with 24 measurements beyond the removed 16-measurement fallback cap. Every query must solve to nonempty measurement parity. Fixture construction and all contract validation occur outside timing; timed samples contain only the public solver and black-box consumption. Paired measurements report query-inclusive Pauli-input-bit and solved-query rates from the same timing sample; overall and active-submatrix density plus active-support checks prevent dense or sparse workload drift, and allocation plus resident evidence is available with allocation tracking. Pinned Stim has no faithful in-process baseline without Python binding overhead",
    )
}

fn measure_case(row: &BenchmarkRow, case: &MatrixCase) -> Result<(), BenchError> {
    let actual = solve_for_flow_measurements(&case.circuit, &case.queries)
        .map_err(|error| stab_runner_error(&row.id, error))?;
    black_box(actual);
    Ok(())
}

fn matrix_cases(row_id: &str) -> Result<Vec<MatrixCase>, BenchError> {
    if row_id != ROW_ID {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "unknown PFM-B4 flow-solver matrix row".to_string(),
        });
    }

    #[cfg(not(test))]
    let specs = PRODUCTION_CASE_SPECS;
    #[cfg(test)]
    let specs = TEST_CASE_SPECS;

    specs
        .into_iter()
        .map(|spec| {
            build_case(
                row_id,
                spec.bits_name,
                spec.queries_name,
                spec.qubit_count,
                spec.active_count,
                spec.measurement_count,
                spec.query_count,
            )
        })
        .collect()
}

fn build_case(
    row_id: &str,
    bits_name: &'static str,
    queries_name: &'static str,
    qubit_count: usize,
    active_count: usize,
    measurement_count: usize,
    query_count: usize,
) -> Result<MatrixCase, BenchError> {
    let active = active_qubits(qubit_count, active_count);
    let measured = active
        .get(..measurement_count)
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver measurement count {measurement_count} exceeds active count {active_count}"
            ),
        })?;
    let circuit = Circuit::from_stim_str(&scrambled_measurement_circuit_text(
        row_id,
        qubit_count,
        &active,
        measured,
    )?)
    .map_err(|error| stab_runner_error(row_id, error))?;
    let expected_rows = qubit_count
        .checked_mul(2)
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix row count overflowed".to_string(),
        })?;
    let generators =
        circuit_flow_generators(&circuit).map_err(|error| stab_runner_error(row_id, error))?;
    if generators.len() != expected_rows {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver matrix case expected {expected_rows} generator rows but got {}",
                generators.len()
            ),
        });
    }
    let measurement_count = usize::try_from(
        circuit
            .count_measurements()
            .map_err(|error| stab_runner_error(row_id, error))?,
    )
    .map_err(|_| BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: "flow-solver measurement count does not fit usize".to_string(),
    })?;
    if measurement_count != measured.len() {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver matrix case expected {} measurements but got {measurement_count}",
                measured.len()
            ),
        });
    }
    validate_removed_fallback_shape(row_id, &circuit)?;
    validate_measurement_signatures(row_id, &generators, measurement_count)?;
    validate_pauli_density(row_id, &generators, qubit_count, &active)?;
    let queries = generator_queries(row_id, &generators, query_count)?;
    let expected_solutions = solve_for_flow_measurements(&circuit, &queries)
        .map_err(|error| stab_runner_error(row_id, error))?;
    if expected_solutions.iter().any(|solution| {
        solution
            .as_ref()
            .is_none_or(|measurements| measurements.is_empty())
    }) {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver matrix queries must all have nonempty solutions, got {expected_solutions:?}"
            ),
        });
    }
    let solved_flows = queries
        .iter()
        .zip(&expected_solutions)
        .map(|(query, solution)| {
            Flow::new(
                query.input().clone(),
                query.output().clone(),
                solution.iter().flatten().copied(),
                [],
            )
        })
        .collect::<Vec<_>>();
    if check_if_circuit_has_unsigned_stabilizer_flows(&circuit, &solved_flows)
        .iter()
        .any(|valid| !valid)
    {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix query validation failed".to_string(),
        });
    }
    Ok(MatrixCase {
        bits_name,
        queries_name,
        circuit,
        queries,
    })
}

fn validate_removed_fallback_shape(row_id: &str, circuit: &Circuit) -> Result<(), BenchError> {
    let has_mixed_controlled_pauli = circuit.items().iter().any(|item| {
        let CircuitItem::Instruction(instruction) = item else {
            return false;
        };
        if !matches!(
            instruction.gate().canonical_name(),
            "CX" | "CY" | "CZ" | "XCZ" | "YCZ"
        ) {
            return false;
        }
        let groups = instruction.target_groups();
        let has_classical_feedback = groups.iter().any(|group| {
            let [left, right] = *group else {
                return false;
            };
            (left.is_classical_bit_target() && right.qubit_id().is_some())
                || (right.is_classical_bit_target() && left.qubit_id().is_some())
        });
        let has_plain_quantum = groups.iter().any(|group| {
            let [left, right] = *group else {
                return false;
            };
            left.qubit_id().is_some() && right.qubit_id().is_some()
        });
        has_classical_feedback && has_plain_quantum
    });
    if !has_mixed_controlled_pauli {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix workload must contain a mixed classical-feedback and plain-quantum controlled-Pauli instruction".to_string(),
        });
    }
    Ok(())
}

fn validate_measurement_signatures(
    row_id: &str,
    generators: &[Flow],
    measurement_count: usize,
) -> Result<(), BenchError> {
    let actual = generators
        .iter()
        .filter_map(|flow| {
            let signature = flow.measurements().collect::<Vec<_>>();
            (signature.len() == 1).then_some(signature)
        })
        .collect::<BTreeSet<_>>();
    let expected = (0..measurement_count)
        .map(|index| {
            i32::try_from(index)
                .map(|record| vec![record])
                .map_err(|_| BenchError::StabRunner {
                    row_id: row_id.to_string(),
                    message: format!(
                        "flow-solver measurement signature index {index} does not fit i32"
                    ),
                })
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    if actual != expected {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver matrix workload expected singleton measurement signatures {expected:?}, got {actual:?}"
            ),
        });
    }
    Ok(())
}

const fn query_inclusive_input_bits(qubit_count: usize, query_count: usize) -> usize {
    (qubit_count * 2 + query_count) * qubit_count * 4
}

fn validate_pauli_density(
    row_id: &str,
    generators: &[Flow],
    qubit_count: usize,
    active: &[usize],
) -> Result<(), BenchError> {
    let total_bits = generators
        .len()
        .checked_mul(qubit_count)
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver density denominator overflowed".to_string(),
        })?;
    let all_qubits = (0..qubit_count).collect::<Vec<_>>();
    let set_bits = generators
        .iter()
        .map(|flow| pauli_set_bits(flow, &all_qubits))
        .sum::<usize>();
    let density = set_bits as f64 / total_bits as f64;
    let dense = active.len() == qubit_count;
    if (dense && density < MIN_DENSE_PAULI_BIT_DENSITY)
        || (!dense && density > MAX_SPARSE_PAULI_BIT_DENSITY)
    {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver {} workload density {density:.4} ({set_bits}/{total_bits}) violated the {} threshold",
                if dense { "dense" } else { "sparse" },
                if dense {
                    MIN_DENSE_PAULI_BIT_DENSITY
                } else {
                    MAX_SPARSE_PAULI_BIT_DENSITY
                }
            ),
        });
    }

    let active_rows = generators
        .iter()
        .filter(|flow| pauli_set_bits(flow, active) > 0)
        .collect::<Vec<_>>();
    let active_total_bits = active_rows
        .len()
        .checked_mul(active.len())
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver active-density denominator overflowed".to_string(),
        })?;
    let active_set_bits = active_rows
        .iter()
        .map(|flow| pauli_set_bits(flow, active))
        .sum::<usize>();
    let active_density = active_set_bits as f64 / active_total_bits as f64;
    if !(MIN_ACTIVE_PAULI_BIT_DENSITY..=MAX_ACTIVE_PAULI_BIT_DENSITY).contains(&active_density) {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver active-submatrix density {active_density:.4} ({active_set_bits}/{active_total_bits}) is outside {MIN_ACTIVE_PAULI_BIT_DENSITY}..={MAX_ACTIVE_PAULI_BIT_DENSITY}"
            ),
        });
    }

    let measurement_support = (0..qubit_count)
        .filter(|qubit| {
            generators.iter().any(|flow| {
                flow.measurements().next().is_some()
                    && pauli_set_bits(flow, std::slice::from_ref(qubit)) > 0
            })
        })
        .collect::<BTreeSet<_>>();
    let expected_support = active.iter().copied().collect::<BTreeSet<_>>();
    if measurement_support != expected_support {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver measurement-bearing support {measurement_support:?} did not match active qubits {expected_support:?}"
            ),
        });
    }
    Ok(())
}

fn pauli_set_bits(flow: &Flow, qubits: &[usize]) -> usize {
    [flow.input(), flow.output()]
        .into_iter()
        .map(|pauli| {
            qubits
                .iter()
                .map(|&qubit| {
                    let basis = pauli.get(qubit).unwrap_or(PauliBasis::I);
                    usize::from(basis.x_bit()) + usize::from(basis.z_bit())
                })
                .sum::<usize>()
        })
        .sum()
}

fn generator_queries(
    row_id: &str,
    generators: &[Flow],
    query_count: usize,
) -> Result<Vec<Flow>, BenchError> {
    let mut records = BTreeSet::new();
    let candidates = generators
        .iter()
        .filter(|flow| {
            let signature = flow.measurements().collect::<Vec<_>>();
            let [record] = signature.as_slice() else {
                return false;
            };
            records.insert(*record)
        })
        .collect::<Vec<_>>();
    if candidates.len() < QUERY_COMPOSITION_WEIGHT || query_count == 0 {
        return Err(BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: format!(
                "flow-solver matrix workload requires at least {QUERY_COMPOSITION_WEIGHT} measurement-bearing generators and one query"
            ),
        });
    }
    (0..query_count)
        .map(|index| {
            let first = candidates
                .get((index * 7) % candidates.len())
                .copied()
                .ok_or_else(|| BenchError::StabRunner {
                    row_id: row_id.to_string(),
                    message: "flow-solver query generator index is out of bounds".to_string(),
                })?;
            let mut composed = first.clone();
            for offset in 1..QUERY_COMPOSITION_WEIGHT {
                let generator = candidates
                    .get((index * 7 + offset * 5) % candidates.len())
                    .copied()
                    .ok_or_else(|| BenchError::StabRunner {
                        row_id: row_id.to_string(),
                        message: "flow-solver composed-query generator index is out of bounds"
                            .to_string(),
                    })?;
                composed = composed
                    .multiply(generator)
                    .map_err(|error| stab_runner_error(row_id, error))?;
            }
            if composed.measurements().count() != QUERY_COMPOSITION_WEIGHT
                || (composed.input().has_no_pauli_terms()
                    && composed.output().has_no_pauli_terms())
            {
                return Err(BenchError::StabRunner {
                    row_id: row_id.to_string(),
                    message: format!(
                        "flow-solver composed query {index} did not retain {QUERY_COMPOSITION_WEIGHT} distinct measurement rows and nonempty Pauli work"
                    ),
                });
            }
            Ok(Flow::new(
                composed.input().clone(),
                composed.output().clone(),
                [],
                [],
            ))
        })
        .collect()
}

fn scrambled_measurement_circuit_text(
    row_id: &str,
    qubit_count: usize,
    active: &[usize],
    measured: &[usize],
) -> Result<String, BenchError> {
    let mut text = format!("I {}\n", qubit_count - 1);
    let mut state = 0xA076_1D64_78BD_642Fu64 ^ qubit_count as u64 ^ (active.len() as u64) << 32;
    let active_len = u64::try_from(active.len()).map_err(|_| BenchError::StabRunner {
        row_id: row_id.to_string(),
        message: "flow-solver active-qubit count does not fit u64".to_string(),
    })?;
    for _ in 0..active.len().saturating_mul(24) {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let left_index =
            usize::try_from(state % active_len).map_err(|_| BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: "flow-solver left scramble index does not fit usize".to_string(),
            })?;
        state ^= state.rotate_left(17);
        let mut right_index =
            usize::try_from(state % active_len).map_err(|_| BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: "flow-solver right scramble index does not fit usize".to_string(),
            })?;
        if right_index == left_index {
            right_index = (right_index + 1) % active.len();
        }
        let left = active
            .get(left_index)
            .copied()
            .ok_or_else(|| BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: "flow-solver left active-qubit index is out of bounds".to_string(),
            })?;
        let right = active
            .get(right_index)
            .copied()
            .ok_or_else(|| BenchError::StabRunner {
                row_id: row_id.to_string(),
                message: "flow-solver right active-qubit index is out of bounds".to_string(),
            })?;
        match state % 5 {
            0 => text.push_str(&format!("H {left}\n")),
            1 => text.push_str(&format!("S {left}\n")),
            2 => text.push_str(&format!("CX {left} {right}\n")),
            3 => text.push_str(&format!("CZ {left} {right}\n")),
            _ => text.push_str(&format!("SWAP {left} {right}\n")),
        }
    }
    text.push('M');
    for qubit in measured {
        text.push(' ');
        text.push_str(&qubit.to_string());
    }
    text.push('\n');
    let feedback_target = active
        .get(active.len().saturating_sub(3))
        .copied()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix workload has no active feedback target".to_string(),
        })?;
    let plain_left = active
        .get(active.len().saturating_sub(2))
        .copied()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix workload has no first plain controlled-Pauli target"
                .to_string(),
        })?;
    let plain_right = active
        .last()
        .copied()
        .ok_or_else(|| BenchError::StabRunner {
            row_id: row_id.to_string(),
            message: "flow-solver matrix workload has no second plain controlled-Pauli target"
                .to_string(),
        })?;
    text.push_str(&format!(
        "CX rec[-1] {feedback_target} {plain_left} {plain_right}\n"
    ));
    Ok(text)
}

fn active_qubits(qubit_count: usize, active_count: usize) -> Vec<usize> {
    if active_count == qubit_count {
        return (0..qubit_count).collect();
    }
    (0..active_count)
        .map(|index| index * (qubit_count - 1) / (active_count - 1))
        .collect()
}

#[cfg(test)]
pub(crate) fn expected_measurement_names() -> [&'static str; 6] {
    [
        DENSE_SMALL_BITS,
        DENSE_SMALL_QUERIES,
        DENSE_MEDIUM_BITS,
        DENSE_MEDIUM_QUERIES,
        SPARSE_LARGE_BITS,
        SPARSE_LARGE_QUERIES,
    ]
}

#[cfg(test)]
pub(crate) fn production_case_contracts() -> [(usize, usize, usize, usize, usize); 3] {
    PRODUCTION_CASE_SPECS.map(|spec| {
        (
            spec.qubit_count * 2,
            spec.qubit_count * 4,
            spec.active_count,
            spec.measurement_count,
            spec.query_count,
        )
    })
}

#[cfg(test)]
pub(crate) fn production_guard_contract() -> (usize, f64, f64, f64, f64) {
    (
        QUERY_COMPOSITION_WEIGHT,
        MIN_DENSE_PAULI_BIT_DENSITY,
        MAX_SPARSE_PAULI_BIT_DENSITY,
        MIN_ACTIVE_PAULI_BIT_DENSITY,
        MAX_ACTIVE_PAULI_BIT_DENSITY,
    )
}

#[cfg(test)]
pub(crate) fn validate_production_cases() -> Result<(), BenchError> {
    for spec in PRODUCTION_CASE_SPECS {
        build_case(
            ROW_ID,
            spec.bits_name,
            spec.queries_name,
            spec.qubit_count,
            spec.active_count,
            spec.measurement_count,
            spec.query_count,
        )?;
    }
    Ok(())
}
