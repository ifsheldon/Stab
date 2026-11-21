#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "diagnostic contract tests must extract the exact expected failure"
)]

//! Exact diagnostics (code, span, typed context, and message) the packed stream reader reports
//! for malformed input in every format.

use std::io::Read;

use stab_records::{
    ByteSpan, DetsLayout, DetsResultType, FormatError, FormatErrorCode, FormatErrorContext,
    RecordFormat, RecordStreamReadError, RecordStreamReader,
};

#[test]
fn zero_one_reports_exact_width_byte_and_terminator_diagnostics() {
    let width = expected(
        FormatErrorCode::InvalidRecordWidth,
        Some(span(1, 1)),
        FormatErrorContext::RecordWidth {
            actual_bits: 1,
            expected_bits: 2,
        },
        "01 record ended after 1 bits; expected 2 bits",
    );
    assert_record_views(b"0\n", RecordFormat::ZeroOne, 2, width);

    for (input, diagnostic) in [
        (
            b"0".as_slice(),
            expected(
                FormatErrorCode::UnexpectedEndOfInput,
                Some(span(1, 0)),
                FormatErrorContext::RecordWidth {
                    actual_bits: 1,
                    expected_bits: 2,
                },
                "01 data ended in the middle of a record at bit 1; expected 2 bits",
            ),
        ),
        (
            b"0\xff\n".as_slice(),
            expected(
                FormatErrorCode::InvalidByte,
                Some(span(1, 1)),
                FormatErrorContext::InvalidByte { byte: 255 },
                "01 record contains non-bit byte 255",
            ),
        ),
        (
            b"01".as_slice(),
            expected(
                FormatErrorCode::MissingRecordTerminator,
                Some(span(2, 0)),
                FormatErrorContext::None,
                "01 data did not end with a newline after the expected record width",
            ),
        ),
        (
            b"01\rX".as_slice(),
            expected(
                FormatErrorCode::MissingRecordTerminator,
                Some(span(3, 1)),
                FormatErrorContext::None,
                "01 carriage return was not followed by a line feed",
            ),
        ),
    ] {
        assert_record_views(input, RecordFormat::ZeroOne, 2, diagnostic);
    }
}

#[test]
fn hits_reports_exact_separator_integer_and_bounds_diagnostics() {
    let missing_index = expected(
        FormatErrorCode::MissingIndex,
        Some(span(2, 1)),
        FormatErrorContext::None,
        "HITS index was not followed by an unsigned integer",
    );
    assert_record_views(b"1,,2\n", RecordFormat::Hits, 4, missing_index);

    for (input, diagnostic) in [
        (
            b"1,2".as_slice(),
            expected(
                FormatErrorCode::InvalidRecordSeparator,
                Some(span(3, 0)),
                FormatErrorContext::None,
                "HITS data was not comma-separated integers terminated by a newline",
            ),
        ),
        (
            b"4\n".as_slice(),
            expected(
                FormatErrorCode::IndexOutOfRange,
                Some(span(0, 1)),
                FormatErrorContext::Index {
                    result_type: None,
                    index: 4,
                    exclusive_bound: 4,
                },
                "HITS index 4 exceeds record width 4",
            ),
        ),
        (
            b"18446744073709551616\n".as_slice(),
            expected(
                FormatErrorCode::IntegerOverflow,
                Some(span(0, 20)),
                FormatErrorContext::None,
                "HITS index overflowed u64",
            ),
        ),
    ] {
        assert_record_views(input, RecordFormat::Hits, 4, diagnostic);
    }
}

#[test]
fn dets_reports_exact_prefix_namespace_and_index_diagnostics() {
    let layout = DetsLayout::try_new(1, 2, 1).expect("bounded DETS layout");
    for (input, diagnostic) in [
        (
            b"xy".as_slice(),
            expected(
                FormatErrorCode::InvalidPrefix,
                Some(span(0, 2)),
                FormatErrorContext::None,
                "DETS data did not start with 'shot'",
            ),
        ),
        (
            b"shot  D0\n".as_slice(),
            expected(
                FormatErrorCode::InvalidPrefix,
                Some(span(5, 1)),
                FormatErrorContext::None,
                "unrecognized DETS prefix; expected M, D, or L",
            ),
        ),
        (
            b"shot D\n".as_slice(),
            expected(
                FormatErrorCode::MissingIndex,
                Some(span(6, 1)),
                FormatErrorContext::None,
                "DETS token index was not followed by an unsigned integer",
            ),
        ),
        (
            b"shot D2\n".as_slice(),
            expected(
                FormatErrorCode::IndexOutOfRange,
                Some(span(6, 1)),
                FormatErrorContext::Index {
                    result_type: Some(DetsResultType::Detector),
                    index: 2,
                    exclusive_bound: 2,
                },
                "DETS token D2 exceeds namespace width 2",
            ),
        ),
    ] {
        let error = first_format_error(RecordStreamReader::dets(input, layout, usize::MAX));
        assert_diagnostic(&error, diagnostic);
    }
}

#[test]
fn b8_reports_exact_partial_record_diagnostics_across_record_views() {
    assert_record_views(
        b"\x01",
        RecordFormat::B8,
        9,
        expected(
            FormatErrorCode::InvalidPackedLength,
            Some(span(0, 1)),
            FormatErrorContext::InputLengthMultiple {
                actual_bytes: 1,
                byte_multiple: 2,
            },
            "b8 input length 1 is not a multiple of record byte width 2",
        ),
    );
    assert_record_views(
        b"",
        RecordFormat::B8,
        0,
        expected(
            FormatErrorCode::InvalidRecordWidth,
            None,
            FormatErrorContext::MinimumRecordWidth {
                actual_bits: 0,
                minimum_bits: 1,
            },
            "b8 input cannot represent zero-width records",
        ),
    );
}

#[test]
fn r8_reports_exact_eof_and_overshoot_diagnostics_across_record_views() {
    assert_record_views(
        b"\x01",
        RecordFormat::R8,
        3,
        expected(
            FormatErrorCode::UnexpectedEndOfInput,
            Some(span(1, 0)),
            FormatErrorContext::RecordWidth {
                actual_bits: 2,
                expected_bits: 3,
            },
            "r8 input ended before record completed",
        ),
    );
    assert_record_views(
        b"\x04",
        RecordFormat::R8,
        3,
        expected(
            FormatErrorCode::RunLengthOvershoot,
            Some(span(0, 1)),
            FormatErrorContext::RunLength {
                decoded_bits: 4,
                expected_bits: 3,
            },
            "r8 run-length overshot record width",
        ),
    );
}

#[test]
fn ptb64_reports_exact_partial_group_and_width_overflow_diagnostics() {
    assert_record_views(
        &[0_u8; 7],
        RecordFormat::Ptb64,
        1,
        expected(
            FormatErrorCode::InvalidPackedLength,
            Some(span(0, 7)),
            FormatErrorContext::InputLengthMultiple {
                actual_bytes: 7,
                byte_multiple: 8,
            },
            "ptb64 input length 7 is not a multiple of shot-group byte width 8",
        ),
    );
    assert_record_views(
        b"",
        RecordFormat::Ptb64,
        usize::MAX,
        expected(
            FormatErrorCode::ArithmeticOverflow,
            None,
            FormatErrorContext::None,
            "ptb64 record byte width overflowed",
        ),
    );
}

#[derive(Clone, Copy)]
struct ExpectedDiagnostic {
    code: FormatErrorCode,
    span: Option<ByteSpan>,
    context: FormatErrorContext,
    message: &'static str,
}

fn expected(
    code: FormatErrorCode,
    span: Option<ByteSpan>,
    context: FormatErrorContext,
    message: &'static str,
) -> ExpectedDiagnostic {
    ExpectedDiagnostic {
        code,
        span,
        context,
        message,
    }
}

fn assert_record_views(
    input: &[u8],
    format: RecordFormat,
    bits_per_record: usize,
    expected: ExpectedDiagnostic,
) {
    let error = first_format_error(RecordStreamReader::measurements(
        input,
        format,
        bits_per_record,
        usize::MAX,
    ));
    assert_diagnostic(&error, expected);
}

/// Drains a packed stream reader one record at a time and returns its format failure.
fn first_format_error<R: Read>(mut reader: RecordStreamReader<R>) -> FormatError {
    loop {
        match reader.read_batch(1) {
            Ok(Some(_)) => {}
            Ok(None) => panic!("streamed input unexpectedly ended cleanly"),
            Err(RecordStreamReadError::Format(error)) => return error,
            Err(error) => panic!("streamed input failed outside its grammar: {error}"),
        }
    }
}

fn assert_diagnostic(error: &FormatError, expected: ExpectedDiagnostic) {
    assert_eq!(error.code(), expected.code);
    assert_eq!(error.span(), expected.span);
    assert_eq!(error.context(), expected.context);
    assert_eq!(error.message(), expected.message);
}

fn span(byte_start: usize, byte_length: usize) -> ByteSpan {
    ByteSpan::try_new(byte_start, byte_length).expect("valid fixture span")
}
