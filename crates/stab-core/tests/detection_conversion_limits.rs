#![allow(
    clippy::expect_used,
    reason = "integration tests use deterministic valid circuits and explicit rejection assertions"
)]

use std::cell::Cell;
use std::hint::black_box;
use std::mem::ManuallyDrop;

use stab_core::advanced::compat::{
    CompiledDetectionConverter, convert_measurements_to_detection_events_with_limits,
    convert_measurements_to_detection_events_with_sweep_and_limits,
    sample_detection_events_with_limits, try_for_each_sampled_detection_event_with_limits,
};
use stab_core::{
    Circuit, CircuitError, CircuitInstruction, DetectionConversionLimits,
    DetectionConversionOptions, Gate, QubitId, RepeatBlock, RepeatCount, RepeatNestingLimit,
    ResourceKind, ResourceOperation, Target, detection_record_width_with_limits,
    measurement_record_count_with_limits, validate_detection_sampling_circuit_with_limits,
};

fn skip_reference_sample() -> DetectionConversionOptions {
    DetectionConversionOptions {
        skip_reference_sample: true,
    }
}

fn parse_circuit(text: &str) -> Circuit {
    Circuit::from_stim_str(text).expect("test circuit should parse")
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

fn assert_repeat_nesting_error(error: CircuitError) {
    let resource = error
        .resource_limit_error()
        .expect("repeat nesting rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::RepeatNesting);
    assert_eq!(resource.actual(), (RepeatNestingLimit::HARD_MAX + 1) as u64);
    assert_eq!(resource.limit(), RepeatNestingLimit::HARD_MAX as u64);
}

fn assert_materialized_bits_error(error: CircuitError, actual: u64, limit: u64) {
    let resource = error
        .resource_limit_error()
        .expect("materialization rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::MaterializedBits);
    assert_eq!(resource.actual(), actual);
    assert_eq!(resource.limit(), limit);
}

#[test]
fn record_width_is_admitted_at_the_limit_and_rejected_above_it() {
    let circuit = parse_circuit("M 0 1\n");
    let accepted = DetectionConversionLimits::default().with_max_record_bits(2);
    let rejected = accepted.with_max_record_bits(1);

    let converter = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        accepted,
    )
    .expect("two measurement bits should fit");
    assert_eq!(converter.measurement_count(), 2);

    let error = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        rejected,
    )
    .expect_err("the first bit above the limit should fail");
    assert!(error.to_string().contains("record width 2"));
    assert!(error.to_string().contains("limit 1"));
    let resource = error
        .resource_limit_error()
        .expect("record rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::RecordBits);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
}

#[test]
fn detection_repeat_nesting_accepts_the_fixed_boundary_and_rejects_the_next() {
    let accepted = nested_circuit(RepeatNestingLimit::HARD_MAX, Circuit::new());
    CompiledDetectionConverter::compile(&accepted, skip_reference_sample())
        .expect("the exact fixed detection nesting boundary should compile");

    let rejected = nested_circuit(RepeatNestingLimit::HARD_MAX + 1, Circuit::new());
    assert_repeat_nesting_error(
        CompiledDetectionConverter::compile(&rejected, skip_reference_sample())
            .expect_err("the first repeat above the fixed boundary must reject"),
    );
    assert_repeat_nesting_error(
        validate_detection_sampling_circuit_with_limits(
            &rejected,
            DetectionConversionLimits::default(),
        )
        .expect_err("detection sampling validation must apply the same fixed boundary"),
    );
}

#[test]
fn deeply_nested_programmatic_detection_circuits_reject_before_recursion() {
    for detector_frame in [false, true] {
        let handle = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(move || {
                let body = if detector_frame {
                    parse_circuit("HERALDED_ERASE(0.125) 0\n")
                } else {
                    Circuit::new()
                };
                let circuit = ManuallyDrop::new(nested_circuit(10_000, body));
                let error = if detector_frame {
                    validate_detection_sampling_circuit_with_limits(
                        &circuit,
                        DetectionConversionLimits::default(),
                    )
                    .expect_err("deep detector-frame validation must reject")
                } else {
                    CompiledDetectionConverter::compile(&circuit, skip_reference_sample())
                        .expect_err("deep direct conversion must reject")
                };
                let resource = error
                    .resource_limit_error()
                    .expect("deep rejection should remain typed");
                (
                    resource.operation(),
                    resource.resource(),
                    resource.actual(),
                    resource.limit(),
                )
            })
            .expect("spawn constrained-stack detection regression");
        let (operation, resource, actual, limit) = handle
            .join()
            .expect("deep detection admission should not overflow the stack");
        assert_eq!(operation, ResourceOperation::DetectionConversion);
        assert_eq!(resource, ResourceKind::RepeatNesting);
        assert_eq!(actual, (RepeatNestingLimit::HARD_MAX + 1) as u64);
        assert_eq!(limit, RepeatNestingLimit::HARD_MAX as u64);
    }
}

#[test]
fn default_record_width_is_executed_exactly() {
    let measurement_circuit = |width: usize| {
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
    };

    CompiledDetectionConverter::compile_with_limits(
        &measurement_circuit(1_000_000),
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect("the exact default measurement width should be accepted");

    let error = CompiledDetectionConverter::compile_with_limits(
        &measurement_circuit(1_000_001),
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect_err("the first measurement bit above the default must reject");
    let resource = error
        .resource_limit_error()
        .expect("record-width rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::RecordBits);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);
}

#[test]
fn zero_width_materialized_sampling_charges_record_ownership() {
    let circuit = parse_circuit("");
    let exact = DetectionConversionLimits::default().with_max_materialized_bits(3);

    let accepted = sample_detection_events_with_limits(&circuit, 3, Some(7), exact)
        .expect("the exact zero-width record ownership maximum should be accepted");
    assert_eq!(accepted.detector_count, 0);
    assert_eq!(accepted.observable_count, 0);
    assert_eq!(accepted.records.len(), 3);
    assert!(
        accepted
            .records
            .iter()
            .all(|record| record.detectors.is_empty() && record.observables.is_empty())
    );

    let first_rejected = sample_detection_events_with_limits(&circuit, 4, Some(7), exact)
        .expect_err("the first zero-width record above the ownership budget should fail");
    assert!(first_rejected.to_string().contains("would require 4"));
    assert_materialized_bits_error(first_rejected, 4, 3);

    let huge_rejected = sample_detection_events_with_limits(
        &circuit,
        usize::MAX,
        Some(7),
        DetectionConversionLimits::default(),
    )
    .expect_err("huge zero-width materialized sampling must fail before allocation");
    assert_materialized_bits_error(
        huge_rejected,
        usize::MAX as u64,
        DetectionConversionLimits::default().max_materialized_bits() as u64,
    );
}

#[test]
fn materialized_sampling_does_not_charge_streamed_measurement_records() {
    let circuit = parse_circuit("M 0 1 2 3 4 5 6 7 8 9\n");
    let limits = DetectionConversionLimits::default().with_max_materialized_bits(2);

    let output = sample_detection_events_with_limits(&circuit, 2, Some(7), limits)
        .expect("streamed measurements must not consume the materialized output budget");
    assert_eq!(output.records.len(), 2);
    assert!(
        output
            .records
            .iter()
            .all(|record| record.detectors.is_empty() && record.observables.is_empty())
    );

    let caller_owned_measurements = vec![vec![false; 10]; 2];
    let conversion_error = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &caller_owned_measurements,
        skip_reference_sample(),
        limits,
    )
    .expect_err("materialized conversion retains its measurement-input admission contract");
    assert_materialized_bits_error(conversion_error, 20, 2);
}

#[test]
fn zero_width_materialized_conversion_charges_record_ownership() {
    let circuit = parse_circuit("");
    let measurements = vec![vec![], vec![]];
    let exact = DetectionConversionLimits::default().with_max_materialized_bits(2);

    let accepted = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &measurements,
        skip_reference_sample(),
        exact,
    )
    .expect("the exact zero-width conversion ownership maximum should be accepted");
    assert_eq!(accepted.records.len(), 2);

    let first_rejected = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &measurements,
        skip_reference_sample(),
        exact.with_max_materialized_bits(1),
    )
    .expect_err("materialized conversion must budget zero-width record ownership");
    assert_materialized_bits_error(first_rejected, 2, 1);
}

#[test]
fn zero_width_materialized_rejection_precedes_record_validation() {
    let circuit = parse_circuit("");
    let malformed_measurements = vec![vec![true]];
    let limits = DetectionConversionLimits::default().with_max_materialized_bits(0);

    let error = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &malformed_measurements,
        skip_reference_sample(),
        limits,
    )
    .expect_err("materialized admission should reject before iterating input records");
    assert_materialized_bits_error(error, 1, 0);
}

#[test]
fn zero_width_streaming_sampling_is_not_materialized_limited() {
    #[derive(Debug, Eq, PartialEq)]
    struct Stop;

    impl From<CircuitError> for Stop {
        fn from(_: CircuitError) -> Self {
            Self
        }
    }

    let circuit = parse_circuit("");
    let limits = DetectionConversionLimits::default().with_max_materialized_bits(0);
    let visits = Cell::new(0);

    let error = try_for_each_sampled_detection_event_with_limits(
        &circuit,
        usize::MAX,
        Some(7),
        limits,
        |record| {
            assert!(record.detectors.is_empty());
            assert!(record.observables.is_empty());
            visits.set(visits.get() + 1);
            if visits.get() == 3 { Err(Stop) } else { Ok(()) }
        },
    )
    .expect_err("the test visitor should stop the unbounded streaming request");
    assert_eq!(error, Stop);
    assert_eq!(visits.get(), 3);
}

#[test]
fn detector_and_observable_width_is_checked_before_materialization() {
    let circuit = parse_circuit(
        "M 0\n\
         DETECTOR rec[-1]\n\
         OBSERVABLE_INCLUDE(0) rec[-1]\n",
    );
    let accepted = DetectionConversionLimits::default().with_max_record_bits(2);
    let rejected = accepted.with_max_record_bits(1);

    let converter = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        accepted,
    )
    .expect("one detector and one observable should fit");
    assert_eq!(converter.detector_count(), 1);
    assert_eq!(converter.observable_count(), 1);

    let error = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        rejected,
    )
    .expect_err("the second output bit should fail");
    assert!(error.to_string().contains("detection record width 2"));
}

#[test]
fn repeat_and_expanded_instruction_limits_are_independent() {
    let circuit = parse_circuit("REPEAT 3 {\nM 0\n}\n");
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(3)
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(3);
    CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
        .expect("all three exact traversal maxima should be accepted");

    let repeat_rejected = DetectionConversionLimits::default()
        .with_max_repeat_unroll(2)
        .with_max_repeat_iterations(100)
        .with_max_expanded_instructions(100);
    let expanded_rejected = repeat_rejected
        .with_max_repeat_unroll(3)
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(2);

    let repeat_error = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        repeat_rejected,
    )
    .expect_err("per-repeat unroll should be admitted independently");
    assert!(repeat_error.to_string().contains("repeat counts up to 2"));

    let expanded_error = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        expanded_rejected,
    )
    .expect_err("expanded instruction work should have its own bound");
    assert!(
        expanded_error
            .to_string()
            .contains("3 expanded instructions")
    );
}

#[test]
fn nested_repeat_iterations_are_bounded_in_aggregate() {
    let circuit = parse_circuit(
        "REPEAT 2 {\n\
         REPEAT 2 {\n\
         M 0\n\
         }\n\
         }\n",
    );
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(2)
        .with_max_repeat_iterations(6)
        .with_max_expanded_instructions(100);
    CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
        .expect("the exact aggregate repeat-iteration maximum should be accepted");

    let error = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        exact.with_max_repeat_iterations(5),
    )
    .expect_err("nested repeats must consume one aggregate budget");
    assert!(error.to_string().contains("6 repeat iterations"));
    assert!(error.to_string().contains("limit is 5"));
}

#[test]
fn compact_nested_repeats_fail_without_expanding_the_product() {
    let circuit = parse_circuit(
        "REPEAT 100000 {\n\
         REPEAT 100000 {\n\
         M 0\n\
         }\n\
         }\n",
    );
    let limits = DetectionConversionLimits::default()
        .with_max_repeat_unroll(100_000)
        .with_max_repeat_iterations(100_001)
        .with_max_expanded_instructions(100_000);

    let error =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), limits)
            .expect_err("the nested product must be rejected before full expansion");
    assert!(error.to_string().contains("200000 repeat iterations"));
}

#[test]
fn defaults_bound_aggregate_detection_traversal() {
    let limits = DetectionConversionLimits::default();
    assert_eq!(limits.max_record_bits(), 1_000_000);
    assert_eq!(limits.max_materialized_bits(), 64_000_000);
    assert_eq!(limits.max_repeat_unroll(), 100_000);
    assert_eq!(limits.max_expanded_instructions(), 1_000_000);
    assert_eq!(limits.max_repeat_iterations(), 1_000_000);
    assert_eq!(limits.max_compiled_terms(), 16_000_000);
    assert_eq!(limits.max_compiled_bytes(), 256 * 1024 * 1024);

    let circuit = parse_circuit(
        "REPEAT 100000 {\n\
         REPEAT 10 {\n\
         TICK\n\
         }\n\
         }\n",
    );
    let error =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), limits)
            .expect_err("aggregate repeat work above the finite default must reject");
    let resource = error
        .resource_limit_error()
        .expect("default aggregate rejection should remain typed");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert!(resource.actual() > resource.limit());
}

#[test]
fn practical_default_traversal_boundaries_are_executed_exactly() {
    let mut expanded_text = String::from("REPEAT 100000 {\n");
    for _ in 0..10 {
        expanded_text.push_str("TICK\n");
    }
    expanded_text.push_str("}\n");
    let exact_expanded = parse_circuit(&expanded_text);
    CompiledDetectionConverter::compile_with_limits(
        &exact_expanded,
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect("one million expanded instructions should be accepted");

    let mut excessive_expanded = exact_expanded;
    excessive_expanded
        .append_from_stim_text("TICK\n")
        .expect("append first excess instruction");
    let error = CompiledDetectionConverter::compile_with_limits(
        &excessive_expanded,
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect_err("the first expanded instruction above the default must reject");
    let resource = error
        .resource_limit_error()
        .expect("expanded-instruction rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);

    let mut inner = Circuit::new();
    inner.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(999).expect("valid repeat count"),
        Circuit::new(),
        None,
    ));
    let mut exact_iterations = Circuit::new();
    exact_iterations.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(1_000).expect("valid repeat count"),
        inner,
        None,
    ));
    CompiledDetectionConverter::compile_with_limits(
        &exact_iterations,
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect("one million aggregate repeat iterations should be accepted");

    exact_iterations.append_repeat_block(RepeatBlock::new(
        RepeatCount::try_new(1).expect("valid repeat count"),
        Circuit::new(),
        None,
    ));
    let error = CompiledDetectionConverter::compile_with_limits(
        &exact_iterations,
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect_err("the first repeat iteration above the default must reject");
    let resource = error
        .resource_limit_error()
        .expect("repeat-iteration rejection should remain typed");
    assert_eq!(resource.resource(), ResourceKind::RepeatIterations);
    assert_eq!(resource.actual(), 1_000_001);
    assert_eq!(resource.limit(), 1_000_000);
}

#[test]
fn compiled_term_and_byte_budgets_preflight_wide_repeats() {
    let circuit = parse_circuit(
        "M 0 1\n\
         REPEAT 3 {\n\
         DETECTOR rec[-1] rec[-2]\n\
         }\n",
    );
    let exact_bytes =
        3 * std::mem::size_of::<Vec<usize>>() as u64 + 6 * std::mem::size_of::<usize>() as u64;
    let exact = DetectionConversionLimits::default()
        .with_max_repeat_unroll(3)
        .with_max_repeat_iterations(3)
        .with_max_expanded_instructions(4)
        .with_max_compiled_terms(6)
        .with_max_compiled_bytes(exact_bytes);
    let converter =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
            .expect("the exact compiled-plan boundaries should be accepted");
    assert_eq!(converter.detector_count(), 3);

    for (limits, expected_resource, actual, limit) in [
        (
            exact.with_max_compiled_terms(5),
            ResourceKind::CompiledTerms,
            6,
            5,
        ),
        (
            exact.with_max_compiled_bytes(exact_bytes - 1),
            ResourceKind::MaterializedBytes,
            exact_bytes,
            exact_bytes - 1,
        ),
    ] {
        let error = CompiledDetectionConverter::compile_with_limits(
            &circuit,
            skip_reference_sample(),
            limits,
        )
        .expect_err("the first compiled-plan unit above its limit must reject");
        let resource = error
            .resource_limit_error()
            .expect("compiled-plan rejection should remain typed");
        assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
        assert_eq!(resource.resource(), expected_resource);
        assert_eq!(resource.actual(), actual);
        assert_eq!(resource.limit(), limit);
    }
}

#[test]
fn materialized_buffers_are_bounded_but_streaming_records_are_not() {
    let circuit = parse_circuit("M 0\nDETECTOR rec[-1]\n");
    let exact = DetectionConversionLimits::default().with_max_materialized_bits(2);
    let converter =
        CompiledDetectionConverter::compile_with_limits(&circuit, skip_reference_sample(), exact)
            .expect("the per-record widths fit");
    let records = [vec![false], vec![true]];
    let visits = Cell::new(0);

    converter
        .try_for_each_detection_event(records.iter().map(Vec::as_slice), |_| {
            visits.set(visits.get() + 1);
            Ok::<(), CircuitError>(())
        })
        .expect("streaming should not consume the materialized-shot budget");
    assert_eq!(visits.get(), 2);

    let accepted = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &records,
        skip_reference_sample(),
        exact,
    )
    .expect("the exact two-bit materialization maximum should be accepted");
    assert_eq!(accepted.records.len(), 2);

    let rejected = exact.with_max_materialized_bits(1);
    let error = convert_measurements_to_detection_events_with_limits(
        &circuit,
        &records,
        skip_reference_sample(),
        rejected,
    )
    .expect_err("materializing two bits should exceed the one-bit budget");
    assert!(error.to_string().contains("would require 2 buffered bits"));
    let resource = error
        .resource_limit_error()
        .expect("materialization rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::MaterializedBits);
    assert_eq!(resource.actual(), 2);
    assert_eq!(resource.limit(), 1);
}

#[test]
fn visitor_cancellation_still_stops_streaming_immediately() {
    #[derive(Debug, Eq, PartialEq)]
    struct Stop;

    impl From<CircuitError> for Stop {
        fn from(_: CircuitError) -> Self {
            Self
        }
    }

    let circuit = parse_circuit("M 0\nDETECTOR rec[-1]\n");
    let converter = CompiledDetectionConverter::compile_with_limits(
        &circuit,
        skip_reference_sample(),
        DetectionConversionLimits::default(),
    )
    .expect("converter should compile");
    let records = [vec![false], vec![true], vec![false]];
    let visits = Cell::new(0);

    let error = converter
        .try_for_each_detection_event(records.iter().map(Vec::as_slice), |_| {
            visits.set(visits.get() + 1);
            Err(Stop)
        })
        .expect_err("the visitor should cancel");
    assert_eq!(error, Stop);
    assert_eq!(visits.get(), 1);
}

#[test]
fn public_count_and_streaming_entry_points_apply_limits_before_work() {
    let circuit = parse_circuit("M 0\nDETECTOR rec[-1]\n");
    let limits = DetectionConversionLimits::default().with_max_expanded_instructions(0);

    for error in [
        measurement_record_count_with_limits(&circuit, limits)
            .expect_err("measurement count must use the supplied traversal policy"),
        detection_record_width_with_limits(&circuit, limits)
            .expect_err("detection width must use the supplied traversal policy"),
        convert_measurements_to_detection_events_with_sweep_and_limits(
            &circuit,
            &[],
            &[],
            skip_reference_sample(),
            limits,
        )
        .expect_err("sweep conversion must use the supplied traversal policy"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("count-query rejection should expose typed resource context");
        assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
        assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
        assert_eq!(resource.actual(), 1);
        assert_eq!(resource.limit(), 0);
    }

    let visits = Cell::new(0);
    let error =
        try_for_each_sampled_detection_event_with_limits(&circuit, 1, Some(7), limits, |_| {
            visits.set(visits.get() + 1);
            Ok::<(), CircuitError>(())
        })
        .expect_err("streaming must reject the compile policy before sampling or visiting");
    let resource = error
        .resource_limit_error()
        .expect("streaming rejection should expose typed resource context");
    assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
    assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
    assert_eq!(resource.actual(), 1);
    assert_eq!(resource.limit(), 0);
    assert_eq!(visits.get(), 0);
}

#[test]
fn frame_validation_and_sampling_apply_policy_before_materialization() {
    let circuit = parse_circuit(
        "REPEAT 2 {\n\
         MPAD 0\n\
         OBSERVABLE_INCLUDE(0) Z0\n\
         }\n",
    );
    let limits = DetectionConversionLimits::default().with_max_repeat_unroll(1);

    let validation_error = validate_detection_sampling_circuit_with_limits(&circuit, limits)
        .expect_err("frame validation must honor custom repeat admission");
    let sampling_error = sample_detection_events_with_limits(&circuit, 1, Some(7), limits)
        .expect_err("frame sampling must honor custom repeat admission");

    for error in [validation_error, sampling_error] {
        assert!(error.to_string().contains("repeat counts up to 1"));
        let resource = error
            .resource_limit_error()
            .expect("frame repeat rejection should expose typed resource context");
        assert_eq!(resource.operation(), ResourceOperation::DetectionConversion);
        assert_eq!(resource.resource(), ResourceKind::RepeatCount);
        assert_eq!(resource.actual(), 2);
        assert_eq!(resource.limit(), 1);
    }

    let small = parse_circuit("H 0\nOBSERVABLE_INCLUDE(0) Z0\n");
    let mut large_text = String::from("H");
    for qubit in 0..4096 {
        large_text.push(' ');
        large_text.push_str(&qubit.to_string());
    }
    large_text.push_str("\nOBSERVABLE_INCLUDE(0) Z0\n");
    let large = parse_circuit(&large_text);
    let limits = DetectionConversionLimits::default().with_max_expanded_instructions(0);

    let rejection_allocations = |circuit: &Circuit| {
        allocation_counter::measure(|| {
            let error = sample_detection_events_with_limits(circuit, 1, Some(7), limits)
                .expect_err("the first frame instruction exceeds the custom traversal budget");
            let resource = error
                .resource_limit_error()
                .expect("frame admission should return a typed resource error");
            assert_eq!(resource.resource(), ResourceKind::ExpandedOperations);
            drop(black_box(error));
        })
    };
    let small_allocations = rejection_allocations(&small);
    let large_allocations = rejection_allocations(&large);

    assert!(
        large_allocations.count_total <= small_allocations.count_total + 2,
        "frame rejection allocations scaled with target storage: small={small_allocations:?}, large={large_allocations:?}"
    );
    assert!(
        large_allocations.bytes_total <= small_allocations.bytes_total + 256,
        "frame rejection bytes scaled with target storage: small={small_allocations:?}, large={large_allocations:?}"
    );
}
