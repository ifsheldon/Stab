use crate::{ByteSpan, CircuitError, ModelDialect, ParseError, ParseErrorCode, ParseErrorContext};

pub(crate) fn line_error(
    dialect: ModelDialect,
    line_number: usize,
    code: ParseErrorCode,
    message: impl Into<String>,
    legacy_detail: impl Into<String>,
    span: ByteSpan,
    context: ParseErrorContext,
) -> CircuitError {
    debug_assert_eq!(context.dialect(), dialect);
    ParseError::with_human_message(
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
) -> CircuitError {
    ParseError::with_human_message(code, message, human_message, span, context).into()
}

pub(crate) fn validation_error(
    dialect: ModelDialect,
    line_number: usize,
    instruction: &str,
    code: ParseErrorCode,
    span: ByteSpan,
    error: CircuitError,
    prefix_line: bool,
) -> CircuitError {
    if matches!(
        error,
        CircuitError::Parse(_) | CircuitError::ResourceLimit(_)
    ) {
        return error;
    }
    let message = match &error {
        CircuitError::ParseLine { message, .. }
        | CircuitError::InvalidDetectorErrorModel { message } => message.clone(),
        _ => error.to_string(),
    };
    let human_message = if prefix_line {
        format!("failed to parse line {line_number}: {error}")
    } else {
        error.to_string()
    };
    let context = match &error {
        CircuitError::UnknownGate(name) => ParseErrorContext::Instruction {
            dialect,
            instruction: name.clone(),
        },
        CircuitError::InvalidDomainValue { kind, value } => ParseErrorContext::DomainValue {
            dialect,
            kind,
            value: value.clone(),
        },
        CircuitError::InvalidArgumentCount {
            gate,
            expected,
            actual,
        } => ParseErrorContext::ArgumentCount {
            dialect,
            instruction: (*gate).to_string(),
            expected,
            actual: *actual,
        },
        CircuitError::InvalidArgument { gate, argument } => ParseErrorContext::Argument {
            dialect,
            instruction: (*gate).to_string(),
            argument: argument.clone(),
        },
        CircuitError::InvalidTarget { gate, target } => ParseErrorContext::Target {
            dialect,
            instruction: (*gate).to_string(),
            target: target.clone(),
        },
        CircuitError::InvalidTargetCount { gate, count } => ParseErrorContext::TargetCount {
            dialect,
            instruction: (*gate).to_string(),
            actual: *count,
        },
        _ if instruction.is_empty() => ParseErrorContext::Model { dialect },
        _ => ParseErrorContext::Instruction {
            dialect,
            instruction: instruction.to_string(),
        },
    };
    ParseError::with_human_message(code, message, human_message, span, context).into()
}

pub(crate) fn unexpected_repeat_terminator(dialect: ModelDialect, span: ByteSpan) -> CircuitError {
    plain_error(
        ParseErrorCode::UnexpectedRepeatTerminator,
        "unexpected repeat block terminator",
        "unexpected repeat block terminator",
        span,
        ParseErrorContext::Model { dialect },
    )
}

pub(crate) fn unterminated_repeat_block(dialect: ModelDialect, input_len: usize) -> CircuitError {
    plain_error(
        ParseErrorCode::UnterminatedRepeatBlock,
        "unterminated repeat block",
        "unterminated repeat block",
        ByteSpan::from_valid_range(input_len, 0),
        ParseErrorContext::Model { dialect },
    )
}
