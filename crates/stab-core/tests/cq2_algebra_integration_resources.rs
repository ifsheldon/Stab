#![allow(
    clippy::expect_used,
    reason = "integration resource regressions require concrete parsed circuits and accepted tableaus"
)]

use stab_core::{
    Circuit, CircuitError, RepeatBlock, RepeatCount, StabilizerResource,
    analysis::circuit_to_tableau, circuit_flow_generators,
};

#[test]
fn cq2_algebra_circuit_tableau_and_flow_generation_admit_limits_before_dense_work() {
    let accepted_tableau = Circuit::from_stim_str("H 511\n")
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_to_tableau(&circuit, false, false, false))
        .expect("maximum circuit-to-Tableau width");
    assert_eq!(
        accepted_tableau.len(),
        StabilizerResource::TableauQubits.limit()
    );

    let rejected_tableau = Circuit::from_stim_str("H 512\n").expect("rejected-width circuit");
    let tableau_allocations = allocation_counter::measure(|| {
        let result = circuit_to_tableau(&rejected_tableau, false, false, false);
        assert!(matches!(
            result,
            Err(CircuitError::InvalidTableauConversion { ref message })
                if message == "Tableau qubits request 513 exceeds limit 512"
        ));
        drop(std::hint::black_box(result));
    });
    assert!(
        tableau_allocations.count_total <= 8
            && tableau_allocations.bytes_total <= 1_024
            && tableau_allocations.bytes_max <= 512,
        "circuit-to-Tableau rejection performed dense work: {tableau_allocations:?}"
    );

    let accepted_flows = Circuit::from_stim_str("QUBIT_COORDS(0) 4095\n")
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_flow_generators(&circuit))
        .expect("maximum ignored-only flow-generator width");
    assert_eq!(accepted_flows.len(), 2 * 4096);

    let rejected_flows =
        Circuit::from_stim_str("QUBIT_COORDS(0) 4096\n").expect("rejected-width flow circuit");
    let flow_allocations = allocation_counter::measure(|| {
        let result = circuit_flow_generators(&rejected_flows);
        assert!(matches!(
            result,
            Err(CircuitError::InvalidDomainValue {
                kind: "ignored-only flow-generator Pauli bits",
                ref value,
            }) if value == "134283272 exceeds current limit 134217728"
        ));
        drop(std::hint::black_box(result));
    });
    assert!(
        flow_allocations.count_total <= 8
            && flow_allocations.bytes_total <= 1_024
            && flow_allocations.bytes_max <= 512,
        "flow-generator rejection performed dense work: {flow_allocations:?}"
    );
}

#[test]
fn cq2_algebra_circuit_tableau_repeat_work_is_logarithmic_and_bounded() {
    let folded = Circuit::from_stim_str("H 0\nREPEAT 37 {\nS 0\nH 0\n}\nSQRT_X 0\n")
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_to_tableau(&circuit, false, false, false))
        .expect("folded noncommuting repeat");
    let unrolled = Circuit::from_stim_str(&format!("H 0\n{}SQRT_X 0\n", "S 0\nH 0\n".repeat(37)))
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_to_tableau(&circuit, false, false, false))
        .expect("unrolled noncommuting repeat");
    assert_eq!(folded, unrolled);

    let huge_repeat =
        Circuit::from_stim_str("REPEAT 1000000000001 {\nREPEAT 1000000000001 {\nH 0\n}\n}\n")
            .expect("parse nested huge repeat");
    let actual =
        circuit_to_tableau(&huge_repeat, false, false, false).expect("fold nested huge repeat");
    let expected = Circuit::from_stim_str("H 0\n")
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_to_tableau(&circuit, false, false, false))
        .expect("H tableau");
    assert_eq!(actual, expected);

    let resource = StabilizerResource::CircuitTableauRepeatWork;
    let width = StabilizerResource::TableauQubits.limit();
    let work_per_composition = width * width;
    let accepted_depth = resource.limit() / work_per_composition;
    assert_eq!(resource.limit() % work_per_composition, 0);

    let nested = |depth: usize| {
        let mut body = Circuit::from_stim_str("H 0\n").expect("parse nested repeat leaf");
        for _ in 0..depth {
            let mut outer = Circuit::new();
            outer.append_repeat_block(RepeatBlock::new(
                RepeatCount::try_new(u64::MAX).expect("programmatic repeat count"),
                body,
                None,
            ));
            body = outer;
        }
        let mut result = Circuit::from_stim_str("I 511\n").expect("parse width marker");
        result.append_circuit(&body);
        result
    };
    let accepted = circuit_to_tableau(&nested(accepted_depth), false, false, false)
        .expect("last accepted aggregate compact-repeat work");
    let accepted_expected = Circuit::from_stim_str("I 511\nH 0\n")
        .map_err(CircuitError::from)
        .and_then(|circuit| circuit_to_tableau(&circuit, false, false, false))
        .expect("wide H tableau");
    assert_eq!(accepted, accepted_expected);
    let rejected = circuit_to_tableau(&nested(accepted_depth + 1), false, false, false);
    assert!(matches!(
        rejected,
        Err(CircuitError::InvalidTableauConversion { ref message })
            if message == &format!(
                "circuit Tableau repeat work units request {} exceeds limit {}",
                resource.limit() + work_per_composition,
                resource.limit()
            )
    ));
}
