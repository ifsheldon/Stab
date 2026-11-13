//! Session-storage admission parity with the detection sessions.
//!
//! The DEM admission estimate covers the reusable detection record, sample planes, and packed
//! output rows for the selected capacities, including the sampled-error storage that a session
//! allocates lazily on its first sampled-error request. Its scratch term also counts the inline
//! `DetectionRecordBuffer` header, which lives in the session value rather than on the heap, so
//! the estimate is exactly that header larger than the retained heap bytes.

#![allow(
    clippy::expect_used,
    reason = "storage admission tests use fixed valid fixtures and fail immediately on errors"
)]

use super::batch::{SessionCapacities, session_storage_bytes};
use super::*;
use crate::{DemSamplingCompiler, Seed};
use stab_model::DetectorErrorModel;

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

fn raised_cap_limits() -> DemSamplerLimits {
    DemSamplerLimits::default().with_max_active_batch_bytes(usize::MAX)
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
fn session_capacity_admission_is_exact_at_the_256_mib_cap() {
    // With the configurable active-batch limit raised, only the 256 MiB session cap binds. At an
    // eight-shot output capacity, 26,843,536 detectors estimate exactly 256 MiB:
    // 48 + 9 * D + 8 * 8 * ceil(D / 64).
    let at_limit = plan(26_843_536, 0, 0);
    let capacities = initial_capacities(&at_limit, raised_cap_limits()).expect("admit cap");
    assert_eq!(
        capacities,
        SessionCapacities {
            output: 8,
            sample: 8
        }
    );
    assert_eq!(
        session_storage_bytes(&at_limit, 8, 8, true),
        u128::from(MAX_DEM_SESSION_STORAGE_BYTES)
    );
    let one_more = plan(26_843_537, 0, 0);
    assert_eq!(
        initial_capacities(&one_more, raised_cap_limits()).expect("admit smaller batch"),
        SessionCapacities {
            output: 7,
            sample: 7
        },
        "the first excess byte must shrink the batch instead of exceeding the cap"
    );
}

#[test]
fn session_storage_rejects_the_first_one_shot_excess_before_allocation() {
    // One-shot storage is 48 + 9 * D + 8 * ceil(D / 64) bytes, which steps by 9 bytes and cannot
    // land exactly on 256 MiB: 29,417,578 detectors estimate 6 bytes under it and one more
    // detector estimates 3 bytes over it.
    let last_admitted = plan(29_417_578, 0, 0);
    assert_eq!(
        initial_capacities(&last_admitted, raised_cap_limits()).expect("admit one-shot batch"),
        SessionCapacities {
            output: 1,
            sample: 1
        }
    );
    let first_rejected = plan(29_417_579, 0, 0);
    let mut rejected = None;
    let allocated = allocation_counter::measure(|| {
        rejected = Some(
            first_rejected
                .session_with_limits(RandomPolicy::Seeded(Seed::new(1)), raised_cap_limits())
                .expect_err("reject the first excess detector before allocation"),
        );
    });
    assert_eq!(allocated.bytes_total, 0);
    assert!(
        matches!(
            rejected,
            Some(DemSamplingExecutionError::SessionStorageLimit {
                estimated_bytes,
                limit_bytes: MAX_DEM_SESSION_STORAGE_BYTES,
            }) if estimated_bytes == u128::from(MAX_DEM_SESSION_STORAGE_BYTES) + 3
        ),
        "{rejected:?}"
    );
}
