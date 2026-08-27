#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "these contract tests must fail immediately when fixed fixtures or pass assertions break"
)]

use std::cell::Cell;
use std::convert::Infallible;

use stab_analysis::{
    CircuitPass, CircuitPassContext, CircuitPassError, CircuitPassInput, CircuitPassLimits,
    CircuitPassOutput, CircuitPassResources, CircuitPassStage, ResourceKind, ResourceOperation,
    WithoutNoiseOptions, WithoutNoisePass, circuit_without_noise, run_circuit_pass,
};
use stab_model::{Circuit, RepeatCount, RepeatNestingLimit, advanced::repeat_block_with_tag_bytes};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("valid test circuit")
}

#[derive(Default)]
struct ObservedIdentityPass {
    ran: Cell<bool>,
}

impl CircuitPass for ObservedIdentityPass {
    type Options = ();
    type Report = ();
    type Diagnostic = Infallible;

    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        Ok(input.resources())
    }

    fn run(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        self.ran.set(true);
        Ok(CircuitPassOutput::new(input.circuit().clone(), ()))
    }
}

struct ProjectionOnlyPass {
    projection: CircuitPassResources,
    ran: Cell<bool>,
}

impl CircuitPass for ProjectionOnlyPass {
    type Options = ();
    type Report = ();
    type Diagnostic = Infallible;

    fn project_output_resources(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        Ok(self.projection)
    }

    fn run(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        self.ran.set(true);
        Ok(CircuitPassOutput::new(Circuit::new(), ()))
    }
}

#[derive(Default)]
struct DeepOutputPass {
    ran: Cell<bool>,
}

impl CircuitPass for DeepOutputPass {
    type Options = ();
    type Report = ();
    type Diagnostic = Infallible;

    fn project_output_resources(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        Ok(CircuitPassResources::try_new(3, 1, 0, 10, 1).expect("projected output"))
    }

    fn run(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        self.ran.set(true);
        let repeat_count = RepeatCount::try_new(2).expect("repeat count");
        let mut middle = Circuit::new();
        middle.append_repeat_block(repeat_block_with_tag_bytes(
            repeat_count,
            circuit("H 0\n"),
            Some(b"inner"),
        ));
        let mut outer = Circuit::new();
        outer.append_repeat_block(repeat_block_with_tag_bytes(
            repeat_count,
            middle,
            Some(b"outer"),
        ));
        Ok(CircuitPassOutput::new(outer, ()))
    }
}

struct UnderestimatingPass;

impl CircuitPass for UnderestimatingPass {
    type Options = ();
    type Report = ();
    type Diagnostic = Infallible;

    fn project_output_resources(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        Ok(CircuitPassResources::try_new(0, 0, 0, 0, 0).expect("empty projection"))
    }

    fn run(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        Ok(CircuitPassOutput::new(circuit("H 0\n"), ()))
    }
}

#[derive(Debug, thiserror::Error)]
#[error("contract diagnostic")]
struct ContractDiagnostic;

struct RejectingPass;

impl CircuitPass for RejectingPass {
    type Options = ();
    type Report = ();
    type Diagnostic = ContractDiagnostic;

    fn project_output_resources(
        &self,
        input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassResources, Self::Diagnostic> {
        Ok(input.resources())
    }

    fn run(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        Err(ContractDiagnostic)
    }
}

#[test]
fn circuit_pass_resource_admission_is_atomic_across_framework_stages() {
    let payload = CircuitPassResources::try_new(1, 1, 0, 0, 0).expect("single instruction");
    let repeat_count = RepeatCount::try_new(2).expect("repeat count");
    let mut nested = Circuit::new();
    nested.append_repeat_block(repeat_block_with_tag_bytes(
        repeat_count,
        circuit("H 0\n"),
        None,
    ));
    let zero_nesting = RepeatNestingLimit::try_new(0).expect("zero nesting");
    for (source, limits, expected_resource) in [
        (
            circuit("H 0\nX 1\n"),
            CircuitPassLimits::default().with_max_represented_items(1),
            ResourceKind::RepresentedItems,
        ),
        (
            circuit("H 0 1\n"),
            CircuitPassLimits::default().with_max_target_occurrences(1),
            ResourceKind::TargetOccurrences,
        ),
        (
            circuit("QUBIT_COORDS(1, 2) 0\n"),
            CircuitPassLimits::default().with_max_argument_values(1),
            ResourceKind::ArgumentValues,
        ),
        (
            circuit("H 0\n"),
            CircuitPassLimits::default()
                .with_max_projected_payload_bytes(payload.projected_payload_bytes() - 1),
            ResourceKind::ProjectedPayloadBytes,
        ),
        (
            nested,
            CircuitPassLimits::default().with_repeat_nesting(zero_nesting),
            ResourceKind::RepeatNesting,
        ),
    ] {
        let pass = ObservedIdentityPass::default();
        let error = run_circuit_pass(&pass, &source, &(), &CircuitPassContext::new(limits))
            .expect_err("reject input before dispatch");
        let resource = error.resource_limit_error().expect("input resource error");
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(resource.operation(), ResourceOperation::CircuitPass);
        assert!(resource.actual() > resource.limit());
        assert_eq!(error.resource_stage(), Some(CircuitPassStage::Input));
        assert!(!pass.ran.get());
    }

    let two_items = CircuitPassResources::try_new(2, 0, 0, 0, 0).expect("items");
    let two_targets = CircuitPassResources::try_new(0, 2, 0, 0, 0).expect("targets");
    let two_arguments = CircuitPassResources::try_new(0, 0, 2, 0, 0).expect("arguments");
    let projected_payload = CircuitPassResources::try_new(1, 1, 1, 1, 0).expect("payload");
    let projected_nesting = CircuitPassResources::try_new(0, 0, 0, 0, 2).expect("nesting");
    let one_nesting = RepeatNestingLimit::try_new(1).expect("one nesting");
    for (projection, limits, expected_resource) in [
        (
            two_items,
            CircuitPassLimits::default().with_max_represented_items(1),
            ResourceKind::RepresentedItems,
        ),
        (
            two_targets,
            CircuitPassLimits::default().with_max_target_occurrences(1),
            ResourceKind::TargetOccurrences,
        ),
        (
            two_arguments,
            CircuitPassLimits::default().with_max_argument_values(1),
            ResourceKind::ArgumentValues,
        ),
        (
            projected_payload,
            CircuitPassLimits::default()
                .with_max_projected_payload_bytes(projected_payload.projected_payload_bytes() - 1),
            ResourceKind::ProjectedPayloadBytes,
        ),
        (
            projected_nesting,
            CircuitPassLimits::default().with_repeat_nesting(one_nesting),
            ResourceKind::RepeatNesting,
        ),
    ] {
        let pass = ProjectionOnlyPass {
            projection,
            ran: Cell::new(false),
        };
        let error = run_circuit_pass(
            &pass,
            &Circuit::new(),
            &(),
            &CircuitPassContext::new(limits),
        )
        .expect_err("reject projected output before dispatch");
        let resource = error
            .resource_limit_error()
            .expect("projection resource error");
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(resource.operation(), ResourceOperation::CircuitPass);
        assert!(resource.actual() > resource.limit());
        assert_eq!(
            error.resource_stage(),
            Some(CircuitPassStage::OutputProjection)
        );
        assert!(!pass.ran.get());
    }

    let deep_output_pass = DeepOutputPass::default();
    let deep_output_error = run_circuit_pass(
        &deep_output_pass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_repeat_nesting(one_nesting)),
    )
    .expect_err("reject actual output beyond the admitted nesting limit");
    assert_eq!(
        deep_output_error
            .resource_limit_error()
            .expect("output resource error")
            .resource(),
        ResourceKind::RepeatNesting
    );
    assert_eq!(
        deep_output_error.resource_stage(),
        Some(CircuitPassStage::Output)
    );
    assert!(deep_output_pass.ran.get());

    let count_overflow = CircuitPassResources::try_new(1, 0, 0, 0, 0)
        .expect("one item")
        .checked_with_additional(u64::MAX, 0, 0, 0)
        .expect_err("resource counts use checked arithmetic");
    assert_eq!(count_overflow.resource(), ResourceKind::RepresentedItems);
    let payload_overflow = CircuitPassResources::try_new(u64::MAX, 0, 0, 0, 0)
        .expect_err("projected payload uses checked arithmetic");
    assert_eq!(
        payload_overflow.resource(),
        ResourceKind::ProjectedPayloadBytes
    );
}

#[test]
fn circuit_pass_validates_output_and_preserves_typed_diagnostics() {
    let admitted_source = circuit("H 0\n");
    let admitted_pass = ObservedIdentityPass::default();
    let admitted = run_circuit_pass(
        &admitted_pass,
        &admitted_source,
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_max_represented_items(1)),
    )
    .expect("admit input and output at the declared boundary");
    assert!(admitted_pass.ran.get());
    assert_eq!(admitted.circuit(), &admitted_source);

    let underestimated = run_circuit_pass(
        &UnderestimatingPass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::default(),
    )
    .expect_err("reject understated output projection");
    assert_eq!(
        underestimated.projection_underestimate(),
        Some((ResourceKind::RepresentedItems, 0, 1))
    );

    let diagnostic = run_circuit_pass(
        &RejectingPass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::default(),
    )
    .expect_err("preserve pass-specific diagnostic");
    assert!(matches!(diagnostic, CircuitPassError::Diagnostic(_)));
    assert_eq!(
        diagnostic
            .diagnostic()
            .expect("typed pass diagnostic")
            .to_string(),
        "contract diagnostic"
    );
}

#[test]
fn without_noise_pass_preserves_legacy_output_report_and_resource_policy() {
    let source = circuit(
        "H[tag] 0\n\
         X_ERROR(0.25) 0\n\
         HERALDED_ERASE[herald](0.5) 1\n\
         REPEAT[loop] 2 {\n\
             DEPOLARIZE1(0.1) 0\n\
             M[measure](0.2) 0\n\
             DETECTOR[det](3, 4) rec[-1]\n\
         }\n",
    );
    let context = CircuitPassContext::default();

    let first = run_circuit_pass(&WithoutNoisePass, &source, &WithoutNoiseOptions, &context)
        .expect("run built-in pass");
    let second = run_circuit_pass(&WithoutNoisePass, &source, &WithoutNoiseOptions, &context)
        .expect("repeat built-in pass");

    assert_eq!(first, second, "the pass must be deterministic");
    assert_eq!(
        first.circuit().to_stim_string(),
        concat!(
            "H[tag] 0\n",
            "MPAD[herald] 0\n",
            "REPEAT[loop] 2 {\n",
            "    M[measure] 0\n",
            "    DETECTOR[det](3, 4) rec[-1]\n",
            "}\n",
        )
    );
    assert_eq!(first.report().removed_noise_instructions(), 2);
    assert_eq!(first.report().stripped_measurement_probabilities(), 1);
    assert_eq!(first.report().replaced_heralded_noise_instructions(), 1);

    let repeat_count = RepeatCount::try_new(1).expect("repeat count");
    let mut deeply_nested = Circuit::new();
    for _ in 0..=RepeatNestingLimit::HARD_MAX {
        let mut wrapper = Circuit::new();
        wrapper.append_repeat_block(repeat_block_with_tag_bytes(
            repeat_count,
            deeply_nested,
            None,
        ));
        deeply_nested = wrapper;
    }
    assert_eq!(
        circuit_without_noise(&deeply_nested).expect("legacy direct resource policy"),
        deeply_nested
    );

    let error = run_circuit_pass(
        &WithoutNoisePass,
        &deeply_nested,
        &WithoutNoiseOptions,
        &CircuitPassContext::default(),
    )
    .expect_err("explicit pass retains repeat-nesting admission");
    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(resource.actual(), 257);
    assert_eq!(resource.limit(), 256);
}
