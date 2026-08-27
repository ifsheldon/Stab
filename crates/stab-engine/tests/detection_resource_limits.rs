#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "resource tests use deterministic model fixtures and exact typed failures"
)]

use std::convert::Infallible;

use stab_engine::{
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionRecordLimitSubject,
    DetectionResourceKind, DetectionResourceLimitError, DetectionSamplingCompiler,
    MeasurementToDetectionCompiler, RandomPolicy, ReferenceSampleMode, Seed, ShotCount,
    circuit_reference_signs_with_limits, validate_detection_sampling_circuit_with_limits,
};
use stab_model::{
    Circuit, CircuitInstruction, Gate, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    Target,
};
use stab_records::{DetectionBatchView, DetectionSink};

#[derive(Default)]
struct NullDetectionSink {
    shots: usize,
}

impl DetectionSink for NullDetectionSink {
    type Error = Infallible;

    fn write_batch(&mut self, batch: DetectionBatchView<'_>) -> Result<(), Self::Error> {
        self.shots += batch.shot_count();
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

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

fn compact_reference_sign_plan_bytes(circuit: &Circuit) -> u64 {
    let mut limit = 0;
    loop {
        match circuit_reference_signs_with_limits(
            circuit,
            DetectionConversionLimits::default().with_max_compiled_bytes(limit),
        ) {
            Ok(_) => return limit,
            Err(error) => {
                let resource = detection_resource(error);
                assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
                assert!(resource.actual() > limit);
                limit = resource.actual();
            }
        }
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

fn shallow_repeat_siblings(count: usize, body: &Circuit) -> Circuit {
    let mut circuit = Circuit::new();
    for _ in 0..count {
        circuit.append_repeat_block(RepeatBlock::new(
            RepeatCount::try_new(1).expect("valid repeat count"),
            body.clone(),
            None,
        ));
    }
    circuit
}

#[test]
fn shallow_repeat_breadth_does_not_consume_nesting_budget() {
    let sibling_count = 4_096;
    let conversion = shallow_repeat_siblings(
        sibling_count,
        &Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").expect("conversion body"),
    );
    compile(&conversion, DetectionConversionLimits::default())
        .expect("shallow conversion siblings must not count as nested repeats");

    let direct = shallow_repeat_siblings(
        sibling_count,
        &Circuit::from_stim_str("RX 0\nOBSERVABLE_INCLUDE(0) X0\n").expect("direct-frame body"),
    );
    DetectionSamplingCompiler::new()
        .compile(&direct)
        .expect("shallow direct-frame siblings must not count as nested repeats");
}

#[test]
fn repeat_nesting_rejects_before_recursive_work() {
    for direct_frame in [false, true] {
        std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || {
                let body = if direct_frame {
                    Circuit::from_stim_str("RX 0\nOBSERVABLE_INCLUDE(0) X0\n")
                        .expect("parse direct-frame fixture")
                } else {
                    Circuit::from_stim_str("M 0\nDETECTOR rec[-1]\n").expect("parse fused fixture")
                };
                let circuit = nested_circuit(RepeatNestingLimit::HARD_MAX, body);
                let plan = DetectionSamplingCompiler::new()
                    .limits(DetectionConversionLimits::default())
                    .compile(&circuit)
                    .expect("the accepted detector nesting boundary must compile");
                let mut session = plan
                    .session(RandomPolicy::Seeded(Seed::new(7)))
                    .expect("the accepted detector nesting boundary must allocate state");
                let mut sink = NullDetectionSink::default();
                session
                    .run(ShotCount::new(1), &mut sink)
                    .expect("the accepted detector nesting boundary must execute");
                assert_eq!(sink.shots, 1);
                drop(session);
                drop(plan);
                drop(circuit);
            })
            .expect("spawn constrained-stack boundary regression")
            .join()
            .expect("accepted nesting must not overflow the stack");
    }

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
                let circuit = nested_circuit(10_000, body);
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

#[test]
fn reference_sign_entrypoint_propagates_exact_resource_boundaries() {
    let record =
        Circuit::from_stim_str("M 0 1\nDETECTOR rec[-1]\n").expect("parse record-width fixture");
    let record_limits = DetectionConversionLimits::default().with_max_record_bits(2);
    circuit_reference_signs_with_limits(&record, record_limits)
        .expect("admit exact measurement-record width");

    let repeated = Circuit::from_stim_str("M 0\nREPEAT 3 {\nDETECTOR rec[-1]\n}\n")
        .expect("parse repeated term fixture");
    let exact_bytes = compact_reference_sign_plan_bytes(&repeated);
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(4)
        .with_max_compiled_terms(1)
        .with_max_compiled_bytes(exact_bytes);
    circuit_reference_signs_with_limits(&repeated, exact)
        .expect("admit exact repeat, term, and byte limits");

    for (circuit, limits, kind, actual, limit) in [
        (
            &record,
            record_limits.with_max_record_bits(1),
            DetectionResourceKind::RecordBits(DetectionRecordLimitSubject::MeasurementRecord),
            2,
            1,
        ),
        (
            &repeated,
            exact.with_max_compiled_terms(0),
            DetectionResourceKind::CompiledTerms,
            1,
            0,
        ),
        (
            &repeated,
            exact.with_max_compiled_bytes(exact_bytes - 1),
            DetectionResourceKind::CompiledBytes,
            exact_bytes,
            exact_bytes - 1,
        ),
    ] {
        let error = detection_resource(
            circuit_reference_signs_with_limits(circuit, limits)
                .expect_err("reject first excess reference-sign resource unit"),
        );
        assert_eq!(error.kind(), kind);
        assert_eq!((error.actual(), error.limit()), (actual, limit));
    }
}
