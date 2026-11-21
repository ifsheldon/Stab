#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "admission tests use direct fixture setup assertions for focused failures"
)]

//! `sample` and `detect` admit expanded work without a work cap and refuse a plan or session
//! only when its conservative storage exceeds the caller's memory budget, before allocating
//! that storage, advancing randomness, or touching a sink.

use std::assert_matches;
use std::convert::Infallible;

use stab_engine::{
    DetectionCompileError, DetectionError, DetectionExecutionError, DetectionResourceKind,
    DetectionSamplingCompiler, DetectionSamplingSessionOptions, MeasurementToDetectionCompiler,
    MeasurementToDetectionSessionOptions, RandomPolicy, ReferenceSampleMode, SamplingCompileError,
    SamplingCompiler, SamplingExecutionError, SamplingPlan, SamplingSessionOptions, Seed,
    ShotCount,
};
use stab_model::{Circuit, MemoryBudget, MemoryBudgetExceeded};
use stab_records::{MeasurementBatchView, MeasurementSink};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse admission fixture")
}

fn sampling(bytes: u64) -> SamplingCompiler {
    SamplingCompiler {
        memory_budget: MemoryBudget::from_bytes(bytes),
        ..SamplingCompiler::default()
    }
}

/// Smallest budget that compiles a plan and opens a session, found by bisection.
fn minimal_session_budget(circuit: &Circuit) -> u64 {
    let fits = |bytes| {
        sampling(bytes)
            .compile(circuit)
            .ok()
            .and_then(|plan| {
                plan.session(&SamplingSessionOptions {
                    random: RandomPolicy::Seeded(Seed::new(1)),
                    ..Default::default()
                })
                .ok()
            })
            .is_some()
    };
    let (mut low, mut high) = (0_u64, 1 << 30);
    assert!(fits(high));
    while low < high {
        let middle = low + (high - low) / 2;
        if fits(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[derive(Default)]
struct CountingSink {
    batches: usize,
}

impl MeasurementSink for CountingSink {
    type Error = Infallible;

    fn write_batch(&mut self, _batch: MeasurementBatchView<'_>) -> Result<(), Self::Error> {
        self.batches += 1;
        Ok(())
    }

    fn finish(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn sample_and_detect_admit_expanded_work_above_the_retired_one_million_cap() {
    // 2,000,002 expanded instructions, which the retired per-shot cap refused.
    let long = circuit("REPEAT 2000000 {\nH 0\n}\nM 0\nDETECTOR rec[-1]\n");
    let plan = SamplingCompiler::default()
        .compile(&long)
        .expect("sample admits expanded work without a cap");
    assert_eq!(plan.measurement_width().get(), 1);
    DetectionSamplingCompiler::default()
        .compile(&long)
        .expect("detect admits the same circuit");
}

#[test]
fn sampling_admits_the_exact_budget_and_rejects_the_first_excess_byte_before_work() {
    let fixture = circuit("REPEAT 3 {\nH 0 1\nCX 0 1\nM 0 1\n}\nDETECTOR rec[-1]\n");
    let exact = minimal_session_budget(&fixture);
    let plan = sampling(exact)
        .compile(&fixture)
        .expect("the exact budget compiles");
    let mut session = plan
        .session(&SamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(5)),
            ..Default::default()
        })
        .expect("the exact budget opens a session");
    let mut sink = CountingSink::default();
    session
        .run(ShotCount::new(3), &mut sink)
        .expect("the admitted session runs");
    assert_eq!(sink.batches, 1);

    // One byte less is refused by a typed budget error at compilation or session creation, so
    // no RNG is seeded and no sink is ever called.
    let refused = match sampling(exact - 1).compile(&fixture) {
        Err(SamplingCompileError::CompiledBytesLimit { exceeded }) => {
            assert_eq!(exceeded.budget().bytes(), exact - 1);
            assert!(!exceeded.required().is_lower_bound());
            exceeded.required().value()
        }
        Err(other) => panic!("unexpected compile error {other:?}"),
        Ok(plan) => match plan.session(&SamplingSessionOptions {
            random: RandomPolicy::Seeded(Seed::new(5)),
            ..Default::default()
        }) {
            Err(SamplingExecutionError::SessionStorageLimit { exceeded }) => {
                assert_eq!(exceeded.budget().bytes(), exact - 1);
                assert!(!exceeded.required().is_lower_bound(), "small estimate");
                exceeded.required().value()
            }
            other => panic!("expected a typed session budget refusal, got {other:?}"),
        },
    };
    assert_eq!(refused, exact);
}

#[test]
fn a_wide_compact_record_is_refused_against_the_budget_without_allocating_it() {
    // A one-shot 10^15-bit record is representable but far above any budget: the session must
    // be refused from its arithmetic size, allocating only constant bookkeeping.
    let wide = circuit("REPEAT 1000000000000000 {\nM 0\n}\n");
    let plan: SamplingPlan = SamplingCompiler::default()
        .compile(&wide)
        .expect("the compact program itself fits the budget");
    let measured = allocation_counter::measure(|| {
        let error = plan
            .session(&SamplingSessionOptions {
                random: RandomPolicy::Seeded(Seed::new(1)),
                ..Default::default()
            })
            .expect_err("the session record exceeds the budget");
        assert_matches!(error, SamplingExecutionError::SessionStorageLimit { .. });
        assert!(error.to_string().contains("memory budget"), "{error}");
    });
    assert!(measured.bytes_max < 1 << 20, "{measured:?}");

    match DetectionSamplingCompiler::default().compile(&circuit(
        "REPEAT 1000000000000000 {\nM 0\n}\nDETECTOR rec[-1]\n",
    )) {
        Err(DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource))) => {
            assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
            assert_eq!(resource.limit(), MemoryBudget::DEFAULT.bytes());
        }
        other => panic!("detect must refuse the record against the budget, got {other:?}"),
    }
}

#[test]
fn detection_sampling_budget_reaches_compilation() {
    let fixture = circuit("M 0\nDETECTOR rec[-1]\n");
    match (DetectionSamplingCompiler {
        memory_budget: MemoryBudget::from_bytes(1),
        ..DetectionSamplingCompiler::default()
    })
    .compile(&fixture)
    {
        Err(DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource))) => {
            assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
            assert_eq!(resource.limit(), 1);
        }
        other => panic!("expected a typed budget refusal, got {other:?}"),
    }
}

/// The budget a session or streaming-run refusal was measured against.
fn refused_budget(exceeded: Option<&MemoryBudgetExceeded>) -> MemoryBudget {
    exceeded.expect("a typed session budget refusal").budget()
}

#[test]
fn session_options_without_a_budget_inherit_the_plan_budget_and_a_set_budget_overrides_it() {
    // A folded record of 100,000 measurements compiles to a few instructions, while every
    // session keeps 100,000 measurement planes or a window of them beside the records.
    let fixture = circuit("REPEAT 100000 {\nM 0\nDETECTOR rec[-1]\n}\n");
    let small = MemoryBudget::from_bytes(64 << 10);
    let one_byte = MemoryBudget::from_bytes(1);
    let shots = ShotCount::new(1);

    let small_plan = sampling(small.bytes())
        .compile(&fixture)
        .expect("the compact program fits the small budget");
    let default_plan = SamplingCompiler::default()
        .compile(&fixture)
        .expect("compile under the default budget");
    let options = |memory_budget| SamplingSessionOptions {
        reference_sample_mode: ReferenceSampleMode::SkipReferenceSample,
        memory_budget,
        ..SamplingSessionOptions::default()
    };
    let refusal = |result: Result<_, SamplingExecutionError>| {
        refused_budget(
            result
                .err()
                .as_ref()
                .and_then(|error| error.memory_budget_exceeded()),
        )
    };
    assert_eq!(refusal(small_plan.session(&options(None)).map(drop)), small);
    assert_eq!(
        refusal(small_plan.streaming_run(&options(None), shots).map(drop)),
        small
    );
    small_plan
        .session(&options(Some(MemoryBudget::DEFAULT)))
        .expect("a session budget above the plan's overrides it");
    small_plan
        .streaming_run(&options(Some(MemoryBudget::DEFAULT)), shots)
        .expect("a streaming budget above the plan's overrides it");
    assert_eq!(
        refusal(default_plan.session(&options(Some(one_byte))).map(drop)),
        one_byte
    );
    assert_eq!(
        refusal(
            default_plan
                .streaming_run(&options(Some(one_byte)), shots)
                .map(drop)
        ),
        one_byte
    );

    let detection = |memory_budget| {
        DetectionSamplingCompiler {
            memory_budget,
            ..DetectionSamplingCompiler::default()
        }
        .compile(&fixture)
        .expect("compile detection sampling")
    };
    // Detection plans keep a byte per record bit, so they need a larger plan budget.
    let small = MemoryBudget::from_bytes(512 << 10);
    let (small_plan, default_plan) = (detection(small), detection(MemoryBudget::DEFAULT));
    let options = |memory_budget| DetectionSamplingSessionOptions {
        memory_budget,
        ..DetectionSamplingSessionOptions::default()
    };
    let refusal = |result: Result<_, DetectionExecutionError>| {
        refused_budget(
            result
                .err()
                .as_ref()
                .and_then(|error| error.memory_budget_exceeded()),
        )
    };
    assert_eq!(refusal(small_plan.session(&options(None)).map(drop)), small);
    assert_eq!(
        refusal(small_plan.streaming_run(&options(None), shots).map(drop)),
        small
    );
    small_plan
        .session(&options(Some(MemoryBudget::DEFAULT)))
        .expect("a session budget above the plan's overrides it");
    small_plan
        .streaming_run(&options(Some(MemoryBudget::DEFAULT)), shots)
        .expect("a streaming budget above the plan's overrides it");
    assert_eq!(
        refusal(default_plan.session(&options(Some(one_byte))).map(drop)),
        one_byte
    );
    assert_eq!(
        refusal(
            default_plan
                .streaming_run(&options(Some(one_byte)), shots)
                .map(drop)
        ),
        one_byte
    );

    let conversion = |memory_budget| {
        MeasurementToDetectionCompiler {
            reference_sample_mode: ReferenceSampleMode::SkipReferenceSample,
            memory_budget,
            ..MeasurementToDetectionCompiler::default()
        }
        .compile(&fixture)
        .expect("compile conversion")
    };
    let (small_plan, default_plan) = (conversion(small), conversion(MemoryBudget::DEFAULT));
    let options = |memory_budget| MeasurementToDetectionSessionOptions { memory_budget };
    assert_eq!(refusal(small_plan.session(&options(None)).map(drop)), small);
    small_plan
        .session(&options(Some(MemoryBudget::DEFAULT)))
        .expect("a session budget above the plan's overrides it");
    assert_eq!(
        refusal(default_plan.session(&options(Some(one_byte))).map(drop)),
        one_byte
    );
}
