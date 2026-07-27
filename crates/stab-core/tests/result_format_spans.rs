#![allow(
    clippy::expect_used,
    clippy::panic_in_result_fn,
    reason = "diagnostic compatibility tests require direct extraction of expected failures and Result propagation for setup"
)]

use stab_core::{
    ByteSpan, CircuitError, CircuitResult, DetsLayout, DiagnosticSeverity, FormatError,
    FormatErrorCode, FormatErrorContext, SampleFormat,
    result_formats::{read_dets_records, read_records},
    result_streaming::{for_each_packed_record, for_each_record, for_each_sparse_record},
};

#[test]
fn result_format_diagnostic_value_contract_is_stable() {
    let span = ByteSpan::try_new(14, 7).expect("valid fixture span");
    assert_eq!(span.byte_start(), 14);
    assert_eq!(span.byte_length(), 7);
    assert_eq!(span.byte_end(), 21);
    assert_eq!(ByteSpan::try_new(usize::MAX, 1), None);
    assert_eq!(DiagnosticSeverity::Error.as_str(), "error");
    assert_eq!(DiagnosticSeverity::Warning.as_str(), "warning");
    for (code, expected) in [
        (FormatErrorCode::InvalidData, "invalid-data"),
        (
            FormatErrorCode::UnexpectedEndOfInput,
            "unexpected-end-of-input",
        ),
        (FormatErrorCode::InvalidRecordWidth, "invalid-record-width"),
        (FormatErrorCode::InvalidByte, "invalid-byte"),
        (
            FormatErrorCode::MissingRecordTerminator,
            "missing-record-terminator",
        ),
        (
            FormatErrorCode::InvalidRecordSeparator,
            "invalid-record-separator",
        ),
        (FormatErrorCode::InvalidPrefix, "invalid-prefix"),
        (FormatErrorCode::MissingIndex, "missing-index"),
        (FormatErrorCode::IntegerOverflow, "integer-overflow"),
        (FormatErrorCode::IndexOutOfRange, "index-out-of-range"),
    ] {
        assert_eq!(code.as_str(), expected);
    }

    let structured = FormatError::new(
        FormatErrorCode::InvalidByte,
        "bad byte",
        ByteSpan::try_new(3, 1),
    );
    let wrapped = CircuitError::from(structured.clone());
    assert_eq!(wrapped.format_error(), Some(&structured));
    assert_eq!(wrapped.to_string(), "invalid result format data: bad byte");

    let generic = CircuitError::invalid_result_format("legacy message");
    let generic_diagnostic = generic.format_error().expect("generic format diagnostic");
    assert_eq!(generic_diagnostic.code(), FormatErrorCode::InvalidData);
    assert_eq!(generic_diagnostic.span(), None);
    assert_eq!(generic_diagnostic.context(), FormatErrorContext::None);
    assert_eq!(generic_diagnostic.message(), "legacy message");
}

#[test]
fn zero_one_errors_report_exact_byte_spans_across_readers() {
    for error in zero_one_reader_errors(b"0\n", 2) {
        assert_format_error(
            error,
            FormatErrorCode::InvalidRecordWidth,
            span(1, 1),
            FormatErrorContext::RecordWidth {
                actual_bits: 1,
                expected_bits: 2,
            },
            "01 record ended after 1 bits; expected 2 bits",
        );
    }

    assert_format_error(
        read_records(b"0", SampleFormat::ZeroOne, 2).expect_err("partial record"),
        FormatErrorCode::UnexpectedEndOfInput,
        span(1, 0),
        FormatErrorContext::RecordWidth {
            actual_bits: 1,
            expected_bits: 2,
        },
        "01 data ended in the middle of a record at bit 1; expected 2 bits",
    );
    assert_format_error(
        read_records(b"0\xff\n", SampleFormat::ZeroOne, 2).expect_err("invalid byte"),
        FormatErrorCode::InvalidByte,
        span(1, 1),
        FormatErrorContext::InvalidByte { byte: 255 },
        "01 record contains non-bit byte 255",
    );
    assert_format_error(
        read_records(b"01", SampleFormat::ZeroOne, 2).expect_err("missing newline"),
        FormatErrorCode::MissingRecordTerminator,
        span(2, 0),
        FormatErrorContext::None,
        "01 data did not end with a newline after the expected record width",
    );
    assert_format_error(
        read_records(b"01\rX", SampleFormat::ZeroOne, 2).expect_err("invalid CRLF"),
        FormatErrorCode::MissingRecordTerminator,
        span(3, 1),
        FormatErrorContext::None,
        "01 carriage return was not followed by a line feed",
    );
    assert_format_error(
        read_records(b"0\xc3\xa9\n", SampleFormat::ZeroOne, 2).expect_err("UTF-8 byte"),
        FormatErrorCode::InvalidByte,
        span(1, 1),
        FormatErrorContext::InvalidByte { byte: 195 },
        "01 record contains non-bit byte 195",
    );

    assert!(read_records(b"01\n", SampleFormat::ZeroOne, 2).is_ok());
    assert!(read_records(b"01\r\n", SampleFormat::ZeroOne, 2).is_ok());
}

#[test]
fn hits_errors_report_separator_integer_and_bounds_spans() {
    assert_format_error(
        read_records(b"1,,2\n", SampleFormat::Hits, 4).expect_err("double comma"),
        FormatErrorCode::MissingIndex,
        span(2, 1),
        FormatErrorContext::None,
        "HITS index was not followed by an unsigned integer",
    );
    assert_format_error(
        read_records(b"1,2", SampleFormat::Hits, 4).expect_err("unterminated"),
        FormatErrorCode::InvalidRecordSeparator,
        span(3, 0),
        FormatErrorContext::None,
        "HITS data was not comma-separated integers terminated by a newline",
    );
    assert_format_error(
        read_records(b"4\n", SampleFormat::Hits, 4).expect_err("out of range"),
        FormatErrorCode::IndexOutOfRange,
        span(0, 1),
        FormatErrorContext::Index {
            result_type: None,
            index: 4,
            exclusive_bound: 4,
        },
        "HITS index 4 exceeds record width 4",
    );
    assert_format_error(
        read_records(b"18446744073709551616\n", SampleFormat::Hits, 4)
            .expect_err("integer overflow"),
        FormatErrorCode::IntegerOverflow,
        span(0, 20),
        FormatErrorContext::None,
        "HITS index overflowed u64",
    );

    assert!(read_records(b"1,2\r\n", SampleFormat::Hits, 4).is_ok());
}

#[test]
fn dets_errors_report_prefix_namespace_and_index_spans() -> CircuitResult<()> {
    let layout = DetsLayout::try_new(1, 2, 1)?;
    assert_format_error(
        read_dets_records(b"xy", layout).expect_err("bad shot prefix"),
        FormatErrorCode::InvalidPrefix,
        span(0, 2),
        FormatErrorContext::None,
        "DETS data did not start with 'shot'",
    );
    assert_format_error(
        read_dets_records(b"shot  D0\n", layout).expect_err("double space"),
        FormatErrorCode::InvalidPrefix,
        span(5, 1),
        FormatErrorContext::None,
        "unrecognized DETS prefix; expected M, D, or L",
    );
    assert_format_error(
        read_dets_records(b"shot D\n", layout).expect_err("missing index"),
        FormatErrorCode::MissingIndex,
        span(6, 1),
        FormatErrorContext::None,
        "DETS token index was not followed by an unsigned integer",
    );
    assert_format_error(
        read_dets_records(b"shot D2\n", layout).expect_err("namespace bound"),
        FormatErrorCode::IndexOutOfRange,
        span(6, 1),
        FormatErrorContext::Index {
            result_type: Some(stab_core::DetsResultType::Detector),
            index: 2,
            exclusive_bound: 2,
        },
        "DETS token D2 exceeds namespace width 2",
    );

    assert_eq!(
        read_dets_records(b"shot M0 D0 L0", layout)?,
        vec![vec![true, true, false, true]]
    );
    Ok(())
}

fn zero_one_reader_errors(input: &[u8], width: usize) -> Vec<CircuitError> {
    let mut errors =
        vec![read_records(input, SampleFormat::ZeroOne, width).expect_err("materialized reader")];
    errors.push(
        for_each_record(input, SampleFormat::ZeroOne, width, |_| Ok(())).expect_err("dense reader"),
    );
    errors.push(
        for_each_packed_record(input, SampleFormat::ZeroOne, width, |_| Ok(()))
            .expect_err("packed reader"),
    );
    errors.push(
        for_each_sparse_record(input, SampleFormat::ZeroOne, width, |_| Ok(()))
            .expect_err("sparse reader"),
    );
    errors
}

fn assert_format_error(
    error: CircuitError,
    expected_code: FormatErrorCode,
    expected_span: ByteSpan,
    expected_context: FormatErrorContext,
    expected_message: &str,
) {
    let diagnostic = error
        .format_error()
        .expect("expected result-format diagnostic payload");
    assert_eq!(diagnostic.code(), expected_code);
    assert_eq!(diagnostic.severity(), DiagnosticSeverity::Error);
    assert_eq!(diagnostic.span(), Some(expected_span));
    assert_eq!(diagnostic.context(), expected_context);
    assert_eq!(diagnostic.message(), expected_message);
    assert_eq!(
        error.to_string(),
        format!("invalid result format data: {expected_message}")
    );
}

fn span(byte_start: usize, byte_length: usize) -> ByteSpan {
    ByteSpan::try_new(byte_start, byte_length).expect("valid fixture span")
}
