#![allow(
    clippy::expect_used,
    reason = "contract tests extract required failures directly"
)]

use stab_model::{
    ByteSpan, DiagnosticSeverity, Estimate, EstimateClass, ModelDialect, ModelError, ParseError,
    ParseErrorCode, ParseErrorContext, ParseLimits, RepeatNestingLimit, ResourceKind,
    ResourceLimitContext, ResourceOperation, SourceLineLimit, advanced,
};

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
fn parse_limits_preserve_default_custom_and_hard_boundaries() {
    let defaults = ParseLimits::default();
    assert_eq!(
        defaults.source_line_limit(),
        ParseLimits::DEFAULT_SOURCE_LINES
    );
    assert_eq!(
        defaults.repeat_nesting_limit(),
        ParseLimits::DEFAULT_REPEAT_NESTING
    );
    assert_eq!(defaults.source_line_limit().get(), 1_000_000);
    assert_eq!(defaults.repeat_nesting_limit().get(), 256);

    let custom = ParseLimits::new(
        SourceLineLimit::new(17),
        RepeatNestingLimit::try_new(3).expect("three levels are admitted"),
    );
    assert_eq!(custom.source_line_limit().get(), 17);
    assert_eq!(custom.repeat_nesting_limit().get(), 3);

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
fn parser_resource_failures_preserve_typed_context_and_human_diagnostics() {
    let span = ByteSpan::try_new(12, 3).expect("valid span");
    let cases = [
        (
            advanced::circuit_source_line_limit_error(3, 2, span),
            ResourceOperation::CircuitParse,
            ResourceKind::SourceLines,
            ResourceLimitContext::CircuitSourceLines,
            "failed to parse line 3: circuit input has more than 2 lines",
        ),
        (
            advanced::circuit_repeat_nesting_limit_error(7, 3, 2, span),
            ResourceOperation::CircuitParse,
            ResourceKind::RepeatNesting,
            ResourceLimitContext::CircuitRepeatNesting { source_line: 7 },
            "failed to parse line 7: repeat nesting exceeds current limit 2",
        ),
        (
            advanced::dem_source_line_limit_error(3, 2, span),
            ResourceOperation::DetectorErrorModelParse,
            ResourceKind::SourceLines,
            ResourceLimitContext::DetectorErrorModelSourceLines,
            "invalid detector error model: DEM input has more than 2 lines",
        ),
        (
            advanced::dem_repeat_nesting_limit_error(3, 2, span),
            ResourceOperation::DetectorErrorModelParse,
            ResourceKind::RepeatNesting,
            ResourceLimitContext::DetectorErrorModelRepeatNesting,
            "invalid detector error model: DEM repeat nesting exceeds current limit 2",
        ),
    ];

    for (error, operation, resource, context, message) in cases {
        let resource_error = error
            .resource_limit_error()
            .expect("advanced constructor returns a resource failure");
        assert_eq!(resource_error.code(), "resource-limit-exceeded");
        assert_eq!(resource_error.severity(), DiagnosticSeverity::Error);
        assert_eq!(resource_error.operation(), operation);
        assert_eq!(resource_error.resource(), resource);
        assert_eq!(resource_error.context(), context);
        assert_eq!(resource_error.actual(), 3);
        assert_eq!(resource_error.limit(), 2);
        assert_eq!(resource_error.span(), span);
        assert_eq!(error.to_string(), message);
    }
}
