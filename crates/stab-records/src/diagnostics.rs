use thiserror::Error;

use crate::DetsResultType;

/// A half-open byte range in the original input.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ByteSpan {
    byte_start: usize,
    byte_length: usize,
}

impl ByteSpan {
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

/// Stable machine-readable result-format failure classes.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

    pub(crate) fn invalid_data(message: impl Into<String>) -> Self {
        Self::new(FormatErrorCode::InvalidData, message, None)
    }

    pub(crate) fn invalid_result_format(message: impl Into<String>) -> Self {
        Self::invalid_data(message)
    }

    pub(crate) fn invalid_result_format_diagnostic(
        code: FormatErrorCode,
        message: impl Into<String>,
        span: Option<ByteSpan>,
    ) -> Self {
        Self::new(code, message, span)
    }

    pub(crate) fn invalid_result_format_diagnostic_with_context(
        code: FormatErrorCode,
        message: impl Into<String>,
        span: Option<ByteSpan>,
        context: FormatErrorContext,
    ) -> Self {
        Self::with_context(code, message, span, context)
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
