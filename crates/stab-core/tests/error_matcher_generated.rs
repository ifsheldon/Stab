#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    CodeDistance, Probability, RepetitionCodeParams, RepetitionCodeTask, RoundCount,
    explain_errors_from_circuit, generate_repetition_code_circuit,
};

#[test]
fn generated_repetition_code_data_depolarization_matches_stim_error_matcher() {
    assert_eq!(
        explain_generated_repetition_code(false),
        r"
ExplainedError {
    dem_error_terms: D0[coords 1,0]
    CircuitErrorLocation {
        flipped_pauli_product: X0
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y0
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
}

ExplainedError {
    dem_error_terms: D0[coords 1,0] D1[coords 3,0]
    CircuitErrorLocation {
        flipped_pauli_product: X2
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y2
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
}

ExplainedError {
    dem_error_terms: D1[coords 3,0] L0
    CircuitErrorLocation {
        flipped_pauli_product: X4
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y4
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
}

ExplainedError {
    dem_error_terms: D2[coords 1,1]
    CircuitErrorLocation {
        flipped_pauli_product: X0
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y0
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
}

ExplainedError {
    dem_error_terms: D2[coords 1,1] D3[coords 3,1]
    CircuitErrorLocation {
        flipped_pauli_product: X2
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y2
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
}

ExplainedError {
    dem_error_terms: D3[coords 3,1] L0
    CircuitErrorLocation {
        flipped_pauli_product: X4
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y4
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
}
"
    );
}

#[test]
fn generated_repetition_code_data_depolarization_representatives_match_stim_error_matcher() {
    assert_eq!(
        explain_generated_repetition_code(true),
        r"
ExplainedError {
    dem_error_terms: D0[coords 1,0]
    CircuitErrorLocation {
        flipped_pauli_product: X0
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
}

ExplainedError {
    dem_error_terms: D0[coords 1,0] D1[coords 3,0]
    CircuitErrorLocation {
        flipped_pauli_product: X2
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
}

ExplainedError {
    dem_error_terms: D1[coords 3,0] L0
    CircuitErrorLocation {
        flipped_pauli_product: X4
        Circuit location stack trace:
            (after 1 TICKs)
            at instruction #3 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
}

ExplainedError {
    dem_error_terms: D2[coords 1,1]
    CircuitErrorLocation {
        flipped_pauli_product: X0
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #1 of the instruction
            resolving to DEPOLARIZE1(0.001) 0
    }
}

ExplainedError {
    dem_error_terms: D2[coords 1,1] D3[coords 3,1]
    CircuitErrorLocation {
        flipped_pauli_product: X2
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #2 of the instruction
            resolving to DEPOLARIZE1(0.001) 2
    }
}

ExplainedError {
    dem_error_terms: D3[coords 3,1] L0
    CircuitErrorLocation {
        flipped_pauli_product: X4
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #12 (DEPOLARIZE1) in the circuit
            at target #3 of the instruction
            resolving to DEPOLARIZE1(0.001) 4
    }
}
"
    );
}

fn explain_generated_repetition_code(reduce_to_one_representative_error: bool) -> String {
    let params = RepetitionCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(3).expect("distance"),
        RepetitionCodeTask::Memory,
    )
    .expect("params")
    .with_before_round_data_depolarization(Probability::try_new(0.001).expect("probability"));
    let generated =
        generate_repetition_code_circuit(&params).expect("generate repetition code circuit");
    let actual = explain_errors_from_circuit(
        generated.circuit(),
        None,
        reduce_to_one_representative_error,
    )
    .expect("explain errors");

    let mut out = String::new();
    for matched_error in actual {
        out.push('\n');
        out.push_str(&matched_error.to_string());
        out.push('\n');
    }
    out
}
