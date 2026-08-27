#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "resource tests use deterministic model fixtures and exact typed failures"
)]

use std::mem::ManuallyDrop;

use stab_engine::{
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionRecordLimitSubject,
    DetectionResourceKind, DetectionResourceLimitError, MeasurementToDetectionCompiler,
    ReferenceSampleMode, validate_detection_sampling_circuit_with_limits,
};
use stab_model::{
    Circuit, CircuitInstruction, Gate, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    Target,
};

fn compile(
    circuit: &Circuit,
    limits: DetectionConversionLimits,
) -> Result<stab_engine::MeasurementToDetectionPlan, DetectionCompileError> {
    MeasurementToDetectionCompiler::new()
        .limits(limits)
        .reference_sample_mode(ReferenceSampleMode::SkipReferenceSample)
        .compile(circuit)
}

fn compile_resource(error: DetectionCompileError) -> DetectionResourceLimitError {
    match error {
        DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource)) => resource,
        other => panic!("expected detection resource limit, got {other:?}"),
    }
}

fn detection_resource(error: DetectionError) -> DetectionResourceLimitError {
    match error {
        DetectionError::ResourceLimit(resource) => resource,
        other => panic!("expected detection resource limit, got {other:?}"),
    }
}

fn nested_circuit(depth: usize, mut body: Circuit) -> Circuit {
    for _ in 0..depth {
        let mut outer = Circuit::new();
        outer.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(1).expect("valid repeat count"),
            body,
            None,
        ));
        body = outer;
    }
    body
}

#[test]
fn repeat_nesting_rejects_before_recursive_work() {
    let accepted = nested_circuit(RepeatNestingLimit::HARD_MAX, Circuit::new());
    compile(&accepted, DetectionConversionLimits::default())
        .expect("the fixed nesting boundary must compile");

    let rejected = nested_circuit(RepeatNestingLimit::HARD_MAX + 1, Circuit::new());
    for resource in [
        compile_resource(
            compile(&rejected, DetectionConversionLimits::default())
                .expect_err("the first excess direct-conversion level must reject"),
        ),
        detection_resource(
            validate_detection_sampling_circuit_with_limits(
                &rejected,
                DetectionConversionLimits::default(),
            )
            .expect_err("detection sampling must apply the same nesting boundary"),
        ),
    ] {
        assert_eq!(resource.kind(), DetectionResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), (RepeatNestingLimit::HARD_MAX + 1) as u64);
        assert_eq!(resource.limit(), RepeatNestingLimit::HARD_MAX as u64);
    }

    for detector_frame in [false, true] {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || {
                let body = if detector_frame {
                    Circuit::from_stim_str("HERALDED_ERASE(0.125) 0\n")
                        .expect("parse frame fixture")
                } else {
                    Circuit::new()
                };
                let circuit = ManuallyDrop::new(nested_circuit(10_000, body));
                if detector_frame {
                    detection_resource(
                        validate_detection_sampling_circuit_with_limits(
                            &circuit,
                            DetectionConversionLimits::default(),
                        )
                        .expect_err("deep frame admission must reject"),
                    )
                } else {
                    compile_resource(
                        compile(&circuit, DetectionConversionLimits::default())
                            .expect_err("deep conversion admission must reject"),
                    )
                }
            })
            .expect("spawn constrained-stack regression");
        let resource = handle
            .join()
            .expect("admission must not overflow the stack");
        assert_eq!(resource.kind(), DetectionResourceKind::RepeatNesting);
        assert_eq!(resource.actual(), (RepeatNestingLimit::HARD_MAX + 1) as u64);
    }
}

fn measurement_circuit(width: usize) -> Circuit {
    let targets = (0..width)
        .map(|index| {
            Target::qubit(
                QubitId::new(u32::try_from(index).expect("test width fits u32"))
                    .expect("test qubit fits Stim target range"),
                false,
            )
        })
        .collect();
    let instruction = CircuitInstruction::new(
        Gate::from_name("M").expect("measurement gate"),
        Vec::new(),
        targets,
        None,
    )
    .expect("valid wide measurement");
    let mut circuit = Circuit::new();
    circuit.append_instruction(instruction);
    circuit
}

#[test]
fn default_record_width_accepts_the_boundary_and_rejects_the_next_bit() {
    compile(
        &measurement_circuit(1_000_000),
        DetectionConversionLimits::default(),
    )
    .expect("the default measurement-width boundary must compile");

    let resource = compile_resource(
        compile(
            &measurement_circuit(1_000_001),
            DetectionConversionLimits::default(),
        )
        .expect_err("the first excess measurement bit must reject"),
    );
    assert_eq!(
        resource.kind(),
        DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::MeasurementRecord)
    );
    assert_eq!(
        (resource.actual(), resource.limit()),
        (1_000_001, 1_000_000)
    );
}

#[test]
fn default_traversal_limits_accept_the_boundary_and_reject_the_next_unit() {
    let mut expanded_text = String::from("REPEAT 100000 {\n");
    for _ in 0..10 {
        expanded_text.push_str("TICK\n");
    }
    expanded_text.push_str("}\n");
    let mut expanded = Circuit::from_stim_str(&expanded_text).expect("parse expanded fixture");
    compile(&expanded, DetectionConversionLimits::default())
        .expect("one million expanded instructions must compile");
    expanded
        .append_from_stim_text("TICK\n")
        .expect("append first excess instruction");
    let resource = compile_resource(
        compile(&expanded, DetectionConversionLimits::default())
            .expect_err("the first excess expanded instruction must reject"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::ExpandedInstructions);
    assert_eq!(
        (resource.actual(), resource.limit()),
        (1_000_001, 1_000_000)
    );

    let mut inner = Circuit::new();
    inner.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(999).expect("valid repeat count"),
        Circuit::new(),
        None,
    ));
    let mut iterations = Circuit::new();
    iterations.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(1_000).expect("valid repeat count"),
        inner,
        None,
    ));
    compile(&iterations, DetectionConversionLimits::default())
        .expect("one million aggregate repeat iterations must compile");
    iterations.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(1).expect("valid repeat count"),
        Circuit::new(),
        None,
    ));
    let resource = compile_resource(
        compile(&iterations, DetectionConversionLimits::default())
            .expect_err("the first excess repeat iteration must reject"),
    );
    assert_eq!(resource.kind(), DetectionResourceKind::RepeatIterations);
    assert_eq!(
        (resource.actual(), resource.limit()),
        (1_000_001, 1_000_000)
    );
}
