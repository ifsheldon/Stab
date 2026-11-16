#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "byte-admission regressions need concrete accepted values before asserting boundaries, and report an unexpected outcome by value"
)]

use std::str::FromStr as _;

use rand::rngs::StdRng;
use rand::{Rng as _, SeedableRng as _};
use stab_algebra::{
    AlgebraMemoryBudget, CliffordString, CommutingPauliStringIterator, FlexPauliString, Flow,
    PauliBasis, PauliPhase, PauliSign, PauliString, PauliStringIterator, SingleQubitClifford,
    StabilizerError, StabilizerResource, StabilizerResult, Tableau, TableauIterator,
    stabilizers_to_tableau_with_budget,
};

#[test]
fn algebra_byte_admission_refuses_before_allocation_or_rng_use() {
    pauli_widths_are_admitted_in_bytes();
    flow_terms_are_bounded_by_their_input();
    tableau_admission_is_exact_and_precedes_allocation();
    random_tableau_admission_is_exact_and_precedes_rng_use();
    stabilizer_solver_admission_is_exact_and_precedes_allocation();
    iterator_widths_keep_their_representation_invariants();
    caller_budgets_govern_requested_string_widths();
}

/// Returns the bytes a refused request reported, after checking the refusal's shape.
fn refused_bytes<T: std::fmt::Debug>(
    result: StabilizerResult<T>,
    resource: StabilizerResource,
    budget: AlgebraMemoryBudget,
) -> u128 {
    match result {
        Err(StabilizerError::MemoryBudgetExceeded {
            resource: actual,
            requested_bytes,
            budget_bytes,
        }) => {
            assert_eq!(actual, resource);
            assert_eq!(budget_bytes, budget.bytes());
            assert!(requested_bytes > u128::from(budget_bytes));
            requested_bytes
        }
        other => panic!("expected a {resource} budget refusal, got {other:?}"),
    }
}

/// Checks that `request` is refused one byte below its requirement without allocating and is
/// admitted at exactly its requirement, returning that requirement.
fn assert_exact_boundary<T: std::fmt::Debug>(
    resource: StabilizerResource,
    mut request: impl FnMut(AlgebraMemoryBudget) -> StabilizerResult<T>,
) -> u64 {
    let required = refused_bytes(
        request(AlgebraMemoryBudget::from_bytes(0)),
        resource,
        AlgebraMemoryBudget::from_bytes(0),
    );
    let required = u64::try_from(required).expect("test requirement fits u64");
    let short = AlgebraMemoryBudget::from_bytes(required - 1);
    let mut refusal = None;
    let allocations = allocation_counter::measure(|| refusal = Some(request(short)));
    assert_eq!(
        refused_bytes(refusal.expect("refusal ran"), resource, short),
        u128::from(required)
    );
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    request(AlgebraMemoryBudget::from_bytes(required)).expect("the exact budget is admitted");
    required
}

fn pauli_widths_are_admitted_in_bytes() {
    let default = AlgebraMemoryBudget::DEFAULT;
    // The 65,536-qubit regressions and one past the former 1,048,576-qubit count cap.
    assert_eq!(
        PauliString::identity(65_536).map(|pauli| pauli.len()),
        Ok(65_536)
    );
    assert_eq!(
        FlexPauliString::from_str("+X65535").map(|pauli| pauli.len()),
        Ok(65_536)
    );
    assert_eq!(
        PauliString::identity(1_048_577).map(|pauli| pauli.len()),
        Ok(1_048_577)
    );
    refused_bytes(
        PauliString::identity(usize::MAX / 2),
        StabilizerResource::PauliString,
        default,
    );
    // A compact sparse literal cannot request a huge allocation.
    refused_bytes(
        FlexPauliString::from_str("+X1000000000000"),
        StabilizerResource::PauliString,
        default,
    );
    // Nor can a flow's sparse term, which reports the refusal rather than malformed flow text.
    refused_bytes(
        Flow::from_str("X1000000000000 -> Z0"),
        StabilizerResource::PauliString,
        default,
    );
    let mut actual_rng = StdRng::seed_from_u64(0x5eed_0001);
    let mut expected_rng = StdRng::seed_from_u64(0x5eed_0001);
    refused_bytes(
        PauliString::random(usize::MAX / 2, &mut actual_rng),
        StabilizerResource::PauliString,
        default,
    );
    assert_eq!(actual_rng.next_u64(), expected_rng.next_u64());
}

fn flow_terms_are_bounded_by_their_input() {
    // One past the former 65,536-term count cap: each stored term is one supplied item.
    let identity = PauliString::identity(0).expect("empty Pauli");
    let terms = 65_537;
    let flow = Flow::new(identity.clone(), identity.clone(), 0..terms, 0..2)
        .expect("flow terms are bounded by the supplied iterator");
    assert_eq!(flow.measurements().count(), 65_537);
    let cancelled = flow.multiply(&flow).expect("overlapping terms cancel");
    assert_eq!(cancelled.measurements().count(), 0);
    assert_eq!(cancelled.observables().count(), 0);
}

fn tableau_admission_is_exact_and_precedes_allocation() {
    let width = 1_000;
    let required = assert_exact_boundary(StabilizerResource::Tableau, |budget| {
        Tableau::identity_with_budget(width, budget)
    });
    // 2n packed rows of two 16-word planes plus a 32-word sign plane.
    assert_eq!(required, 2_000 * 2 * 16 * 8 + 32 * 8);

    let default = AlgebraMemoryBudget::DEFAULT;
    refused_bytes(
        Tableau::identity(usize::MAX / 4),
        StabilizerResource::Tableau,
        default,
    );
    let phase = Tableau::gate1("+Y", "+Z").expect("phase gate");
    refused_bytes(
        phase.embedded(1_000_000, &[0]),
        StabilizerResource::Tableau,
        default,
    );
    let wide_pauli = PauliString::identity(100_000).expect("wide Pauli string");
    refused_bytes(
        Tableau::from_pauli_string(&wide_pauli),
        StabilizerResource::Tableau,
        default,
    );
}

fn random_tableau_admission_is_exact_and_precedes_rng_use() {
    let width = 1_000;
    let mut rng = StdRng::seed_from_u64(0x5eed);
    let mut expected_rng = StdRng::seed_from_u64(0x5eed);
    let required = refused_bytes(
        Tableau::random_with_budget(width, AlgebraMemoryBudget::from_bytes(0), &mut rng),
        StabilizerResource::RandomTableau,
        AlgebraMemoryBudget::from_bytes(0),
    );
    let required = u64::try_from(required).expect("random requirement fits u64");
    let short = AlgebraMemoryBudget::from_bytes(required - 1);
    let mut refusal = None;
    let allocations = allocation_counter::measure(|| {
        refusal = Some(Tableau::random_with_budget(width, short, &mut rng))
    });
    refused_bytes(
        refusal.expect("refusal ran"),
        StabilizerResource::RandomTableau,
        short,
    );
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(rng.next_u64(), expected_rng.next_u64());
    let tableau =
        Tableau::random_with_budget(width, AlgebraMemoryBudget::from_bytes(required), &mut rng)
            .expect("the exact budget is admitted");
    assert_eq!(tableau.len(), width);
    // The sampling matrices stay within a small multiple of the output's packed rows.
    let output = 2_000 * 2 * 16 * 8 + 32 * 8;
    assert!(
        required <= 8 * output,
        "{required} bytes for a {output}-byte output"
    );
}

fn stabilizer_solver_admission_is_exact_and_precedes_allocation() {
    let width = 1_000;
    let stabilizers = (0..width)
        .map(|qubit| {
            let mut text = vec!['_'; width];
            if let Some(slot) = text.get_mut(qubit) {
                *slot = 'Z';
            }
            PauliString::from_str(&text.into_iter().collect::<String>()).expect("Z stabilizer")
        })
        .collect::<Vec<_>>();
    for inverse in [false, true] {
        assert_exact_boundary(StabilizerResource::StabilizerSolve, |budget| {
            stabilizers_to_tableau_with_budget(&stabilizers, false, false, inverse, budget)
        });
    }
    // A single compact stabilizer cannot request an unbounded underconstrained completion.
    let wide = PauliString::identity(100_000).expect("wide stabilizer");
    refused_bytes(
        stabilizers_to_tableau_with_budget(
            std::slice::from_ref(&wide),
            false,
            true,
            false,
            AlgebraMemoryBudget::DEFAULT,
        ),
        StabilizerResource::StabilizerSolve,
        AlgebraMemoryBudget::DEFAULT,
    );
}

fn iterator_widths_keep_their_representation_invariants() {
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
}

/// Bytes of `planes` packed planes of `bits` bits, the documented algebra storage formula.
fn packed_plane_bytes(planes: u64, bits: u64) -> u64 {
    planes * bits.div_ceil(64) * 8
}

/// Checks that an item-collecting constructor admitting `required` bytes for its whole input is
/// refused one byte short, naming that budget, and is admitted at exactly `required`.
fn assert_collected_boundary<T: std::fmt::Debug>(
    resource: StabilizerResource,
    required: u64,
    mut request: impl FnMut(AlgebraMemoryBudget) -> StabilizerResult<T>,
) {
    let short = AlgebraMemoryBudget::from_bytes(required - 1);
    assert_eq!(
        refused_bytes(request(short), resource, short),
        u128::from(required)
    );
    request(AlgebraMemoryBudget::from_bytes(required)).expect("the exact budget is admitted");
}

fn caller_budgets_govern_requested_string_widths() {
    let width = 1_000_u64;
    let n = 1_000_usize;
    let pauli = packed_plane_bytes(2, width);
    let clifford = packed_plane_bytes(6, width) + width;

    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            PauliString::identity_with_budget(n, budget)
        }),
        pauli
    );
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            PauliString::random_with_budget(n, budget, &mut StdRng::seed_from_u64(1))
        }),
        pauli
    );
    let dense = format!("-{}", "X".repeat(n));
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            PauliString::from_str_with_budget(&dense, budget)
        }),
        pauli
    );
    // A short sparse literal requests its largest index plus one qubits.
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            FlexPauliString::from_str_with_budget("iX999", budget)
        }),
        pauli
    );
    // Each Pauli side of a flow is admitted on its own against the caller budget.
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            Flow::from_str_with_budget("X999 -> Z999 xor rec[-1]", budget)
        }),
        pauli
    );
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            Flow::from_str_with_budget("1 -> -Y999", budget)
        }),
        pauli
    );
    // Collected bases are charged one byte each beside the packed planes.
    assert_collected_boundary(StabilizerResource::PauliString, pauli + width, |budget| {
        PauliString::from_bases_with_budget(PauliSign::Plus, vec![PauliBasis::Y; n], budget)
    });
    assert_collected_boundary(StabilizerResource::PauliString, pauli + width, |budget| {
        FlexPauliString::from_phase_and_bases_with_budget(
            PauliPhase::MinusI,
            vec![PauliBasis::Z; n],
            budget,
        )
    });
    assert_eq!(
        assert_exact_boundary(StabilizerResource::PauliString, |budget| {
            PauliStringIterator::new_with_budget(n, 1, 1, true, true, true, budget)
        }),
        pauli + width * (size_of::<usize>() + size_of::<PauliBasis>()) as u64
    );

    // Six packed planes plus one table-index byte per gate.
    assert_eq!(
        assert_exact_boundary(StabilizerResource::CliffordString, |budget| {
            CliffordString::identity_with_budget(n, budget)
        }),
        clifford
    );
    assert_eq!(
        assert_exact_boundary(StabilizerResource::CliffordString, |budget| {
            CliffordString::random_with_budget(n, budget, &mut StdRng::seed_from_u64(2))
        }),
        clifford
    );
    let half = CliffordString::identity(n / 2).expect("half-width string");
    assert_eq!(
        assert_exact_boundary(StabilizerResource::CliffordString, |budget| {
            half.concat_with_budget(&half, budget)
        }),
        clifford
    );
    let one = CliffordString::from_gates([SingleQubitClifford::H]).expect("one gate");
    assert_eq!(
        assert_exact_boundary(StabilizerResource::CliffordString, |budget| {
            one.repeat_with_budget(n, budget)
        }),
        clifford
    );
    // Collected gates are charged their own size beside the planes and table indices.
    assert_collected_boundary(
        StabilizerResource::CliffordString,
        clifford + width * size_of::<SingleQubitClifford>() as u64,
        |budget| CliffordString::from_gates_with_budget(vec![SingleQubitClifford::S; n], budget),
    );

    // 2n packed rows of two planes plus one packed sign plane.
    let tableau = 2 * width * packed_plane_bytes(2, width) + packed_plane_bytes(1, 2 * width);
    let source = PauliString::identity(n).expect("conjugating Pauli");
    assert_eq!(
        assert_exact_boundary(StabilizerResource::Tableau, |budget| {
            Tableau::from_pauli_string_with_budget(&source, budget)
        }),
        tableau
    );
    let phase = Tableau::gate1("+Y", "+Z").expect("phase gate");
    assert_eq!(
        assert_exact_boundary(StabilizerResource::Tableau, |budget| {
            phase.embedded_with_budget(n, &[n - 1], budget)
        }),
        tableau
    );
}
