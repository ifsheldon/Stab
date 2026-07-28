#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "limit tests use direct fixture assertions for compact diagnostics"
)]

use stab_core::{
    CircuitError, CompiledDemSampler, DemSamplerLimits, DetectionEventRecord, DetectorErrorModel,
    ResourceKind, ResourceOperation,
};

fn compile_dem(text: &str) -> CompiledDemSampler {
    let model = DetectorErrorModel::from_dem_str(text).expect("parse DEM");
    CompiledDemSampler::compile(&model).expect("compile DEM sampler")
}

macro_rules! sampler_limits {
    ($max_sampled_error_applications:expr, $max_materialized_units:expr, $max_materialized_bytes:expr $(,)?) => {
        DemSamplerLimits::default()
            .with_max_sampled_error_applications($max_sampled_error_applications)
            .with_max_materialized_units($max_materialized_units)
            .with_max_materialized_bytes($max_materialized_bytes)
    };
}

fn assert_sampler_error<T>(result: Result<T, CircuitError>, expected_message: &str) {
    let error = match result {
        Ok(_) => panic!("expected DEM sampler admission error"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains(expected_message),
        "expected message fragment {expected_message:?}, got {error}"
    );
}

#[test]
fn custom_limits_accept_exact_sampled_work_maximum_and_reject_first_excess() {
    let sampler = compile_dem("error(0.5) D0\n");
    let limits = sampler_limits!(3, 1024, 1024 * 1024);

    let mut accepted_visits = 0;
    sampler
        .try_for_each_detection_event_with_seed_and_limits(3, Some(123), limits, |_record| {
            accepted_visits += 1;
            Ok::<(), CircuitError>(())
        })
        .expect("exact sampled-error work maximum is accepted");
    assert_eq!(accepted_visits, 3);

    let mut rejected_visits = 0;
    assert_sampler_error(
        sampler.try_for_each_detection_event_with_seed_and_limits(
            4,
            Some(123),
            limits,
            |_record| {
                rejected_visits += 1;
                Ok::<(), CircuitError>(())
            },
        ),
        "would apply 4 sampled errors; current limit is 3",
    );
    assert_eq!(
        rejected_visits, 0,
        "sample-work admission must fail before visitor-observable sampling"
    );
}

#[test]
fn custom_limits_accept_exact_materialized_unit_maximum_and_reject_first_excess() {
    let sampler = compile_dem("");
    let limits = sampler_limits!(1024, 3, 1024 * 1024);

    let output = sampler
        .sample_detection_events_with_seed_and_limits(3, Some(5), limits)
        .expect("exact materialized-unit maximum is accepted");
    assert_eq!(output.records.len(), 3);

    assert_sampler_error(
        sampler.sample_detection_events_with_seed_and_limits(4, Some(5), limits),
        "would require 4 buffered units; current limit is 3",
    );

    let error = sampler
        .sample_detection_events_with_seed_and_limits(4, Some(5), limits)
        .expect_err("the first excess unit should expose typed context");
    let resource = error
        .resource_limit_error()
        .expect("DEM sampling limit should be structured");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelSampling
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);
}

#[test]
fn custom_limits_accept_exact_materialized_byte_maximum_and_reject_first_excess() {
    let sampler = compile_dem("");
    let bytes_per_empty_record = std::mem::size_of::<DetectionEventRecord>().max(1);
    let limits = sampler_limits!(1024, 1024, bytes_per_empty_record * 3);

    let output = sampler
        .sample_detection_events_with_seed_and_limits(3, Some(5), limits)
        .expect("exact materialized-byte maximum is accepted");
    assert_eq!(output.records.len(), 3);

    assert_sampler_error(
        sampler.sample_detection_events_with_seed_and_limits(4, Some(5), limits),
        &format!(
            "would require at least {} materialized bytes; current limit is {}",
            bytes_per_empty_record * 4,
            bytes_per_empty_record * 3
        ),
    );
}

#[test]
fn default_limits_match_existing_dem_sampler_admission_contract() {
    let stochastic = compile_dem("error(0.5) D0\n");
    let mut visits = 0;
    assert_sampler_error(
        stochastic.try_for_each_detection_event_with_seed_and_limits(
            64_000_001,
            Some(5),
            Default::default(),
            |_record| {
                visits += 1;
                Ok::<(), CircuitError>(())
            },
        ),
        "would apply 64000001 sampled errors; current limit is 64000000",
    );
    assert_eq!(visits, 0);

    let empty = compile_dem("");
    assert_sampler_error(
        empty.validate_sample_buffer_units_with_limits(64_000_001, false, Default::default()),
        "would require 64000001 buffered units; current limit is 64000000",
    );

    let bytes_per_empty_record = std::mem::size_of::<DetectionEventRecord>().max(1);
    let first_default_byte_rejection = (64 * 1024 * 1024 / bytes_per_empty_record) + 1;
    assert_sampler_error(
        empty.validate_sample_buffer_units_with_limits(
            first_default_byte_rejection,
            false,
            Default::default(),
        ),
        "materialized bytes; current limit is 67108864",
    );
}

#[test]
fn default_limits_preserve_materialized_replay_and_sample_outputs() {
    let sampler = compile_dem("error(0.25) D0 L0\nerror(1) D1\n");

    let default_output = sampler
        .sample_detection_events_with_seed(8, Some(7))
        .expect("default sample");
    let limited_output = sampler
        .sample_detection_events_with_seed_and_limits(8, Some(7), Default::default())
        .expect("default-limit sample");
    assert_eq!(limited_output, default_output);

    let (default_output, default_errors) = sampler
        .sample_detection_events_and_errors_with_seed(8, Some(7))
        .expect("default sample with errors");
    let (limited_output, limited_errors) = sampler
        .sample_detection_events_and_errors_with_seed_and_limits(8, Some(7), Default::default())
        .expect("default-limit sample with errors");
    assert_eq!(limited_output, default_output);
    assert_eq!(limited_errors, default_errors);

    let replayed = sampler
        .sample_detection_events_from_error_records(&default_errors)
        .expect("default replay");
    let limited_replayed = sampler
        .sample_detection_events_from_error_records_with_limits(&default_errors, Default::default())
        .expect("default-limit replay");
    assert_eq!(limited_replayed, replayed);
}

#[test]
fn replay_charges_only_newly_materialized_detection_output() {
    let sampler = compile_dem("error(1) D0\n");
    let replay_records = vec![vec![true], vec![false]];
    let limits = sampler_limits!(1024, 2, 1024 * 1024);

    let replayed = sampler
        .sample_detection_events_from_error_records_with_limits(&replay_records, limits)
        .expect("two returned detector records reach the exact two-unit maximum");
    assert_eq!(replayed.records.len(), 2);
    let first_detector = replayed
        .records
        .first()
        .and_then(|record| record.detectors.first())
        .copied();
    let second_detector = replayed
        .records
        .get(1)
        .and_then(|record| record.detectors.first())
        .copied();
    assert_eq!(first_detector, Some(true));
    assert_eq!(second_detector, Some(false));

    let caller_owned_wide_records = vec![vec![false; 4096], vec![false; 4096]];
    let wide_sampler = compile_dem(
        &(0..4096)
            .map(|detector| format!("error(0) D{detector}\n"))
            .collect::<String>(),
    );
    wide_sampler
        .sample_detection_events_from_error_records_with_limits(
            &caller_owned_wide_records,
            sampler_limits!(1024, 8192, 1024 * 1024),
        )
        .expect("caller-owned replay storage is not charged as returned output");

    let excessive_replay_records = vec![vec![false], vec![false], vec![false]];
    assert_sampler_error(
        sampler.sample_detection_events_from_error_records_with_limits(
            &excessive_replay_records,
            limits,
        ),
        "would require 3 buffered units; current limit is 2",
    );
}

#[test]
fn replay_work_is_bounded_separately_from_returned_output() {
    let sampler = compile_dem("error(1) D0\n");
    let replay_records = vec![vec![true], vec![false]];
    let accepted = DemSamplerLimits::default()
        .with_max_sampled_error_applications(0)
        .with_max_replay_work_units(4)
        .with_max_materialized_units(2)
        .with_max_materialized_bytes(1024 * 1024);

    let output = sampler
        .sample_detection_events_from_error_records_with_limits(&replay_records, accepted)
        .expect("two replay records should reach both independent exact maxima");
    assert_eq!(output.records.len(), 2);
    assert_eq!(accepted.max_replay_work_units(), 4);
    sampler
        .validate_replay_work_units_with_limits(replay_records.len(), accepted)
        .expect("the exact replay-work preflight maximum should be accepted");

    let mut streamed_records = 0;
    sampler
        .try_for_each_detection_event_from_error_records_with_limits(
            replay_records.iter().map(Vec::as_slice),
            accepted,
            |_, _| {
                streamed_records += 1;
                Ok::<(), CircuitError>(())
            },
        )
        .expect("streaming replay should accept the exact traversal-work maximum");
    assert_eq!(streamed_records, 2);

    let error = sampler
        .sample_detection_events_from_error_records_with_limits(
            &replay_records,
            accepted.with_max_replay_work_units(3),
        )
        .expect_err("the first excess replay work unit must be rejected");
    let resource = error
        .resource_limit_error()
        .expect("replay-work rejection should expose typed context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelSampling
    );
    assert_eq!(resource.resource(), ResourceKind::ReplayWorkUnits);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);

    let mut visits_before_rejection = 0;
    let error = sampler
        .try_for_each_detection_event_from_error_records_with_limits(
            replay_records.iter().map(Vec::as_slice),
            accepted.with_max_replay_work_units(3),
            |_, _| {
                visits_before_rejection += 1;
                Ok::<(), CircuitError>(())
            },
        )
        .expect_err("streaming replay must reject before forwarding the first excess record");
    let resource = error
        .resource_limit_error()
        .expect("streaming replay should expose typed work context");
    assert_eq!(resource.resource(), ResourceKind::ReplayWorkUnits);
    assert_eq!(resource.actual(), 4);
    assert_eq!(resource.limit(), 3);
    assert_eq!(visits_before_rejection, 1);
}

#[test]
fn policy_admission_reports_arithmetic_overflow_before_sampling_or_allocation() {
    let generous_limits = sampler_limits!(usize::MAX, usize::MAX, usize::MAX);

    let two_output_units_per_shot = compile_dem("detector D1\n");
    assert_sampler_error(
        two_output_units_per_shot.sample_detection_events_with_seed_and_limits(
            usize::MAX,
            Some(1),
            generous_limits,
        ),
        "DEM sampler buffer size overflowed",
    );

    let empty = compile_dem("");
    assert_sampler_error(
        empty.sample_detection_events_with_seed_and_limits(usize::MAX, Some(1), generous_limits),
        "DEM sampler buffer byte size overflowed",
    );

    let two_errors_per_shot = compile_dem("error(0.5) D0\nerror(0.5) D0\n");
    let mut visits = 0;
    assert_sampler_error(
        two_errors_per_shot.try_for_each_detection_event_with_seed_and_limits(
            usize::MAX,
            Some(1),
            generous_limits,
            |_record| {
                visits += 1;
                Ok::<(), CircuitError>(())
            },
        ),
        "DEM sampler sample work overflowed",
    );
    assert_eq!(visits, 0);
}

#[test]
fn caller_raised_limits_cannot_bypass_platform_vector_capacity() {
    let sampler = compile_dem("");
    let first_unrepresentable_record_count =
        (isize::MAX as usize / std::mem::size_of::<DetectionEventRecord>()) + 1;
    let generous_limits = sampler_limits!(usize::MAX, usize::MAX, usize::MAX);

    assert_sampler_error(
        sampler.sample_detection_events_with_seed_and_limits(
            first_unrepresentable_record_count,
            Some(1),
            generous_limits,
        ),
        "DEM detection record container exceeds the platform vector capacity",
    );
}

#[test]
fn public_validation_and_error_materialization_obey_custom_dem_sampler_limits() {
    let sampler = compile_dem("error(1) D0\n");
    let exact = sampler_limits!(1024, 4, 1024 * 1024);

    sampler
        .validate_sample_buffer_units_with_limits(2, true, exact)
        .expect("two detector and error records reach the exact four-unit maximum");
    let (output, errors) = sampler
        .sample_detection_events_and_errors_with_seed_and_limits(2, Some(5), exact)
        .expect("materialized detector and error records obey the exact custom maximum");
    assert_eq!(output.records.len(), 2);
    assert_eq!(errors.len(), 2);

    let rejected = exact.with_max_materialized_units(3);
    assert_sampler_error(
        sampler.validate_sample_buffer_units_with_limits(2, true, rejected),
        "would require 4 buffered units; current limit is 3",
    );
    assert_sampler_error(
        sampler.sample_detection_events_and_errors_with_seed_and_limits(2, Some(5), rejected),
        "would require 4 buffered units; current limit is 3",
    );

    let streaming_rejected = exact.with_max_materialized_units(1);
    let mut visits = 0;

    assert_sampler_error(
        sampler.try_for_each_detection_event_and_error_with_seed_and_limits(
            1,
            Some(5),
            streaming_rejected,
            |_record, _error_record| {
                visits += 1;
                Ok::<(), CircuitError>(())
            },
        ),
        "would require 2 buffered units; current limit is 1",
    );
    assert_eq!(visits, 0);
}

#[test]
fn detection_streaming_charges_one_scratch_record_not_total_shots() {
    let sampler = compile_dem("error(1) D0 L0\n");
    let visitor_scratch_bytes = std::mem::size_of::<DetectionEventRecord>() + 2;
    let minimum_session_bytes =
        std::mem::size_of::<DetectionEventRecord>() + 2 + 2 * std::mem::size_of::<u64>();
    let scratch_bytes = visitor_scratch_bytes + minimum_session_bytes;
    let rejected_limits = sampler_limits!(1024, 1, scratch_bytes);
    let mut rejected_visits = 0;

    let error = sampler
        .try_for_each_detection_event_with_seed_and_limits(3, Some(5), rejected_limits, |_record| {
            rejected_visits += 1;
            Ok::<(), CircuitError>(())
        })
        .expect_err("two-unit streaming scratch must not bypass a one-unit policy");
    assert_eq!(rejected_visits, 0);
    let resource = error
        .resource_limit_error()
        .expect("streaming scratch rejection should expose typed context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelSampling
    );
    assert_eq!(resource.resource(), ResourceKind::MaterializedUnits);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);

    let accepted_limits = sampler_limits!(1024, 2, scratch_bytes);
    let mut accepted_visits = 0;
    sampler
        .try_for_each_detection_event_with_seed_and_limits(3, Some(5), accepted_limits, |_record| {
            accepted_visits += 1;
            Ok::<(), CircuitError>(())
        })
        .expect("streaming should charge one reusable record instead of all shots");
    assert_eq!(accepted_visits, 3);
}

#[test]
fn sampled_error_streaming_charges_both_compatibility_records() {
    let sampler = compile_dem("error(1) D0 L0\n");
    let compatibility_bytes =
        std::mem::size_of::<DetectionEventRecord>() + 2 + std::mem::size_of::<Vec<bool>>() + 1;
    let minimum_session_bytes = compatibility_bytes + 3 * std::mem::size_of::<u64>();
    let exact_bytes = compatibility_bytes + minimum_session_bytes;
    let exact_limits = sampler_limits!(1, 3, exact_bytes);
    let mut accepted_visits = 0;

    sampler
        .try_for_each_detection_event_and_error_with_seed_and_limits(
            1,
            Some(5),
            exact_limits,
            |record, errors| {
                accepted_visits += 1;
                assert_eq!(record.detectors, [true]);
                assert_eq!(record.observables, [true]);
                assert_eq!(errors, [true]);
                Ok::<(), CircuitError>(())
            },
        )
        .expect("the exact compatibility and one-shot session byte boundary is admitted");
    assert_eq!(accepted_visits, 1);

    let rejected_limits = exact_limits.with_max_materialized_bytes(exact_bytes - 1);
    let mut rejected_visits = 0;
    let error = sampler
        .try_for_each_detection_event_and_error_with_seed_and_limits(
            1,
            Some(5),
            rejected_limits,
            |_record, _errors| {
                rejected_visits += 1;
                Ok::<(), CircuitError>(())
            },
        )
        .expect_err("the first byte above the combined compatibility envelope must fail");
    assert_eq!(rejected_visits, 0);
    let resource = error
        .resource_limit_error()
        .expect("sampled-error scratch rejection should expose typed context");
    assert_eq!(resource.resource(), ResourceKind::MaterializedBytes);
    assert_eq!(
        resource.limit(),
        u64::try_from(minimum_session_bytes - 1).expect("test byte limit fits u64")
    );
    assert!(
        resource.actual() > resource.limit(),
        "sampled-error session storage must remain above the post-sink byte budget"
    );
}

#[test]
fn detection_streaming_preserves_sampled_work_precedence_before_scratch_admission() {
    let sampler = compile_dem("error(1) D0 L0\n");
    let limits = sampler_limits!(0, 0, usize::MAX);
    let mut visits = 0;

    let error = sampler
        .try_for_each_detection_event_with_seed_and_limits(1, Some(5), limits, |_record| {
            visits += 1;
            Ok::<(), CircuitError>(())
        })
        .expect_err("historical sampled-work rejection must precede reusable scratch admission");

    let resource = error
        .resource_limit_error()
        .expect("sampled-work rejection should expose typed context");
    assert_eq!(
        resource.operation(),
        ResourceOperation::DetectorErrorModelSampling
    );
    assert_eq!(resource.resource(), ResourceKind::SampledErrorApplications);
    assert_eq!(resource.actual(), 1);
    assert_eq!(resource.limit(), 0);
    assert_eq!(visits, 0);
}

#[test]
fn zero_shot_streaming_needs_no_scratch_budget() {
    let sampler = compile_dem("error(1) D0 L0\n");
    let limits = sampler_limits!(0, 0, 0);
    let mut detection_visits = 0;
    let mut error_visits = 0;

    sampler
        .try_for_each_detection_event_with_seed_and_limits(0, None, limits, |_record| {
            detection_visits += 1;
            Ok::<(), CircuitError>(())
        })
        .expect("zero detection shots should not allocate reusable scratch");
    sampler
        .try_for_each_detection_event_and_error_with_seed_and_limits(
            0,
            None,
            limits,
            |_record, _errors| {
                error_visits += 1;
                Ok::<(), CircuitError>(())
            },
        )
        .expect("zero error-record shots should not allocate reusable scratch");

    assert_eq!(detection_visits, 0);
    assert_eq!(error_visits, 0);
}
