//! Session-storage admission parity with the detection sessions.
//!
//! The DEM admission estimate covers the reusable detection record, sample planes, and packed
//! output rows for the selected capacities, including the sampled-error storage that a session
//! allocates lazily on its first sampled-error request. Its scratch term also counts the inline
//! `DetectionRecordBuffer` header, which lives in the session value rather than on the heap, so
//! the estimate is exactly that header larger than the retained heap bytes.

#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "storage admission tests use fixed valid fixtures and fail immediately on errors"
)]

use super::batch::{SessionCapacities, session_storage_bytes};
use super::*;
use crate::{DemResourceKind, DemSamplingCompiler, Seed};
use stab_model::{DetectorErrorModel, MemoryBudget};

const INLINE_RECORD_BYTES: u128 = size_of::<DetectionRecordBuffer>() as u128;

fn plan(detectors: usize, observables: usize, errors: usize) -> DemSamplingPlan {
    let mut text = String::new();
    for error in 0..errors {
        text.push_str(&format!("error(0.125) D{}", error % detectors));
        if observables > 0 {
            text.push_str(&format!(" L{}", error % observables));
        }
        text.push('\n');
    }
    text.push_str(&format!("detector D{}\n", detectors - 1));
    if observables > 0 {
        text.push_str(&format!("logical_observable L{}\n", observables - 1));
    }
    DemSamplingCompiler::new()
        .compile(&DetectorErrorModel::from_dem_str(&text).expect("parse storage DEM"))
        .expect("compile storage DEM")
}

fn budget(bytes: u128) -> DemSamplerLimits {
    DemSamplerLimits::default().with_memory_budget(MemoryBudget::from_bytes(
        u64::try_from(bytes).expect("test budget fits u64"),
    ))
}

#[test]
fn session_storage_estimate_matches_retained_allocation_including_lazy_sampled_errors() {
    for (detectors, observables, errors) in [
        (1, 0, 1),
        (1, 1, 1),
        (63, 64, 65),
        (64, 63, 2),
        (65, 65, 130),
        (1000, 3, 7),
    ] {
        let plan = plan(detectors, observables, errors);
        let label = format!("D={detectors} L={observables} E={errors}");
        let mut session = None;
        let constructed = allocation_counter::measure(|| {
            session = Some(
                DemSamplingSession::new(
                    plan.clone(),
                    RandomPolicy::Seeded(Seed::new(1)),
                    DemSamplerLimits::default(),
                )
                .expect("allocate DEM session"),
            );
        });
        let mut session = session.expect("constructed DEM session");
        let output = session.batch.capacity();
        let sample = session.sample_capacity;
        assert_eq!(
            session_storage_bytes(&plan, output, sample, false),
            u128::try_from(constructed.bytes_current).expect("retained bytes")
                + INLINE_RECORD_BYTES,
            "construction: {label}"
        );

        let sampled_errors = allocation_counter::measure(|| {
            session
                .ensure_sampled_error_storage()
                .expect("allocate sampled-error storage");
        });
        assert_eq!(
            session_storage_bytes(&plan, output, sample, true),
            u128::try_from(constructed.bytes_current + sampled_errors.bytes_current)
                .expect("retained bytes")
                + INLINE_RECORD_BYTES,
            "with sampled errors: {label}"
        );
    }
}

#[test]
fn batch_shape_follows_the_preferred_64_mib_target_whatever_the_budget() {
    // Batch shape, and therefore the seeded stream, is chosen against the historical 64 MiB
    // active-batch target even when the budget is larger. At an eight-shot output capacity,
    // 48 + 9 * D + 8 * 8 * ceil(D / 64) bytes first exceed 64 MiB at 6,710,879 detectors.
    let preferred = 64 * 1024 * 1024_u128;
    let fits = plan(6_710_878, 0, 0);
    assert_eq!(session_storage_bytes(&fits, 8, 8, true), preferred - 2);
    for limits in [DemSamplerLimits::default(), budget(u128::from(u64::MAX))] {
        assert_eq!(
            initial_capacities(&fits, limits).expect("admit an eight-shot batch"),
            SessionCapacities {
                output: 8,
                sample: 8
            }
        );
        assert_eq!(
            initial_capacities(&plan(6_710_879, 0, 0), limits).expect("admit a smaller batch"),
            SessionCapacities {
                output: 7,
                sample: 7
            },
            "one more detector shrinks the batch instead of growing past the target"
        );
    }
}

#[test]
fn session_storage_admits_the_exact_budget_and_rejects_the_first_excess_byte_before_allocation() {
    let model = plan(1000, 3, 7);
    let exact = session_storage_bytes(&model, 1, 1, false);
    assert_eq!(
        initial_capacities(&model, budget(exact)).expect("admit the exact one-shot budget"),
        SessionCapacities {
            output: 1,
            sample: 1
        }
    );
    model
        .session_with_limits(RandomPolicy::Seeded(Seed::new(1)), budget(exact))
        .expect("open a session at the exact budget");

    let mut rejected = None;
    let allocated = allocation_counter::measure(|| {
        rejected = Some(
            model
                .session_with_limits(RandomPolicy::Seeded(Seed::new(1)), budget(exact - 1))
                .expect_err("reject the first excess byte before allocation"),
        );
    });
    assert_eq!(allocated.bytes_total, 0);
    match rejected {
        Some(DemSamplingExecutionError::InvalidRequest(DemError::ResourceLimit(resource))) => {
            assert_eq!(resource.kind(), DemResourceKind::ActiveBatchBytes);
            assert_eq!(u128::from(resource.actual()), exact);
            assert_eq!(u128::from(resource.limit()), exact - 1);
        }
        other => panic!("expected a typed budget refusal, got {other:?}"),
    }

    // A one-shot session above the preferred batch target is admitted within the budget.
    let above_target = plan(8_000_000, 0, 0);
    assert_eq!(
        initial_capacities(&above_target, DemSamplerLimits::default())
            .expect("a one-shot batch above 64 MiB fits the default budget"),
        SessionCapacities {
            output: 1,
            sample: 1
        }
    );
}
