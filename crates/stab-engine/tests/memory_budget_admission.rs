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
    DetectionCompileError, DetectionConversionLimits, DetectionError, DetectionResourceKind,
    DetectionSamplingCompiler, RandomPolicy, SamplingCompileError, SamplingCompiler,
    SamplingExecutionError, SamplingPlan, Seed, ShotCount,
};
use stab_model::{Circuit, MemoryBudget};
use stab_records::{MeasurementBatchView, MeasurementSink};

fn circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("parse admission fixture")
}

fn sampling(bytes: u64) -> SamplingCompiler {
    SamplingCompiler::new().memory_budget(MemoryBudget::from_bytes(bytes))
}

/// Smallest budget that compiles a plan and opens a session, found by bisection.
fn minimal_session_budget(circuit: &Circuit) -> u64 {
    let fits = |bytes| {
        sampling(bytes)
            .compile(circuit)
            .ok()
            .and_then(|plan| plan.session(RandomPolicy::Seeded(Seed::new(1))).ok())
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
    let plan = SamplingCompiler::new()
        .compile(&long)
        .expect("sample admits expanded work without a cap");
    assert_eq!(plan.measurement_width().get(), 1);
    DetectionSamplingCompiler::new()
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
        .session(RandomPolicy::Seeded(Seed::new(5)))
        .expect("the exact budget opens a session");
    let mut sink = CountingSink::default();
    session
        .run(ShotCount::new(3), &mut sink)
        .expect("the admitted session runs");
    assert_eq!(sink.batches, 1);

    // One byte less is refused by a typed budget error at compilation or session creation, so
    // no RNG is seeded and no sink is ever called.
    let refused = match sampling(exact - 1).compile(&fixture) {
        Err(SamplingCompileError::CompiledBytesLimit { required, budget }) => {
            assert_eq!(budget, exact - 1);
            required
        }
        Err(other) => panic!("unexpected compile error {other:?}"),
        Ok(plan) => match plan.session(RandomPolicy::Seeded(Seed::new(5))) {
            Err(SamplingExecutionError::SessionStorageLimit {
                estimated_bytes,
                limit_bytes,
            }) => {
                assert_eq!(limit_bytes, exact - 1);
                u64::try_from(estimated_bytes).expect("small estimate")
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
    let plan: SamplingPlan = SamplingCompiler::new()
        .compile(&wide)
        .expect("the compact program itself fits the budget");
    let measured = allocation_counter::measure(|| {
        let error = plan
            .session(RandomPolicy::Seeded(Seed::new(1)))
            .expect_err("the session record exceeds the budget");
        assert_matches!(error, SamplingExecutionError::SessionStorageLimit { .. });
        assert!(error.to_string().contains("memory budget"), "{error}");
    });
    assert!(measured.bytes_max < 1 << 20, "{measured:?}");

    match DetectionSamplingCompiler::new().compile(&circuit(
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
    match DetectionSamplingCompiler::new()
        .limits(
            DetectionConversionLimits::default().with_memory_budget(MemoryBudget::from_bytes(1)),
        )
        .compile(&fixture)
    {
        Err(DetectionCompileError::InvalidCircuit(DetectionError::ResourceLimit(resource))) => {
            assert_eq!(resource.kind(), DetectionResourceKind::CompiledBytes);
            assert_eq!(resource.limit(), 1);
        }
        other => panic!("expected a typed budget refusal, got {other:?}"),
    }
}
