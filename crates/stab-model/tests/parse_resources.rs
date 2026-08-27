#![allow(
    clippy::expect_used,
    reason = "contract tests extract required failures directly"
)]

use stab_model::{
    ByteSpan, Circuit, DetectorErrorModel, DiagnosticSeverity, Estimate, EstimateClass,
    ModelDialect, ModelError, ModelResult, ParseError, ParseErrorCode, ParseErrorContext,
    ParseLimits, RepeatNestingLimit, RepresentedInstructionLimit, RepresentedTargetLimit,
    ResourceKind, ResourceOperation, SourceByteLimit, SourceLineLimit, advanced,
};

type ParseStr = fn(&str, ParseLimits) -> ModelResult<()>;
type ParseBytes = fn(&[u8], ParseLimits) -> ModelResult<()>;

fn parse_circuit_str(input: &str, limits: ParseLimits) -> ModelResult<()> {
    Circuit::from_stim_str_with_limits(input, limits).map(drop)
}

fn parse_circuit_bytes(input: &[u8], limits: ParseLimits) -> ModelResult<()> {
    Circuit::from_stim_bytes_with_limits(input, limits).map(drop)
}

fn parse_dem_str(input: &str, limits: ParseLimits) -> ModelResult<()> {
    DetectorErrorModel::from_dem_str_with_limits(input, limits).map(drop)
}

fn parse_dem_bytes(input: &[u8], limits: ParseLimits) -> ModelResult<()> {
    DetectorErrorModel::from_dem_bytes_with_limits(input, limits).map(drop)
}

fn span(byte_start: usize, byte_length: usize) -> ByteSpan {
    ByteSpan::try_new(byte_start, byte_length).expect("test span is representable")
}

#[test]
fn model_dialects_preserve_the_frozen_names_and_order() {
    assert_eq!(
        ModelDialect::all().collect::<Vec<_>>(),
        vec![ModelDialect::StimCircuit, ModelDialect::DetectorErrorModel]
    );
    assert_eq!(ModelDialect::StimCircuit.as_str(), "stim-circuit");
    assert_eq!(
        ModelDialect::DetectorErrorModel.as_str(),
        "detector-error-model"
    );
}

#[test]
fn byte_spans_reject_overflow_and_preserve_half_open_offsets() {
    let span = ByteSpan::try_new(5, 7).expect("small span is representable");
    assert_eq!(span.byte_start(), 5);
    assert_eq!(span.byte_length(), 7);
    assert_eq!(span.byte_end(), 12);
    assert_eq!(ByteSpan::try_new(usize::MAX, 1), None);
    assert_eq!(advanced::byte_span_from_valid_range(5, 7), span);
}

#[test]
fn utf8_diagnostics_preserve_code_span_context_and_human_text() {
    let error = ParseError::decode_utf8(ModelDialect::DetectorErrorModel, b"error(0.1) D0\n\xc3")
        .expect_err("incomplete UTF-8 sequence is rejected");

    assert_eq!(error.code(), ParseErrorCode::InvalidUtf8Input);
    assert_eq!(error.code().as_str(), "invalid-utf8-input");
    assert_eq!(error.severity(), DiagnosticSeverity::Error);
    assert_eq!(error.severity().as_str(), "error");
    assert_eq!(error.message(), "input is not valid UTF-8 text");
    assert_eq!(error.to_string(), "input is not valid UTF-8 text");
    assert_eq!(error.span(), ByteSpan::try_new(14, 1).expect("valid span"));
    assert_eq!(
        error.context(),
        &ParseErrorContext::Utf8 {
            dialect: ModelDialect::DetectorErrorModel,
            valid_up_to: 14,
            error_length: None,
        }
    );
    assert_eq!(error.context().dialect(), ModelDialect::DetectorErrorModel);

    let model_error = ModelError::from(error.clone());
    assert_eq!(model_error, ModelError::Parse(error));
}

#[test]
fn repeat_nesting_configuration_cannot_exceed_the_recursive_safety_envelope() {
    let error = RepeatNestingLimit::try_new(RepeatNestingLimit::HARD_MAX + 1)
        .expect_err("the recursive hard maximum cannot be overridden");
    assert_eq!(error.requested(), 257);
    assert_eq!(error.hard_max(), 256);
    assert_eq!(
        error.to_string(),
        "repeat nesting limit 257 exceeds the non-overridable hard maximum 256"
    );
}

#[test]
fn parse_estimates_preserve_exact_line_and_byte_accounting() {
    let estimate = ParseLimits::default().estimate("H 0\r\nM 0\n");
    assert_eq!(estimate.input_bytes(), Estimate::Exact(9));
    assert_eq!(estimate.input_items(), Estimate::Exact(2));
    assert_eq!(estimate.input_bytes().class(), EstimateClass::Exact);
    assert_eq!(estimate.input_bytes().value(), Some(&9));
    assert_eq!(estimate.expanded_operations(), Estimate::Unknown);
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

    let bytes = ParseLimits::default().estimate_bytes(b"H 0 # \xff\nM[\xfe] 0");
    assert_eq!(bytes.input_bytes(), Estimate::Exact(14));
    assert_eq!(bytes.input_items(), Estimate::Exact(2));
}

#[test]
fn parser_admission_accepts_exact_limits_and_rejects_the_first_excess_unit() {
    struct Case {
        name: &'static str,
        accepted: &'static str,
        rejected: &'static str,
        limits: ParseLimits,
        parse_str: ParseStr,
        parse_bytes: ParseBytes,
        dialect: ModelDialect,
        operation: ResourceOperation,
        resource: ResourceKind,
        actual: u64,
        limit: u64,
        source_line: Option<usize>,
        rejected_span: ByteSpan,
    }

    let circuit_instructions = "REPEAT 99 {\nM 0\n}\nM 1\n";
    let dem_instructions = "repeat 99 {\nerror(0.1) D0\n}\nerror(0.2) D1\n";
    let circuit_targets = "MPP X0*Y1 Z2";
    let dem_targets = "error(0.1) D0 ^ L0 D1";
    let cases = [
        Case {
            name: "circuit source bytes",
            accepted: "H 0",
            rejected: "H 0\n",
            limits: ParseLimits::default()
                .with_source_byte_limit(SourceByteLimit::new("H 0".len())),
            parse_str: parse_circuit_str,
            parse_bytes: parse_circuit_bytes,
            dialect: ModelDialect::StimCircuit,
            operation: ResourceOperation::CircuitParse,
            resource: ResourceKind::SourceBytes,
            actual: 4,
            limit: 3,
            source_line: None,
            rejected_span: span(3, 1),
        },
        Case {
            name: "DEM source bytes",
            accepted: "error(0) D0",
            rejected: "error(0) D0\n",
            limits: ParseLimits::default()
                .with_source_byte_limit(SourceByteLimit::new("error(0) D0".len())),
            parse_str: parse_dem_str,
            parse_bytes: parse_dem_bytes,
            dialect: ModelDialect::DetectorErrorModel,
            operation: ResourceOperation::DetectorErrorModelParse,
            resource: ResourceKind::SourceBytes,
            actual: 12,
            limit: 11,
            source_line: None,
            rejected_span: span(11, 1),
        },
        Case {
            name: "circuit represented instructions",
            accepted: "REPEAT 99 {\nM 0\n}\n",
            rejected: circuit_instructions,
            limits: ParseLimits::default()
                .with_represented_instruction_limit(RepresentedInstructionLimit::new(2)),
            parse_str: parse_circuit_str,
            parse_bytes: parse_circuit_bytes,
            dialect: ModelDialect::StimCircuit,
            operation: ResourceOperation::CircuitParse,
            resource: ResourceKind::RepresentedInstructions,
            actual: 3,
            limit: 2,
            source_line: Some(4),
            rejected_span: span(circuit_instructions.find("M 1").expect("marker"), 3),
        },
        Case {
            name: "DEM represented instructions",
            accepted: "repeat 99 {\nerror(0.1) D0\n}\n",
            rejected: dem_instructions,
            limits: ParseLimits::default()
                .with_represented_instruction_limit(RepresentedInstructionLimit::new(2)),
            parse_str: parse_dem_str,
            parse_bytes: parse_dem_bytes,
            dialect: ModelDialect::DetectorErrorModel,
            operation: ResourceOperation::DetectorErrorModelParse,
            resource: ResourceKind::RepresentedInstructions,
            actual: 3,
            limit: 2,
            source_line: Some(4),
            rejected_span: span(dem_instructions.find("error(0.2)").expect("marker"), 13),
        },
        Case {
            name: "circuit represented targets",
            accepted: "MPP X0*Y1",
            rejected: circuit_targets,
            limits: ParseLimits::default()
                .with_represented_target_limit(RepresentedTargetLimit::new(3)),
            parse_str: parse_circuit_str,
            parse_bytes: parse_circuit_bytes,
            dialect: ModelDialect::StimCircuit,
            operation: ResourceOperation::CircuitParse,
            resource: ResourceKind::RepresentedTargets,
            actual: 4,
            limit: 3,
            source_line: Some(1),
            rejected_span: span(circuit_targets.find("Z2").expect("marker"), 2),
        },
        Case {
            name: "DEM represented targets",
            accepted: "error(0.1) D0 ^ L0",
            rejected: dem_targets,
            limits: ParseLimits::default()
                .with_represented_target_limit(RepresentedTargetLimit::new(3)),
            parse_str: parse_dem_str,
            parse_bytes: parse_dem_bytes,
            dialect: ModelDialect::DetectorErrorModel,
            operation: ResourceOperation::DetectorErrorModelParse,
            resource: ResourceKind::RepresentedTargets,
            actual: 4,
            limit: 3,
            source_line: Some(1),
            rejected_span: span(dem_targets.find("D1").expect("marker"), 2),
        },
    ];

    for case in cases {
        let accepted_str = (case.parse_str)(case.accepted, case.limits);
        assert!(accepted_str.is_ok(), "{}: {accepted_str:?}", case.name);
        let accepted_bytes = (case.parse_bytes)(case.accepted.as_bytes(), case.limits);
        assert!(accepted_bytes.is_ok(), "{}: {accepted_bytes:?}", case.name);

        for error in [
            (case.parse_str)(case.rejected, case.limits)
                .expect_err("string parser rejects first excess unit"),
            (case.parse_bytes)(case.rejected.as_bytes(), case.limits)
                .expect_err("byte parser rejects first excess unit"),
        ] {
            let resource = error
                .resource_limit_error()
                .expect("typed parser resource failure");
            assert_eq!(resource.code(), "resource-limit-exceeded", "{}", case.name);
            assert_eq!(
                resource.severity(),
                DiagnosticSeverity::Error,
                "{}",
                case.name
            );
            assert_eq!(resource.dialect(), case.dialect, "{}", case.name);
            assert_eq!(resource.operation(), case.operation, "{}", case.name);
            assert_eq!(resource.resource(), case.resource, "{}", case.name);
            assert_eq!(resource.actual(), case.actual, "{}", case.name);
            assert_eq!(resource.limit(), case.limit, "{}", case.name);
            assert_eq!(resource.source_line(), case.source_line, "{}", case.name);
            assert_eq!(resource.span(), case.rejected_span, "{}", case.name);
        }
    }

    let zero = ParseLimits::new(
        SourceByteLimit::new(0),
        SourceLineLimit::new(0),
        RepresentedInstructionLimit::new(0),
        RepresentedTargetLimit::new(0),
        RepeatNestingLimit::try_new(0).expect("zero nesting is valid"),
    );
    parse_circuit_str("", zero).expect("empty circuit fits zero limits");
    parse_dem_str("", zero).expect("empty DEM fits zero limits");

    for error in [
        parse_circuit_bytes(b"\xff", zero).expect_err("byte admission precedes circuit UTF-8"),
        parse_dem_bytes(b"\xff", zero).expect_err("byte admission precedes DEM UTF-8"),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("source byte policy wins before UTF-8 decoding");
        assert_eq!(resource.resource(), ResourceKind::SourceBytes);
        assert_eq!((resource.actual(), resource.limit()), (1, 0));
        assert_eq!(resource.span(), span(0, 1));
    }
}

#[test]
fn parser_admission_accumulates_fast_path_targets_and_prefusion_declarations() {
    let target_limits =
        ParseLimits::default().with_represented_target_limit(RepresentedTargetLimit::new(11));
    for (name, error, operation) in [
        (
            "circuit",
            parse_circuit_str("H 0\nM 1 2 3 4 5 6 7 8 9 10 11\n", target_limits)
                .expect_err("the first fast-path target contributes to the cumulative budget"),
            ResourceOperation::CircuitParse,
        ),
        (
            "DEM",
            parse_dem_str(
                "error(0) D0\nerror(0) D1 D2 D3 D4 D5 D6 D7 D8 D9 D10 D11\n",
                target_limits,
            )
            .expect_err("the first DEM fast-path target contributes to the cumulative budget"),
            ResourceOperation::DetectorErrorModelParse,
        ),
    ] {
        let resource = error
            .resource_limit_error()
            .expect("cumulative target rejection is typed");
        assert_eq!(resource.operation(), operation, "{name}");
        assert_eq!(
            resource.resource(),
            ResourceKind::RepresentedTargets,
            "{name}"
        );
        assert_eq!((resource.actual(), resource.limit()), (12, 11), "{name}");
        assert_eq!(resource.source_line(), Some(2), "{name}");
    }

    let instruction_limits = ParseLimits::default()
        .with_represented_instruction_limit(RepresentedInstructionLimit::new(1));
    let error = parse_circuit_str("H 0\nH 1\n", instruction_limits)
        .expect_err("adjacent instructions are admitted before fusion");
    let resource = error
        .resource_limit_error()
        .expect("prefusion declaration rejection is typed");
    assert_eq!(resource.resource(), ResourceKind::RepresentedInstructions);
    assert_eq!((resource.actual(), resource.limit()), (2, 1));
    assert_eq!(resource.source_line(), Some(2));
}

#[test]
fn parser_allocation_depends_only_on_admitted_model_storage() {
    fn allocations(
        input: &str,
        limits: ParseLimits,
        parse: ParseStr,
    ) -> allocation_counter::AllocationInfo {
        allocation_counter::measure(|| {
            std::hint::black_box(parse(input, limits).expect_err("fixture is rejected"));
        })
    }

    fn byte_allocations(
        input: &[u8],
        limits: ParseLimits,
        parse: ParseBytes,
    ) -> allocation_counter::AllocationInfo {
        allocation_counter::measure(|| {
            std::hint::black_box(parse(input, limits).expect_err("byte fixture is rejected"));
        })
    }

    let instruction_limits = ParseLimits::default()
        .with_represented_instruction_limit(RepresentedInstructionLimit::new(1));
    let target_limits =
        ParseLimits::default().with_represented_target_limit(RepresentedTargetLimit::new(1));
    let cases = [
        (
            "circuit instructions",
            "H 0\nM 1\n".to_string(),
            format!("H 0\nM 1\n{}", "M 2\n".repeat(4_096)),
            instruction_limits,
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM instructions",
            "error(0.1) D0\nerror(0.2) D1\n".to_string(),
            format!(
                "error(0.1) D0\nerror(0.2) D1\n{}",
                "error(0.3) D2\n".repeat(4_096)
            ),
            instruction_limits,
            parse_dem_str as ParseStr,
        ),
        (
            "circuit targets",
            "# target prefix\nM 0 1".to_string(),
            format!("# target prefix\nM 0 1 {}", "2 ".repeat(4_096)),
            target_limits,
            parse_circuit_str as ParseStr,
        ),
        (
            "circuit declarations after rejected target",
            "M 0 1".to_string(),
            format!("M 0 1\n{}", "TICK\n".repeat(4_096)),
            target_limits,
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM targets",
            "# target prefix\nerror(0.1) D0 D1".to_string(),
            format!("# target prefix\nerror(0.1) D0 D1 {}", "D2 ".repeat(4_096)),
            target_limits,
            parse_dem_str as ParseStr,
        ),
        (
            "DEM declarations after rejected target",
            "error(0.1) D0 D1".to_string(),
            format!("error(0.1) D0 D1\n{}", "shift_detectors 1\n".repeat(4_096)),
            target_limits,
            parse_dem_str as ParseStr,
        ),
        (
            "circuit declarations after invalid command",
            "NOT_A_GATE\n".to_string(),
            format!("NOT_A_GATE\n{}", "TICK\n".repeat(4_096)),
            ParseLimits::default(),
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM declarations after invalid command",
            "not_a_dem_instruction\n".to_string(),
            format!(
                "not_a_dem_instruction\n{}",
                "shift_detectors 1\n".repeat(4_096)
            ),
            ParseLimits::default(),
            parse_dem_str as ParseStr,
        ),
    ];

    for (name, short, long, limits, parse) in cases {
        let short = allocations(&short, limits, parse);
        let long = allocations(&long, limits, parse);
        assert_eq!(long.count_total, short.count_total, "{name}: {long:?}");
        assert_eq!(long.bytes_total, short.bytes_total, "{name}: {long:?}");
        assert_eq!(long.count_max, short.count_max, "{name}: {long:?}");
        assert_eq!(long.bytes_max, short.bytes_max, "{name}: {long:?}");
    }

    let byte_limits = ParseLimits::default().with_source_byte_limit(SourceByteLimit::new(3));
    for input in ["H 0\n", &format!("H 0\n{}", "M 1\n".repeat(4_096))] {
        let measured = allocations(input, byte_limits, parse_circuit_str);
        assert_eq!(measured.count_total, 0, "{measured:?}");
        assert_eq!(measured.bytes_total, 0, "{measured:?}");
    }

    for parse in [
        parse_circuit_bytes as ParseBytes,
        parse_dem_bytes as ParseBytes,
    ] {
        let measured = byte_allocations(
            b"\xff",
            ParseLimits::default().with_source_byte_limit(SourceByteLimit::new(0)),
            parse,
        );
        assert_eq!(measured.count_total, 0, "{measured:?}");
        assert_eq!(measured.bytes_total, 0, "{measured:?}");

        let line_limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(1));
        let short = b"#\n\xff";
        let mut long = short.to_vec();
        long.extend(std::iter::repeat_n(0xff, 32_768));
        let short = byte_allocations(short, line_limits, parse);
        let long = byte_allocations(&long, line_limits, parse);
        assert_eq!(long.count_total, short.count_total, "{long:?}");
        assert_eq!(long.bytes_total, short.bytes_total, "{long:?}");
        assert_eq!(long.count_max, short.count_max, "{long:?}");
        assert_eq!(long.bytes_max, short.bytes_max, "{long:?}");
    }

    let short = "\n# ignored\n".repeat(32);
    let long = "\n# ignored\n".repeat(32_768);
    for (name, parse) in [
        ("circuit", parse_circuit_str as ParseStr),
        ("DEM", parse_dem_str as ParseStr),
    ] {
        for input in [&short, &long] {
            let measured = allocation_counter::measure(|| {
                parse(input, ParseLimits::default()).expect("fixture is an empty model");
            });
            assert_eq!(measured.count_total, 0, "{name}: {measured:?}");
            assert_eq!(measured.bytes_total, 0, "{name}: {measured:?}");
        }
    }
}
