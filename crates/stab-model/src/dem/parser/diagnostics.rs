//! Diagnostics the DEM text parser reports: typed parse errors with their codes, spans,
//! contexts, and legacy messages.

use super::MAX_DEM_TEXT_INTEGER;
use crate::diagnostics::bounded_parse_diagnostic_text;
use crate::model_parse::line_error;
use crate::source_text::SourceSlice;
use crate::{
    ByteSpan, ModelDialect, ModelError, ParseErrorCode, ParseErrorContext, ValidationError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UnsignedSourceError {
    InvalidDigit { byte_offset: usize },
    OutOfRange,
}

pub(super) fn unsigned_target_error(
    line_number: usize,
    instruction: &str,
    full_token: SourceSlice<'_>,
    number_token: SourceSlice<'_>,
    target_kind: &'static str,
    error: UnsignedSourceError,
) -> ModelError {
    let (code, span, message) = match error {
        UnsignedSourceError::InvalidDigit { byte_offset } => (
            ParseErrorCode::InvalidTargetSyntax,
            character_span_at(number_token, byte_offset.min(number_token.text().len())),
            format!("invalid {target_kind} {:?}", number_token.text()),
        ),
        UnsignedSourceError::OutOfRange => (
            ParseErrorCode::IntegerOutOfRange,
            full_token.span(),
            number_token.text().parse::<u64>().map_or_else(
                |_| format!("invalid {target_kind} {:?}", number_token.text()),
                |value| format!("{target_kind} {value} exceeds {MAX_DEM_TEXT_INTEGER}"),
            ),
        ),
    };
    let legacy = ModelError::invalid_detector_error_model(message.clone()).to_string();
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        code,
        message,
        legacy,
        span,
        target_context(instruction, full_token.text()),
    )
}

pub(super) fn repeat_count_error(
    line_number: usize,
    token: SourceSlice<'_>,
    error: UnsignedSourceError,
) -> ModelError {
    let (code, span, message, legacy_message) = match error {
        UnsignedSourceError::InvalidDigit { byte_offset } => (
            ParseErrorCode::InvalidRepeatCount,
            character_span_at(token, byte_offset),
            "invalid repeat count",
            format!("invalid repeat count {:?}", token.text()),
        ),
        UnsignedSourceError::OutOfRange => (
            ParseErrorCode::IntegerOutOfRange,
            token.span(),
            "repeat count is out of range",
            token.text().parse::<u64>().map_or_else(
                |_| format!("invalid repeat count {:?}", token.text()),
                |value| format!("repeat count {value} exceeds {MAX_DEM_TEXT_INTEGER}"),
            ),
        ),
    };
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        code,
        message,
        ModelError::invalid_detector_error_model(legacy_message).to_string(),
        span,
        ParseErrorContext::DomainValue {
            dialect: ModelDialect::DetectorErrorModel,
            kind: "repeat count",
            value: bounded_parse_diagnostic_text(token.text()),
        },
    )
}

pub(super) fn invalid_number_error(
    line_number: usize,
    instruction: &str,
    token: SourceSlice<'_>,
) -> ModelError {
    let span = token.span();
    let token = bounded_parse_diagnostic_text(token.text());
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidNumber,
        format!("invalid argument {token}"),
        format!("invalid argument {token}"),
        span,
        ParseErrorContext::Argument {
            dialect: ModelDialect::DetectorErrorModel,
            instruction: bounded_parse_diagnostic_text(instruction),
            argument: token,
        },
    )
}

pub(super) fn target_syntax_error(
    line_number: usize,
    instruction: &str,
    target: &str,
    span: ByteSpan,
    message: &'static str,
) -> ModelError {
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidTargetSyntax,
        message,
        format!("{message}: {target:?}"),
        span,
        target_context(instruction, target),
    )
}

pub(super) fn missing_repeat_count_error(line_number: usize, span: ByteSpan) -> ModelError {
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::MissingRepeatCount,
        "missing repeat count",
        "missing repeat count",
        span,
        ParseErrorContext::DomainValue {
            dialect: ModelDialect::DetectorErrorModel,
            kind: "repeat count",
            value: String::new(),
        },
    )
}

pub(super) fn parse_line_error(line_number: usize, message: impl Into<String>) -> ModelError {
    let message = message.into();
    line_error(
        ModelDialect::DetectorErrorModel,
        line_number,
        ParseErrorCode::InvalidSyntax,
        message.clone(),
        message,
        ByteSpan::empty_at(0),
        ParseErrorContext::Model {
            dialect: ModelDialect::DetectorErrorModel,
        },
    )
}

pub(super) fn instruction_context(instruction: &str) -> ParseErrorContext {
    ParseErrorContext::Instruction {
        dialect: ModelDialect::DetectorErrorModel,
        instruction: bounded_parse_diagnostic_text(instruction),
    }
}

pub(super) fn target_context(instruction: &str, target: &str) -> ParseErrorContext {
    ParseErrorContext::Target {
        dialect: ModelDialect::DetectorErrorModel,
        instruction: bounded_parse_diagnostic_text(instruction),
        target: bounded_parse_diagnostic_text(target),
    }
}

pub(super) fn dem_error_message(error: &ModelError) -> String {
    match error.validation_error() {
        Some(ValidationError::InvalidDetectorErrorModel { message }) => message.clone(),
        _ => error.to_string(),
    }
}

pub(super) fn first_character_span(source: SourceSlice<'_>) -> ByteSpan {
    character_span_at(source, 0)
}

pub(super) fn character_span_at(source: SourceSlice<'_>, byte_offset: usize) -> ByteSpan {
    let Some(tail) = source.suffix(byte_offset) else {
        return source.span();
    };
    tail.text()
        .chars()
        .next()
        .and_then(|character| tail.subspan(0, character.len_utf8()))
        .unwrap_or_else(|| tail.end_span())
}
