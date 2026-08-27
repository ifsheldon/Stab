use std::cell::Cell;

use num_complex::Complex32;
use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng as _};
use stab_algebra::{
    CommutingPauliStringIterator, FlexPauliString, Flow, PauliBasis, PauliPhase, PauliSign,
    PauliString, PauliStringIterator, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau, TableauIterator, stabilizers_to_tableau, unitary_to_tableau,
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
fn algebra_size_limits_reject_before_materialization_or_rng_use() {
    pauli_materialization_has_a_typed_first_rejection();
    flow_terms_have_an_aggregate_typed_limit();
    tableau_admission_precedes_materialization_and_rng_use();
    iterators_and_stabilizer_solver_fail_at_owned_boundaries();
    unitary_dimension_limit_precedes_shape_and_numeric_work();
}

fn pauli_materialization_has_a_typed_first_rejection() {
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
        FlexPauliString::from_phase_and_bases(PauliPhase::Plus, std::iter::repeat(PauliBasis::I)),
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

#[allow(
    clippy::expect_used,
    reason = "the resource regression needs concrete accepted values before asserting boundaries"
)]
fn flow_terms_have_an_aggregate_typed_limit() {
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

fn tableau_admission_precedes_materialization_and_rng_use() {
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

fn iterators_and_stabilizer_solver_fail_at_owned_boundaries() {
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

fn unitary_dimension_limit_precedes_shape_and_numeric_work() {
    let resource = StabilizerResource::UnitaryMatrixDimension;
    let oversized_malformed = vec![Vec::<Complex32>::new(); resource.limit() * 2];
    assert_resource_limit(
        unitary_to_tableau(&oversized_malformed, false),
        resource,
        resource.limit() * 2,
    );
}
