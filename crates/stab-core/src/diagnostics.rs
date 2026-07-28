use thiserror::Error;

use crate::result_formats::DetsResultType;
pub use stab_model::{ByteSpan, DiagnosticSeverity, ParseError, ParseErrorCode, ParseErrorContext};

pub(crate) fn bounded_parse_diagnostic_text(value: &str) -> String {
    stab_model::advanced::bounded_parse_diagnostic_text(value)
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

impl From<stab_records::FormatError> for FormatError {
    fn from(error: stab_records::FormatError) -> Self {
        let span = error.span().map(|span| {
            stab_model::advanced::byte_span_from_valid_range(span.byte_start(), span.byte_length())
        });
        Self::with_context(
            format_error_code_from_records(error.code()),
            error.message(),
            span,
            format_error_context_from_records(error.context()),
        )
    }
}

const fn format_error_code_from_records(code: stab_records::FormatErrorCode) -> FormatErrorCode {
    match code {
        stab_records::FormatErrorCode::InvalidData => FormatErrorCode::InvalidData,
        stab_records::FormatErrorCode::UnexpectedEndOfInput => {
            FormatErrorCode::UnexpectedEndOfInput
        }
        stab_records::FormatErrorCode::InvalidRecordWidth => FormatErrorCode::InvalidRecordWidth,
        stab_records::FormatErrorCode::InvalidByte => FormatErrorCode::InvalidByte,
        stab_records::FormatErrorCode::MissingRecordTerminator => {
            FormatErrorCode::MissingRecordTerminator
        }
        stab_records::FormatErrorCode::InvalidRecordSeparator => {
            FormatErrorCode::InvalidRecordSeparator
        }
        stab_records::FormatErrorCode::InvalidPrefix => FormatErrorCode::InvalidPrefix,
        stab_records::FormatErrorCode::MissingIndex => FormatErrorCode::MissingIndex,
        stab_records::FormatErrorCode::IntegerOverflow => FormatErrorCode::IntegerOverflow,
        stab_records::FormatErrorCode::IndexOutOfRange => FormatErrorCode::IndexOutOfRange,
        stab_records::FormatErrorCode::InvalidPackedLength => FormatErrorCode::InvalidPackedLength,
        stab_records::FormatErrorCode::RunLengthOvershoot => FormatErrorCode::RunLengthOvershoot,
        stab_records::FormatErrorCode::ArithmeticOverflow => FormatErrorCode::ArithmeticOverflow,
    }
}

const fn format_error_context_from_records(
    context: stab_records::FormatErrorContext,
) -> FormatErrorContext {
    match context {
        stab_records::FormatErrorContext::None => FormatErrorContext::None,
        stab_records::FormatErrorContext::RecordWidth {
            actual_bits,
            expected_bits,
        } => FormatErrorContext::RecordWidth {
            actual_bits,
            expected_bits,
        },
        stab_records::FormatErrorContext::MinimumRecordWidth {
            actual_bits,
            minimum_bits,
        } => FormatErrorContext::MinimumRecordWidth {
            actual_bits,
            minimum_bits,
        },
        stab_records::FormatErrorContext::InvalidByte { byte } => {
            FormatErrorContext::InvalidByte { byte }
        }
        stab_records::FormatErrorContext::Index {
            result_type,
            index,
            exclusive_bound,
        } => FormatErrorContext::Index {
            result_type,
            index,
            exclusive_bound,
        },
        stab_records::FormatErrorContext::InputLengthMultiple {
            actual_bytes,
            byte_multiple,
        } => FormatErrorContext::InputLengthMultiple {
            actual_bytes,
            byte_multiple,
        },
        stab_records::FormatErrorContext::MinimumInputLength {
            actual_bytes,
            minimum_bytes,
        } => FormatErrorContext::MinimumInputLength {
            actual_bytes,
            minimum_bytes,
        },
        stab_records::FormatErrorContext::RunLength {
            decoded_bits,
            expected_bits,
        } => FormatErrorContext::RunLength {
            decoded_bits,
            expected_bits,
        },
    }
}
