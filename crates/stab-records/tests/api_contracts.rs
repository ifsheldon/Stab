#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "contract tests use compact deterministic fixtures"
)]

use std::collections::HashSet;

use stab_records::{
    ByteSpan, DiagnosticSeverity, Estimate, FormatError, FormatErrorCode, FormatErrorContext,
    RecordEncoding, RecordFormat, RecordStreamReadError, RecordStreamReader, codec_capabilities,
};

#[test]
fn record_diagnostics_preserve_machine_readable_failure_context() {
    let span = ByteSpan::try_new(4, 3).expect("bounded span");
    assert_eq!(span.byte_start(), 4);
    assert_eq!(span.byte_length(), 3);
    assert_eq!(span.byte_end(), 7);
    assert!(ByteSpan::try_new(usize::MAX, 1).is_none());

    let mut reader = RecordStreamReader::measurements(&b"0x\n"[..], RecordFormat::ZeroOne, 2, 64);
    let Err(RecordStreamReadError::Format(error)) = reader.read_batch(1) else {
        panic!("invalid byte must be diagnosed");
    };
    assert_eq!(error.code(), FormatErrorCode::InvalidByte);
    assert_eq!(error.severity(), DiagnosticSeverity::Error);
    assert_eq!(error.span(), ByteSpan::try_new(1, 1));
    assert_eq!(
        error.context(),
        FormatErrorContext::InvalidByte { byte: b'x' }
    );
    assert!(error.message().contains("non-bit byte"));

    let constructed =
        FormatError::new(FormatErrorCode::InvalidData, "invalid record fixture", None);
    assert_eq!(constructed.code().as_str(), "invalid-data");
    assert_eq!(constructed.severity().as_str(), "error");
    assert_eq!(constructed.context(), FormatErrorContext::None);
    assert_eq!(constructed.to_string(), "invalid record fixture");
}

#[test]
fn record_capabilities_report_exact_formats_layouts_and_sizes() {
    let capabilities = codec_capabilities();
    assert_eq!(capabilities.len(), 6);
    let formats = capabilities
        .iter()
        .map(|capability| {
            assert!(capability.can_decode());
            assert!(capability.can_encode());
            capability.format()
        })
        .collect::<HashSet<_>>();
    assert_eq!(formats.len(), 6);
    assert_eq!(formats, RecordFormat::all().collect());

    assert_eq!(RecordFormat::ZeroOne.as_str(), "01");
    assert_eq!(RecordFormat::B8.encoding(), RecordEncoding::BytePacked);
    assert_eq!(RecordFormat::R8.encoding().as_str(), "run-length");
    assert_eq!(RecordFormat::Ptb64.records_per_group(), 64);
    let dets = capabilities
        .iter()
        .find(|capability| capability.format() == RecordFormat::Dets)
        .expect("DETS capability");
    let hits = capabilities
        .iter()
        .find(|capability| capability.format() == RecordFormat::Hits)
        .expect("HITS capability");
    assert!(dets.requires_typed_layout());
    assert!(!hits.requires_typed_layout());

    assert_eq!(
        RecordFormat::ZeroOne.estimate_output_bytes(3, 7),
        Estimate::Exact(24)
    );
    assert_eq!(
        RecordFormat::B8.estimate_output_bytes(3, 9),
        Estimate::Exact(6)
    );
    assert_eq!(
        RecordFormat::Ptb64.estimate_output_bytes(64, 17),
        Estimate::Exact(136)
    );
    assert_eq!(
        RecordFormat::Ptb64.estimate_output_bytes(63, 17),
        Estimate::Unknown
    );
    assert_eq!(RecordFormat::Hits.estimate_output_bytes(3, 7).value(), None);
}
