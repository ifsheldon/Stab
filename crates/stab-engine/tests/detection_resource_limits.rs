#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "resource tests use deterministic model fixtures and exact typed failures"
)]

use std::convert::Infallible;

use stab_engine::{
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionResourceKind,
    DetectionResourceLimitError, DetectionSamplingCompiler, DetectionSamplingSessionOptions,
    MeasurementToDetectionCompiler, RandomPolicy, ReferenceSampleMode, Seed, ShotCount,
    circuit_reference_signs,
};
use stab_model::{
    Circuit, CircuitInstruction, Gate, MemoryBudget, QubitId, RepeatBlock, RepeatCount,
    RepeatNestingLimit, Target,
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
    MeasurementToDetectionCompiler {
        reference_sample_mode: ReferenceSampleMode::SkipReferenceSample,
        memory_budget: limits.memory_budget,
        cancellation: limits.cancellation,
    }
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
        match circuit_reference_signs(
            circuit,
            &DetectionConversionLimits {
                memory_budget: MemoryBudget::from_bytes(limit),
                ..Default::default()
            },
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
    DetectionSamplingCompiler::default()
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
                let plan = DetectionSamplingCompiler::default()
                    .compile(&circuit)
                    .expect("the accepted detector nesting boundary must compile");
                let mut session = plan
                    .session(&DetectionSamplingSessionOptions {
                        random: RandomPolicy::Seeded(Seed::new(7)),
                        ..Default::default()
                    })
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
        compile_resource(
            DetectionSamplingCompiler::default()
                .compile(&rejected)
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
                    compile_resource(
                        DetectionSamplingCompiler::default()
                            .compile(&circuit)
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
fn record_width_above_the_retired_one_million_bit_cap_compiles() {
    let plan = compile(
        &measurement_circuit(1_000_001),
        DetectionConversionLimits::default(),
    )
    .expect("a record wider than the retired cap fits the default budget");
    assert_eq!(plan.measurement_width().get(), 1_000_001);
}

#[test]
fn reference_sign_entrypoint_propagates_the_exact_budget() {
    let repeated = Circuit::from_stim_str("M 0\nREPEAT 3 {\nDETECTOR rec[-1]\n}\n")
        .expect("parse repeated term fixture");
    let exact_bytes = compact_reference_sign_plan_bytes(&repeated);
    let limits = |bytes| DetectionConversionLimits {
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..Default::default()
    };
    circuit_reference_signs(&repeated, &limits(exact_bytes)).expect("admit the exact budget");
    let error = detection_resource(
        circuit_reference_signs(&repeated, &limits(exact_bytes - 1))
            .expect_err("reject the first excess byte"),
    );
    assert_eq!(error.kind(), DetectionResourceKind::CompiledBytes);
    assert_eq!(error.limit(), exact_bytes - 1);
    assert!(error.actual() >= exact_bytes);
}
