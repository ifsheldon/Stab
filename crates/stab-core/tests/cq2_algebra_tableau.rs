#![allow(
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "CQ2 Tableau qualification uses compact exact contracts and direct assertions"
)]

use std::str::FromStr;

use num_complex::Complex32;
use rand::SeedableRng as _;
use rand::rngs::SmallRng;
use stab_core::{
    CommutingPauliStringIterator, PauliString, PauliStringIterator, StabilizerError, Tableau,
    TableauIterator, stabilizers_to_tableau, unitary_to_tableau,
};

#[test]
fn cq2_algebra_tableau_access_and_value_contract_is_exact() {
    let empty = Tableau::identity(0).expect("empty Tableau");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert!(empty.satisfies_invariants().expect("empty invariants"));

    let cnot = cnot_tableau();
    assert_eq!(cnot.len(), 2);
    assert!(!cnot.is_empty());
    assert!(cnot.satisfies_invariants().expect("CNOT invariants"));
    assert_eq!(
        cnot.to_string(),
        "+-xz-xz-\n\
         | ++ ++\n\
         | XZ _Z\n\
         | X_ XZ"
    );
    assert_eq!(cnot.x_output(0).expect("X0").to_string(), "+XX");
    assert_eq!(cnot.y_output(0).expect("Y0").to_string(), "+YX");
    assert_eq!(cnot.z_output(0).expect("Z0").to_string(), "+Z_");
    assert_eq!(cnot.x_output(1).expect("X1").to_string(), "+_X");
    assert_eq!(cnot.y_output(1).expect("Y1").to_string(), "+ZY");
    assert_eq!(cnot.z_output(1).expect("Z1").to_string(), "+ZZ");

    let x_rows = [[1, 1], [0, 1]];
    let y_rows = [[2, 1], [3, 2]];
    let z_rows = [[3, 0], [3, 3]];
    for input in 0..2 {
        for output in 0..2 {
            assert_eq!(
                cnot.x_output_pauli_xyz(input, output),
                Ok(x_rows[input][output])
            );
            assert_eq!(
                cnot.y_output_pauli_xyz(input, output),
                Ok(y_rows[input][output])
            );
            assert_eq!(
                cnot.z_output_pauli_xyz(input, output),
                Ok(z_rows[input][output])
            );
        }
    }
    assert_eq!(
        cnot.x_output(2),
        Err(StabilizerError::TableauIndexOutOfRange { index: 2, len: 2 })
    );
    assert_eq!(
        cnot.y_output(2),
        Err(StabilizerError::TableauIndexOutOfRange { index: 2, len: 2 })
    );
    assert_eq!(
        cnot.z_output(2),
        Err(StabilizerError::TableauIndexOutOfRange { index: 2, len: 2 })
    );
    for actual in [
        cnot.x_output_pauli_xyz(2, 0),
        cnot.x_output_pauli_xyz(0, 2),
        cnot.y_output_pauli_xyz(2, 0),
        cnot.y_output_pauli_xyz(0, 2),
        cnot.z_output_pauli_xyz(2, 0),
        cnot.z_output_pauli_xyz(0, 2),
    ] {
        assert_eq!(
            actual,
            Err(StabilizerError::TableauIndexOutOfRange { index: 2, len: 2 })
        );
    }

    assert_eq!(
        Tableau::gate1("+X", "-Z")
            .expect("signed Tableau")
            .to_string(),
        "+-xz-\n\
         | +-\n\
         | XZ"
    );
    assert!(
        !Tableau::gate1("+X", "+X")
            .expect("invalid one-qubit Tableau")
            .satisfies_invariants()
            .expect("invalid one-qubit invariants")
    );
    assert!(
        !Tableau::gate2("+X_", "+Z_", "+X_", "+Z_")
            .expect("duplicate-output Tableau")
            .satisfies_invariants()
            .expect("duplicate-output invariants")
    );
    assert!(
        !Tableau::gate2("+X_", "+__", "+_X", "+_Z")
            .expect("identity-generator Tableau")
            .satisfies_invariants()
            .expect("identity-generator invariants")
    );
    assert_eq!(
        Tableau::gate1("XX", "Z"),
        Err(StabilizerError::LengthMismatch { left: 2, right: 1 })
    );
    assert_eq!(
        cnot.apply(&pauli("X")),
        Err(StabilizerError::LengthMismatch { left: 1, right: 2 })
    );
    assert_eq!(
        cnot.then(&Tableau::identity(1).expect("one-qubit identity")),
        Err(StabilizerError::LengthMismatch { left: 2, right: 1 })
    );

    let product = pauli("+_XZY");
    let product_tableau = Tableau::from_pauli_string(&product).expect("Pauli Tableau");
    assert_eq!(
        product_tableau.to_pauli_string().expect("Pauli round trip"),
        product
    );
    assert_eq!(
        Tableau::gate1("+Z", "+X")
            .expect("Hadamard Tableau")
            .to_pauli_string(),
        Err(StabilizerError::NotPauliProduct)
    );

    let pauli_tableau = Tableau::from_pauli_string(&pauli("+XZ_Y")).expect("Pauli-product Tableau");
    for (index, expected) in ["+X___", "-_X__", "+__X_", "-___X"].into_iter().enumerate() {
        assert_eq!(
            pauli_tableau.x_output(index).expect("Pauli X output"),
            &pauli(expected)
        );
    }
    for (index, expected) in ["-Z___", "+_Z__", "+__Z_", "-___Z"].into_iter().enumerate() {
        assert_eq!(
            pauli_tableau.z_output(index).expect("Pauli Z output"),
            &pauli(expected)
        );
    }

    let wide = Tableau::identity(500).expect("wide identity Tableau");
    assert!(wide.satisfies_invariants().expect("wide invariants"));
    assert_eq!(
        wide.x_output(0)
            .expect("first wide X")
            .active_terms()
            .collect::<Vec<_>>(),
        vec![(0, stab_core::PauliBasis::X)]
    );
    assert_eq!(
        wide.x_output(499)
            .expect("last wide X")
            .active_terms()
            .collect::<Vec<_>>(),
        vec![(499, stab_core::PauliBasis::X)]
    );
}

#[test]
fn cq2_algebra_tableau_forward_and_inverse_actions_are_separating() {
    let phase = Tableau::gate1("+Y", "+Z").expect("phase Tableau");
    let inverse = phase.inverse().expect("inverse phase Tableau");

    assert_eq!(phase.apply(&pauli("+X")).expect("forward X"), pauli("+Y"));
    assert_eq!(phase.apply(&pauli("+Y")).expect("forward Y"), pauli("-X"));
    assert_eq!(inverse.apply(&pauli("+X")).expect("inverse X"), pauli("-Y"));
    assert_eq!(inverse.apply(&pauli("+Y")).expect("inverse Y"), pauli("+X"));

    for input in [pauli("+X"), pauli("-Y"), pauli("+Z")] {
        let forward = phase.apply(&input).expect("forward action");
        assert_eq!(inverse.apply(&forward).expect("inverse action"), input);
    }
}

#[test]
fn cq2_algebra_pauli_iterator_state_contract_is_restartable_and_typed() {
    let mut paulis = PauliStringIterator::new(3, 1, 1, true, false, true).expect("Pauli iterator");
    assert!(paulis.iter_next());
    assert_eq!(paulis.result().to_string(), "+X__");
    let mut paulis_clone = paulis.clone();
    assert_eq!(paulis.next(), paulis_clone.next());
    assert_eq!(paulis, paulis_clone);
    paulis.restart();
    assert_eq!(paulis.next().expect("restarted Pauli").to_string(), "+X__");
}

#[test]
fn cq2_algebra_commuting_iterator_state_contract_is_restartable_and_typed() {
    let mut one_qubit = CommutingPauliStringIterator::new(1).expect("one-qubit iterator");
    assert_eq!(
        one_qubit
            .by_ref()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        vec!["+X", "+Z", "+Y"]
    );
    one_qubit
        .restart_iter(std::slice::from_ref(&pauli("+X")), &[])
        .expect("commute with X");
    assert_eq!(
        one_qubit
            .by_ref()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        vec!["+X"]
    );
    one_qubit
        .restart_iter(&[], std::slice::from_ref(&pauli("+X")))
        .expect("anticommute with X");
    assert_eq!(
        one_qubit
            .by_ref()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        vec!["+Z", "+Y"]
    );

    let z = pauli("+Z_");
    let xx = pauli("+XX");
    let mut commuting = CommutingPauliStringIterator::new(2).expect("commuting iterator");
    commuting
        .restart_iter(std::slice::from_ref(&z), std::slice::from_ref(&xx))
        .expect("constraints");
    let first_pass = commuting
        .by_ref()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    commuting.restart_iter_same_constraints();
    let second_pass = commuting
        .by_ref()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    assert_eq!(first_pass, second_pass);
    assert_eq!(first_pass, vec!["+Z_", "+ZX", "+_Z", "+_Y"]);

    let mut four_qubit = CommutingPauliStringIterator::new(4).expect("four-qubit iterator");
    assert_eq!(
        four_qubit
            .by_ref()
            .take(11)
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        vec![
            "+X___", "+_X__", "+XX__", "+__X_", "+X_X_", "+_XX_", "+XXX_", "+Z___", "+Y___",
            "+ZX__", "+YX__"
        ]
    );
    let commute = [pauli("+Z___"), pauli("+_Z__"), pauli("+___Z")];
    let anticommute = [pauli("+X___"), pauli("+_X__"), pauli("+___X")];
    four_qubit
        .restart_iter(&commute, &anticommute)
        .expect("four-qubit constraints");
    assert_eq!(
        four_qubit
            .by_ref()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        vec!["+ZZ_Z", "+ZZXZ", "+ZZZZ", "+ZZYZ"]
    );
    four_qubit
        .restart_iter(
            std::slice::from_ref(&commute[0]),
            std::slice::from_ref(&commute[0]),
        )
        .expect("contradictory constraints");
    assert_eq!(four_qubit.next(), None);

    commuting.restart_iter_same_constraints();
    let before_error = commuting.clone();
    assert_eq!(
        commuting.restart_iter(&[pauli("+Z")], &[]),
        Err(StabilizerError::LengthMismatch { left: 1, right: 2 })
    );
    assert_eq!(commuting, before_error);
}

#[test]
fn cq2_algebra_tableau_iterator_state_contract_is_restartable_and_typed() {
    for signed in [false, true] {
        let mut empty = TableauIterator::new(0, signed).expect("empty Tableau iterator");
        assert_eq!(
            empty.next(),
            Some(Tableau::identity(0).expect("empty Tableau"))
        );
        assert_eq!(empty.next(), None);
        empty.restart().expect("restart empty Tableau iterator");
        assert_eq!(empty.count(), 1);
    }
    let mut tableaus = TableauIterator::new(1, true).expect("signed Tableau iterator");
    let first = tableaus.next().expect("first Tableau");
    let mut tableaus_clone = tableaus.clone();
    assert_eq!(tableaus.next(), tableaus_clone.next());
    assert_eq!(tableaus, tableaus_clone);
    assert!(first.satisfies_invariants().expect("iterator invariants"));
    tableaus.restart().expect("restart Tableau iterator");
    assert_eq!(tableaus.next(), Some(first));
}

#[test]
fn cq2_algebra_stabilizer_solver_error_contract_is_exact() {
    assert_eq!(
        stabilizers_to_tableau(&[pauli("+X"), pauli("+Z")], false, false, false),
        Err(StabilizerError::AntiCommutingStabilizer {
            stabilizer: "+Z".to_owned(),
            conflict: "+X".to_owned(),
        })
    );
    assert_eq!(
        stabilizers_to_tableau(&[pauli("+Z"), pauli("+Z")], false, false, false),
        Err(StabilizerError::RedundantStabilizer {
            stabilizer: "+Z".to_owned(),
        })
    );
    assert_eq!(
        stabilizers_to_tableau(&[pauli("+Z"), pauli("-Z")], true, false, false),
        Err(StabilizerError::InconsistentStabilizer {
            stabilizer: "-Z".to_owned(),
        })
    );
    assert_eq!(
        stabilizers_to_tableau(&[pauli("+Z_")], false, false, false),
        Err(StabilizerError::UnderconstrainedStabilizers {
            independent: 1,
            num_qubits: 2,
        })
    );

    let completed =
        stabilizers_to_tableau(&[pauli("+Z_")], false, true, false).expect("complete solver");
    assert_eq!(
        completed.z_output(0).expect("owned stabilizer"),
        &pauli("+Z_")
    );
    assert!(completed.satisfies_invariants().expect("solver invariants"));
    assert_eq!(
        stabilizers_to_tableau(&[pauli("+Z_")], false, true, true).expect("inverse solver"),
        completed.inverse().expect("expected inverse")
    );

    let distant_x = pauli(&format!("+X{}", "_".repeat(500)));
    assert!(matches!(
        stabilizers_to_tableau(&[pauli("+Z"), distant_x], false, false, false),
        Err(StabilizerError::AntiCommutingStabilizer { .. })
    ));
    let distant_zx = pauli(&format!("+Z{}X", "_".repeat(500)));
    assert!(matches!(
        stabilizers_to_tableau(
            &[pauli("+Z_"), pauli("-_Z"), distant_zx.clone(), pauli("+ZZ")],
            false,
            false,
            false,
        ),
        Err(StabilizerError::InconsistentStabilizer { .. })
    ));
    assert!(matches!(
        stabilizers_to_tableau(
            &[
                pauli("-Z_"),
                distant_zx,
                pauli("-__Z"),
                pauli("+_Z_"),
                pauli("+Z_Z"),
            ],
            false,
            false,
            false,
        ),
        Err(StabilizerError::RedundantStabilizer { .. })
    ));
}

#[test]
fn cq2_algebra_stabilizer_solver_overconstrained_contract_matches_stim() {
    let mut rng = SmallRng::seed_from_u64(0x0a11_ce55);
    for num_qubits in 4..10 {
        let source = Tableau::random(num_qubits, &mut rng).expect("source Tableau");
        let dependent = source
            .z_output(1)
            .expect("Z1")
            .multiply_real(source.z_output(3).expect("Z3"))
            .expect("dependent stabilizer");
        let mut stabilizers = vec![
            PauliString::identity(num_qubits).expect("identity stabilizer"),
            dependent,
        ];
        for index in 0..num_qubits {
            stabilizers.push(source.z_output(index).expect("source stabilizer").clone());
        }

        assert!(matches!(
            stabilizers_to_tableau(&stabilizers, false, false, false),
            Err(StabilizerError::RedundantStabilizer { .. })
        ));
        let actual = stabilizers_to_tableau(&stabilizers, true, false, false)
            .expect("ignore dependent stabilizers");
        for index in 0..num_qubits {
            let expected_index = index + 1 + usize::from(index > 3);
            assert_eq!(
                actual.z_output(index).expect("actual stabilizer"),
                &stabilizers[expected_index]
            );
        }
        assert!(actual.satisfies_invariants().expect("solver invariants"));
    }
}

#[test]
fn cq2_algebra_unitary_conversion_python_contract_is_exact() {
    let scale = f32::sqrt(0.5);
    let hadamard = [
        vec![c(scale, 0.0), c(scale, 0.0)],
        vec![c(scale, 0.0), c(-scale, 0.0)],
    ];
    assert_eq!(
        unitary_to_tableau(&hadamard, true).expect("Hadamard unitary"),
        Tableau::gate1("+Z", "+X").expect("Hadamard Tableau")
    );
    assert_eq!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0)],
            ],
            true,
        ),
        Err(StabilizerError::MatrixNotUnitary)
    );
}

#[test]
fn cq2_algebra_unitary_conversion_error_contract_is_exact() {
    assert_eq!(
        unitary_to_tableau(&[], true),
        Err(StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height: 0 })
    );
    assert_eq!(
        unitary_to_tableau(&[vec![], vec![]], true),
        Err(StabilizerError::UnitaryMatrixRowWidthMismatch {
            row: 0,
            width: 0,
            height: 2,
        })
    );
    let non_power_of_two = vec![vec![c(1.0, 0.0), c(0.0, 0.0)]; 3];
    assert_eq!(
        unitary_to_tableau(&non_power_of_two, true),
        Err(StabilizerError::UnitaryMatrixHeightNotPowerOfTwo { height: 3 })
    );
    assert_eq!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(0.0, 0.0)],
            ],
            true,
        ),
        Err(StabilizerError::MatrixNotUnitary)
    );

    let eighth_turn = f32::sqrt(0.5);
    assert_eq!(
        unitary_to_tableau(
            &[
                vec![c(1.0, 0.0), c(0.0, 0.0)],
                vec![c(0.0, 0.0), c(eighth_turn, eighth_turn)],
            ],
            true,
        ),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );

    let mut controlled_phase = identity_unitary(4);
    controlled_phase[3][3] = c(0.0, 1.0);
    assert_eq!(
        unitary_to_tableau(&controlled_phase, false),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );

    let mut controlled_controlled_x = identity_unitary(8);
    controlled_controlled_x[6][6] = c(0.0, 0.0);
    controlled_controlled_x[7][7] = c(0.0, 0.0);
    controlled_controlled_x[6][7] = c(1.0, 0.0);
    controlled_controlled_x[7][6] = c(1.0, 0.0);
    assert_eq!(
        unitary_to_tableau(&controlled_controlled_x, false),
        Err(StabilizerError::UnitaryMatrixNotClifford)
    );
}

fn pauli(text: &str) -> PauliString {
    PauliString::from_str(text).expect("parse Pauli")
}

fn cnot_tableau() -> Tableau {
    Tableau::gate2("+XX", "+Z_", "+_X", "+ZZ").expect("CNOT Tableau")
}

fn c(real: f32, imaginary: f32) -> Complex32 {
    Complex32::new(real, imaginary)
}

fn identity_unitary(dimension: usize) -> Vec<Vec<Complex32>> {
    let mut result = vec![vec![c(0.0, 0.0); dimension]; dimension];
    for (index, row) in result.iter_mut().enumerate() {
        row[index] = c(1.0, 0.0);
    }
    result
}
