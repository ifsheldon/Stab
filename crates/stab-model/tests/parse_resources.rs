#![allow(
    clippy::expect_used,
    reason = "contract tests extract required failures directly"
)]

use stab_model::{
    ByteSpan, Circuit, DetectorErrorModel, DiagnosticSeverity, Estimate, EstimateClass,
    MemoryBudget, ModelDialect, ModelError, ModelResult, ParseError, ParseErrorCode,
    ParseErrorContext, ParseLimits, RepeatNestingLimit, ResourceKind, ResourceOperation, advanced,
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

fn budget(bytes: usize) -> ParseLimits {
    ParseLimits::default().with_memory_budget(MemoryBudget::from_bytes(bytes as u64))
}

/// Smallest memory budget that admits `input`, found by bisection over a monotone predicate.
fn minimal_budget(input: &str, parse: ParseStr) -> usize {
    let (mut low, mut high) = (0_usize, 1 << 20);
    assert!(
        parse(input, budget(high)).is_ok(),
        "fixture fits the search bound"
    );
    while low < high {
        let middle = low + (high - low) / 2;
        if parse(input, budget(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn default_parse_budget_is_one_gibibyte_with_the_shared_nesting_envelope() {
    let limits = ParseLimits::default();
    assert_eq!(limits.memory_budget(), MemoryBudget::from_bytes(1 << 30));
    assert_eq!(
        limits.repeat_nesting_limit().get(),
        RepeatNestingLimit::HARD_MAX
    );
}

#[test]
fn parser_admission_accepts_the_exact_budget_and_rejects_the_first_excess_byte() {
    struct Case {
        name: &'static str,
        input: &'static str,
        parse_str: ParseStr,
        parse_bytes: ParseBytes,
        dialect: ModelDialect,
        operation: ResourceOperation,
        last_line: usize,
    }
    let cases = [
        Case {
            name: "circuit",
            input: "REPEAT 99 {\nM 0\n}\nMPP X0*Y1 Z2\n",
            parse_str: parse_circuit_str,
            parse_bytes: parse_circuit_bytes,
            dialect: ModelDialect::StimCircuit,
            operation: ResourceOperation::CircuitParse,
            last_line: 4,
        },
        Case {
            name: "DEM",
            input: "repeat 99 {\nerror(0.1) D0\n}\nerror(0.1) D0 ^ L0 D1\n",
            parse_str: parse_dem_str,
            parse_bytes: parse_dem_bytes,
            dialect: ModelDialect::DetectorErrorModel,
            operation: ResourceOperation::DetectorErrorModelParse,
            last_line: 4,
        },
    ];

    for case in cases {
        let exact = minimal_budget(case.input, case.parse_str);
        assert!(
            exact > case.input.len(),
            "{}: retained model storage is charged beyond the source bytes",
            case.name
        );
        assert!((case.parse_bytes)(case.input.as_bytes(), budget(exact)).is_ok());

        for error in [
            (case.parse_str)(case.input, budget(exact - 1))
                .expect_err("string parser rejects the first excess byte"),
            (case.parse_bytes)(case.input.as_bytes(), budget(exact - 1))
                .expect_err("byte parser rejects the first excess byte"),
        ] {
            let resource = error
                .resource_limit_error()
                .expect("typed parser resource failure");
            assert_eq!(resource.code(), "resource-limit-exceeded", "{}", case.name);
            assert_eq!(resource.severity(), DiagnosticSeverity::Error);
            assert_eq!(resource.dialect(), case.dialect, "{}", case.name);
            assert_eq!(resource.operation(), case.operation, "{}", case.name);
            assert_eq!(resource.resource(), ResourceKind::RetainedBytes);
            assert_eq!(resource.actual(), exact as u64, "{}", case.name);
            assert_eq!(resource.limit(), exact as u64 - 1, "{}", case.name);
            assert_eq!(
                resource.source_line(),
                Some(case.last_line),
                "{}",
                case.name
            );
            assert!(
                resource.to_string().contains("memory budget"),
                "{}: {resource}",
                case.name
            );
        }

        let source_limit = budget(case.input.len() - 1);
        for error in [
            (case.parse_str)(case.input, source_limit).expect_err("source bytes exceed"),
            (case.parse_bytes)(case.input.as_bytes(), source_limit).expect_err("source bytes"),
        ] {
            let resource = error.resource_limit_error().expect("typed source failure");
            assert_eq!(
                resource.resource(),
                ResourceKind::SourceBytes,
                "{}",
                case.name
            );
            assert_eq!(resource.actual(), case.input.len() as u64);
            assert_eq!(resource.limit(), case.input.len() as u64 - 1);
            assert_eq!(resource.source_line(), None);
            assert_eq!(resource.span(), span(case.input.len() - 1, 1));
        }
    }

    let zero = ParseLimits::new(
        MemoryBudget::from_bytes(0),
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
fn parser_admission_charges_fast_path_targets_and_prefusion_declarations() {
    for (name, prefix, input, parse) in [
        (
            "circuit",
            "H 0\n",
            "H 0\nM 1 2 3 4 5 6 7 8 9 10 11\n",
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM",
            "error(0) D0\n",
            "error(0) D0\nerror(0) D1 D2 D3 D4 D5 D6 D7 D8 D9 D10 D11\n",
            parse_dem_str as ParseStr,
        ),
    ] {
        // A budget that admits the source and the first line's retained storage rejects the
        // fast-path line, so its targets are charged before retention.
        let first_line = minimal_budget(prefix, parse) - prefix.len();
        let error = parse(input, budget(input.len() + first_line))
            .expect_err("fast-path targets are charged");
        let resource = error.resource_limit_error().expect("typed rejection");
        assert_eq!(resource.resource(), ResourceKind::RetainedBytes, "{name}");
        assert_eq!(resource.source_line(), Some(2), "{name}");
    }

    // Adjacent instructions are charged before fusion: two declarations retain more than one
    // declaration with the same targets.
    let fused = minimal_budget("H 0 1\n", parse_circuit_str) - "H 0 1\n".len();
    let split = minimal_budget("H 0\nH 1\n", parse_circuit_str) - "H 0\nH 1\n".len();
    assert!(split > fused, "{split} <= {fused}");
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

    // Each budget admits the source plus the retained storage of `first`, so every case is
    // rejected at the same declaration however long its rejected suffix is.
    let cases = [
        (
            "circuit declarations",
            "H 0\n",
            "H 0\nM 1\n".to_string(),
            format!("H 0\nM 1\n{}", "M 2\n".repeat(4_096)),
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM declarations",
            "error(0.1) D0\n",
            "error(0.1) D0\nerror(0.2) D1\n".to_string(),
            format!(
                "error(0.1) D0\nerror(0.2) D1\n{}",
                "error(0.3) D2\n".repeat(4_096)
            ),
            parse_dem_str as ParseStr,
        ),
        (
            "circuit targets",
            "M 0\n",
            "M 0\nM 1 2 3".to_string(),
            format!("M 0\nM 1 2 3 {}", "2 ".repeat(4_096)),
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM targets",
            "error(0.1) D0\n",
            "error(0.1) D0\nerror(0.1) D0 D1 D2".to_string(),
            format!("error(0.1) D0\nerror(0.1) D0 D1 D2 {}", "D2 ".repeat(4_096)),
            parse_dem_str as ParseStr,
        ),
    ];

    for (name, first, short, long, parse) in cases {
        let first = minimal_budget(first, parse) - first.len();
        let short = allocations(&short, budget(short.len() + first), parse);
        let long = allocations(&long, budget(long.len() + first), parse);
        assert_eq!(long.count_total, short.count_total, "{name}: {long:?}");
        assert_eq!(long.bytes_total, short.bytes_total, "{name}: {long:?}");
        assert_eq!(long.count_max, short.count_max, "{name}: {long:?}");
        assert_eq!(long.bytes_max, short.bytes_max, "{name}: {long:?}");
    }

    for (name, short, long, parse) in [
        (
            "circuit declarations after invalid command",
            "NOT_A_GATE\n".to_string(),
            format!("NOT_A_GATE\n{}", "TICK\n".repeat(4_096)),
            parse_circuit_str as ParseStr,
        ),
        (
            "DEM declarations after invalid command",
            "not_a_dem_instruction\n".to_string(),
            format!(
                "not_a_dem_instruction\n{}",
                "shift_detectors 1\n".repeat(4_096)
            ),
            parse_dem_str as ParseStr,
        ),
    ] {
        let short = allocations(&short, ParseLimits::default(), parse);
        let long = allocations(&long, ParseLimits::default(), parse);
        assert_eq!(long.count_total, short.count_total, "{name}: {long:?}");
        assert_eq!(long.bytes_total, short.bytes_total, "{name}: {long:?}");
    }

    for input in ["H 0\n", &format!("H 0\n{}", "M 1\n".repeat(4_096))] {
        let measured = allocations(input, budget(3), parse_circuit_str);
        assert_eq!(measured.count_total, 0, "{measured:?}");
        assert_eq!(measured.bytes_total, 0, "{measured:?}");
    }

    for parse in [
        parse_circuit_bytes as ParseBytes,
        parse_dem_bytes as ParseBytes,
    ] {
        let measured = byte_allocations(b"\xff", budget(0), parse);
        assert_eq!(measured.count_total, 0, "{measured:?}");
        assert_eq!(measured.bytes_total, 0, "{measured:?}");
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
