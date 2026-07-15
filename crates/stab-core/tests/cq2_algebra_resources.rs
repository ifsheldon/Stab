use std::cell::Cell;

use num_complex::Complex32;
use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng as _};
use stab_core::{
    Circuit, CircuitError, CliffordString, CommutingPauliStringIterator, FlexPauliString, Flow,
    PauliBasis, PauliSign, PauliString, PauliStringIterator, SingleQubitClifford, StabilizerError,
    StabilizerResource, StabilizerResult, Tableau, TableauIterator, circuit_flow_generators,
    stabilizers_to_tableau, unitary_to_tableau,
};

fn assert_resource_limit<T>(
    result: StabilizerResult<T>,
    resource: StabilizerResource,
    requested: usize,
) {
    assert_eq!(
        result.err(),
        Some(StabilizerError::ResourceLimitExceeded {
            resource,
            requested,
            limit: resource.limit(),
        })
    );
}

#[test]
fn cq2_algebra_pauli_materialization_has_a_typed_first_rejection() {
    let resource = StabilizerResource::PauliQubits;
    assert_eq!(
        PauliString::identity(65_536).as_ref().map(PauliString::len),
        Ok(65_536)
    );
    assert_eq!(
        PauliString::identity(resource.limit())
            .as_ref()
            .map(PauliString::len),
        Ok(resource.limit())
    );
    assert_resource_limit(
        PauliString::identity(resource.limit() + 1),
        resource,
        resource.limit() + 1,
    );

    let consumed = Cell::new(0_usize);
    let bases = std::iter::from_fn(|| {
        consumed.set(consumed.get() + 1);
        Some(PauliBasis::I)
    });
    assert_resource_limit(
        PauliString::from_bases(PauliSign::Plus, bases),
        resource,
        resource.limit() + 1,
    );
    assert_eq!(consumed.get(), resource.limit() + 1);

    assert_resource_limit(
        "I".repeat(resource.limit() + 1).parse::<PauliString>(),
        resource,
        resource.limit() + 1,
    );
    assert_resource_limit(
        format!("+X{}", resource.limit()).parse::<FlexPauliString>(),
        resource,
        resource.limit() + 1,
    );
    assert_resource_limit(
        FlexPauliString::identity(resource.limit() + 1),
        resource,
        resource.limit() + 1,
    );
    assert_resource_limit(
        FlexPauliString::from_phase_and_bases(
            stab_core::PauliPhase::Plus,
            std::iter::repeat(PauliBasis::I),
        ),
        resource,
        resource.limit() + 1,
    );

    let mut actual_rng = StdRng::seed_from_u64(0x5eed_0001);
    let mut expected_rng = StdRng::seed_from_u64(0x5eed_0001);
    assert_resource_limit(
        PauliString::random(resource.limit() + 1, &mut actual_rng),
        resource,
        resource.limit() + 1,
    );
    assert_eq!(actual_rng.next_u64(), expected_rng.next_u64());
}

#[test]
fn cq2_algebra_clifford_growth_rejects_limits_and_overflow() {
    let resource = StabilizerResource::CliffordQubits;
    assert_resource_limit(
        CliffordString::identity(resource.limit() + 1),
        resource,
        resource.limit() + 1,
    );
    let consumed = Cell::new(0_usize);
    let gates = std::iter::from_fn(|| {
        consumed.set(consumed.get() + 1);
        Some(SingleQubitClifford::I)
    });
    assert_resource_limit(
        CliffordString::from_gates(gates),
        resource,
        resource.limit() + 1,
    );
    assert_eq!(consumed.get(), resource.limit() + 1);

    let left_len = resource.limit() / 2 + 1;
    let right_len = resource.limit() - left_len + 1;
    let concat = CliffordString::identity(left_len)
        .and_then(|left| CliffordString::identity(right_len).and_then(|right| left.concat(&right)));
    assert_resource_limit(concat, resource, resource.limit() + 1);

    let repeated = CliffordString::identity(left_len).and_then(|value| value.repeat(2));
    assert_resource_limit(repeated, resource, left_len * 2);

    assert_eq!(
        CliffordString::identity(2)
            .and_then(|value| value.repeat(usize::MAX))
            .err(),
        Some(StabilizerError::ResourceSizeOverflow {
            resource,
            item_count: 2,
            repetitions: usize::MAX,
        })
    );
    assert_eq!(
        CliffordString::identity(0)
            .and_then(|value| value.repeat(usize::MAX))
            .as_ref()
            .map(CliffordString::len),
        Ok(0)
    );

    let mut actual_rng = StdRng::seed_from_u64(0x5eed_0002);
    let mut expected_rng = StdRng::seed_from_u64(0x5eed_0002);
    assert_resource_limit(
        CliffordString::random(resource.limit() + 1, &mut actual_rng),
        resource,
        resource.limit() + 1,
    );
    assert_eq!(actual_rng.next_u64(), expected_rng.next_u64());
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "the resource regression needs concrete accepted values before asserting boundaries"
)]
fn cq2_algebra_flow_terms_have_an_aggregate_typed_limit() {
    let resource = StabilizerResource::FlowClassicalTerms;
    let limit_i32 = i32::try_from(resource.limit()).expect("Flow term limit fits i32");
    let identity = PauliString::identity(0).expect("empty Pauli");

    let accepted = Flow::new(identity.clone(), identity.clone(), 0..limit_i32, [])
        .expect("maximum Flow term count");
    assert_eq!(accepted.measurements().count(), resource.limit());
    let cancelled = accepted
        .multiply(&accepted)
        .expect("maximum overlapping Flow terms cancel");
    assert_eq!(cancelled.measurements().count(), 0);

    let limit_u32 = u32::try_from(resource.limit()).expect("Flow term limit fits u32");
    let observable_only = Flow::new(identity.clone(), identity.clone(), [], 0..limit_u32)
        .expect("maximum observable-only Flow term count");
    assert_resource_limit(
        accepted.multiply(&observable_only),
        resource,
        resource.limit() + 1,
    );

    assert_resource_limit(
        Flow::new(identity.clone(), identity.clone(), 0..limit_i32, [0]),
        resource,
        resource.limit() + 1,
    );

    let consumed = Cell::new(0_usize);
    let measurements = std::iter::from_fn(|| {
        consumed.set(consumed.get() + 1);
        Some(0)
    });
    assert_resource_limit(
        Flow::new(identity.clone(), identity, measurements, []),
        resource,
        resource.limit() + 1,
    );
    assert_eq!(consumed.get(), resource.limit() + 1);

    let oversized_text = format!(
        "1 -> {}",
        std::iter::repeat_n("rec[0]", resource.limit() + 1)
            .collect::<Vec<_>>()
            .join(" xor ")
    );
    assert_resource_limit(
        oversized_text.parse::<Flow>(),
        resource,
        resource.limit() + 1,
    );
}

#[test]
fn cq2_algebra_tableau_admission_precedes_materialization_and_rng_use() {
    let tableau_resource = StabilizerResource::TableauQubits;
    assert_eq!(Tableau::identity(500).as_ref().map(Tableau::len), Ok(500));
    assert_eq!(
        Tableau::identity(tableau_resource.limit())
            .as_ref()
            .map(Tableau::len),
        Ok(tableau_resource.limit())
    );
    assert_resource_limit(
        Tableau::identity(tableau_resource.limit() + 1),
        tableau_resource,
        tableau_resource.limit() + 1,
    );

    let wide_pauli = PauliString::identity(tableau_resource.limit() + 1);
    let from_pauli = wide_pauli.and_then(|pauli| Tableau::from_pauli_string(&pauli));
    assert_resource_limit(from_pauli, tableau_resource, tableau_resource.limit() + 1);

    let random_resource = StabilizerResource::RandomTableauQubits;
    let mut actual_rng = StdRng::seed_from_u64(0x5eed);
    let mut expected_rng = StdRng::seed_from_u64(0x5eed);
    assert_resource_limit(
        Tableau::random(random_resource.limit() + 1, &mut actual_rng),
        random_resource,
        random_resource.limit() + 1,
    );
    assert_eq!(actual_rng.next_u64(), expected_rng.next_u64());
}

#[test]
#[allow(
    clippy::expect_used,
    reason = "the admission regression needs concrete parsed circuits before measuring conversion work"
)]
fn cq2_algebra_circuit_tableau_and_flow_generation_admit_limits_before_dense_work() {
    let accepted_tableau = Circuit::from_stim_str("H 511\n")
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("maximum circuit-to-Tableau width");
    assert_eq!(
        accepted_tableau.len(),
        StabilizerResource::TableauQubits.limit()
    );

    let rejected_tableau = Circuit::from_stim_str("H 512\n").expect("rejected-width circuit");
    let tableau_allocations = allocation_counter::measure(|| {
        let result = rejected_tableau.to_tableau(false, false, false);
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
#[allow(
    clippy::expect_used,
    reason = "the resource regression needs concrete Tableaus before asserting exact folding"
)]
fn cq2_algebra_circuit_tableau_repeat_work_is_logarithmic_and_bounded() {
    let folded = Circuit::from_stim_str("H 0\nREPEAT 37 {\nS 0\nH 0\n}\nSQRT_X 0\n")
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("folded noncommuting repeat");
    let unrolled = Circuit::from_stim_str(&format!("H 0\n{}SQRT_X 0\n", "S 0\nH 0\n".repeat(37)))
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("unrolled noncommuting repeat");
    assert_eq!(folded, unrolled);

    let huge_repeat =
        Circuit::from_stim_str("REPEAT 1000000000001 {\nREPEAT 1000000000001 {\nH 0\n}\n}\n")
            .expect("parse nested huge repeat");
    let actual = huge_repeat
        .to_tableau(false, false, false)
        .expect("fold nested huge repeat");
    let expected = Circuit::from_stim_str("H 0\n")
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("H tableau");
    assert_eq!(actual, expected);

    let resource = StabilizerResource::CircuitTableauRepeatWork;
    let width = StabilizerResource::TableauQubits.limit();
    let work_per_composition = width * width;
    let accepted_depth = resource.limit() / work_per_composition;
    assert_eq!(resource.limit() % work_per_composition, 0);

    let nested = |depth: usize| {
        let mut body = "H 0\n".to_owned();
        for _ in 0..depth {
            body = format!("REPEAT 18446744073709551615 {{\n{body}}}\n");
        }
        format!("I 511\n{body}")
    };
    let accepted = Circuit::from_stim_str(&nested(accepted_depth))
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("last accepted aggregate compact-repeat work");
    let accepted_expected = Circuit::from_stim_str("I 511\nH 0\n")
        .and_then(|circuit| circuit.to_tableau(false, false, false))
        .expect("wide H tableau");
    assert_eq!(accepted, accepted_expected);
    let rejected = Circuit::from_stim_str(&nested(accepted_depth + 1))
        .and_then(|circuit| circuit.to_tableau(false, false, false));
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

#[test]
fn cq2_algebra_iterators_and_stabilizer_solver_fail_at_owned_boundaries() {
    let pauli_resource = StabilizerResource::PauliQubits;
    assert_resource_limit(
        PauliStringIterator::new(pauli_resource.limit() + 1, 0, 0, true, true, true),
        pauli_resource,
        pauli_resource.limit() + 1,
    );
    assert_eq!(
        CommutingPauliStringIterator::new(0).err(),
        Some(StabilizerError::InvalidCommutingPauliIteratorQubitCount { num_qubits: 0 })
    );
    assert_eq!(
        CommutingPauliStringIterator::new(64).err(),
        Some(StabilizerError::InvalidCommutingPauliIteratorQubitCount { num_qubits: 64 })
    );
    assert_eq!(
        TableauIterator::new(64, false).err(),
        Some(StabilizerError::InvalidTableauIteratorQubitCount { num_qubits: 64 })
    );

    let solve_resource = StabilizerResource::StabilizerSolveQubits;
    let solve_result = PauliString::identity(solve_resource.limit() + 1)
        .and_then(|stabilizer| stabilizers_to_tableau(&[stabilizer], false, true, false));
    assert_resource_limit(solve_result, solve_resource, solve_resource.limit() + 1);
}

#[test]
fn cq2_algebra_unitary_dimension_limit_precedes_shape_and_numeric_work() {
    let resource = StabilizerResource::UnitaryMatrixDimension;
    let oversized_malformed = vec![Vec::<Complex32>::new(); resource.limit() * 2];
    assert_resource_limit(
        unitary_to_tableau(&oversized_malformed, false),
        resource,
        resource.limit() * 2,
    );
}
