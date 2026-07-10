use crate::{CircuitResult, Flow, PauliSign, PauliString};

use super::helpers::{internal_flow_error, record_index_i32, stabilizer_to_circuit_error};

pub(super) fn final_canonicalize_measurement_generators(
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
