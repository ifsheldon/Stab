#![allow(
    clippy::expect_used,
    reason = "closed Stim gate contract fixtures use named lookups for compact diagnostics"
)]

use stab_model::{
    Gate, GateArgumentRule, GateCategory, GateTargetGroupKind, GateTargetRule, ModelError, QubitId,
    Target, ValidationError,
    advanced::{
        gate_decomposition, gate_flow_descriptors, gate_unitary_rows, validate_gate,
        validate_gate_targets,
    },
};

#[test]
fn canonical_gate_registry_owns_aliases_and_metadata() {
    assert_eq!(Gate::all().len(), 81);

    let h = Gate::from_name("h").expect("case-insensitive H");
    assert_eq!(h.canonical_name(), "H");
    assert_eq!(h.aliases(), &["H", "H_XZ"]);
    assert_eq!(h.category(), GateCategory::HadamardLike);
    assert_eq!(h.argument_rule(), GateArgumentRule::Exact(0));
    assert_eq!(h.target_rule(), GateTargetRule::AnySingleQubit);
    assert_eq!(h.target_group_kind(), GateTargetGroupKind::Singles);
    assert!(h.is_unitary());
    assert!(h.is_single_qubit_gate());

    let cx = Gate::from_name("CNOT").expect("CNOT alias");
    assert_eq!(cx.canonical_name(), "CX");
    assert_eq!(cx.aliases(), &["CNOT", "CX", "ZCX"]);
    assert_eq!(cx.target_rule(), GateTargetRule::ClassicalControlPairs);
    assert_eq!(cx.target_group_kind(), GateTargetGroupKind::Pairs);
    assert!(cx.is_two_qubit_gate());
    assert!(cx.takes_measurement_record_targets());

    let mpp = Gate::from_name("MPP").expect("MPP");
    assert_eq!(mpp.argument_rule(), GateArgumentRule::OptionalProbability);
    assert_eq!(mpp.target_rule(), GateTargetRule::PauliProducts);
    assert_eq!(mpp.target_group_kind(), GateTargetGroupKind::PauliProducts);
    assert!(mpp.produces_measurements());
    assert!(mpp.takes_pauli_targets());

    let detector = Gate::from_name("DETECTOR").expect("DETECTOR");
    assert!(!detector.can_fuse());
    assert_eq!(detector.target_rule(), GateTargetRule::RecOnly);
    assert_eq!(Gate::from_name("MZ").expect("MZ").canonical_name(), "M");
    assert_eq!(
        Gate::from_name("SWAPCZ").expect("SWAPCZ").canonical_name(),
        "CZSWAP"
    );
    assert_eq!(
        Gate::from_name("missing"),
        Err(ModelError::Validation(ValidationError::UnknownGate(
            "missing".to_string()
        )))
    );
}

#[test]
fn gate_classification_views_agree_with_stim_v1_16_lists_for_every_gate() {
    // Independent expectations: the frozen Stim v1.16.0 classification lists that the
    // deleted string-matching sites used to encode (WS5 one-owner consolidation).
    const PRODUCES_RESULTS: &[&str] = &[
        "MPAD",
        "MX",
        "MY",
        "M",
        "MRX",
        "MRY",
        "MR",
        "MPP",
        "HERALDED_ERASE",
        "HERALDED_PAULI_CHANNEL_1",
        "MXX",
        "MYY",
        "MZZ",
    ];
    const RESETS: &[&str] = &["RX", "RY", "R", "MRX", "MRY", "MR"];
    const NOISY: &[&str] = &[
        "DEPOLARIZE1",
        "DEPOLARIZE2",
        "X_ERROR",
        "Y_ERROR",
        "Z_ERROR",
        "I_ERROR",
        "II_ERROR",
        "PAULI_CHANNEL_1",
        "PAULI_CHANNEL_2",
        "E",
        "ELSE_CORRELATED_ERROR",
        "HERALDED_ERASE",
        "HERALDED_PAULI_CHANNEL_1",
        "MX",
        "MY",
        "M",
        "MRX",
        "MRY",
        "MR",
        "MPP",
        "MXX",
        "MYY",
        "MZZ",
    ];
    const UNITARY: &[&str] = &[
        "XCX",
        "XCY",
        "XCZ",
        "YCX",
        "YCY",
        "YCZ",
        "CX",
        "CY",
        "CZ",
        "H",
        "H_XY",
        "H_YZ",
        "H_NXY",
        "H_NXZ",
        "H_NYZ",
        "I",
        "X",
        "Y",
        "Z",
        "C_XYZ",
        "C_ZYX",
        "C_NXYZ",
        "C_XNYZ",
        "C_XYNZ",
        "C_NZYX",
        "C_ZNYX",
        "C_ZYNX",
        "SQRT_X",
        "SQRT_X_DAG",
        "SQRT_Y",
        "SQRT_Y_DAG",
        "S",
        "S_DAG",
        "II",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SPP",
        "SPP_DAG",
        "SWAP",
        "ISWAP",
        "CXSWAP",
        "SWAPCX",
        "CZSWAP",
        "ISWAP_DAG",
    ];
    const SYMMETRIC_PAIRS: &[&str] = &[
        "DEPOLARIZE2",
        "II_ERROR",
        "XCX",
        "YCY",
        "CZ",
        "II",
        "SQRT_XX",
        "SQRT_XX_DAG",
        "SQRT_YY",
        "SQRT_YY_DAG",
        "SQRT_ZZ",
        "SQRT_ZZ_DAG",
        "SWAP",
        "ISWAP",
        "ISWAP_DAG",
        "CZSWAP",
        "MXX",
        "MYY",
        "MZZ",
    ];
    const HERALDED: &[&str] = &["HERALDED_ERASE", "HERALDED_PAULI_CHANNEL_1"];
    const PAD_TARGETS: &[&str] = &["MPAD"];

    for list in [
        PRODUCES_RESULTS,
        RESETS,
        NOISY,
        UNITARY,
        SYMMETRIC_PAIRS,
        HERALDED,
        PAD_TARGETS,
    ] {
        for name in list {
            assert_eq!(
                Gate::from_name(name)
                    .expect("expectation names a gate")
                    .canonical_name(),
                *name,
                "classification lists must use canonical gate names"
            );
        }
    }

    assert_eq!(Gate::all().len(), 81);
    for gate in Gate::all() {
        let name = gate.canonical_name();
        assert_eq!(
            gate.produces_measurements(),
            PRODUCES_RESULTS.contains(&name),
            "produces_measurements for {name}"
        );
        assert_eq!(
            gate.is_reset(),
            RESETS.contains(&name),
            "is_reset for {name}"
        );
        assert_eq!(
            gate.is_noisy(),
            NOISY.contains(&name),
            "is_noisy for {name}"
        );
        assert_eq!(
            gate.is_unitary(),
            UNITARY.contains(&name),
            "is_unitary for {name}"
        );
        let symmetric = matches!(
            gate.target_rule(),
            GateTargetRule::AnySingleQubit | GateTargetRule::MeasurementQubits
        ) || SYMMETRIC_PAIRS.contains(&name);
        assert_eq!(
            gate.is_symmetric_gate(),
            symmetric,
            "is_symmetric_gate for {name}"
        );
        assert_eq!(
            gate.produces_heralded_results(),
            HERALDED.contains(&name),
            "produces_heralded_results for {name}"
        );
        assert_eq!(
            gate.targets_are_pad_values(),
            PAD_TARGETS.contains(&name),
            "targets_are_pad_values for {name}"
        );
        if gate.produces_heralded_results() || gate.targets_are_pad_values() {
            assert!(
                gate.produces_measurements(),
                "heralds and pads are results for {name}"
            );
        }
    }
}

#[test]
fn gate_alias_lists_agree_with_the_gate_table_for_every_gate() {
    // Independent expectation: the frozen Stim v1.16.0 alias lists that the deleted
    // per-name aliases() match used to encode; every other gate aliases only itself.
    const MULTI_ALIAS: &[(&str, &[&str])] = &[
        ("M", &["M", "MZ"]),
        ("MR", &["MR", "MRZ"]),
        ("R", &["R", "RZ"]),
        ("CX", &["CNOT", "CX", "ZCX"]),
        ("CY", &["CY", "ZCY"]),
        ("CZ", &["CZ", "ZCZ"]),
        ("H", &["H", "H_XZ"]),
        ("E", &["CORRELATED_ERROR", "E"]),
        ("S", &["S", "SQRT_Z"]),
        ("S_DAG", &["S_DAG", "SQRT_Z_DAG"]),
        ("CZSWAP", &["CZSWAP", "SWAPCZ"]),
    ];

    let mut seen = std::collections::BTreeSet::new();
    let mut accepted_names = 0_usize;
    for gate in Gate::all() {
        let name = gate.canonical_name();
        match MULTI_ALIAS.iter().find(|(canonical, _)| *canonical == name) {
            Some((_, aliases)) => assert_eq!(gate.aliases(), *aliases, "aliases for {name}"),
            None => assert_eq!(gate.aliases(), [name].as_slice(), "aliases for {name}"),
        }
        assert!(
            gate.aliases().contains(&name),
            "canonical name is a member of its alias list for {name}"
        );
        for alias in gate.aliases() {
            assert!(
                seen.insert(*alias),
                "alias {alias} is claimed by one gate only"
            );
            assert_eq!(
                Gate::from_name(alias).expect("alias resolves"),
                gate,
                "alias {alias} resolves to {name}"
            );
            accepted_names += 1;
        }
    }
    assert_eq!(accepted_names, 93, "Stim v1.16.0 accepted-name count");
}

#[test]
fn gate_validation_reports_model_owned_errors() {
    let h = Gate::from_name("H").expect("H");
    let q0 = Target::qubit(QubitId::new(0).expect("q0"), false);
    let q1 = Target::qubit(QubitId::new(1).expect("q1"), false);
    validate_gate(h, &[], std::slice::from_ref(&q0)).expect("valid H");
    assert!(matches!(
        validate_gate(h, &[0.5], std::slice::from_ref(&q0)),
        Err(ModelError::Validation(
            ValidationError::InvalidArgumentCount { gate: "H", .. }
        ))
    ));

    let cx = Gate::from_name("CX").expect("CX");
    validate_gate_targets(cx, &[q0.clone(), q1]).expect("valid CX");
    assert!(matches!(
        validate_gate_targets(cx, std::slice::from_ref(&q0)),
        Err(ModelError::Validation(
            ValidationError::InvalidTargetCount {
                gate: "CX",
                count: 1
            }
        ))
    ));
    assert!(matches!(
        validate_gate_targets(cx, &[q0.clone(), q0]),
        Err(ModelError::Validation(ValidationError::InvalidTarget {
            gate: "CX",
            ..
        }))
    ));

    let channel = Gate::from_name("PAULI_CHANNEL_1").expect("channel");
    assert!(matches!(
        validate_gate(channel, &[0.6, 0.6, 0.0], &[]),
        Err(ModelError::Validation(
            ValidationError::InvalidArgument { .. }
        ))
    ));
}

#[test]
fn raw_gate_descriptors_stay_model_owned_and_scalar() {
    let measurement = Gate::from_name("M").expect("M");
    assert_eq!(
        gate_flow_descriptors(measurement),
        Some(&["Z -> rec[-1]", "Z -> Z"][..])
    );

    let h = Gate::from_name("H").expect("H");
    assert!(gate_unitary_rows(h).is_some());
    assert_eq!(
        gate_decomposition(h)
            .expect("H decomposition")
            .as_stim_str(),
        "\nH 0\n"
    );

    let detector = Gate::from_name("DETECTOR").expect("DETECTOR");
    assert!(gate_flow_descriptors(detector).is_none());
    assert!(gate_unitary_rows(detector).is_none());
    assert!(gate_decomposition(detector).is_none());
}
