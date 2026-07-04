#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "PF1 gate metadata compatibility tests use direct assertions for compact diagnostics"
)]

use std::collections::BTreeSet;

use stab_core::{
    Circuit, CircuitItem, Gate, GateArgumentRule, GateTargetGroupKind, GateTargetRule, Probability,
};

#[test]
fn gate_metadata_accessors_match_owned_stim_gatedata_semantics() {
    // Adapted from the non-binding-specific flags in Stim v1.16.0 src/stim/gates/gates_test.py.
    let h = Gate::from_name("H").unwrap();
    let cx = Gate::from_name("CX").unwrap();
    let r = Gate::from_name("R").unwrap();
    let mr = Gate::from_name("MR").unwrap();
    let mxx = Gate::from_name("MXX").unwrap();
    let mpp = Gate::from_name("MPP").unwrap();
    let x_error = Gate::from_name("X_ERROR").unwrap();
    let detector = Gate::from_name("DETECTOR").unwrap();

    assert_eq!(h.aliases(), &["H", "H_XZ"]);
    assert_eq!(cx.aliases(), &["CNOT", "CX", "ZCX"]);
    assert_eq!(Gate::from_name("MZ").unwrap().aliases(), &["M", "MZ"]);
    assert_eq!(
        Gate::from_name("SWAPCZ").unwrap().aliases(),
        &["CZSWAP", "SWAPCZ"]
    );

    assert_eq!(h.argument_rule(), GateArgumentRule::Exact(0));
    assert_eq!(
        Gate::from_name("M").unwrap().argument_rule(),
        GateArgumentRule::OptionalProbability
    );
    assert_eq!(
        Gate::from_name("PAULI_CHANNEL_2").unwrap().argument_rule(),
        GateArgumentRule::ProbabilityList(15)
    );
    assert_eq!(
        Gate::from_name("I_ERROR").unwrap().argument_rule(),
        GateArgumentRule::AnyProbabilityList
    );
    assert_eq!(
        Gate::from_name("II_ERROR").unwrap().argument_rule(),
        GateArgumentRule::AnyProbabilityList
    );
    assert_eq!(
        Gate::from_name("OBSERVABLE_INCLUDE")
            .unwrap()
            .argument_rule(),
        GateArgumentRule::UnsignedInteger
    );
    assert_eq!(detector.argument_rule(), GateArgumentRule::Any);

    assert_eq!(h.target_rule(), GateTargetRule::AnySingleQubit);
    assert_eq!(cx.target_rule(), GateTargetRule::ClassicalControlPairs);
    assert_eq!(
        Gate::from_name("XCX").unwrap().target_rule(),
        GateTargetRule::PlainPairs
    );
    assert_eq!(mpp.target_rule(), GateTargetRule::PauliProducts);
    assert_eq!(detector.target_rule(), GateTargetRule::RecOnly);
    assert_eq!(
        Gate::from_name("E").unwrap().target_group_kind(),
        GateTargetGroupKind::AllTargets
    );

    assert!(h.is_unitary());
    assert!(cx.is_unitary());
    assert!(!r.is_unitary());
    assert!(!mxx.is_unitary());
    assert!(!x_error.is_unitary());
    assert!(!detector.is_unitary());

    assert!(r.is_reset());
    assert!(mr.is_reset());
    assert!(!h.is_reset());
    assert!(!mxx.is_reset());

    assert!(x_error.is_noisy());
    assert!(mxx.is_noisy());
    assert!(mpp.is_noisy());
    assert!(!h.is_noisy());
    assert!(!r.is_noisy());
    assert!(!Gate::from_name("MPAD").unwrap().is_noisy());
    assert!(!detector.is_noisy());

    assert!(mr.produces_measurements());
    assert!(mxx.produces_measurements());
    assert!(mpp.produces_measurements());
    assert!(!r.produces_measurements());
    assert!(!h.produces_measurements());
    assert!(!x_error.produces_measurements());

    assert!(h.is_single_qubit_gate());
    assert!(!cx.is_single_qubit_gate());
    assert!(cx.is_two_qubit_gate());
    assert!(mxx.is_two_qubit_gate());
    assert!(!mpp.is_two_qubit_gate());
    assert!(!detector.is_two_qubit_gate());

    assert!(mpp.takes_pauli_targets());
    assert!(Gate::from_name("E").unwrap().takes_pauli_targets());
    assert!(!mxx.takes_pauli_targets());
    assert!(!detector.takes_pauli_targets());

    assert!(detector.takes_measurement_record_targets());
    assert!(cx.takes_measurement_record_targets());
    assert!(
        Gate::from_name("XCZ")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("XCX")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("XCY")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("YCX")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(
        !Gate::from_name("YCY")
            .unwrap()
            .takes_measurement_record_targets()
    );
    assert!(!h.takes_measurement_record_targets());
    assert!(!mpp.takes_measurement_record_targets());

    assert!(Gate::from_name("SWAP").unwrap().is_symmetric_gate());
    assert!(h.is_symmetric_gate());
    assert!(mxx.is_symmetric_gate());
    assert!(Gate::from_name("DEPOLARIZE2").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("XCX").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("YCY").unwrap().is_symmetric_gate());
    assert!(Gate::from_name("CZ").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("CX").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("XCY").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("YCX").unwrap().is_symmetric_gate());
    assert!(!Gate::from_name("MPAD").unwrap().is_symmetric_gate());
    assert!(
        !Gate::from_name("PAULI_CHANNEL_2")
            .unwrap()
            .is_symmetric_gate()
    );
    assert!(!detector.is_symmetric_gate());

    assert_eq!(h.inverse().unwrap().canonical_name(), "H");
    assert_eq!(
        Gate::from_name("S")
            .unwrap()
            .inverse()
            .unwrap()
            .canonical_name(),
        "S_DAG"
    );
    assert_eq!(Gate::from_name("M").unwrap().inverse(), None);
    assert_eq!(
        Gate::from_name("M")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "M"
    );
    assert_eq!(
        Gate::from_name("R")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "M"
    );
    assert_eq!(
        Gate::from_name("SPP")
            .unwrap()
            .generalized_inverse()
            .unwrap()
            .canonical_name(),
        "SPP_DAG"
    );
}

#[test]
fn gate_tableau_metadata_matches_owned_unitary_gate_data() {
    // Adapted from Stim v1.16.0 src/stim/gates/gates.test.cc tableau and unitary inverse checks.
    let h = Gate::from_name("H").expect("H");
    let h_tableau = h.tableau().expect("H tableau");
    assert_eq!(h_tableau.x_output(0).expect("H X").to_string(), "+Z");
    assert_eq!(h_tableau.z_output(0).expect("H Z").to_string(), "+X");
    assert!(h.has_tableau());

    let cx = Gate::from_name("CX").expect("CX");
    let cx_tableau = cx.tableau().expect("CX tableau");
    assert_eq!(cx_tableau.x_output(0).expect("CX X0").to_string(), "+XX");
    assert_eq!(cx_tableau.z_output(0).expect("CX Z0").to_string(), "+Z_");
    assert_eq!(cx_tableau.x_output(1).expect("CX X1").to_string(), "+_X");
    assert_eq!(cx_tableau.z_output(1).expect("CX Z1").to_string(), "+ZZ");

    let expected_tableau_names = BTreeSet::from([
        "C_NXYZ",
        "C_NZYX",
        "C_XNYZ",
        "C_XYNZ",
        "C_XYZ",
        "C_ZNYX",
        "C_ZYNX",
        "C_ZYX",
        "CX",
        "CXSWAP",
        "CY",
        "CZ",
        "CZSWAP",
        "H",
        "H_NXY",
        "H_NXZ",
        "H_NYZ",
        "H_XY",
        "H_YZ",
        "I",
        "II",
        "ISWAP",
        "ISWAP_DAG",
        "S",
        "S_DAG",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SWAP",
        "SWAPCX",
        "X",
        "XCX",
        "XCY",
        "XCZ",
        "Y",
        "YCX",
        "YCY",
        "YCZ",
        "Z",
    ]);
    assert_eq!(expected_tableau_names.len(), 46);
    let actual_tableau_names = Gate::all()
        .filter(|gate| gate.has_tableau())
        .map(|gate| gate.canonical_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual_tableau_names, expected_tableau_names);

    for gate_name in expected_tableau_names {
        let gate = Gate::from_name(gate_name).expect("gate");
        let inverse = gate.inverse().expect("unitary inverse");
        let gate_inverse_tableau = gate
            .tableau()
            .expect("gate tableau")
            .inverse()
            .expect("inverse tableau");
        assert_eq!(
            gate_inverse_tableau,
            inverse.tableau().expect("inverse gate tableau"),
            "{gate_name} inverse tableau should match inverse gate metadata"
        );
    }

    for gate in Gate::all() {
        assert_eq!(
            gate.has_tableau(),
            gate.tableau().is_ok(),
            "{} has_tableau should match tableau materialization",
            gate.canonical_name()
        );
    }

    for unsupported in ["M", "R", "DETECTOR", "SPP"] {
        let gate = Gate::from_name(unsupported).expect("unsupported gate");
        assert!(!gate.has_tableau(), "{unsupported}");
        let error = gate.tableau().expect_err("reject missing tableau data");
        assert!(
            error.to_string().contains("does not have tableau data"),
            "{error}"
        );
    }
}

#[test]
fn parses_identity_error_disjoint_probability_lists_like_stim() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_noisy.cc I_ERROR and II_ERROR examples.
    let circuit = Circuit::from_stim_str(concat!(
        "I_ERROR(0.1, 0.2) 0 2 4\n",
        "II_ERROR(0.1, 0.2) 0 2 4 6\n",
    ))
    .expect("parse identity error probability lists");
    assert_eq!(
        circuit.to_stim_string(),
        concat!("I_ERROR(0.1, 0.2) 0 2 4\n", "II_ERROR(0.1, 0.2) 0 2 4 6\n",)
    );

    let instructions = circuit
        .items()
        .iter()
        .map(|item| match item {
            CircuitItem::Instruction(instruction) => Some(instruction),
            CircuitItem::RepeatBlock(_) => None,
        })
        .collect::<Option<Vec<_>>>()
        .expect("identity error fixture should not repeat");
    let mut instructions = instructions.into_iter();
    let i_error = instructions.next().expect("I_ERROR");
    let ii_error = instructions.next().expect("II_ERROR");
    assert!(instructions.next().is_none());
    assert_eq!(
        i_error.probability_arguments().unwrap(),
        Some(vec![
            Probability::try_new(0.1).unwrap(),
            Probability::try_new(0.2).unwrap(),
        ])
    );
    assert_eq!(
        ii_error.probability_arguments().unwrap(),
        Some(vec![
            Probability::try_new(0.1).unwrap(),
            Probability::try_new(0.2).unwrap(),
        ])
    );

    for invalid in [
        "I_ERROR(0.8, 0.4) 0\n",
        "II_ERROR(0.8, 0.4) 0 1\n",
        "I_ERROR(-0.1) 0\n",
        "II_ERROR(2) 0 1\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn bit_target_capability_matches_stim_controlled_gate_flags() {
    // Adapted from Stim v1.16.0 src/stim/gates/gate_data_controlled.cc target flag split.
    for invalid in [
        "XCX rec[-1] 0\n",
        "XCY rec[-1] 0\n",
        "YCX rec[-1] 0\n",
        "YCY rec[-1] 0\n",
        "XCX sweep[0] 0\n",
        "XCY sweep[0] 0\n",
        "YCX sweep[0] 0\n",
        "YCY sweep[0] 0\n",
    ] {
        assert!(Circuit::from_stim_str(invalid).is_err(), "{invalid}");
    }

    for valid in [
        "XCZ 0 rec[-1]\n",
        "YCZ 0 rec[-1]\n",
        "CX rec[-1] 0\n",
        "CY rec[-1] 0\n",
        "CZ rec[-1] 0\n",
        "XCZ 0 sweep[0]\n",
        "YCZ 0 sweep[0]\n",
        "CX sweep[0] 0\n",
        "CY sweep[0] 0\n",
        "CZ sweep[0] 0\n",
    ] {
        assert!(Circuit::from_stim_str(valid).is_ok(), "{valid}");
    }
}
