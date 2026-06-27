#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct assertions for compact diagnostics"
)]

use stab_core::{Circuit, ExplainedError, explain_errors_from_circuit};

#[test]
fn pauli_channel2_components_match_stim_error_matcher_order() {
    // Ported from Stim v1.16.0 src/stim/simulators/error_matcher.test.cc PAULI_CHANNEL_2.
    let expected_products = [
        "X1", "Y1", "Z1", "X0", "X0*X1", "X0*Y1", "X0*Z1", "Y0", "Y0*X1", "Y0*Y1", "Y0*Z1", "Z0",
        "Z0*X1", "Z0*Y1", "Z0*Z1",
    ];

    for (component_index, expected_product) in expected_products.iter().copied().enumerate() {
        let actual = explain_errors_from_circuit(
            &Circuit::from_stim_str(&pauli_channel2_circuit(component_index)).expect("circuit"),
            None,
            false,
        )
        .expect("explain errors");

        assert_eq!(actual.len(), 1, "component {component_index}");
        let error = actual.first().expect("one explained error");
        assert_eq!(
            error.circuit_error_locations.len(),
            1,
            "component {component_index}"
        );
        assert_eq!(
            flipped_pauli_product(error),
            expected_product,
            "component {component_index}"
        );
    }
}

fn pauli_channel2_circuit(component_index: usize) -> String {
    let mut args = ["0"; 15];
    let slot = args
        .get_mut(component_index)
        .expect("component index is inside PAULI_CHANNEL_2 argument range");
    *slot = "0.1";
    format!(
        "MXX 0 2 1 3\n\
         MZZ 0 2 1 3\n\
         PAULI_CHANNEL_2({}) 0 1\n\
         MXX 0 2 1 3\n\
         MZZ 0 2 1 3\n\
         DETECTOR rec[-1] rec[-5]\n\
         DETECTOR rec[-2] rec[-6]\n\
         DETECTOR rec[-3] rec[-7]\n\
         DETECTOR rec[-4] rec[-8]\n",
        args.join(", ")
    )
}

fn flipped_pauli_product(error: &ExplainedError) -> String {
    error
        .circuit_error_locations
        .first()
        .expect("one circuit error location")
        .flipped_pauli_product
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("*")
}
