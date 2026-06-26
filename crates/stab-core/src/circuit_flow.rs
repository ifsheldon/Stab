use crate::{Circuit, CircuitError, CircuitResult, Flow, PauliBasis, PauliSign, PauliString};

/// Returns unsigned stabilizer-flow generators for the supported unitary tableau subset.
///
/// The current M6 implementation derives generators from `Circuit::to_tableau`, so
/// measurement, observable, detector, and noisy-flow semantics are not included.
pub fn circuit_flow_generators(circuit: &Circuit) -> CircuitResult<Vec<Flow>> {
    let tableau = circuit.to_tableau(false, false, false)?;
    let mut flows = Vec::with_capacity(tableau.len() * 2);
    for index in (0..tableau.len()).rev() {
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::X),
            tableau
                .x_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
        flows.push(Flow::new(
            single_pauli(tableau.len(), index, PauliBasis::Z),
            tableau
                .z_output(index)
                .map_err(stabilizer_to_circuit_error)?
                .clone(),
            [],
            [],
        ));
    }
    Ok(flows)
}

/// Checks unsigned stabilizer flows against the supported unitary tableau subset.
///
/// Flows with measurement or observable dependencies return `false` in M6 because
/// those Stim flow semantics require later simulator and detector milestones.
pub fn check_if_circuit_has_unsigned_stabilizer_flows(
    circuit: &Circuit,
    flows: &[Flow],
) -> Vec<bool> {
    let Ok(tableau) = circuit.to_tableau(false, false, false) else {
        return vec![false; flows.len()];
    };
    flows
        .iter()
        .map(|flow| {
            if flow.measurements().next().is_some() || flow.observables().next().is_some() {
                return false;
            }
            tableau
                .apply(flow.input())
                .is_ok_and(|actual| paulis_match_unsigned(&actual, flow.output()))
        })
        .collect()
}

fn paulis_match_unsigned(left: &PauliString, right: &PauliString) -> bool {
    (0..left.len().max(right.len())).all(|index| {
        left.get(index).unwrap_or(PauliBasis::I) == right.get(index).unwrap_or(PauliBasis::I)
    })
}

fn single_pauli(len: usize, index: usize, basis: PauliBasis) -> PauliString {
    PauliString::from_bases(
        PauliSign::Plus,
        (0..len).map(|candidate| {
            if candidate == index {
                basis
            } else {
                PauliBasis::I
            }
        }),
    )
}

fn stabilizer_to_circuit_error(error: crate::StabilizerError) -> CircuitError {
    CircuitError::invalid_tableau_conversion(error.to_string())
}
