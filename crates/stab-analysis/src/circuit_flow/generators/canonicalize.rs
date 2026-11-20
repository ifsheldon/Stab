use stab_algebra::{Flow, PauliSign, PauliString};

use crate::AnalysisResult;

use super::helpers::{internal_flow_error, record_index_i32, stabilizer_to_circuit_error};

pub(super) fn final_canonicalize_measurement_generators(
    flows: &mut [Flow],
    qubit_count: usize,
    measurement_count: usize,
) -> AnalysisResult<()> {
    // Row products only ever set bits some row already has, so a column absent from the union of
    // every row stays absent throughout elimination and needs no scan.
    let support = ColumnSupport::of(flows);
    let mut eliminated = 0;
    for qubit in 0..qubit_count {
        if support.input_x.has(qubit) {
            eliminate_rows_with(flows, &mut eliminated, |flow| {
                flow.input().get(qubit).is_some_and(|basis| basis.x_bit())
            })?;
        }
        if support.input_z.has(qubit) {
            eliminate_rows_with(flows, &mut eliminated, |flow| {
                flow.input().get(qubit).is_some_and(|basis| basis.z_bit())
            })?;
        }
    }
    for qubit in 0..qubit_count {
        if support.output_x.has(qubit) {
            eliminate_rows_with(flows, &mut eliminated, |flow| {
                flow.output().get(qubit).is_some_and(|basis| basis.x_bit())
            })?;
        }
        if support.output_z.has(qubit) {
            eliminate_rows_with(flows, &mut eliminated, |flow| {
                flow.output().get(qubit).is_some_and(|basis| basis.z_bit())
            })?;
        }
    }
    for measurement in 0..measurement_count {
        let measurement = record_index_i32(measurement)?;
        eliminate_rows_with(flows, &mut eliminated, |flow| {
            flow.measurements().any(|record| record == measurement)
        })?;
    }

    for flow in flows.iter_mut() {
        *flow = flow_with_final_sign_and_trimmed_identities(flow)?;
    }
    flows.sort();
    Ok(())
}

/// Union of one packed Pauli plane over every row.
struct PlaneUnion(Vec<u64>);

impl PlaneUnion {
    fn of<'a>(planes: impl Iterator<Item = &'a [u64]>) -> Self {
        let mut union = Vec::new();
        for plane in planes {
            if union.len() < plane.len() {
                union.resize(plane.len(), 0);
            }
            for (word, bits) in union.iter_mut().zip(plane) {
                *word |= bits;
            }
        }
        Self(union)
    }

    fn has(&self, qubit: usize) -> bool {
        self.0
            .get(qubit / u64::BITS as usize)
            .is_some_and(|word| word >> (qubit % u64::BITS as usize) & 1 == 1)
    }
}

/// Which Pauli columns any row touches.
struct ColumnSupport {
    input_x: PlaneUnion,
    input_z: PlaneUnion,
    output_x: PlaneUnion,
    output_z: PlaneUnion,
}

impl ColumnSupport {
    fn of(flows: &[Flow]) -> Self {
        Self {
            input_x: PlaneUnion::of(flows.iter().map(|flow| flow.input().x_bits())),
            input_z: PlaneUnion::of(flows.iter().map(|flow| flow.input().z_bits())),
            output_x: PlaneUnion::of(flows.iter().map(|flow| flow.output().x_bits())),
            output_z: PlaneUnion::of(flows.iter().map(|flow| flow.output().z_bits())),
        }
    }
}

fn eliminate_rows_with(
    flows: &mut [Flow],
    eliminated: &mut usize,
    predicate: impl Fn(&Flow) -> bool,
) -> AnalysisResult<()> {
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

fn flow_with_final_sign_and_trimmed_identities(flow: &Flow) -> AnalysisResult<Flow> {
    let output_sign = xor_sign(flow.output().sign(), flow.input().sign());
    Flow::new(
        trimmed_pauli_with_sign(flow.input(), PauliSign::Plus),
        trimmed_pauli_with_sign(flow.output(), output_sign),
        flow.measurements(),
        flow.observables(),
    )
    .map_err(stabilizer_to_circuit_error)
}

fn trimmed_pauli_with_sign(pauli: &PauliString, sign: PauliSign) -> PauliString {
    if pauli.has_no_pauli_terms() {
        stab_algebra::advanced::pauli_from_bases_unbudgeted(sign, [])
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
