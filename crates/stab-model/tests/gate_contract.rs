#![allow(
    clippy::expect_used,
    reason = "closed Stim gate contract fixtures use named lookups for compact diagnostics"
)]

use stab_model::{
    Gate, GateArgumentRule, GateCategory, GateTargetGroupKind, GateTargetRule, ModelError, QubitId,
    Target,
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
        Err(ModelError::UnknownGate("missing".to_string()))
    );
}

#[test]
fn gate_validation_reports_model_owned_errors() {
    let h = Gate::from_name("H").expect("H");
    let q0 = Target::qubit(QubitId::new(0).expect("q0"), false);
    let q1 = Target::qubit(QubitId::new(1).expect("q1"), false);
    validate_gate(h, &[], std::slice::from_ref(&q0)).expect("valid H");
    assert!(matches!(
        validate_gate(h, &[0.5], std::slice::from_ref(&q0)),
        Err(ModelError::InvalidArgumentCount { gate: "H", .. })
    ));

    let cx = Gate::from_name("CX").expect("CX");
    validate_gate_targets(cx, &[q0.clone(), q1]).expect("valid CX");
    assert!(matches!(
        validate_gate_targets(cx, std::slice::from_ref(&q0)),
        Err(ModelError::InvalidTargetCount {
            gate: "CX",
            count: 1
        })
    ));
    assert!(matches!(
        validate_gate_targets(cx, &[q0.clone(), q0]),
        Err(ModelError::InvalidTarget { gate: "CX", .. })
    ));

    let channel = Gate::from_name("PAULI_CHANNEL_1").expect("channel");
    assert!(matches!(
        validate_gate(channel, &[0.6, 0.6, 0.0], &[]),
        Err(ModelError::InvalidArgument { .. })
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
