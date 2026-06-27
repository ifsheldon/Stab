#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "compatibility tests use direct assertions for compact diagnostics"
)]

use stab_core::{
    CodeDistance, DetectorErrorModel, Probability, RepetitionCodeParams, RepetitionCodeTask,
    RoundCount, SurfaceCodeParams, SurfaceCodeTask, explain_errors_from_circuit,
    generate_repetition_code_circuit, generate_surface_code_circuit,
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

#[test]
fn generated_surface_code_clifford_depolarization_filter_matches_stim_error_matcher() {
    let params = SurfaceCodeParams::new(
        RoundCount::try_new(2).expect("rounds"),
        CodeDistance::try_new(2).expect("distance"),
        SurfaceCodeTask::RotatedMemoryZ,
    )
    .expect("params")
    .with_after_clifford_depolarization(Probability::try_new(0.001).expect("probability"));
    let generated = generate_surface_code_circuit(&params).expect("generate surface code circuit");
    let filter = DetectorErrorModel::from_dem_str("error(1) D0 D1\n").expect("filter DEM");
    let actual = explain_errors_from_circuit(generated.circuit(), Some(&filter), false)
        .expect("explain errors");

    assert_eq!(
        format_explanations(actual),
        r"
ExplainedError {
    dem_error_terms: D0[coords 2,2,0] D1[coords 2,0,1]
    CircuitErrorLocation {
        flipped_pauli_product: X6[coords 1,3]*Y7[coords 2,2]
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #20 (DEPOLARIZE2) in the circuit
            at targets #3 to #4 of the instruction
            resolving to DEPOLARIZE2(0.001) 6[coords 1,3] 7[coords 2,2]
    }
    CircuitErrorLocation {
        flipped_pauli_product: Y6[coords 1,3]*Y7[coords 2,2]
        Circuit location stack trace:
            (after 4 TICKs)
            at instruction #20 (DEPOLARIZE2) in the circuit
            at targets #3 to #4 of the instruction
            resolving to DEPOLARIZE2(0.001) 6[coords 1,3] 7[coords 2,2]
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

    format_explanations(actual)
}

fn format_explanations(actual: Vec<stab_core::ExplainedError>) -> String {
    let mut out = String::new();
    for matched_error in actual {
        out.push('\n');
        out.push_str(&matched_error.to_string());
        out.push('\n');
    }
    out
}
