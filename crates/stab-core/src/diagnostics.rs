use thiserror::Error;

use crate::ModelDialect;
use crate::result_formats::DetsResultType;

const MAX_PARSE_DIAGNOSTIC_TEXT_BYTES: usize = 256;

pub(crate) fn bounded_parse_diagnostic_text(value: &str) -> String {
    if value.len() <= MAX_PARSE_DIAGNOSTIC_TEXT_BYTES {
        return value.to_owned();
    }

    let suffix = format!("... [truncated; original length: {} bytes]", value.len());
    let prefix_byte_budget = MAX_PARSE_DIAGNOSTIC_TEXT_BYTES.saturating_sub(suffix.len());
    let mut excerpt = String::with_capacity(MAX_PARSE_DIAGNOSTIC_TEXT_BYTES);
    for character in value.chars() {
        if excerpt.len().saturating_add(character.len_utf8()) > prefix_byte_budget {
            break;
        }
        excerpt.push(character);
    }
    excerpt.push_str(&suffix);
    excerpt
}

/// A half-open byte range in the original input.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByteSpan {
    byte_start: usize,
    byte_length: usize,
}

impl ByteSpan {
    pub(crate) const fn from_valid_range(byte_start: usize, byte_length: usize) -> Self {
        Self {
            byte_start,
            byte_length,
        }
    }

    pub const fn try_new(byte_start: usize, byte_length: usize) -> Option<Self> {
        match byte_start.checked_add(byte_length) {
            Some(_) => Some(Self {
                byte_start,
                byte_length,
            }),
            None => None,
        }
    }

    pub const fn byte_start(self) -> usize {
        self.byte_start
    }

    pub const fn byte_length(self) -> usize {
        self.byte_length
    }

    pub const fn byte_end(self) -> usize {
        self.byte_start + self.byte_length
    }
}

/// Machine-readable diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

impl DiagnosticSeverity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

/// Stable machine-readable model-parse failure classes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ParseErrorCode {
    InvalidUtf8Input,
    InvalidSyntax,
    MissingInstructionName,
    UnknownInstruction,
    InvalidTagEscape,
    UnterminatedTag,
    UnterminatedTagEscape,
    UnterminatedArgumentList,
    InvalidNumber,
    InvalidArgument,
    InvalidArgumentCount,
    InvalidTargetSyntax,
    InvalidTarget,
    InvalidTargetCount,
    InvalidRepeatBlock,
    MissingRepeatCount,
    InvalidRepeatCount,
    IntegerOutOfRange,
    UnexpectedRepeatTerminator,
    UnterminatedRepeatBlock,
}

impl ParseErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidUtf8Input => "invalid-utf8-input",
            Self::InvalidSyntax => "invalid-syntax",
            Self::MissingInstructionName => "missing-instruction-name",
            Self::UnknownInstruction => "unknown-instruction",
            Self::InvalidTagEscape => "invalid-tag-escape",
            Self::UnterminatedTag => "unterminated-tag",
            Self::UnterminatedTagEscape => "unterminated-tag-escape",
            Self::UnterminatedArgumentList => "unterminated-argument-list",
            Self::InvalidNumber => "invalid-number",
            Self::InvalidArgument => "invalid-argument",
            Self::InvalidArgumentCount => "invalid-argument-count",
            Self::InvalidTargetSyntax => "invalid-target-syntax",
            Self::InvalidTarget => "invalid-target",
            Self::InvalidTargetCount => "invalid-target-count",
            Self::InvalidRepeatBlock => "invalid-repeat-block",
            Self::MissingRepeatCount => "missing-repeat-count",
            Self::InvalidRepeatCount => "invalid-repeat-count",
            Self::IntegerOutOfRange => "integer-out-of-range",
            Self::UnexpectedRepeatTerminator => "unexpected-repeat-terminator",
            Self::UnterminatedRepeatBlock => "unterminated-repeat-block",
        }
    }
}

/// Typed machine-readable details attached to model-parse failures.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ParseErrorContext {
    Model {
        dialect: ModelDialect,
    },
    Utf8 {
        dialect: ModelDialect,
        valid_up_to: usize,
        error_length: Option<usize>,
    },
    Instruction {
        dialect: ModelDialect,
        instruction: String,
    },
    DomainValue {
        dialect: ModelDialect,
        kind: &'static str,
        value: String,
    },
    ArgumentCount {
        dialect: ModelDialect,
        instruction: String,
        expected: &'static str,
        actual: usize,
    },
    Argument {
        dialect: ModelDialect,
        instruction: String,
        argument: String,
    },
    Target {
        dialect: ModelDialect,
        instruction: String,
        target: String,
    },
    TargetCount {
        dialect: ModelDialect,
        instruction: String,
        actual: usize,
    },
}

impl ParseErrorContext {
    pub const fn dialect(&self) -> ModelDialect {
        match self {
            Self::Model { dialect }
            | Self::Utf8 { dialect, .. }
            | Self::Instruction { dialect, .. }
            | Self::DomainValue { dialect, .. }
            | Self::ArgumentCount { dialect, .. }
            | Self::Argument { dialect, .. }
            | Self::Target { dialect, .. }
            | Self::TargetCount { dialect, .. } => *dialect,
        }
    }

    fn with_bounded_text(self) -> Self {
        match self {
            Self::Model { .. } | Self::Utf8 { .. } => self,
            Self::Instruction {
                dialect,
                instruction,
            } => Self::Instruction {
                dialect,
                instruction: bounded_parse_diagnostic_text(&instruction),
            },
            Self::DomainValue {
                dialect,
                kind,
                value,
            } => Self::DomainValue {
                dialect,
                kind,
                value: bounded_parse_diagnostic_text(&value),
            },
            Self::ArgumentCount {
                dialect,
                instruction,
                expected,
                actual,
            } => Self::ArgumentCount {
                dialect,
                instruction: bounded_parse_diagnostic_text(&instruction),
                expected,
                actual,
            },
            Self::Argument {
                dialect,
                instruction,
                argument,
            } => Self::Argument {
                dialect,
                instruction: bounded_parse_diagnostic_text(&instruction),
                argument: bounded_parse_diagnostic_text(&argument),
            },
            Self::Target {
                dialect,
                instruction,
                target,
            } => Self::Target {
                dialect,
                instruction: bounded_parse_diagnostic_text(&instruction),
                target: bounded_parse_diagnostic_text(&target),
            },
            Self::TargetCount {
                dialect,
                instruction,
                actual,
            } => Self::TargetCount {
                dialect,
                instruction: bounded_parse_diagnostic_text(&instruction),
                actual,
            },
        }
    }
}

/// A structured model-parse diagnostic.
///
/// `message` is the concise machine-facing description. `Display` retains the compatibility
/// human rendering used by the facade, which may include the historical source-line prefix.
/// Attacker-controlled text is stored as a bounded UTF-8 excerpt whose truncation suffix reports
/// the original byte length; `span` always refers to the complete original source region.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{human_message}")]
pub struct ParseError {
    code: ParseErrorCode,
    severity: DiagnosticSeverity,
    message: String,
    human_message: String,
    span: ByteSpan,
    context: Box<ParseErrorContext>,
}

impl ParseError {
    pub(crate) fn new(
        code: ParseErrorCode,
        message: impl Into<String>,
        span: ByteSpan,
        context: ParseErrorContext,
    ) -> Self {
        let message = message.into();
        let message = bounded_parse_diagnostic_text(&message);
        Self {
            code,
            severity: DiagnosticSeverity::Error,
            human_message: message.clone(),
            message,
            span,
            context: Box::new(context.with_bounded_text()),
        }
    }

    /// Decodes model input as UTF-8 or returns a diagnostic derived from the same byte slice.
    pub fn decode_utf8(dialect: ModelDialect, input: &[u8]) -> Result<&str, Self> {
        std::str::from_utf8(input)
            .map_err(|error| Self::from_utf8_error(dialect, error, input.len()))
    }

    fn from_utf8_error(
        dialect: ModelDialect,
        error: std::str::Utf8Error,
        input_len: usize,
    ) -> Self {
        let byte_start = error.valid_up_to();
        let byte_length = error
            .error_len()
            .unwrap_or_else(|| input_len.saturating_sub(byte_start));
        Self::invalid_utf8_at(dialect, byte_start, byte_length, error.error_len())
    }

    pub(crate) fn invalid_utf8_at(
        dialect: ModelDialect,
        byte_start: usize,
        byte_length: usize,
        error_length: Option<usize>,
    ) -> Self {
        let span = ByteSpan::try_new(byte_start, byte_length)
            .unwrap_or_else(|| ByteSpan::from_valid_range(byte_start, 0));
        Self::new(
            ParseErrorCode::InvalidUtf8Input,
            "input is not valid UTF-8 text",
            span,
            ParseErrorContext::Utf8 {
                dialect,
                valid_up_to: byte_start,
                error_length,
            },
        )
    }

    pub(crate) fn with_human_message(
        code: ParseErrorCode,
        message: impl Into<String>,
        human_message: impl Into<String>,
        span: ByteSpan,
        context: ParseErrorContext,
    ) -> Self {
        let message = message.into();
        let human_message = human_message.into();
        Self {
            code,
            severity: DiagnosticSeverity::Error,
            message: bounded_parse_diagnostic_text(&message),
            human_message: bounded_parse_diagnostic_text(&human_message),
            span,
            context: Box::new(context.with_bounded_text()),
        }
    }

    pub const fn code(&self) -> ParseErrorCode {
        self.code
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> ByteSpan {
        self.span
    }

    pub fn context(&self) -> &ParseErrorContext {
        &self.context
    }
}

/// Stable machine-readable result-format failure classes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum FormatErrorCode {
    InvalidData,
    UnexpectedEndOfInput,
    InvalidRecordWidth,
    InvalidByte,
    MissingRecordTerminator,
    InvalidRecordSeparator,
    InvalidPrefix,
    MissingIndex,
    IntegerOverflow,
    IndexOutOfRange,
    InvalidPackedLength,
    RunLengthOvershoot,
    ArithmeticOverflow,
}

impl FormatErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidData => "invalid-data",
            Self::UnexpectedEndOfInput => "unexpected-end-of-input",
            Self::InvalidRecordWidth => "invalid-record-width",
            Self::InvalidByte => "invalid-byte",
            Self::MissingRecordTerminator => "missing-record-terminator",
            Self::InvalidRecordSeparator => "invalid-record-separator",
            Self::InvalidPrefix => "invalid-prefix",
            Self::MissingIndex => "missing-index",
            Self::IntegerOverflow => "integer-overflow",
            Self::IndexOutOfRange => "index-out-of-range",
            Self::InvalidPackedLength => "invalid-packed-length",
            Self::RunLengthOvershoot => "run-length-overshoot",
            Self::ArithmeticOverflow => "arithmetic-overflow",
        }
    }
}

/// Typed machine-readable details attached to result-format failures.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FormatErrorContext {
    None,
    RecordWidth {
        actual_bits: usize,
        expected_bits: usize,
    },
    MinimumRecordWidth {
        actual_bits: usize,
        minimum_bits: usize,
    },
    InvalidByte {
        byte: u8,
    },
    Index {
        result_type: Option<DetsResultType>,
        index: u64,
        exclusive_bound: usize,
    },
    InputLengthMultiple {
        actual_bytes: usize,
        byte_multiple: usize,
    },
    MinimumInputLength {
        actual_bytes: usize,
        minimum_bytes: usize,
    },
    RunLength {
        decoded_bits: usize,
        expected_bits: usize,
    },
}

/// A structured result-format diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct FormatError {
    code: FormatErrorCode,
    severity: DiagnosticSeverity,
    message: String,
    span: Option<ByteSpan>,
    context: FormatErrorContext,
}

impl FormatError {
    pub fn new(code: FormatErrorCode, message: impl Into<String>, span: Option<ByteSpan>) -> Self {
        Self::with_context(code, message, span, FormatErrorContext::None)
    }

    pub(crate) fn with_context(
        code: FormatErrorCode,
        message: impl Into<String>,
        span: Option<ByteSpan>,
        context: FormatErrorContext,
    ) -> Self {
        Self {
            code,
            severity: DiagnosticSeverity::Error,
            message: message.into(),
            span,
            context,
        }
    }

    pub fn invalid_data(message: impl Into<String>) -> Self {
        Self::new(FormatErrorCode::InvalidData, message, None)
    }

    pub const fn code(&self) -> FormatErrorCode {
        self.code
    }

    pub const fn severity(&self) -> DiagnosticSeverity {
        self.severity
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn span(&self) -> Option<ByteSpan> {
        self.span
    }

    pub const fn context(&self) -> FormatErrorContext {
        self.context
    }
}
