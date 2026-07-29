use crate::{Circuit, CircuitResult, DetectorErrorModel, ExplainedError};

pub fn explain_errors_from_circuit(
    circuit: &Circuit,
    filter: Option<&DetectorErrorModel>,
    reduce_to_one_representative_error: bool,
) -> CircuitResult<Vec<ExplainedError>> {
    stab_analysis::explain_errors_from_circuit(circuit, filter, reduce_to_one_representative_error)
        .map(|errors| {
            errors
                .into_iter()
                .map(ExplainedError::from_analysis)
                .collect()
        })
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::expect_used,
        reason = "facade tests use direct assertions for compact diagnostics"
    )]

    use super::explain_errors_from_circuit;
    use crate::Circuit;

    #[test]
    fn facade_preserves_exact_error_explanation_output() {
        let circuit = Circuit::from_stim_str(
            "\
                R 0
                X_ERROR(0.25) 0
                M 0
                DETECTOR rec[-1]
            ",
        )
        .expect("valid circuit");

        let errors =
            explain_errors_from_circuit(&circuit, None, false).expect("explain circuit error");
        assert_eq!(errors.len(), 1);
        assert!(
            errors
                .first()
                .expect("one explained error")
                .to_string()
                .contains("flipped_pauli_product: X0")
        );
    }
}
