use crate::{
    ByteSpan, ModelDialect, ModelError, ParseErrorCode, ParseErrorContext, ValidationError,
    advanced::{byte_span_from_valid_range, parse_error_with_human_message},
};

pub(crate) fn line_error(
    dialect: ModelDialect,
    line_number: usize,
    code: ParseErrorCode,
    message: impl Into<String>,
    legacy_detail: impl Into<String>,
    span: ByteSpan,
    context: ParseErrorContext,
) -> ModelError {
    debug_assert_eq!(context.dialect(), dialect);
    parse_error_with_human_message(
        code,
        message,
        format!(
            "failed to parse line {line_number}: {}",
            legacy_detail.into()
        ),
        span,
        context,
    )
    .into()
}

pub(crate) fn plain_error(
    code: ParseErrorCode,
    message: impl Into<String>,
    human_message: impl Into<String>,
    span: ByteSpan,
    context: ParseErrorContext,
) -> ModelError {
    parse_error_with_human_message(code, message, human_message, span, context).into()
}

pub(crate) fn validation_error(
    dialect: ModelDialect,
    line_number: usize,
    instruction: &str,
    code: ParseErrorCode,
    span: ByteSpan,
    error: ModelError,
    prefix_line: bool,
) -> ModelError {
    let validation = match error {
        ModelError::Parse(_) | ModelError::ResourceLimit(_) => return error,
        ModelError::Validation(validation) => validation,
    };
    let message = match &validation {
        ValidationError::InvalidDetectorErrorModel { message } => message.clone(),
        _ => validation.to_string(),
    };
    let human_message = if prefix_line {
        format!("failed to parse line {line_number}: {validation}")
    } else {
        validation.to_string()
    };
    let context = match &validation {
        ValidationError::UnknownGate(name) => ParseErrorContext::Instruction {
            dialect,
            instruction: name.clone(),
        },
        ValidationError::InvalidDomainValue { kind, value } => ParseErrorContext::DomainValue {
            dialect,
            kind,
            value: value.clone(),
        },
        ValidationError::InvalidArgumentCount {
            gate,
            expected,
            actual,
        } => ParseErrorContext::ArgumentCount {
            dialect,
            instruction: (*gate).to_string(),
            expected,
            actual: *actual,
        },
        ValidationError::InvalidArgument { gate, argument } => ParseErrorContext::Argument {
            dialect,
            instruction: (*gate).to_string(),
            argument: argument.clone(),
        },
        ValidationError::InvalidTarget { gate, target } => ParseErrorContext::Target {
            dialect,
            instruction: (*gate).to_string(),
            target: target.clone(),
        },
        ValidationError::InvalidTargetCount { gate, count } => ParseErrorContext::TargetCount {
            dialect,
            instruction: (*gate).to_string(),
            actual: *count,
        },
        ValidationError::CircuitCountOverflow
        | ValidationError::CoordinateShiftDimensionMissing
        | ValidationError::CoordinateShiftOverflow
        | ValidationError::DetectorCountOverflow
        | ValidationError::DetectorIndexOutOfRange { .. }
        | ValidationError::DetectorCoordinateLookupFailed
            if instruction.is_empty() =>
        {
            ParseErrorContext::Model { dialect }
        }
        _ => ParseErrorContext::Instruction {
            dialect,
            instruction: instruction.to_string(),
        },
    };
    parse_error_with_human_message(code, message, human_message, span, context).into()
}

pub(crate) fn unexpected_repeat_terminator(dialect: ModelDialect, span: ByteSpan) -> ModelError {
    plain_error(
        ParseErrorCode::UnexpectedRepeatTerminator,
        "unexpected repeat block terminator",
        "unexpected repeat block terminator",
        span,
        ParseErrorContext::Model { dialect },
    )
}

pub(crate) fn unterminated_repeat_block(dialect: ModelDialect, input_len: usize) -> ModelError {
    plain_error(
        ParseErrorCode::UnterminatedRepeatBlock,
        "unterminated repeat block",
        "unterminated repeat block",
        byte_span_from_valid_range(input_len, 0),
        ParseErrorContext::Model { dialect },
    )
}
