#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "closed Stim gate contract fixtures use named lookups for compact diagnostics"
)]

use std::assert_matches;

use std::collections::BTreeMap;

use stab_model::{
    Circuit, Gate, GateArgumentRule, GateCategory, GateTargetGroupKind, GateTargetRule, ModelError,
    QubitId, Target, ValidationError,
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
fn gate_validation_reports_model_owned_errors() {
    let h = Gate::from_name("H").expect("H");
    let q0 = Target::qubit(QubitId::new(0).expect("q0"), false);
    let q1 = Target::qubit(QubitId::new(1).expect("q1"), false);
    h.validate(&[], std::slice::from_ref(&q0)).expect("valid H");
    assert_matches!(
        h.validate(&[0.5], std::slice::from_ref(&q0)),
        Err(ModelError::Validation(
            ValidationError::InvalidArgumentCount { gate: "H", .. }
        ))
    );

    let cx = Gate::from_name("CX").expect("CX");
    cx.validate_targets(&[q0, q1]).expect("valid CX");
    assert_matches!(
        cx.validate_targets(std::slice::from_ref(&q0)),
        Err(ModelError::Validation(
            ValidationError::InvalidTargetCount {
                gate: "CX",
                count: 1
            }
        ))
    );
    assert_matches!(
        cx.validate_targets(&[q0, q0]),
        Err(ModelError::Validation(ValidationError::InvalidTarget {
            gate: "CX",
            ..
        }))
    );

    let channel = Gate::from_name("PAULI_CHANNEL_1").expect("channel");
    assert_matches!(
        channel.validate(&[0.6, 0.6, 0.0], &[]),
        Err(ModelError::Validation(
            ValidationError::InvalidArgument { .. }
        ))
    );
}

/// The typed flow and decomposition tables are transcriptions of Stim's gate data, so every
/// gate's typed entry must render back to the vendored Stim v1.16.0 source it came from.
#[test]
fn typed_gate_flows_and_decompositions_render_pinned_stim_gate_data() {
    let vendored = vendored_gate_blocks();
    let mut flow_gates = 0;
    let mut decomposed_gates = 0;
    for gate in Gate::all() {
        let name = gate.canonical_name();
        let block = vendored
            .get(name)
            .unwrap_or_else(|| panic!("{name} is missing from the vendored Stim gate data"));

        let stim_flows = flow_data(block);
        match gate.flow_descriptors() {
            Some(flows) => {
                let rendered: Vec<String> = flows.iter().map(ToString::to_string).collect();
                let expected: Vec<String> = stim_flows.iter().map(|f| normalize_flow(f)).collect();
                assert_eq!(rendered, expected, "{name} flows");
                flow_gates += 1;
            }
            // Stim lists a unitary gate's tableau images under the same field, without arrows.
            None => assert!(
                stim_flows.iter().all(|flow| !flow.contains("->")),
                "{name} has Stim flows but no typed flows: {stim_flows:?}"
            ),
        }

        let stim_decomposition = decomposition_text(block);
        match (gate.h_s_cx_m_r_decomposition(), stim_decomposition) {
            (Some(decomposition), Some(text)) => {
                let circuit = decomposition.to_circuit();
                let stim = Circuit::from_stim_str(text).expect("vendored decomposition parses");
                assert_eq!(circuit, stim, "{name} decomposition");
                let rendered = Circuit::from_stim_str(&decomposition.to_string())
                    .expect("rendered decomposition parses");
                assert_eq!(rendered, circuit, "{name} decomposition text");
                for step in decomposition.steps() {
                    assert!(
                        ["H", "S", "CX", "M", "R"].contains(&step.gate().canonical_name()),
                        "{name} step {step}"
                    );
                }
                decomposed_gates += 1;
            }
            (None, None) => {}
            (typed, stim) => panic!("{name}: typed {typed:?} but Stim {stim:?}"),
        }
    }
    assert_eq!(flow_gates, 15);
    assert_eq!(decomposed_gates, 61);

    let measurement = Gate::from_name("M").expect("M");
    let flows = measurement.flow_descriptors().expect("M flows");
    let [record, kept] = flows else {
        panic!("M has two flows");
    };
    assert_eq!(
        record
            .measurements()
            .iter()
            .map(|m| m.get())
            .collect::<Vec<_>>(),
        [-1]
    );
    assert!(record.output().paulis().is_empty());
    assert!(kept.measurements().is_empty());
    assert!(Gate::from_name("H").expect("H").unitary_rows().is_some());
    assert!(
        Gate::from_name("DETECTOR")
            .expect("DETECTOR")
            .unitary_rows()
            .is_none()
    );
}

/// Stim writes `+` on unsigned outputs and `I` for identity; the typed rendering omits `+` and
/// writes `_`, as Stab's flow text does.
fn normalize_flow(flow: &str) -> String {
    let (input, output) = flow.split_once(" -> ").expect("flow arrow");
    let side = |text: &str| {
        let text = text.strip_prefix('+').unwrap_or(text);
        if text.starts_with("rec[") {
            text.to_string()
        } else {
            text.replace('I', "_")
        }
    };
    format!("{} -> {}", side(input), side(output))
}

fn vendored_gate_blocks() -> BTreeMap<&'static str, &'static str> {
    let files = [
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_annotations.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_blocks.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_collapsing.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_controlled.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_hada.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_heralded.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_noisy.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pair_measure.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pauli.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pauli_product.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_period_3.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_period_4.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_pp.cc"),
        include_str!("../../../vendor/stim/src/stim/gates/gate_data_swaps.cc"),
    ];
    let mut blocks = BTreeMap::new();
    for file in files {
        for block in file.split(".name = \"").skip(1) {
            let (name, rest) = block.split_once('"').expect("gate name");
            assert!(blocks.insert(name, rest).is_none(), "{name} defined twice");
        }
    }
    blocks
}

fn flow_data(block: &'static str) -> Vec<&'static str> {
    let (_, rest) = block.split_once(".flow_data =").expect("flow_data field");
    let (_, rest) = rest.split_once('{').expect("flow_data list");
    let (list, _) = rest.split_once('}').expect("flow_data list end");
    list.lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .flat_map(|line| line.split('"').skip(1).step_by(2))
        .collect()
}

fn decomposition_text(block: &'static str) -> Option<&'static str> {
    let (_, rest) = block.split_once(".h_s_cx_m_r_decomposition = ")?;
    let rest = rest.strip_prefix("R\"CIRCUIT(")?;
    Some(rest.split_once(")CIRCUIT\"").expect("raw circuit end").0)
}
