use crate::{Circuit, Flow, PauliBasis, PauliString};

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
