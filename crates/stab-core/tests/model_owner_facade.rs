#![allow(
    clippy::expect_used,
    reason = "facade tests extract required failures directly"
)]

use stab_core::{
    ByteSpan, Circuit, CircuitError, ModelDialect, ModelError, ParseError, ParseErrorCode,
    ParseErrorContext, ParseLimits, RepeatNestingLimit, SourceLineLimit,
};
use stab_model::ResourceKind as ModelResourceKind;

#[test]
fn model_parse_error_conversion_preserves_the_complete_diagnostic() {
    let parse_error = ParseError::decode_utf8(ModelDialect::StimCircuit, b"H 0\n\xff")
        .expect_err("invalid UTF-8 is rejected");
    let expected = parse_error.clone();

    let error = CircuitError::from(ModelError::from(parse_error));
    assert_eq!(error.parse_error(), Some(&expected));
    assert_eq!(error.to_string(), expected.to_string());
    assert_eq!(expected.code(), ParseErrorCode::InvalidUtf8Input);
    assert_eq!(
        expected.span(),
        ByteSpan::try_new(4, 1).expect("valid span")
    );
    assert_eq!(
        expected.context(),
        &ParseErrorContext::Utf8 {
            dialect: ModelDialect::StimCircuit,
            valid_up_to: 4,
            error_length: Some(1),
        }
    );
}

#[test]
fn model_owned_parse_limits_remain_the_core_parser_policy() {
    let source_limits = ParseLimits::default().with_source_line_limit(SourceLineLimit::new(1));
    let source_error = Circuit::from_stim_str_with_limits("H 0\nM 0\n", source_limits)
        .expect_err("the second source line exceeds the model-owned policy");
    let source_resource = source_error
        .resource_limit_error()
        .expect("source-line rejection remains structured");
    assert_eq!(source_resource.resource(), ModelResourceKind::SourceLines);
    assert_eq!(source_resource.actual(), 2);
    assert_eq!(source_resource.limit(), 1);
    assert_eq!(
        source_error.to_string(),
        "failed to parse line 2: circuit input has more than 1 lines"
    );

    let repeat_limits = ParseLimits::default().with_repeat_nesting_limit(
        RepeatNestingLimit::try_new(1).expect("one repeat level is admitted"),
    );
    let repeat_error =
        Circuit::from_stim_str_with_limits("REPEAT 1 {\nREPEAT 1 {\nH 0\n}\n}\n", repeat_limits)
            .expect_err("the second repeat level exceeds the model-owned policy");
    let repeat_resource = repeat_error
        .resource_limit_error()
        .expect("repeat rejection remains structured");
    assert_eq!(repeat_resource.resource(), ModelResourceKind::RepeatNesting);
    assert_eq!(repeat_resource.actual(), 2);
    assert_eq!(repeat_resource.limit(), 1);
    assert_eq!(
        repeat_error.to_string(),
        "failed to parse line 2: repeat nesting exceeds current limit 1"
    );
}
