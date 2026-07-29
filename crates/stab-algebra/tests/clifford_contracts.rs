#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "Clifford qualification uses a reviewed 24-element compatibility table and exact failures keep diagnostics readable"
)]

use std::cell::Cell;
use std::collections::BTreeSet;

use rand::Rng as _;
use rand::SeedableRng as _;
use rand::rngs::{SmallRng, StdRng};
use stab_algebra::{
    CliffordString, SingleQubitClifford, StabilizerError, StabilizerResource, StabilizerResult,
    Tableau,
};

const CLIFFORD_CONTRACT: [(SingleQubitClifford, &str, &str, &str, &str); 24] = [
    (SingleQubitClifford::I, "I", "_I", "+X", "+Z"),
    (SingleQubitClifford::X, "X", "_X", "+X", "-Z"),
    (SingleQubitClifford::Y, "Y", "_Y", "-X", "-Z"),
    (SingleQubitClifford::Z, "Z", "_Z", "-X", "+Z"),
    (SingleQubitClifford::H, "H", "HI", "+Z", "+X"),
    (
        SingleQubitClifford::SqrtYDag,
        "SQRT_Y_DAG",
        "HX",
        "+Z",
        "-X",
    ),
    (SingleQubitClifford::Hnxz, "H_NXZ", "HY", "-Z", "-X"),
    (SingleQubitClifford::SqrtY, "SQRT_Y", "HZ", "-Z", "+X"),
    (SingleQubitClifford::S, "S", "SI", "+Y", "+Z"),
    (SingleQubitClifford::Hxy, "H_XY", "SX", "+Y", "-Z"),
    (SingleQubitClifford::Hnxy, "H_NXY", "SY", "-Y", "-Z"),
    (SingleQubitClifford::SDag, "S_DAG", "SZ", "-Y", "+Z"),
    (
        SingleQubitClifford::SqrtXDag,
        "SQRT_X_DAG",
        "VI",
        "+X",
        "+Y",
    ),
    (SingleQubitClifford::SqrtX, "SQRT_X", "VX", "+X", "-Y"),
    (SingleQubitClifford::Hnyz, "H_NYZ", "VY", "-X", "-Y"),
    (SingleQubitClifford::Hyz, "H_YZ", "VZ", "-X", "+Y"),
    (SingleQubitClifford::Cxyz, "C_XYZ", "uI", "+Y", "+X"),
    (SingleQubitClifford::Cxynz, "C_XYNZ", "uX", "+Y", "-X"),
    (SingleQubitClifford::Cnxyz, "C_NXYZ", "uY", "-Y", "-X"),
    (SingleQubitClifford::Cxnyz, "C_XNYZ", "uZ", "-Y", "+X"),
    (SingleQubitClifford::Czyx, "C_ZYX", "dI", "+Z", "+Y"),
    (SingleQubitClifford::Cznyx, "C_ZNYX", "dX", "+Z", "-Y"),
    (SingleQubitClifford::Cnzyx, "C_NZYX", "dY", "-Z", "-Y"),
    (SingleQubitClifford::Czynx, "C_ZYNX", "dZ", "-Z", "+Y"),
];

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

fn contract_gates() -> impl ExactSizeIterator<Item = SingleQubitClifford> {
    CLIFFORD_CONTRACT.map(|(gate, _, _, _, _)| gate).into_iter()
}

fn contract_tableaus() -> Vec<Tableau> {
    CLIFFORD_CONTRACT
        .iter()
        .map(|(_, _, _, x_output, z_output)| {
            Tableau::gate1(x_output, z_output).expect("reviewed Clifford output pair")
        })
        .collect()
}

#[test]
fn cq2_algebra_single_qubit_clifford_contract_covers_values_and_names() {
    assert_eq!(
        SingleQubitClifford::all().collect::<Vec<_>>(),
        contract_gates().collect::<Vec<_>>()
    );

    let mut names = BTreeSet::new();
    let mut tokens = BTreeSet::new();
    for (gate, name, token, x_output, z_output) in CLIFFORD_CONTRACT {
        assert!(names.insert(name));
        assert!(tokens.insert(token));
        assert_eq!(gate.canonical_name(), name);
        assert_eq!(gate.token(), token);
        assert_eq!(gate.to_string(), token);
        assert_eq!(
            gate.tableau(),
            Tableau::gate1(x_output, z_output).expect("reviewed Clifford output pair")
        );
    }
    assert_eq!(names.len(), 24);
    assert_eq!(tokens.len(), 24);
}

#[test]
fn cq2_algebra_clifford_string_contract_covers_growth_and_composition() {
    let empty = CliffordString::identity(0).expect("empty Clifford string");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(empty.to_string(), "");

    let mut value = CliffordString::from_gates([
        SingleQubitClifford::H,
        SingleQubitClifford::S,
        SingleQubitClifford::I,
    ])
    .expect("Clifford string");
    assert_eq!(value.len(), 3);
    assert!(!value.is_empty());
    assert_eq!(value.to_string(), "HI SI _I");
    assert_eq!(value.gate_at(0), Some(SingleQubitClifford::H));
    assert_eq!(value.gate_at(2), Some(SingleQubitClifford::I));
    assert_eq!(value.gate_at(3), None);
    assert_eq!(
        value.set_gate_at(3, SingleQubitClifford::X),
        Err(StabilizerError::CliffordIndexOutOfRange { index: 3, len: 3 })
    );
    assert_eq!(value.to_string(), "HI SI _I");
    value
        .set_gate_at(2, SingleQubitClifford::Z)
        .expect("set Clifford gate");
    assert_eq!(value.to_string(), "HI SI _Z");

    let suffix = CliffordString::from_gates([SingleQubitClifford::X]).expect("suffix");
    assert_eq!(
        value.concat(&suffix).expect("concat").to_string(),
        "HI SI _Z _X"
    );
    assert_eq!(suffix.repeat(3).expect("repeat").to_string(), "_X _X _X");
    assert_eq!(suffix.repeat(0).expect("zero repeat"), empty);

    let left = CliffordString::from_gates([SingleQubitClifford::H]).expect("left");
    let right = CliffordString::from_gates([SingleQubitClifford::H, SingleQubitClifford::S])
        .expect("right");
    let product = left.multiply(&right).expect("multiply with padding");
    let mut in_place = left.clone();
    in_place
        .right_multiply_in_place(&right)
        .expect("in-place multiply");
    assert_eq!(product, in_place);
    assert_eq!(product.to_string(), "_I SI");

    let contract = contract_gates().collect::<Vec<_>>();
    let width = 552;
    let mut identity_left =
        CliffordString::from_gates((0..width).map(|index| contract[index % contract.len()]))
            .expect("equal-width identity left operand");
    let identity_left_before = identity_left.clone();
    let identity_right = CliffordString::identity(width).expect("identity right operand");
    let identity_right_before = identity_right.clone();
    identity_left
        .right_multiply_in_place(&identity_right)
        .expect("equal-width identity multiplication");
    assert_eq!(identity_left, identity_left_before);
    assert_eq!(identity_right, identity_right_before);

    let mut cycle_left =
        CliffordString::from_gates((0..width).map(|index| contract[index % contract.len()]))
            .expect("complete Clifford cycle left operand");
    let cycle_right = CliffordString::from_gates(
        (0..width).map(|index| contract[1 + (index / contract.len()) % (contract.len() - 1)]),
    )
    .expect("complete non-identity cycle right operand");
    let cycle_right_before = cycle_right.clone();
    let expected_cycle = (0..width)
        .map(|index| {
            contract[index % contract.len()]
                .multiply(contract[1 + (index / contract.len()) % (contract.len() - 1)])
                .expect("single-qubit Clifford product")
        })
        .collect::<Vec<_>>();
    cycle_left
        .right_multiply_in_place(&cycle_right)
        .expect("complete non-identity cycle multiplication");
    for (index, expected) in expected_cycle.into_iter().enumerate() {
        assert_eq!(cycle_left.gate_at(index), Some(expected));
    }
    assert_eq!(cycle_right, cycle_right_before);

    let mut first_rng = SmallRng::seed_from_u64(0x0051_ab1e);
    let mut second_rng = SmallRng::seed_from_u64(0x0051_ab1e);
    let mut first = CliffordString::random(32, &mut first_rng).expect("random Clifford string");
    let mut second = CliffordString::random(32, &mut second_rng).expect("random Clifford string");
    assert_eq!(first, second);
    first.randomize(&mut first_rng);
    second.randomize(&mut second_rng);
    assert_eq!(first, second);
}

#[test]
fn cq2_algebra_clifford_growth_rejects_limits_and_overflow() {
    let resource = StabilizerResource::CliffordQubits;

    let mut maximum_left = CliffordString::identity(resource.limit()).expect("maximum left");
    let mut maximum_right = CliffordString::identity(resource.limit()).expect("maximum right");
    maximum_right
        .set_gate_at(resource.limit() - 1, SingleQubitClifford::X)
        .expect("set maximum right tail");
    let maximum_right_before = maximum_right.clone();
    maximum_left
        .right_multiply_in_place(&maximum_right)
        .expect("maximum equal-width multiplication");
    assert_eq!(maximum_left.len(), resource.limit());
    assert_eq!(maximum_left.gate_at(0), Some(SingleQubitClifford::I));
    assert_eq!(
        maximum_left.gate_at(resource.limit() - 1),
        Some(SingleQubitClifford::X)
    );
    assert_eq!(maximum_right, maximum_right_before);

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
fn stabilizers_clifford_string_set_gate_at_vs_str_vs_gate_at_matches_stim() {
    let gates = contract_gates().collect::<Vec<_>>();
    let mut cliffords = CliffordString::identity(gates.len()).expect("Clifford identity");
    for (index, gate) in gates.iter().copied().enumerate() {
        cliffords
            .set_gate_at(index, gate)
            .expect("set Clifford gate");
    }

    assert_eq!(
        cliffords.to_string(),
        "_I _X _Y _Z HI HX HY HZ SI SX SY SZ VI VX VY VZ uI uX uY uZ dI dX dY dZ"
    );
    for (index, gate) in gates.into_iter().enumerate() {
        assert_eq!(cliffords.gate_at(index), Some(gate));
    }
    assert_eq!(cliffords.gate_at(24), None);
}

#[test]
fn stabilizers_clifford_string_known_identities_match_stim() {
    let h = CliffordString::from_gates([SingleQubitClifford::H]).expect("H Clifford");
    let s = CliffordString::from_gates([SingleQubitClifford::S]).expect("S Clifford");
    let s_dag = CliffordString::from_gates([SingleQubitClifford::SDag]).expect("S_DAG Clifford");

    assert_eq!(
        h.multiply(&h).expect("H*H"),
        CliffordString::identity(1).expect("Clifford identity")
    );
    assert_eq!(
        s.multiply(&s).expect("S*S"),
        CliffordString::from_gates([SingleQubitClifford::Z]).expect("Z Clifford")
    );
    assert_eq!(
        h.multiply(&s_dag).expect("H*S_DAG"),
        CliffordString::from_gates([SingleQubitClifford::Cxyz]).expect("C_XYZ Clifford")
    );
}

#[test]
fn stabilizers_single_qubit_clifford_multiplication_is_associative() {
    let gates = contract_gates().collect::<Vec<_>>();
    let tableaus = contract_tableaus();

    for (left_index, left) in gates.iter().copied().enumerate() {
        for (middle_index, middle) in gates.iter().copied().enumerate() {
            let product = left.multiply(middle).expect("Clifford product");
            let product_index = gates
                .iter()
                .position(|candidate| *candidate == product)
                .expect("product in Clifford group");
            assert_eq!(
                tableaus[middle_index]
                    .then(&tableaus[left_index])
                    .expect("Tableau product"),
                tableaus[product_index],
                "{} * {}",
                left.canonical_name(),
                middle.canonical_name()
            );

            for right in gates.iter().copied() {
                let lhs = left
                    .multiply(middle)
                    .expect("left middle")
                    .multiply(right)
                    .expect("(left middle) right");
                let rhs = left
                    .multiply(middle.multiply(right).expect("middle right"))
                    .expect("left (middle right)");
                assert_eq!(lhs, rhs);
            }
        }
    }
}

#[test]
fn stabilizers_clifford_random_hook_covers_single_qubit_cliffords() {
    let gates = contract_gates().collect::<Vec<_>>();
    let mut direct_first_rng = SmallRng::seed_from_u64(0xc11f_f07d);
    let mut direct_second_rng = SmallRng::seed_from_u64(0xc11f_f07d);
    let mut direct_counts = vec![0usize; gates.len()];
    for _ in 0..16_384 {
        let first = SingleQubitClifford::random(&mut direct_first_rng);
        let second = SingleQubitClifford::random(&mut direct_second_rng);
        assert_eq!(first, second);
        let count_index = gates
            .iter()
            .position(|candidate| *candidate == first)
            .expect("random gate is in reviewed Clifford set");
        direct_counts[count_index] += 1;
    }
    let direct_expected = 16_384.0 / 24.0;
    for (gate, count) in gates.iter().copied().zip(direct_counts) {
        assert!(
            (direct_expected * 0.5) < count as f64 && (count as f64) < (direct_expected * 1.5),
            "direct {gate:?} count {count} outside broad uniformity band around {direct_expected}"
        );
    }

    let mut rng = SmallRng::seed_from_u64(0xc11f_f07d);
    let mut cliffords = CliffordString::random(128, &mut rng).expect("random Clifford string");
    let mut counts = vec![0usize; gates.len()];
    for _ in 0..128 {
        for index in 0..cliffords.len() {
            let gate = cliffords.gate_at(index).expect("random Clifford gate");
            let count_index = gates
                .iter()
                .position(|candidate| *candidate == gate)
                .expect("random gate is in reviewed Clifford set");
            counts[count_index] += 1;
        }
        cliffords.randomize(&mut rng);
    }

    let expected = 128.0 * 128.0 / 24.0;
    for (gate, count) in gates.into_iter().zip(counts) {
        assert!(
            (expected * 0.5) < count as f64 && (count as f64) < (expected * 1.5),
            "{gate:?} count {count} outside broad uniformity band around {expected}"
        );
    }
}
