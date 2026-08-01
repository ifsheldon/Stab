#![allow(
    clippy::expect_used,
    reason = "external pass contract tests use compact, admitted circuit fixtures"
)]

use stab_analysis::{
    CircuitPassContext, CircuitPassLimits, CircuitPassResources, CircuitPassStage, ResourceKind,
    ResourceOperation, run_circuit_pass,
};
use stab_model::{Circuit, Probability};
use stab_reference_noise_pass::{
    XErrorAfterSingleQubitUnitariesOptions, XErrorAfterSingleQubitUnitariesPass,
};

fn probability(value: f64) -> Probability {
    Probability::try_new(value).expect("valid test probability")
}

#[test]
fn inserts_deterministic_noise_after_single_target_group_unitaries() {
    let source = Circuit::from_stim_str(
        "H[tag] 0 1\n\
         X_ERROR(0.125) 3\n\
         CX 0 1\n\
         X 2\n\
         M 0\n",
    )
    .expect("valid source circuit");
    let options = XErrorAfterSingleQubitUnitariesOptions::new(probability(0.125));
    let context = CircuitPassContext::default();

    let first = run_circuit_pass(
        &XErrorAfterSingleQubitUnitariesPass,
        &source,
        &options,
        &context,
    )
    .expect("run external pass");
    let second = run_circuit_pass(
        &XErrorAfterSingleQubitUnitariesPass,
        &source,
        &options,
        &context,
    )
    .expect("repeat external pass");

    assert_eq!(
        first, second,
        "same typed input must produce the same result"
    );
    assert_eq!(
        first.circuit().to_stim_string(),
        concat!(
            "H[tag] 0 1\n",
            "X_ERROR(0.125) 0 1\n",
            "X_ERROR(0.125) 3\n",
            "CX 0 1\n",
            "X 2\n",
            "X_ERROR(0.125) 2\n",
            "M 0\n",
        )
    );
    assert_eq!(first.report().inserted_represented_instruction_count(), 2);
    assert_eq!(first.report().affected_target_count(), 3);
}

#[test]
fn preserves_nested_repeats_annotations_targets_and_opaque_tags() {
    let source = Circuit::from_stim_bytes(
        b"QUBIT_COORDS[origin](1, 2) 0\n\
          REPEAT[\xff] 3 {\n\
              H[\xfe] 0 1\n\
              REPEAT[inner] 2 {\n\
                  SHIFT_COORDS[shift](0, 1)\n\
                  X[axis] 2\n\
                  CX[pair] 2 3\n\
                  M[readout] 0\n\
              }\n\
          }\n",
    )
    .expect("valid source circuit with opaque tags");
    let options = XErrorAfterSingleQubitUnitariesOptions::new(probability(0.25));

    let output = run_circuit_pass(
        &XErrorAfterSingleQubitUnitariesPass,
        &source,
        &options,
        &CircuitPassContext::default(),
    )
    .expect("run external pass");

    let expected_lines: &[&[u8]] = &[
        b"QUBIT_COORDS[origin](1, 2) 0",
        b"REPEAT[\xff] 3 {",
        b"    H[\xfe] 0 1",
        b"    X_ERROR(0.25) 0 1",
        b"    REPEAT[inner] 2 {",
        b"        SHIFT_COORDS[shift](0, 1)",
        b"        X[axis] 2",
        b"        X_ERROR(0.25) 2",
        b"        CX[pair] 2 3",
        b"        M[readout] 0",
        b"    }",
        b"}",
    ];
    let mut expected = expected_lines.join(&b'\n');
    expected.push(b'\n');
    assert_eq!(output.circuit().to_stim_bytes(), expected);
    assert_eq!(output.report().inserted_represented_instruction_count(), 2);
    assert_eq!(output.report().affected_target_count(), 3);
}

#[test]
fn excludes_non_unitary_pair_and_pauli_product_operations() {
    let source = Circuit::from_stim_str(
        "CX 0 1\n\
         SQRT_XX 2 3\n\
         SPP X4*Y5\n\
         M 0\n\
         DETECTOR(1, 2) rec[-1]\n",
    )
    .expect("valid source circuit");
    let options = XErrorAfterSingleQubitUnitariesOptions::new(probability(0.5));

    let output = run_circuit_pass(
        &XErrorAfterSingleQubitUnitariesPass,
        &source,
        &options,
        &CircuitPassContext::default(),
    )
    .expect("run external pass");

    assert_eq!(output.circuit().to_stim_string(), source.to_stim_string());
    assert_eq!(output.report().inserted_represented_instruction_count(), 0);
    assert_eq!(output.report().affected_target_count(), 0);
}

#[test]
fn common_admission_rejects_expanded_output_after_admitting_input() {
    let source = Circuit::from_stim_str("H 0\n").expect("valid source circuit");
    let options = XErrorAfterSingleQubitUnitariesOptions::new(probability(0.1));
    let limits = CircuitPassLimits::default().with_max_represented_items(1);

    let error = run_circuit_pass(
        &XErrorAfterSingleQubitUnitariesPass,
        &source,
        &options,
        &CircuitPassContext::new(limits),
    )
    .expect_err("one admitted input item becomes two output items");

    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.operation(), ResourceOperation::CircuitPass);
    assert_eq!(resource.resource(), ResourceKind::RepresentedItems);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
    assert_eq!(
        error.resource_stage(),
        Some(CircuitPassStage::OutputProjection)
    );
}

#[test]
fn opaque_tag_output_projection_rejects_before_pass_allocation() {
    let source = Circuit::from_stim_bytes(b"H[\xff] 0\n").expect("opaque source tag");
    let options = XErrorAfterSingleQubitUnitariesOptions::new(probability(0.1));
    let input_resources =
        CircuitPassResources::try_new(1, 1, 0, 1, 0).expect("representable input resources");
    let limits = CircuitPassLimits::default()
        .with_max_projected_payload_bytes(input_resources.projected_payload_bytes());

    let mut result = None;
    let allocations = allocation_counter::measure(|| {
        result = Some(run_circuit_pass(
            &XErrorAfterSingleQubitUnitariesPass,
            &source,
            &options,
            &CircuitPassContext::new(limits),
        ));
    });
    assert_eq!(allocations.count_total, 0, "{allocations:?}");
    assert_eq!(allocations.bytes_total, 0, "{allocations:?}");

    let error = result
        .expect("allocation measurement executed")
        .expect_err("projected output adds an instruction, target, and argument");
    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.resource(), ResourceKind::ProjectedPayloadBytes);
    assert_eq!(resource.limit(), input_resources.projected_payload_bytes());
    assert_eq!(
        error.resource_stage(),
        Some(CircuitPassStage::OutputProjection)
    );
}
