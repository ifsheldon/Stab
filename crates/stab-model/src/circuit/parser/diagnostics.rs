//! Diagnostics the circuit text parser reports: typed parse errors with their codes, spans, and
//! contexts.

use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::line_error;
use crate::source_text::SourceSlice;
use crate::{
    ByteSpan, ModelDialect, ModelError, ParseErrorCode, ParseErrorContext, ValidationError,
};

/// A refused repeat count, with `token` as the reported value.
pub(super) fn repeat_count_error(
    line_number: usize,
    token: SourceSlice<'_>,
    code: ParseErrorCode,
    message: &'static str,
    span: ByteSpan,
) -> ModelError {
    line_error(
        ModelDialect::StimCircuit,
        line_number,
        code,
        message,
        message,
        span,
        ParseErrorContext::DomainValue {
            dialect: ModelDialect::StimCircuit,
            kind: "repeat count",
            value: bounded_parse_diagnostic_text(token.text()),
        },
    )
}

pub(super) fn target_parse_error_code(error: &ModelError) -> ParseErrorCode {
    match error {
        ModelError::Validation(ValidationError::InvalidDomainValue { value, .. }) => {
            let digits = value.trim_prefix('-');
            if !digits.is_empty() && digits.as_bytes().iter().all(u8::is_ascii_digit) {
                ParseErrorCode::IntegerOutOfRange
            } else {
                ParseErrorCode::InvalidTargetSyntax
            }
        }
        _ => ParseErrorCode::InvalidTargetSyntax,
    }
}

pub(super) fn invalid_number_error(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
    message: &'static str,
) -> ModelError {
    let span = token.span();
    let token = bounded_parse_diagnostic_text(token.text());
    line_error(
        ModelDialect::StimCircuit,
        line_number,
        ParseErrorCode::InvalidNumber,
        format!("{message} {token}"),
        format!("{message} {token}"),
        span,
        ParseErrorContext::Argument {
            dialect: ModelDialect::StimCircuit,
            instruction: bounded_parse_diagnostic_text(instruction),
            argument: token,
        },
    )
}

pub(super) fn instruction_context(instruction: &str) -> ParseErrorContext {
    ParseErrorContext::Instruction {
        dialect: ModelDialect::StimCircuit,
        instruction: bounded_parse_diagnostic_text(instruction),
    }
}

pub(super) fn first_character_span(source: SourceSlice<'_>) -> ByteSpan {
    source
        .text()
        .chars()
        .next()
        .and_then(|character| source.subspan(0, character.len_utf8()))
        .unwrap_or_else(|| source.end_span())
}

pub(super) fn internal_parser_error(line_number: usize, message: impl ToString) -> ModelError {
    ModelError::invalid_domain_value(
        "Stim circuit parser",
        format!("line {line_number}: {}", message.to_string()),
    )
}
