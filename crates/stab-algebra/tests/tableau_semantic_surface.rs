#![allow(
    clippy::expect_used,
    reason = "fixed Clifford witnesses make setup failures test bugs"
)]

use std::str::FromStr as _;

use stab_algebra::{
    PauliBasis, PauliString, SingleQubitClifford, StabilizerError, StabilizerResource, Tableau,
    TableauIterator, stabilizers_to_tableau,
};

#[test]
fn tableau_semantic_surface_matches_stim() {
    for clifford in SingleQubitClifford::all() {
        assert_stable_surface(&clifford.tableau());
    }

    let tableaus = TableauIterator::new(2, false).expect("two-qubit Tableau iterator");
    let mut count = 0;
    for tableau in tableaus {
        assert_stable_surface(&tableau);
        count += 1;
    }
    assert_eq!(
        count, 720,
        "the structural matrix must cover every unsigned 2q Tableau"
    );
    assert_powers_direct_sums_and_embedding();
    assert_local_append_matches_full_composition();
    assert_canonical_stabilizers();
}

#[test]
fn local_tableau_append_allocations_are_width_independent() {
    let local = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT Tableau");
    let mut narrow = Tableau::identity(16).expect("narrow Tableau");
    let mut wide = Tableau::identity(256).expect("wide Tableau");

    let narrow_allocations = allocation_counter::measure(|| {
        narrow.append(&local, &[15, 3]).expect("narrow append");
    });
    let wide_allocations = allocation_counter::measure(|| {
        wide.append(&local, &[255, 3]).expect("wide append");
    });

    assert_eq!(narrow_allocations.count_total, 0, "{narrow_allocations:?}");
    assert_eq!(narrow_allocations.bytes_total, 0, "{narrow_allocations:?}");
    assert_eq!(
        wide_allocations.count_total, narrow_allocations.count_total,
        "local append allocation count grew with destination width: narrow={narrow_allocations:?}, wide={wide_allocations:?}"
    );
    assert_eq!(
        wide_allocations.bytes_total, narrow_allocations.bytes_total,
        "local append allocated destination-width storage: narrow={narrow_allocations:?}, wide={wide_allocations:?}"
    );
}

#[test]
fn checked_generators_reject_invalid_shape_and_symplectic_relations() {
    assert_eq!(
        Tableau::from_conjugated_generators(vec![pauli("+X")], vec![]),
        Err(StabilizerError::TableauGeneratorCountMismatch {
            x_generators: 1,
            z_generators: 0,
        })
    );
    assert_eq!(
        Tableau::from_conjugated_generators(
            vec![pauli("+X"), pauli("+_X")],
            vec![pauli("+Z_"), pauli("+_Z")],
        ),
        Err(StabilizerError::TableauGeneratorWidthMismatch {
            basis: PauliBasis::X,
            index: 0,
            width: 1,
            expected: 2,
        })
    );
    assert_eq!(
        Tableau::from_conjugated_generators(vec![pauli("+X")], vec![pauli("-X")]),
        Err(StabilizerError::ConjugatedGeneratorPairCommutes { index: 0 })
    );
    assert_eq!(
        Tableau::from_conjugated_generators(
            vec![pauli("+X_"), pauli("+XX")],
            vec![pauli("+Z_"), pauli("+_Z")],
        ),
        Err(StabilizerError::ConjugatedGeneratorsAnticommute {
            left_basis: PauliBasis::Z,
            left_index: 0,
            right_basis: PauliBasis::X,
            right_index: 1,
        })
    );

    let signed = Tableau::from_conjugated_generators(vec![pauli("-Z")], vec![pauli("+X")])
        .expect("real negative generator signs are Hermitian and valid");
    assert_eq!(signed, Tableau::gate1("-Z", "+X").expect("signed Hadamard"));

    let too_wide = pauli(&format!("+{}", "_".repeat(513)));
    assert_eq!(
        Tableau::from_conjugated_generators(vec![too_wide.clone(); 513], vec![too_wide; 513],),
        Err(StabilizerError::ResourceLimitExceeded {
            resource: StabilizerResource::TableauQubits,
            requested: 513,
            limit: 512,
        })
    );
}

fn assert_powers_direct_sums_and_embedding() {
    let phase = Tableau::gate1("+Y", "+Z").expect("phase Tableau");
    let hadamard = Tableau::gate1("+Z", "+X").expect("Hadamard Tableau");
    let identity = Tableau::identity(1).expect("one-qubit identity");

    assert_eq!(phase.pow(0).expect("zero power"), identity);
    assert_eq!(
        phase.pow(2).expect("square"),
        phase.then(&phase).expect("S then S")
    );
    let cube = phase
        .then(&phase)
        .expect("S squared")
        .then(&phase)
        .expect("S cubed");
    assert_eq!(phase.pow(3).expect("cube"), cube);
    assert_eq!(
        phase.pow(-3).expect("negative cube"),
        cube.inverse().expect("inverse cube")
    );
    assert_eq!(
        phase.pow(-1).expect("negative power"),
        phase.inverse().expect("inverse")
    );
    assert_eq!(
        phase.pow(i64::MIN).expect("minimum exponent"),
        identity,
        "S has order four and i64::MIN is divisible by four"
    );

    let product = phase.direct_sum(&hadamard).expect("disjoint direct sum");
    assert_eq!(
        product.apply(&pauli("+XZ")).expect("factor action"),
        pauli("+YX")
    );

    let cnot = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT Tableau");
    let embedded = cnot.embedded(4, &[3, 1]).expect("non-contiguous embedding");
    for (input, expected) in [
        ("+___X", "+_X_X"),
        ("+___Z", "+___Z"),
        ("+_X__", "+_X__"),
        ("+_Z__", "+_Z_Z"),
        ("+Z___", "+Z___"),
        ("+__X_", "+__X_"),
        ("+__Z_", "+__Z_"),
    ] {
        assert_eq!(
            embedded.apply(&pauli(input)).expect("embedded action"),
            pauli(expected),
            "input {input}"
        );
    }

    assert_eq!(
        cnot.embedded(4, &[1]),
        Err(StabilizerError::TableauTargetCountMismatch {
            tableau_qubits: 2,
            target_count: 1,
        })
    );
    assert_eq!(
        cnot.embedded(4, &[1, 1]),
        Err(StabilizerError::DuplicateTableauTarget { target: 1 })
    );
    assert_eq!(
        cnot.embedded(4, &[1, 4]),
        Err(StabilizerError::TableauTargetOutOfRange {
            target: 4,
            num_qubits: 4,
        })
    );

    let wide = Tableau::identity(300).expect("admitted wide Tableau");
    assert_eq!(
        wide.direct_sum(&wide),
        Err(StabilizerError::ResourceLimitExceeded {
            resource: StabilizerResource::TableauQubits,
            requested: 600,
            limit: 512,
        })
    );
    assert_eq!(
        phase.embedded(513, &[0]),
        Err(StabilizerError::ResourceLimitExceeded {
            resource: StabilizerResource::TableauQubits,
            requested: 513,
            limit: 512,
        })
    );
}

fn assert_local_append_matches_full_composition() {
    let hadamard = Tableau::gate1("+Z", "+X").expect("Hadamard Tableau");
    let phase = Tableau::gate1("+Y", "+Z").expect("phase Tableau");
    let cnot = Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT Tableau");
    let signed = Tableau::gate2("-XX", "+Z_", "+_X", "-ZZ").expect("signed CNOT Tableau");

    let parent = hadamard
        .embedded(4, &[1])
        .expect("embedded H")
        .then(&cnot.embedded(4, &[0, 3]).expect("embedded CNOT"))
        .expect("parent composition")
        .then(&phase.embedded(4, &[2]).expect("embedded S"))
        .expect("parent phase");

    let mut local_actions = TableauIterator::new(2, false)
        .expect("unsigned two-qubit Tableaus")
        .collect::<Vec<_>>();
    local_actions.push(signed);
    for local in local_actions {
        let expected = parent
            .then(&local.embedded(4, &[3, 1]).expect("full-width local action"))
            .expect("full composition");
        let mut actual = parent.clone();
        actual
            .append(&local, &[3, 1])
            .expect("in-place local append");
        assert_eq!(actual, expected);
    }

    for local in SingleQubitClifford::all().map(|clifford| clifford.tableau()) {
        let expected = parent
            .then(
                &local
                    .embedded(4, &[2])
                    .expect("full-width one-qubit action"),
            )
            .expect("full one-qubit composition");
        let mut actual = parent.clone();
        actual.append(&local, &[2]).expect("one-qubit append");
        assert_eq!(actual, expected);
    }

    let mut invalid = Tableau::identity(4).expect("append validation destination");
    assert_eq!(
        invalid.append(&cnot, &[1]),
        Err(StabilizerError::TableauTargetCountMismatch {
            tableau_qubits: 2,
            target_count: 1,
        })
    );
    assert_eq!(
        invalid.append(&cnot, &[1, 1]),
        Err(StabilizerError::DuplicateTableauTarget { target: 1 })
    );
    assert_eq!(
        invalid.append(&cnot, &[1, 4]),
        Err(StabilizerError::TableauTargetOutOfRange {
            target: 4,
            num_qubits: 4,
        })
    );
}

fn assert_canonical_stabilizers() {
    let source = stabilizers_to_tableau(&[pauli("+ZZ"), pauli("+XX")], false, false, false)
        .expect("Bell stabilizers");
    let canonical = source
        .canonical_stabilizers()
        .expect("canonical Bell stabilizers");
    assert_eq!(canonical, [pauli("+XX"), pauli("+ZZ")]);

    let reconstructed = stabilizers_to_tableau(&canonical, false, false, false)
        .expect("reconstruct canonical stabilizers");
    assert_eq!(
        reconstructed
            .canonical_stabilizers()
            .expect("canonical reconstructed state"),
        canonical
    );

    let four_qubit = stabilizers_to_tableau(
        &[
            pauli("+XXXX"),
            pauli("+YYYY"),
            pauli("+YYZZ"),
            pauli("+XXZZ"),
        ],
        false,
        false,
        false,
    )
    .expect("four-qubit Stim canonicalization fixture");
    assert_eq!(
        four_qubit
            .canonical_stabilizers()
            .expect("canonical four-qubit stabilizers"),
        [
            pauli("-XX__"),
            pauli("-ZZ__"),
            pauli("-__XX"),
            pauli("-__ZZ"),
        ]
    );
}

fn assert_stable_surface(tableau: &Tableau) {
    let xs = (0..tableau.len())
        .map(|index| tableau.x_output(index).expect("X output").clone())
        .collect::<Vec<_>>();
    let zs = (0..tableau.len())
        .map(|index| tableau.z_output(index).expect("Z output").clone())
        .collect::<Vec<_>>();
    assert_eq!(
        Tableau::from_conjugated_generators(xs, zs).expect("checked reconstruction"),
        *tableau
    );

    let identity = Tableau::identity(tableau.len()).expect("identity");
    assert_eq!(tableau.pow(0).expect("zero power"), identity);
    assert_eq!(tableau.pow(1).expect("first power"), *tableau);
    assert_eq!(
        tableau.pow(-1).expect("negative power"),
        tableau.inverse().expect("inverse")
    );

    let canonical = tableau
        .canonical_stabilizers()
        .expect("canonical stabilizers");
    let alternative_generators = if tableau.len() == 2 {
        vec![
            tableau.z_output(1).expect("Z1 output").clone(),
            tableau
                .z_output(0)
                .expect("Z0 output")
                .multiply_real(tableau.z_output(1).expect("Z1 output"))
                .expect("commuting generator product"),
        ]
    } else {
        canonical.clone()
    };
    let reconstructed = stabilizers_to_tableau(&alternative_generators, false, false, false)
        .expect("reconstruct stabilizer state from an equivalent generator basis");
    assert_eq!(
        reconstructed
            .canonical_stabilizers()
            .expect("reconstructed canonical stabilizers"),
        canonical
    );
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("valid Pauli fixture")
}
