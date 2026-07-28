#![allow(
    clippy::expect_used,
    reason = "resource contract tests use direct fixture assertions for precise failures"
)]

use stab_core::{
    Circuit, CircuitError, DetectorErrorModel, Estimate, EstimateClass, ModelError, ParseLimits,
    RepeatNestingLimit, ResourceKind, ResourceOperation, SourceLineLimit,
};

#[test]
fn parse_limit_defaults_preserve_existing_boundaries() {
    let limits = ParseLimits::default();
    assert_eq!(
        limits.source_line_limit(),
        ParseLimits::DEFAULT_SOURCE_LINES
    );
    assert_eq!(
        limits.repeat_nesting_limit(),
        ParseLimits::DEFAULT_REPEAT_NESTING
    );
    assert_eq!(limits.source_line_limit().get(), 1_000_000);
    assert_eq!(limits.repeat_nesting_limit().get(), 256);

    let custom = ParseLimits::new(
        SourceLineLimit::new(17),
        RepeatNestingLimit::try_new(3).expect("three repeat levels are safe"),
    );
    assert_eq!(custom.source_line_limit().get(), 17);
    assert_eq!(custom.repeat_nesting_limit().get(), 3);

    let error = RepeatNestingLimit::try_new(RepeatNestingLimit::HARD_MAX + 1)
        .expect_err("policies cannot override recursive model safety");
    assert_eq!(error.requested(), 257);
    assert_eq!(error.hard_max(), 256);
    assert_eq!(
        error.to_string(),
        "repeat nesting limit 257 exceeds the non-overridable hard maximum 256"
    );
}

#[test]
fn parse_estimate_classifies_only_cheaply_known_text_properties() {
    let estimate = ParseLimits::default().estimate("H 0\r\nM 0\n");

    assert_eq!(estimate.input_bytes(), Estimate::Exact(9));
    assert_eq!(estimate.input_items(), Estimate::Exact(2));
    assert_eq!(estimate.input_bytes().class(), EstimateClass::Exact);
    assert_eq!(estimate.input_bytes().value(), Some(&9));
    assert_eq!(estimate.expanded_operations(), Estimate::Unknown);
    assert_eq!(
        estimate.expanded_operations().class(),
        EstimateClass::Unknown
    );
    assert_eq!(estimate.expanded_operations().value(), None);
    assert_eq!(estimate.folded_traversal(), Estimate::Unknown);
    assert_eq!(estimate.scratch_bytes(), Estimate::Unknown);
    assert_eq!(estimate.resident_bytes(), Estimate::Unknown);
    assert_eq!(estimate.output_bytes(), Estimate::Unknown);
    assert_eq!(estimate.work_units(), Estimate::Unknown);

    assert_eq!(
        ParseLimits::default().estimate("").input_items(),
        Estimate::Exact(0)
    );
    assert_eq!(
        ParseLimits::default().estimate("H 0").input_items(),
        Estimate::Exact(1)
    );

    let byte_estimate = ParseLimits::default().estimate_bytes(b"H 0 # \xff\nM[\xfe] 0");
    assert_eq!(byte_estimate.input_bytes(), Estimate::Exact(14));
    assert_eq!(byte_estimate.input_items(), Estimate::Exact(2));
}

#[test]
fn resource_identifiers_are_stable() {
    for (operation, code) in [
        (ResourceOperation::CircuitParse, "circuit-parse"),
        (
            ResourceOperation::DetectorErrorModelParse,
            "detector-error-model-parse",
        ),
        (ResourceOperation::CircuitFlatten, "circuit-flatten"),
        (
            ResourceOperation::DetectionConversion,
            "detection-conversion",
        ),
        (
            ResourceOperation::DetectorErrorModelFlatten,
            "detector-error-model-flatten",
        ),
        (
            ResourceOperation::DetectorErrorModelSampling,
            "detector-error-model-sampling",
        ),
        (
            ResourceOperation::LogicalErrorSearch,
            "logical-error-search",
        ),
        (ResourceOperation::SatMaterialization, "sat-materialization"),
    ] {
        assert_eq!(operation.as_str(), code);
    }

    for (resource, code) in [
        (ResourceKind::SourceLines, "source-lines"),
        (ResourceKind::RepeatNesting, "repeat-nesting"),
        (ResourceKind::ExpandedOperations, "expanded-operations"),
        (ResourceKind::RecordBits, "record-bits"),
        (ResourceKind::MaterializedBits, "materialized-bits"),
        (ResourceKind::RepeatCount, "repeat-count"),
        (ResourceKind::RepeatIterations, "repeat-iterations"),
        (ResourceKind::MaterializedUnits, "materialized-units"),
        (ResourceKind::MaterializedBytes, "materialized-bytes"),
        (
            ResourceKind::SampledErrorApplications,
            "sampled-error-applications",
        ),
        (ResourceKind::ReplayWorkUnits, "replay-work-units"),
        (ResourceKind::ErrorMechanisms, "error-mechanisms"),
        (ResourceKind::TargetOccurrences, "target-occurrences"),
        (
            ResourceKind::ErrorTargetOccurrencesPerMechanism,
            "error-target-occurrences-per-mechanism",
        ),
        (
            ResourceKind::TotalErrorTargetOccurrences,
            "total-error-target-occurrences",
        ),
        (
            ResourceKind::EffectiveDetectorNodes,
            "effective-detector-nodes",
        ),
        (ResourceKind::UniqueGraphEdges, "unique-graph-edges"),
        (ResourceKind::StoredGraphTerms, "stored-graph-terms"),
        (ResourceKind::HyperedgeDegree, "hyperedge-degree"),
        (ResourceKind::HyperedgeIncidences, "hyperedge-incidences"),
        (ResourceKind::SearchStates, "search-states"),
        (ResourceKind::SearchTransitions, "search-transitions"),
        (ResourceKind::SearchStateTerms, "search-state-terms"),
        (
            ResourceKind::StoredSearchStateTerms,
            "stored-search-state-terms",
        ),
        (ResourceKind::Variables, "variables"),
        (ResourceKind::Clauses, "clauses"),
        (ResourceKind::ClauseLiterals, "clause-literals"),
        (ResourceKind::OutputBytes, "output-bytes"),
    ] {
        assert_eq!(resource.as_str(), code);
    }
}

#[test]
fn circuit_parse_limits_accept_boundary_and_reject_first_excess_line() {
    let limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(2));
    Circuit::from_stim_str_with_limits("H 0\nM 0\n", limits).expect("two lines are admitted");

    let error = Circuit::from_stim_str_with_limits("H 0\nM 0\nTICK\n", limits)
        .expect_err("third line exceeds the configured limit");
    assert_resource_limit(
        &error,
        ResourceOperation::CircuitParse,
        ResourceKind::SourceLines,
        3,
        2,
    );
    assert_eq!(
        error.to_string(),
        "failed to parse line 3: circuit input has more than 2 lines"
    );
}

#[test]
fn dem_parse_limits_accept_boundary_and_reject_first_excess_line() {
    let limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(2));
    DetectorErrorModel::from_dem_str_with_limits("error(0.1) D0\nshift_detectors 1\n", limits)
        .expect("two lines are admitted");

    let error = DetectorErrorModel::from_dem_str_with_limits(
        "error(0.1) D0\nshift_detectors 1\nlogical_observable L0\n",
        limits,
    )
    .expect_err("third line exceeds the configured limit");
    assert_resource_limit(
        &error,
        ResourceOperation::DetectorErrorModelParse,
        ResourceKind::SourceLines,
        3,
        2,
    );
    assert_eq!(
        error.to_string(),
        "invalid detector error model: DEM input has more than 2 lines"
    );
}

#[test]
fn circuit_and_dem_repeat_limits_reject_the_first_excess_level() {
    let limits = ParseLimits::default().with_repeat_nesting_limit(
        RepeatNestingLimit::try_new(1).expect("one repeat level is safe"),
    );

    Circuit::from_stim_str_with_limits("REPEAT 2 {\nH 0\n}\n", limits)
        .expect("one circuit repeat level is admitted");
    let circuit_error =
        Circuit::from_stim_str_with_limits("REPEAT 2 {\nREPEAT 2 {\nH 0\n}\n}\n", limits)
            .expect_err("second circuit repeat level exceeds the limit");
    assert_resource_limit(
        &circuit_error,
        ResourceOperation::CircuitParse,
        ResourceKind::RepeatNesting,
        2,
        1,
    );
    assert_eq!(
        circuit_error.to_string(),
        "failed to parse line 2: repeat nesting exceeds current limit 1"
    );

    DetectorErrorModel::from_dem_str_with_limits("repeat 2 {\nerror(0.1) D0\n}\n", limits)
        .expect("one DEM repeat level is admitted");
    let dem_error = DetectorErrorModel::from_dem_str_with_limits(
        "repeat 2 {\nrepeat 2 {\nerror(0.1) D0\n}\n}\n",
        limits,
    )
    .expect_err("second DEM repeat level exceeds the limit");
    assert_resource_limit(
        &dem_error,
        ResourceOperation::DetectorErrorModelParse,
        ResourceKind::RepeatNesting,
        2,
        1,
    );
    assert_eq!(
        dem_error.to_string(),
        "invalid detector error model: DEM repeat nesting exceeds current limit 1"
    );
}

#[test]
fn default_policies_accept_the_exact_boundary_and_reject_the_first_excess() {
    let mut lines = "\n".repeat(ParseLimits::DEFAULT_SOURCE_LINES.get());
    Circuit::from_stim_str(&lines).expect("default circuit line boundary is admitted");
    DetectorErrorModel::from_dem_str(&lines).expect("default DEM line boundary is admitted");

    lines.push('\n');
    let circuit_error =
        Circuit::from_stim_str(&lines).expect_err("first excess circuit line is rejected");
    assert_resource_limit(
        &circuit_error,
        ResourceOperation::CircuitParse,
        ResourceKind::SourceLines,
        1_000_001,
        1_000_000,
    );
    let dem_error =
        DetectorErrorModel::from_dem_str(&lines).expect_err("first excess DEM line is rejected");
    assert_resource_limit(
        &dem_error,
        ResourceOperation::DetectorErrorModelParse,
        ResourceKind::SourceLines,
        1_000_001,
        1_000_000,
    );

    let circuit_at_limit = nested_repeat_text("REPEAT 2 {", "H 0", 256);
    Circuit::from_stim_str(&circuit_at_limit).expect("default circuit repeat boundary is admitted");
    let circuit_over_limit = nested_repeat_text("REPEAT 2 {", "H 0", 257);
    let circuit_error = Circuit::from_stim_str(&circuit_over_limit)
        .expect_err("first excess circuit repeat is rejected");
    assert_resource_limit(
        &circuit_error,
        ResourceOperation::CircuitParse,
        ResourceKind::RepeatNesting,
        257,
        256,
    );

    let dem_at_limit = nested_repeat_text("repeat 2 {", "error(0.1) D0", 256);
    DetectorErrorModel::from_dem_str(&dem_at_limit)
        .expect("default DEM repeat boundary is admitted");
    let dem_over_limit = nested_repeat_text("repeat 2 {", "error(0.1) D0", 257);
    let dem_error = DetectorErrorModel::from_dem_str(&dem_over_limit)
        .expect_err("first excess DEM repeat is rejected");
    assert_resource_limit(
        &dem_error,
        ResourceOperation::DetectorErrorModelParse,
        ResourceKind::RepeatNesting,
        257,
        256,
    );
}

#[test]
fn parse_preallocation_is_bounded_by_the_admitted_line_prefix() {
    let limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(1));

    let circuit_small = "H 0\nM 0\n";
    let mut circuit_large = String::from("H 0\n");
    circuit_large.push_str(&"M 0\n".repeat(100_000));
    let circuit_small_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_str_with_limits(circuit_small, limits)
                .expect_err("second circuit line is rejected"),
        );
    });
    let circuit_large_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_str_with_limits(&circuit_large, limits)
                .expect_err("second circuit line is rejected"),
        );
    });
    assert_eq!(
        circuit_large_allocations, circuit_small_allocations,
        "circuit preallocation must not scale with rejected trailing input"
    );

    let dem_small = "error(0.1) D0\nshift_detectors 1\n";
    let mut dem_large = String::from("error(0.1) D0\n");
    dem_large.push_str(&"shift_detectors 1\n".repeat(100_000));
    let dem_small_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_str_with_limits(dem_small, limits)
                .expect_err("second DEM line is rejected"),
        );
    });
    let dem_large_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_str_with_limits(&dem_large, limits)
                .expect_err("second DEM line is rejected"),
        );
    });
    assert_eq!(
        dem_large_allocations, dem_small_allocations,
        "DEM preallocation must not scale with rejected trailing input"
    );

    let circuit_small_bytes = b"H 0\nM 0\n".as_slice();
    let mut circuit_large_bytes = circuit_small_bytes.to_vec();
    circuit_large_bytes.extend_from_slice("M 0\n".repeat(100_000).as_bytes());
    let circuit_small_byte_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_bytes_with_limits(circuit_small_bytes, limits)
                .expect_err("second circuit byte line is rejected"),
        );
    });
    let circuit_large_byte_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_bytes_with_limits(&circuit_large_bytes, limits)
                .expect_err("second circuit byte line is rejected"),
        );
    });
    assert_eq!(
        circuit_large_byte_allocations, circuit_small_byte_allocations,
        "circuit byte preparation must stop after the first rejected physical line"
    );

    let dem_small_bytes = b"error(0.1) D0\nshift_detectors 1\n".as_slice();
    let mut dem_large_bytes = dem_small_bytes.to_vec();
    dem_large_bytes.extend_from_slice("shift_detectors 1\n".repeat(100_000).as_bytes());
    let dem_small_byte_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_bytes_with_limits(dem_small_bytes, limits)
                .expect_err("second DEM byte line is rejected"),
        );
    });
    let dem_large_byte_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_bytes_with_limits(&dem_large_bytes, limits)
                .expect_err("second DEM byte line is rejected"),
        );
    });
    assert_eq!(
        dem_large_byte_allocations, dem_small_byte_allocations,
        "DEM byte preparation must stop after the first rejected physical line"
    );
}

#[test]
fn byte_parse_admission_does_not_copy_an_unterminated_rejected_line() {
    let limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(1));

    let circuit_small = b"H 0\nM".as_slice();
    let mut circuit_large = b"H 0\n".to_vec();
    circuit_large.extend(std::iter::repeat_n(b'M', 1_000_000));
    let circuit_small_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_bytes_with_limits(circuit_small, limits)
                .expect_err("second circuit line is rejected"),
        );
    });
    let circuit_large_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            Circuit::from_stim_bytes_with_limits(&circuit_large, limits)
                .expect_err("large second circuit line is rejected"),
        );
    });
    assert_eq!(
        circuit_large_allocations, circuit_small_allocations,
        "circuit byte admission must retain only the first byte of a rejected line"
    );

    let dem_small = b"error(0.1) D0\ns".as_slice();
    let mut dem_large = b"error(0.1) D0\n".to_vec();
    dem_large.extend(std::iter::repeat_n(b's', 1_000_000));
    let dem_small_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_bytes_with_limits(dem_small, limits)
                .expect_err("second DEM line is rejected"),
        );
    });
    let dem_large_allocations = allocation_counter::measure(|| {
        std::hint::black_box(
            DetectorErrorModel::from_dem_bytes_with_limits(&dem_large, limits)
                .expect_err("large second DEM line is rejected"),
        );
    });
    assert_eq!(
        dem_large_allocations, dem_small_allocations,
        "DEM byte admission must retain only the first byte of a rejected line"
    );

    let circuit_error = Circuit::from_stim_bytes_with_limits(b"H 0\n\xff", limits)
        .expect_err("line admission precedes decoding rejected circuit bytes");
    assert_resource_limit(
        &circuit_error,
        ResourceOperation::CircuitParse,
        ResourceKind::SourceLines,
        2,
        1,
    );
    let dem_error = DetectorErrorModel::from_dem_bytes_with_limits(b"error(0.1) D0\n\xff", limits)
        .expect_err("line admission precedes decoding rejected DEM bytes");
    assert_resource_limit(
        &dem_error,
        ResourceOperation::DetectorErrorModelParse,
        ResourceKind::SourceLines,
        2,
        1,
    );
}

fn nested_repeat_text(header: &str, body: &str, depth: usize) -> String {
    let mut text = String::with_capacity(depth.saturating_mul(header.len() + 3) + body.len() + 1);
    for _ in 0..depth {
        text.push_str(header);
        text.push('\n');
    }
    text.push_str(body);
    text.push('\n');
    for _ in 0..depth {
        text.push_str("}\n");
    }
    text
}

fn assert_resource_limit(
    error: &ModelError,
    operation: ResourceOperation,
    resource: ResourceKind,
    actual: u64,
    limit: u64,
) {
    let error = CircuitError::from(error.clone());
    let typed = error
        .resource_limit_error()
        .expect("expected typed resource-limit payload");
    assert_eq!(typed.code(), "resource-limit-exceeded");
    assert_eq!(typed.operation(), operation);
    assert_eq!(typed.resource(), resource);
    assert_eq!(typed.actual(), actual);
    assert_eq!(typed.limit(), limit);
}
