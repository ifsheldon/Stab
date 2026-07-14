use std::cell::Cell;

use num_complex::Complex32;
use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng as _};
use stab_core::{
    Circuit, CircuitError, CliffordString, CommutingPauliStringIterator, FlexPauliString,
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
fn cq2_algebra_tableau_admission_precedes_materialization_and_rng_use() {
    let tableau_resource = StabilizerResource::TableauQubits;
    assert_eq!(Tableau::identity(500).as_ref().map(Tableau::len), Ok(500));
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
fn cq2_algebra_circuit_tableau_and_flow_generation_reject_before_dense_work() {
    let tableau_result = Circuit::from_stim_str("H 512\n")
        .and_then(|circuit| circuit.to_tableau(false, false, false));
    assert!(matches!(
        tableau_result,
        Err(CircuitError::InvalidTableauConversion { ref message })
            if message.contains("Tableau qubits request 513 exceeds limit 512")
    ));

    let flow_result = Circuit::from_stim_str("QUBIT_COORDS(0) 4096\n")
        .and_then(|circuit| circuit_flow_generators(&circuit));
    assert!(matches!(
        flow_result,
        Err(CircuitError::InvalidDomainValue {
            kind: "ignored-only flow-generator Pauli bits",
            ..
        })
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
