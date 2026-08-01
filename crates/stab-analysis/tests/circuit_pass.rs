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
use stab_model::{
    Circuit, Gate, ModelError, RepeatCount, RepeatNestingLimit,
    advanced::repeat_block_with_tag_bytes,
};

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
        assert_eq!(
            input.context().limits().max_represented_items(),
            input.circuit().items().len() as u64
        );
        assert_eq!(input.resources().represented_items(), 1);
        Ok(CircuitPassOutput::new(input.circuit().clone(), ()))
    }
}

#[test]
fn pass_input_is_admitted_before_implementation_runs() {
    let source = circuit("H 0\nX 1\n");
    let pass = ObservedIdentityPass::default();
    let limits = CircuitPassLimits::default().with_max_represented_items(1);
    let error = run_circuit_pass(&pass, &source, &(), &CircuitPassContext::new(limits))
        .expect_err("reject input before invoking pass");

    assert!(!pass.ran.get());
    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.operation(), ResourceOperation::CircuitPass);
    assert_eq!(resource.resource(), ResourceKind::RepresentedItems);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
    assert_eq!(error.resource_stage(), Some(CircuitPassStage::Input));

    let admitted_source = circuit("H 0\n");
    let admitted_pass = ObservedIdentityPass::default();
    let admitted = run_circuit_pass(
        &admitted_pass,
        &admitted_source,
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_max_represented_items(1)),
    )
    .expect("dispatch admitted input with framework-owned context and resources");
    assert!(admitted_pass.ran.get());
    assert_eq!(admitted.circuit(), &admitted_source);
}

#[test]
fn pass_input_resource_dimensions_are_admitted_independently() {
    let explicit_nesting = RepeatNestingLimit::try_new(3).expect("valid explicit nesting");
    let explicit = CircuitPassLimits::new(5, 7, 11, 13, explicit_nesting);
    assert_eq!(explicit.max_represented_items(), 5);
    assert_eq!(explicit.max_target_occurrences(), 7);
    assert_eq!(explicit.max_argument_values(), 11);
    assert_eq!(explicit.max_projected_payload_bytes(), 13);
    assert_eq!(explicit.repeat_nesting(), explicit_nesting);

    let defaults = CircuitPassLimits::default();
    assert_eq!(
        defaults.max_represented_items(),
        CircuitPassLimits::DEFAULT_MAX_REPRESENTED_ITEMS
    );
    assert_eq!(
        defaults.max_target_occurrences(),
        CircuitPassLimits::DEFAULT_MAX_TARGET_OCCURRENCES
    );
    assert_eq!(
        defaults.max_argument_values(),
        CircuitPassLimits::DEFAULT_MAX_ARGUMENT_VALUES
    );
    assert_eq!(
        defaults.max_projected_payload_bytes(),
        CircuitPassLimits::DEFAULT_MAX_PROJECTED_PAYLOAD_BYTES
    );

    let maximal = CircuitPassLimits::maximal();
    assert_eq!(maximal.max_represented_items(), u64::MAX);
    assert_eq!(maximal.max_target_occurrences(), u64::MAX);
    assert_eq!(maximal.max_argument_values(), u64::MAX);
    assert_eq!(maximal.max_projected_payload_bytes(), u64::MAX);
    assert_eq!(maximal.repeat_nesting().get(), RepeatNestingLimit::HARD_MAX);

    let pass = ObservedIdentityPass::default();
    let target_source = circuit("H 0 1\n");
    let target_limits = CircuitPassLimits::default().with_max_target_occurrences(1);
    let target_error = run_circuit_pass(
        &pass,
        &target_source,
        &(),
        &CircuitPassContext::new(target_limits),
    )
    .expect_err("reject the second represented target occurrence");
    let target_resource = target_error
        .resource_limit_error()
        .expect("target resource diagnostic");
    assert_eq!(target_resource.resource(), ResourceKind::TargetOccurrences);
    assert_eq!(target_resource.operation(), ResourceOperation::CircuitPass);
    assert_eq!(target_resource.operation().as_str(), "circuit-pass");
    assert_eq!(target_resource.actual(), 2);
    assert_eq!(target_resource.limit(), 1);

    let argument_source = circuit("QUBIT_COORDS(1, 2) 0\n");
    let argument_limits = CircuitPassLimits::default().with_max_argument_values(1);
    let argument_error = run_circuit_pass(
        &pass,
        &argument_source,
        &(),
        &CircuitPassContext::new(argument_limits),
    )
    .expect_err("reject the second represented argument value");
    let argument_resource = argument_error
        .resource_limit_error()
        .expect("argument resource diagnostic");
    assert_eq!(argument_resource.resource(), ResourceKind::ArgumentValues);
    assert_eq!(argument_resource.actual(), 2);
    assert_eq!(argument_resource.limit(), 1);

    let item_source = circuit("H 0\nX 1\n");
    let item_limits = CircuitPassLimits::default().with_max_represented_items(1);
    let item_error = run_circuit_pass(
        &pass,
        &item_source,
        &(),
        &CircuitPassContext::new(item_limits),
    )
    .expect_err("reject the second represented circuit item");
    let item_resource = item_error
        .resource_limit_error()
        .expect("represented-item resource diagnostic");
    assert_eq!(item_resource.resource(), ResourceKind::RepresentedItems);
    assert_eq!(item_resource.resource().as_str(), "represented-items");

    let single_instruction =
        CircuitPassResources::try_new(1, 1, 0, 0, 0).expect("representable resource projection");
    let byte_limits = CircuitPassLimits::default()
        .with_max_projected_payload_bytes(single_instruction.projected_payload_bytes() - 1);
    let byte_error = run_circuit_pass(
        &pass,
        &circuit("H 0\n"),
        &(),
        &CircuitPassContext::new(byte_limits),
    )
    .expect_err("reject input above the projected-payload limit");
    let byte_resource = byte_error
        .resource_limit_error()
        .expect("projected-payload resource diagnostic");
    assert_eq!(
        byte_resource.resource(),
        ResourceKind::ProjectedPayloadBytes
    );
    assert_eq!(
        byte_resource.actual(),
        single_instruction.projected_payload_bytes()
    );

    let repeat_count = RepeatCount::try_new(2).expect("repeat count");
    let mut nested = Circuit::new();
    nested.append_repeat_block(repeat_block_with_tag_bytes(
        repeat_count,
        circuit("H 0\n"),
        None,
    ));
    let zero_nesting = RepeatNestingLimit::try_new(0).expect("zero nesting policy");
    let nesting_limits = CircuitPassLimits::default().with_repeat_nesting(zero_nesting);
    let nesting_error = run_circuit_pass(
        &pass,
        &nested,
        &(),
        &CircuitPassContext::new(nesting_limits),
    )
    .expect_err("reject input repeat nesting before pass dispatch");
    let nesting_resource = nesting_error
        .resource_limit_error()
        .expect("repeat-nesting resource diagnostic");
    assert_eq!(nesting_resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(nesting_resource.actual(), 1);
    assert!(!pass.ran.get(), "no rejected input may dispatch the pass");
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

#[test]
fn pass_output_projection_admits_every_resource_before_lowering() {
    let two_items = CircuitPassResources::try_new(2, 0, 0, 0, 0).expect("items");
    let two_targets = CircuitPassResources::try_new(0, 2, 0, 0, 0).expect("targets");
    let two_arguments = CircuitPassResources::try_new(0, 0, 2, 0, 0).expect("arguments");
    let payload = CircuitPassResources::try_new(1, 1, 1, 1, 0).expect("payload bytes");
    let nested = CircuitPassResources::try_new(0, 0, 0, 0, 2).expect("nesting");
    let one_nesting = RepeatNestingLimit::try_new(1).expect("one nesting level");

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
            payload,
            CircuitPassLimits::default()
                .with_max_projected_payload_bytes(payload.projected_payload_bytes() - 1),
            ResourceKind::ProjectedPayloadBytes,
        ),
        (
            nested,
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
        .expect_err("reject projected output before lowering");
        let resource = error.resource_limit_error().expect("resource diagnostic");
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(
            error.resource_stage(),
            Some(CircuitPassStage::OutputProjection)
        );
        assert!(!pass.ran.get(), "projection rejection dispatched lowering");
    }

    let empty = CircuitPassResources::try_new(0, 0, 0, 0, 0).expect("empty resources");
    assert_eq!(empty.tag_bytes(), 0);
    assert_eq!(empty.repeat_nesting(), 0);
    assert_eq!(empty.with_repeat_nesting(3).repeat_nesting(), 3);
    let count_overflow = CircuitPassResources::try_new(1, 0, 0, 0, 0)
        .expect("one item")
        .checked_with_additional(u64::MAX, 0, 0, 0)
        .expect_err("represented-item projection must use checked arithmetic");
    assert_eq!(count_overflow.resource(), ResourceKind::RepresentedItems);
    let byte_overflow = CircuitPassResources::try_new(u64::MAX, 0, 0, 0, 0)
        .expect_err("projected-payload calculation must use checked arithmetic");
    assert_eq!(
        byte_overflow.resource(),
        ResourceKind::ProjectedPayloadBytes
    );
}

#[test]
fn circuit_pass_resource_stages_are_typed_across_framework_phases() {
    fn assert_stage(
        error: CircuitPassError<Infallible>,
        expected: CircuitPassStage,
        expected_code: &str,
    ) {
        let stage = error.resource_stage().expect("typed circuit-pass stage");
        assert_eq!(stage, expected);
        assert_eq!(stage.as_str(), expected_code);
    }

    let input_error = run_circuit_pass(
        &ObservedIdentityPass::default(),
        &circuit("H 0\nX 1\n"),
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_max_represented_items(1)),
    )
    .expect_err("reject input before dispatch");
    assert_stage(input_error, CircuitPassStage::Input, "input");

    let projection_pass = ProjectionOnlyPass {
        projection: CircuitPassResources::try_new(2, 0, 0, 0, 0).expect("projection"),
        ran: Cell::new(false),
    };
    let projection_error = run_circuit_pass(
        &projection_pass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_max_represented_items(1)),
    )
    .expect_err("reject projected output before lowering");
    assert_stage(
        projection_error,
        CircuitPassStage::OutputProjection,
        "projected-output",
    );

    let output_error = run_circuit_pass(
        &DeepOutputPass::default(),
        &Circuit::new(),
        &(),
        &CircuitPassContext::new(
            CircuitPassLimits::default()
                .with_repeat_nesting(RepeatNestingLimit::try_new(1).expect("one nesting level")),
        ),
    )
    .expect_err("reject actual output after lowering");
    assert_stage(output_error, CircuitPassStage::Output, "output");
}

#[test]
fn circuit_pass_errors_preserve_framework_and_diagnostic_categories() {
    #[derive(Debug, thiserror::Error)]
    #[error("contract diagnostic")]
    struct ContractDiagnostic;

    struct ContractPass;

    impl CircuitPass for ContractPass {
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
            input: CircuitPassInput<'_>,
            _options: &Self::Options,
        ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
            Ok(CircuitPassOutput::new(input.circuit().clone(), ()))
        }
    }

    let resource_error = run_circuit_pass(
        &ContractPass,
        &circuit("H 0\nX 1\n"),
        &(),
        &CircuitPassContext::new(CircuitPassLimits::default().with_max_represented_items(1)),
    )
    .expect_err("construct the framework resource category");
    assert!(matches!(
        &resource_error,
        CircuitPassError::ResourceLimit(_)
    ));
    let resource = resource_error
        .resource_limit_error()
        .expect("resource rejection must retain its typed category");
    assert_eq!(resource.resource(), ResourceKind::RepresentedItems);

    let projection_error = CircuitPassError::<ContractDiagnostic>::ProjectionUnderestimated {
        resource: ResourceKind::TargetOccurrences,
        projected: 1,
        actual: 2,
    };
    assert!(matches!(
        &projection_error,
        CircuitPassError::ProjectionUnderestimated {
            resource: ResourceKind::TargetOccurrences,
            projected: 1,
            actual: 2,
        }
    ));
    assert_eq!(
        projection_error.projection_underestimate(),
        Some((ResourceKind::TargetOccurrences, 1, 2))
    );

    let diagnostic_error = CircuitPassError::Diagnostic(ContractDiagnostic);
    assert!(matches!(
        &diagnostic_error,
        CircuitPassError::Diagnostic(ContractDiagnostic)
    ));
    assert_eq!(
        diagnostic_error
            .diagnostic()
            .expect("concrete pass diagnostic")
            .to_string(),
        "contract diagnostic"
    );
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
        Ok(CircuitPassResources::try_new(3, 1, 0, 10, 1).expect("representable projected output"))
    }

    fn run(
        &self,
        _input: CircuitPassInput<'_>,
        _options: &Self::Options,
    ) -> Result<CircuitPassOutput<Self::Report>, Self::Diagnostic> {
        self.ran.set(true);
        let repeat_count = RepeatCount::try_new(2).expect("valid repeat count");
        let nested = Circuit::from_stim_str("H 0\n").expect("body");
        let mut middle = Circuit::new();
        middle.append_repeat_block(repeat_block_with_tag_bytes(
            repeat_count,
            nested,
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

#[test]
fn pass_output_is_validated_before_it_is_returned() {
    let nesting = RepeatNestingLimit::try_new(1).expect("valid tightened limit");
    let limits = CircuitPassLimits::default().with_repeat_nesting(nesting);
    let pass = DeepOutputPass::default();
    let error = run_circuit_pass(
        &pass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::new(limits),
    )
    .expect_err("reject pass output outside the Stim-compatible nesting envelope");

    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
    assert_eq!(error.resource_stage(), Some(CircuitPassStage::Output));
    assert!(
        pass.ran.get(),
        "actual-output admission must follow lowering"
    );

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

    let underestimated = run_circuit_pass(
        &UnderestimatingPass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::default(),
    )
    .expect_err("reject a pass that understates its projected output payload");
    assert_eq!(
        underestimated.projection_underestimate(),
        Some((ResourceKind::RepresentedItems, 0, 1))
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

    let (second_circuit, second_report) = second.into_parts();
    assert_eq!(&second_circuit, first.circuit());
    assert_eq!(&second_report, first.report());

    let repeat_count = RepeatCount::try_new(1).expect("valid repeat count");
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
        circuit_without_noise(&deeply_nested).expect("preserve the legacy direct resource policy"),
        deeply_nested
    );

    let error = run_circuit_pass(
        &WithoutNoisePass,
        &deeply_nested,
        &WithoutNoiseOptions,
        &CircuitPassContext::default(),
    )
    .expect_err("the explicit admitted pass retains its repeat-nesting safety policy");
    let resource = error.resource_limit_error().expect("resource diagnostic");
    assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(resource.actual(), 257);
    assert_eq!(resource.limit(), 256);
}

#[test]
fn pass_specific_diagnostics_remain_typed() {
    #[derive(Debug, thiserror::Error)]
    #[error("unsupported research operation")]
    struct UnsupportedOperation;

    struct RejectingPass;

    impl CircuitPass for RejectingPass {
        type Options = ();
        type Report = ();
        type Diagnostic = UnsupportedOperation;

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
            Err(UnsupportedOperation)
        }
    }

    let error = run_circuit_pass(
        &RejectingPass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::default(),
    )
    .expect_err("preserve pass diagnostic");

    assert!(matches!(error, CircuitPassError::Diagnostic(_)));
    assert!(error.diagnostic().is_some());
    assert!(error.to_string().contains("unsupported research operation"));
}

#[test]
fn closed_model_rejects_unsupported_gate_lowering() {
    struct UnsupportedGatePass;

    impl CircuitPass for UnsupportedGatePass {
        type Options = ();
        type Report = ();
        type Diagnostic = ModelError;

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
            Gate::from_name("CUSTOM_RESEARCH_GATE")?;
            Ok(CircuitPassOutput::new(Circuit::new(), ()))
        }
    }

    let error = run_circuit_pass(
        &UnsupportedGatePass,
        &Circuit::new(),
        &(),
        &CircuitPassContext::default(),
    )
    .expect_err("the closed Stim model must reject an unknown extension gate");
    let diagnostic = error.diagnostic().expect("typed model diagnostic");
    assert!(diagnostic.to_string().contains("CUSTOM_RESEARCH_GATE"));
}
